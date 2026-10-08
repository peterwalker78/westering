//! Choosing what the sky asks, and when. At most two questions a visit,
//! one on the first night, none repeated within thirty days, and each one
//! riding on something just caught.

use crate::catalogues::Kind;
use crate::ephem::Body;
use crate::events::{Kind as EventKind, SkyEvent};
use crate::finds::{Find, Target, stable_hash};
use crate::journal::{Ask, Asked, Course, Person, Plan, Weight};
use crate::sky::Sky;
use crate::time::{DAY, HOUR, MONTHS, UnixMs, civil_date, midnight_utc, weekday};
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AnswerKind {
    Name,
    Text,
    Plan,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Question {
    pub id: String,
    pub thread: String,
    pub on: String,
    pub answer: AnswerKind,
    #[serde(default)]
    pub ask: Option<String>,
    #[serde(default)]
    pub needs: Option<String>,
    pub text: String,
}

#[derive(Deserialize)]
struct Bank {
    question: Vec<Question>,
}

pub fn bundled() -> Vec<Question> {
    toml::from_str::<Bank>(include_str!("../data/questions.toml"))
        .expect("questions.toml")
        .question
}

/// Words no question, card or page may use: the app never names the
/// feeling it hopes for.
pub const UNSAID: [&str; 6] = [
    "grateful",
    "gratitude",
    "thankful",
    "appreciate",
    "blessed",
    "lucky",
];

/// What was caught, as the bank's `on` values see it.
pub fn triggers(find: &Find, sky: &Sky) -> Vec<&'static str> {
    let mut out = match find.target {
        Target::Body(Body::Moon) => vec!["moon"],
        Target::Body(Body::Jupiter) => vec!["moons"],
        Target::Body(Body::Saturn) => vec!["rings"],
        Target::Body(Body::Mars) => vec!["mars"],
        Target::Body(Body::Venus) => vec!["venus"],
        Target::Body(_) => vec![],
        Target::Star(424) => vec!["pole"],
        Target::Star(4730) => vec!["pole-south", "distant"],
        Target::Star(2061) | Target::Star(1713) => vec!["orion", "distant"],
        Target::Star(_) => vec!["distant"],
        Target::Showpiece(i) => {
            let p = &sky.lists.showpieces[i];
            match p.kind {
                Kind::Cluster => vec!["cluster"],
                Kind::Galaxy if p.id == "andromeda" => vec!["andromeda"],
                Kind::Galaxy => vec!["galaxy"],
                Kind::Nebula => vec!["nebula"],
                Kind::Double => vec!["double"],
                Kind::Star => vec!["distant"],
                Kind::Dark => vec![],
                // Looking deep into our own galaxy.
                Kind::Cloud => vec!["distant"],
                // Not born together, so not the cluster questions.
                Kind::Asterism => vec![],
            }
        }
        Target::Meteor(_) => vec!["meteor"],
        Target::Figure(i) if sky.figures[i].abbrev == "Ori" => vec!["orion"],
        Target::Figure(_) => vec![],
        Target::MoonWalk => vec!["moon"],
        Target::Hop(_) | Target::Story(_) => vec![],
    };
    out.push("any");
    out
}

/// Whole days from one night key ("YYYY-MM-DD") to another.
pub fn days_between(from: &str, to: &str) -> i64 {
    let parse = |k: &str| {
        let mut p = k.split('-').map(|x| x.parse::<i64>().unwrap_or(1));
        let (y, m, d) = (
            p.next().unwrap_or(2000),
            p.next().unwrap_or(1),
            p.next().unwrap_or(1),
        );
        midnight_utc(y as i32, m as u32, d as u32) / DAY
    };
    parse(to) - parse(from)
}

pub fn key_of(at: UnixMs, offset_s: i32) -> String {
    let (y, m, d) = civil_date(at, offset_s);
    format!("{y:04}-{m:02}-{d:02}")
}

pub struct Context<'a> {
    pub night: &'a str,
    pub ask: Ask,
    /// No earlier night in the logbook.
    pub first_night: bool,
    /// Questions already asked this visit, by thread.
    pub asked_now: &'a [String],
    pub asked: &'a [Asked],
    pub has_weights: bool,
    pub people: &'a [&'a Person],
    pub plans: &'a [Plan],
    pub events: &'a [SkyEvent],
    pub now: UnixMs,
    pub offset_s: i32,
}

#[derive(Clone, Debug)]
pub struct Chosen {
    pub question: Question,
    /// The words as asked, placeholders filled.
    pub text: String,
    pub person: Option<String>,
    pub event: Option<SkyEvent>,
}

/// How many questions a visit may hold.
pub fn budget(ask: Ask, night: &str) -> usize {
    match ask {
        Ask::Never => 0,
        Ask::Sometimes => 2,
        Ask::Rarely => usize::from(stable_hash((0, 0, 0), night).is_multiple_of(3)),
    }
}

fn recently_asked(asked: &[Asked], id: &str, night: &str) -> bool {
    asked
        .iter()
        .any(|a| a.question == id && days_between(&a.night, night) < 30)
}

/// "Friday" within the week ahead, "26 October" beyond it.
pub fn day_words(at: UnixMs, now: UnixMs, offset_s: i32) -> String {
    let (_, m, d) = civil_date(at, offset_s);
    if at - now < 6 * DAY {
        weekday(at, offset_s).to_owned()
    } else {
        format!("{d} {}", MONTHS[m as usize - 1])
    }
}

fn capitalised(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// The sky event a plan question would hang on, if one is due.
fn event_for<'a>(on: &str, ctx: &'a Context) -> Option<&'a SkyEvent> {
    let ahead = |e: &SkyEvent, lo: i64, hi: i64| {
        let days = (e.at - ctx.now) as f64 / DAY as f64;
        days >= lo as f64 && days <= hi as f64
    };
    let taken = |e: &SkyEvent| {
        let date = key_of(e.at, ctx.offset_s);
        ctx.plans
            .iter()
            .any(|p| p.event == e.title && p.date == date)
    };
    ctx.events.iter().find(|e| {
        !taken(e)
            && match (&e.kind, on) {
                (EventKind::FullMoon, "plan-moon") => ahead(e, 2, 10),
                (EventKind::Shower { zhr, .. }, "plan-shower") => *zhr >= 50 && ahead(e, 5, 40),
                (EventKind::Opposition(_), "plan-opposition") => ahead(e, 5, 60),
                (EventKind::Pairing(_), "plan-pairing") => ahead(e, 2, 10),
                _ => false,
            }
    })
}

/// The pretend event for the one seasonal plan: Orion's return by December.
/// The one plan that hangs on a constellation coming back, not on a date
/// in the sky's calendar.
pub const ORION_RETURN: &str = "Orion's return";

fn season_event(ctx: &Context) -> Option<SkyEvent> {
    let (y, m, _) = civil_date(ctx.now, ctx.offset_s);
    if !(9..=11).contains(&m) {
        return None;
    }
    let at = midnight_utc(y, 12, 1) + 20 * HOUR;
    let event = SkyEvent {
        at,
        kind: EventKind::NewMoon,
        title: ORION_RETURN.into(),
    };
    let date = key_of(at, ctx.offset_s);
    (!ctx
        .plans
        .iter()
        .any(|p| p.event == event.title && p.date == date))
    .then_some(event)
}

fn fill(q: &Question, person: Option<&str>, event: Option<&SkyEvent>, ctx: &Context) -> String {
    let mut text = q.text.clone();
    if let Some(name) = person {
        text = text.replace("{name}", name);
    }
    if let Some(e) = event {
        text = text.replace("{event}", &capitalised(&e.title));
        text = text.replace("{day}", &day_words(e.at, ctx.now, ctx.offset_s));
        let (planet, gap) = match e.kind {
            EventKind::Opposition(b) | EventKind::Pairing(b) => (
                b.name(),
                if b == Body::Mars {
                    "about two years"
                } else {
                    "about a year"
                },
            ),
            _ => ("", ""),
        };
        text = text.replace("{planet}", planet).replace("{gap}", gap);
    }
    text
}

/// The question to ask on catching something with these triggers, if any.
pub fn choose(bank: &[Question], triggers: &[&str], ctx: &Context) -> Option<Chosen> {
    // The first night asks one, so the sky's other half is met straight away.
    let most = if ctx.first_night {
        budget(ctx.ask, ctx.night).min(1)
    } else {
        budget(ctx.ask, ctx.night)
    };
    if ctx.asked_now.len() >= most {
        return None;
    }
    let fresh = |q: &&Question| {
        !recently_asked(ctx.asked, &q.id, ctx.night)
            && !ctx.asked_now.iter().any(|t| t == &q.id)
            && (q.needs.as_deref() != Some("weights") || ctx.has_weights)
    };
    let threads_now: Vec<&str> = ctx
        .asked_now
        .iter()
        .filter_map(|id| bank.iter().find(|q| &q.id == id))
        .map(|q| q.thread.as_str())
        .collect();
    let order = |q: &Question| stable_hash((0, 0, 0), &format!("{}:{}", ctx.night, q.id));
    let pick = |mut qs: Vec<&Question>| -> Option<Question> {
        qs.sort_by_key(|q| (threads_now.contains(&q.thread.as_str()), order(q)));
        qs.first().map(|q| (*q).clone())
    };
    let night_roll = stable_hash((0, 0, 0), &format!("roll:{}", ctx.night));

    // About one visit in three offers a plan, never more than one.
    if night_roll.is_multiple_of(3) && !threads_now.contains(&"plans") {
        let plans: Vec<(&Question, SkyEvent)> = bank
            .iter()
            .filter(|q| q.thread == "plans")
            .filter(fresh)
            .filter_map(|q| {
                let e = if q.on == "plan-season" {
                    season_event(ctx)
                } else {
                    event_for(&q.on, ctx).cloned()
                };
                e.map(|e| (q, e))
            })
            .collect();
        if let Some((q, e)) = plans.iter().min_by_key(|(q, _)| order(q)) {
            return Some(Chosen {
                text: fill(q, None, Some(e), ctx),
                question: (*q).clone(),
                person: None,
                event: Some(e.clone()),
            });
        }
    }

    // Someone named before comes back now and then, a fortnight on at least.
    if (night_roll >> 8).is_multiple_of(4)
        && let Some(person) = ctx.people.iter().find(|p| {
            p.mentions
                .iter()
                .map(|m| days_between(&m.night, ctx.night))
                .min()
                .is_some_and(|d| d >= 14)
        })
    {
        let qs: Vec<&Question> = bank
            .iter()
            .filter(|q| q.on == "person")
            .filter(fresh)
            .collect();
        if let Some(q) = pick(qs) {
            return Some(Chosen {
                text: fill(&q, Some(&person.name), None, ctx),
                question: q,
                person: Some(person.name.clone()),
                event: None,
            });
        }
    }

    let specific: Vec<&Question> = bank
        .iter()
        .filter(|q| q.thread != "plans" && q.on != "any" && triggers.contains(&q.on.as_str()))
        .filter(fresh)
        .collect();
    let chosen = pick(specific).or_else(|| {
        // Only now and then does a question ride on nothing in particular.
        if (night_roll >> 16).is_multiple_of(2) {
            pick(
                bank.iter()
                    .filter(|q| q.on == "any")
                    .filter(fresh)
                    .collect(),
            )
        } else {
            None
        }
    })?;
    Some(Chosen {
        text: fill(&chosen, None, None, ctx),
        question: chosen,
        person: None,
        event: None,
    })
}

/// A plan whose day has passed and hasn't been asked about, three weeks at most.
pub fn plan_to_ask_about<'a>(plans: &'a [Plan], today: &str) -> Option<&'a Plan> {
    plans.iter().find(|p| {
        p.outcome.is_none() && {
            let d = days_between(&p.date, today);
            (1..=21).contains(&d)
        }
    })
}

/// A weight set down a week to six weeks ago, still open, that hasn't been
/// looked back at for three weeks and has no course being charted for it.
pub fn weight_to_look_back<'a>(
    weights: &'a [Weight],
    courses: &[Course],
    today: &str,
) -> Option<&'a Weight> {
    weights
        .iter()
        .filter(|w| !w.sorted && !courses.iter().any(|c| c.weight == w.id && !c.draft))
        .filter(|w| {
            w.nights
                .last()
                .is_some_and(|last| (6..=42).contains(&days_between(last, today)))
        })
        .filter(|w| w.looks.iter().all(|k| days_between(&k.night, today) >= 21))
        .min_by_key(|w| w.looks.len())
}

/// A course whose check-back is due and hasn't been answered since.
pub fn course_to_check<'a>(courses: &'a [Course], today: &str) -> Option<&'a Course> {
    courses.iter().find(|c| {
        !c.draft
            && c.check_after.as_deref().is_some_and(|after| {
                days_between(after, today) >= 0
                    && !c.checks.iter().any(|k| days_between(after, &k.night) >= 0)
            })
    })
}

/// A line for the arrival about a plan coming up in the next few days.
pub fn plan_nearby(plans: &[Plan], today: &str) -> Option<String> {
    const WORDS: [&str; 5] = ["", "", "Two", "Three", "Four"];
    let p = plans
        .iter()
        .filter(|p| p.outcome.is_none())
        .map(|p| (days_between(today, &p.date), p))
        .filter(|(d, _)| (0..=4).contains(d))
        .min_by_key(|(d, _)| *d)?;
    let (days, plan) = p;
    let who = plan
        .who
        .as_deref()
        .map(|w| format!(", with {w}"))
        .unwrap_or_default();
    Some(match days {
        0 => format!("Tonight: {}{who}.", plan.what),
        1 => format!("Tomorrow: {}{who}.", plan.what),
        n => format!(
            "{} nights to {}: {}{who}.",
            WORDS[n as usize], plan.event, plan.what
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::{Check, Mention};

    fn ctx<'a>(night: &'a str, asked: &'a [Asked], now_ids: &'a [String]) -> Context<'a> {
        Context {
            night,
            ask: Ask::Sometimes,
            first_night: false,
            asked_now: now_ids,
            asked,
            has_weights: false,
            people: &[],
            plans: &[],
            events: &[],
            now: midnight_utc(2026, 10, 1) + 21 * HOUR,
            offset_s: 0,
        }
    }

    #[test]
    fn no_question_ever_names_the_feeling() {
        for q in bundled() {
            let lower = q.text.to_lowercase();
            for word in UNSAID {
                assert!(!lower.contains(word), "{} says {word}", q.id);
            }
        }
    }

    #[test]
    fn every_trigger_in_the_bank_is_one_the_app_raises() {
        let known = [
            "pole",
            "pole-south",
            "moons",
            "rings",
            "mars",
            "venus",
            "moon",
            "meteor",
            "cluster",
            "galaxy",
            "andromeda",
            "nebula",
            "double",
            "distant",
            "orion",
            "reveal",
            "any",
            "person",
            "plan-moon",
            "plan-shower",
            "plan-opposition",
            "plan-pairing",
            "plan-season",
            "day",
        ];
        for q in bundled() {
            assert!(
                known.contains(&q.on.as_str()),
                "{} rides on unknown {}",
                q.id,
                q.on
            );
        }
    }

    #[test]
    fn every_question_says_what_it_is_about() {
        // Read on its own, a question has to name what it's talking about:
        // no opening on a bare "it" or "this" that only makes sense if you
        // already know what was just caught.
        for q in bundled() {
            let first = q.text.split_whitespace().next().unwrap_or("");
            assert!(
                ![
                    "It", "It's", "Its", "This", "They", "These", "Gone", "Often", "Back"
                ]
                .contains(&first),
                "{}: {}",
                q.id,
                q.text
            );
        }
    }

    #[test]
    fn the_first_night_asks_one_and_two_is_the_most() {
        let bank = bundled();
        let mut c = ctx("2026-10-01", &[], &[]);
        c.first_night = true;
        assert!(choose(&bank, &["pole", "any"], &c).is_some());
        let one = ["a".to_owned()];
        let mut c = ctx("2026-10-01", &[], &one);
        c.first_night = true;
        assert!(choose(&bank, &["pole", "any"], &c).is_none());
        let two = ["a".to_owned(), "b".to_owned()];
        let c = ctx("2026-10-01", &[], &two);
        assert!(choose(&bank, &["pole", "any"], &c).is_none());
    }

    #[test]
    fn polaris_asks_who_would_pick_up_and_not_again_for_a_month() {
        let bank = bundled();
        let c = ctx("2026-10-01", &[], &[]);
        let q = choose(&bank, &["pole", "any"], &c).expect("a question");
        assert!(
            q.question.on == "pole" || q.question.thread == "plans",
            "{}",
            q.question.id
        );
        if q.question.on == "pole" {
            let asked: Vec<Asked> = bank
                .iter()
                .filter(|x| x.on == "pole")
                .map(|x| Asked {
                    question: x.id.clone(),
                    night: "2026-09-20".into(),
                })
                .collect();
            let c = ctx("2026-10-01", &asked, &[]);
            let again = choose(&bank, &["pole"], &c);
            assert!(again.is_none_or(|q| q.question.on != "pole"));
        }
    }

    #[test]
    fn someone_named_before_can_come_back() {
        let bank = bundled();
        let sam = Person {
            name: "Sam".into(),
            stars: vec![424],
            mentions: vec![Mention {
                night: "2026-08-01".into(),
                question: "pole-3am".into(),
            }],
        };
        let people = [&sam];
        let mut seen = false;
        for day in 1..=28 {
            let night = format!("2026-10-{day:02}");
            let mut c = ctx(&night, &[], &[]);
            c.people = &people;
            if let Some(q) = choose(&bank, &["any"], &c)
                && q.question.on == "person"
            {
                assert!(q.text.contains("Sam"), "{}", q.text);
                seen = true;
            }
        }
        assert!(seen, "a month of nights never brought Sam back");
    }

    #[test]
    fn plans_hang_on_real_events_ahead() {
        let bank = bundled();
        let now = midnight_utc(2026, 11, 20) + 21 * HOUR;
        let showers = crate::catalogues::Catalogues::bundled().showers;
        let events = crate::events::upcoming(&showers, now, 60);
        let mut found = false;
        for day in 1..=30 {
            let night = format!("2026-11-{day:02}");
            let mut c = ctx(&night, &[], &[]);
            c.now = now;
            c.events = &events;
            if let Some(q) = choose(&bank, &["any"], &c)
                && q.question.thread == "plans"
            {
                assert!(q.event.is_some());
                assert!(!q.text.contains('{'), "{}", q.text);
                found = true;
            }
        }
        assert!(found, "no plan offered across a month");
    }

    #[test]
    fn old_weights_are_looked_back_at_now_and_then() {
        let mut w = Weight {
            id: 1,
            text: "the move".into(),
            nights: vec!["2026-11-01".into()],
            ..Weight::default()
        };
        let weights = std::slice::from_ref(&w);
        assert!(
            weight_to_look_back(weights, &[], "2026-11-03").is_none(),
            "too soon"
        );
        assert!(weight_to_look_back(weights, &[], "2026-11-10").is_some());
        assert!(
            weight_to_look_back(weights, &[], "2027-01-10").is_none(),
            "long gone"
        );
        w.looks.push(Check {
            night: "2026-11-10".into(),
            answer: "Lighter".into(),
        });
        let weights = std::slice::from_ref(&w);
        assert!(
            weight_to_look_back(weights, &[], "2026-11-15").is_none(),
            "just asked"
        );
        w.sorted = true;
        assert!(weight_to_look_back(std::slice::from_ref(&w), &[], "2026-12-05").is_none());
    }

    #[test]
    fn a_plan_near_at_hand_is_mentioned() {
        let plans = [Plan {
            id: 1,
            what: "Watching the meteors".into(),
            who: Some("Sam".into()),
            date: "2026-12-14".into(),
            event: "the Geminids".into(),
            made: "2026-11-20".into(),
            ..Plan::default()
        }];
        assert_eq!(
            plan_nearby(&plans, "2026-12-11").as_deref(),
            Some("Three nights to the Geminids: Watching the meteors, with Sam.")
        );
        assert_eq!(plan_nearby(&plans, "2026-12-01"), None);
        assert!(plan_to_ask_about(&plans, "2026-12-15").is_some());
        assert!(plan_to_ask_about(&plans, "2027-02-15").is_none());
    }
}
