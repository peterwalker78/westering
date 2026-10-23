//! Time kept the way the sky keeps it. The wisp marks the longer journey in
//! Moons, seasons and the year coming round, and in what the sky itself has
//! done: never in visits counted, and never minding how long anyone was
//! away.

use serde::Deserialize;

use crate::coords::{Observer, angles, observe};
use crate::ephem::{Body, solar_longitude};
use crate::finale::whereabouts;
use crate::finds::stable_hash;
use crate::journal::{Person, long_date};
use crate::questions::{days_between, key_of};
use crate::sky::{Sky, see};
use crate::time::{DAY, HOUR, UnixMs, weekday};
use crate::wisp::Season;

#[derive(Deserialize)]
struct Greeting {
    #[serde(default)]
    when: Vec<String>,
    text: String,
}

#[derive(Deserialize)]
pub struct Round {
    pub text: String,
    pub person: String,
    pub find: String,
}

#[derive(Deserialize)]
pub struct News {
    pub risen: String,
    pub moon: String,
    #[serde(rename = "darker-one")]
    pub darker_one: String,
    #[serde(rename = "darker-many")]
    pub darker_many: String,
}

#[derive(Deserialize)]
struct Turn {
    spring: String,
    #[serde(rename = "longest-day")]
    longest_day: String,
    autumn: String,
    #[serde(rename = "longest-night")]
    longest_night: String,
}

#[derive(Deserialize)]
struct StarLine {
    text: String,
}

/// What the wisp says by day.
#[derive(Deserialize)]
pub struct DayLines {
    pub first: Vec<String>,
    pub hello: Vec<String>,
    pub list: String,
    pub done: String,
    pub ending: String,
    pub outside: String,
    pub dusk: String,
}

/// Free look, where the wisp keeps quiet.
#[derive(Deserialize)]
pub struct FreeLines {
    /// The list is done: the wisp hands the sky over.
    pub handover: String,
    /// F before anything's been found tonight, at the foot of the screen.
    pub wait: String,
}

/// What's said beside the mark on the horizon on the night of a plan.
#[derive(Deserialize)]
pub struct PlanLines {
    /// What it's about is up: look above the mark.
    pub up: String,
    /// It's still down: where it comes up, and when.
    pub rises: String,
    pub shower_up: String,
    pub shower_later: String,
}

/// What's said about the evening itself.
#[derive(Deserialize)]
pub struct EveningLines {
    /// The way to wind down, offered once.
    pub offer: String,
}

/// What the wisp says as it shows the way.
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct TeachLines {
    pub hello: String,
    pub why: String,
    pub parts: String,
    pub one_thing: String,
    pub things: String,
    pub found_already: String,
    pub still_light: String,
    pub late: String,
    pub weights: String,
    pub placing: String,
    pub list_one: String,
    pub list: String,
    pub compass: String,
    pub point: String,
    pub ring: String,
    pub card: String,
    pub draw: String,
    pub drawing: String,
    pub meteor: String,
    pub lost: String,
    pub question: String,
    pub plans: String,
    pub names: String,
    pub logbook: String,
    pub look_back: String,
    pub kept_behind: String,
    pub kept_beside: String,
    pub course: String,
    pub outside: String,
    pub bed: String,
    pub dimming: String,
    pub setting_one: String,
    pub setting: String,
    pub turning: String,
    pub goodnight: String,
}

#[derive(Deserialize)]
pub struct Lines {
    pub day: DayLines,
    pub free: FreeLines,
    pub evening: EveningLines,
    pub teach: TeachLines,
    pub plan: PlanLines,
    greeting: Vec<Greeting>,
    pub round: Round,
    pub news: News,
    turn: Turn,
    star: StarLine,
}

/// What tonight is like, for choosing words.
pub struct Tonight<'a> {
    pub night: &'a str,
    /// The local hour.
    pub hour: u32,
    pub weekday: &'a str,
    /// The Moon's phase, in words.
    pub moon: &'a str,
    pub month: u32,
    pub lat: f64,
}

/// Where and when an evening begins, and what the logbook holds from
/// before it, for what the wisp says on arriving.
pub struct Arrival<'a> {
    pub tonight: Tonight<'a>,
    pub sky: &'a Sky,
    pub observer: Observer,
    pub now: UnixMs,
    /// The local offset from UTC, in seconds, at any moment.
    pub zone: &'a dyn Fn(UnixMs) -> i32,
    /// The first evening in the logbook, and the last one before tonight.
    pub first: Option<&'a str>,
    pub last: Option<&'a str>,
    /// The first thing found on that first evening.
    pub first_find: Option<&'a str>,
    pub people: &'a [Person],
    /// What has been said once already, never to be said again.
    pub seen: &'a [String],
}

/// Something more worth saying on arriving.
#[derive(Debug, PartialEq)]
pub enum More {
    /// The year has come round since the first evening: said once for
    /// each year, remembered under `key`.
    Round { key: String, text: String },
    /// The Sun has passed an equinox or a solstice since the last evening.
    Turn(String),
    /// Someone's star is up, here in the sky.
    Star { text: String, ra: f64, dec: f64 },
}

impl Lines {
    pub fn bundled() -> Lines {
        toml::from_str(include_str!("../data/wisp-lines.toml")).expect("wisp-lines.toml")
    }

    fn holds(&self, when: &str, t: &Tonight) -> bool {
        match when {
            "full-moon" => t.moon == "Full Moon",
            "new-moon" => t.moon == "New Moon",
            "crescent" => t.moon.ends_with("crescent"),
            "late" => t.hour >= 22 || t.hour < 5,
            "early" => (12..20).contains(&t.hour),
            "monday" | "friday" | "saturday" | "sunday" => t.weekday.eq_ignore_ascii_case(when),
            "winter" | "spring" | "summer" | "autumn" => {
                season(t.month, t.lat).is_some_and(|s| s.name() == when)
            }
            _ => false,
        }
    }

    /// Tonight's hello: now and then one that fits the night, otherwise
    /// one of the everyday ones, the same all evening.
    pub fn greeting(&self, t: &Tonight) -> &str {
        let fits: Vec<&Greeting> = self
            .greeting
            .iter()
            .filter(|g| !g.when.is_empty() && g.when.iter().all(|w| self.holds(w, t)))
            .collect();
        let plain: Vec<&Greeting> = self.greeting.iter().filter(|g| g.when.is_empty()).collect();
        let roll = stable_hash((1, 2, 3), t.night);
        let pool = if !fits.is_empty() && roll % 5 < 3 {
            fits
        } else {
            plain
        };
        &pool[(roll / 5) as usize % pool.len()].text
    }

    /// A line for the Sun passing an equinox or solstice at `at`.
    pub fn turn(&self, turn: TurnKind, at: UnixMs, now: UnixMs, offset_s: i32) -> String {
        let days = (now + offset_s as i64 * 1000).div_euclid(DAY)
            - (at + offset_s as i64 * 1000).div_euclid(DAY);
        let day = match days {
            0 => "today".to_owned(),
            1 => "yesterday".to_owned(),
            _ => format!("on {}", weekday(at, offset_s)),
        };
        let text = match turn {
            TurnKind::Spring => &self.turn.spring,
            TurnKind::LongestDay => &self.turn.longest_day,
            TurnKind::Autumn => &self.turn.autumn,
            TurnKind::LongestNight => &self.turn.longest_night,
        };
        text.replace("{day}", &day)
    }

    pub fn star(&self, name: &str, star: &str, place: &str) -> String {
        self.star
            .text
            .replace("{name}", name)
            .replace("{star}", star)
            .replace("{where}", place)
    }

    /// Tonight's hello: one that fits the night, or, after a while away,
    /// what the sky has done since.
    pub fn hello(&self, a: &Arrival) -> String {
        if let Some(last) = a.last {
            let days = days_between(last, a.tonight.night);
            if days >= 10 {
                if let Some(name) = risen(a.sky, a.observer, a.now - days * DAY, a.now, &[]) {
                    return format!("Hello. {}", self.news.risen.replace("{name}", &name));
                }
                if (30..=45).contains(&days) {
                    return format!("Hello. {}", self.news.moon);
                }
            }
        }
        self.greeting(&a.tonight).to_owned()
    }

    /// At most one thing more on arriving, when there's something worth
    /// it: the year come round, the Sun at an equinox or solstice, or the
    /// star of someone named, now and then.
    pub fn one_more_thing(&self, a: &Arrival) -> Option<More> {
        let night = a.tonight.night;
        if let Some(first) = a.first
            && let Some(years) = year_round(first, night)
            && !a.seen.contains(&format!("round-{years}"))
        {
            let key = format!("round-{years}");
            let mut text = self.round.text.replace("{date}", &long_date(first));
            let person = a
                .people
                .iter()
                .find(|p| p.mentions.iter().any(|m| m.night == first));
            if let Some(person) = person {
                let named = self.round.person.replace("{name}", &person.name);
                text = format!("{text} {named}");
            } else if let Some(find) = a.first_find {
                let found = self.round.find.replace("{find}", &lower_first(find));
                text = format!("{text} {found}");
            }
            return Some(More::Round { key, text });
        }
        if let Some((kind, at)) = turned(a.now, a.observer.lat)
            && a.last.is_none_or(|l| l < key_of(at, (a.zone)(at)).as_str())
        {
            return Some(More::Turn(self.turn(kind, at, a.now, (a.zone)(a.now))));
        }
        // Someone's star, about one evening in three that it's up.
        if !stable_hash((0, 0, 1), night).is_multiple_of(3) {
            return None;
        }
        a.people.iter().find_map(|p| {
            p.stars.iter().find_map(|&hr| {
                let star = a.sky.stars.get(hr)?;
                let (ra, dec) = angles(star.dir);
                let (alt, az) = observe(a.observer, a.now, ra, dec);
                let name = &a.sky.lists.star_name(hr)?.name;
                (alt > 20.0).then(|| More::Star {
                    text: self.star(&p.name, name, &whereabouts(alt, az)),
                    ra,
                    dec,
                })
            })
        })
    }
}

/// A name as it reads mid-sentence: "the Pleiades", not "The Pleiades".
pub fn lower_first(name: &str) -> String {
    match name.strip_prefix("The ") {
        Some(rest) => format!("the {rest}"),
        None => name.to_owned(),
    }
}

/// The season by the month, for the user's hemisphere; none near the
/// equator, where the year turns by rain rather than cold.
pub fn season(month: u32, lat: f64) -> Option<Season> {
    if lat.abs() < 23.5 {
        return None;
    }
    let m = if lat < 0.0 {
        (month + 5) % 12 + 1
    } else {
        month
    };
    Some(match m {
        12 | 1 | 2 => Season::Winter,
        3..=5 => Season::Spring,
        6..=8 => Season::Summer,
        _ => Season::Autumn,
    })
}

/// How long ago, as the sky tells it: "one Moon ago", "a season ago".
/// None for spans with no easy sky name.
pub fn sky_since(days: i64) -> Option<&'static str> {
    Some(match days {
        27..=32 => "One Moon ago",
        56..=62 => "Two Moons ago",
        85..=95 => "A season ago",
        175..=190 => "Half a year ago",
        360..=372 => "A year ago",
        _ => return None,
    })
}

/// How many times the year has come round since the first evening, when
/// tonight is within a few weeks after the day it did.
pub fn year_round(first: &str, tonight: &str) -> Option<i64> {
    let days = days_between(first, tonight);
    let years = days / 365;
    (years >= 1 && days - years * 365 <= 21).then_some(years)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnKind {
    Spring,
    LongestDay,
    Autumn,
    LongestNight,
}

/// The equinox or solstice the Sun has passed in the last six days, if
/// any, and when, named for the hemisphere.
pub fn turned(now: UnixMs, lat: f64) -> Option<(TurnKind, UnixMs)> {
    let quarter = |t: UnixMs| (solar_longitude(t) / 90.0).floor() as i64;
    let q = quarter(now);
    let mut lo = now - 6 * DAY;
    if quarter(lo) == q {
        return None;
    }
    let mut hi = now;
    while hi - lo > HOUR {
        let mid = lo + (hi - lo) / 2;
        if quarter(mid) == q {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    let north = match q.rem_euclid(4) {
        0 => TurnKind::Spring,
        1 => TurnKind::LongestDay,
        2 => TurnKind::Autumn,
        _ => TurnKind::LongestNight,
    };
    let kind = if lat >= 0.0 {
        north
    } else {
        match north {
            TurnKind::Spring => TurnKind::Autumn,
            TurnKind::LongestDay => TurnKind::LongestNight,
            TurnKind::Autumn => TurnKind::Spring,
            TurnKind::LongestNight => TurnKind::LongestDay,
        }
    };
    Some((kind, hi))
}

/// Something bright that was below the horizon at an earlier moment
/// (`then`) and is well up now: a planet if one fits, otherwise one of the
/// brightest named stars. Anything in `except` is passed over.
pub fn risen(
    sky: &Sky,
    observer: Observer,
    then: UnixMs,
    now: UnixMs,
    except: &[String],
) -> Option<String> {
    let new = |name: &str| !except.iter().any(|e| e == name);
    for body in [Body::Venus, Body::Jupiter, Body::Mars, Body::Saturn] {
        if new(body.name())
            && see(body, observer, then).alt < 0.0
            && see(body, observer, now).alt > 15.0
        {
            return Some(body.name().to_owned());
        }
    }
    let mut stars: Vec<_> = sky
        .stars
        .stars
        .iter()
        .filter(|s| s.mag < 1.0)
        .filter_map(|s| Some((s, sky.lists.star_name(s.hr)?)))
        .filter(|(_, n)| new(&n.name))
        .collect();
    stars.sort_by(|a, b| a.0.mag.total_cmp(&b.0.mag));
    stars.into_iter().find_map(|(s, named)| {
        let (ra, dec) = angles(s.dir);
        let (was, _) = observe(observer, then, ra, dec);
        let (is, _) = observe(observer, now, ra, dec);
        (was < 0.0 && is > 15.0).then(|| named.name.clone())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::questions::UNSAID;
    use crate::time::midnight_utc;

    const LONDON: Observer = Observer {
        lat: 51.5,
        lon: -0.13,
    };

    fn tonight(night: &str) -> Tonight<'_> {
        Tonight {
            night,
            hour: 21,
            weekday: "Tuesday",
            moon: "Waxing gibbous",
            month: 10,
            lat: 51.5,
        }
    }

    #[test]
    fn the_sky_tells_the_time_without_counting_visits() {
        assert_eq!(sky_since(29), Some("One Moon ago"));
        assert_eq!(sky_since(90), Some("A season ago"));
        assert_eq!(sky_since(12), None);
        assert_eq!(year_round("2026-09-29", "2027-10-03"), Some(1));
        assert_eq!(year_round("2026-09-29", "2027-12-03"), None);
        assert_eq!(year_round("2026-09-29", "2026-12-03"), None);
    }

    #[test]
    fn nothing_counts_or_minds_an_absence_or_names_the_feeling() {
        let text = include_str!("../data/wisp-lines.toml").to_lowercase();
        let lines: Vec<&str> = text
            .lines()
            .filter(|l| !l.trim_start().starts_with('#'))
            .collect();
        let said = lines.join("\n");
        for word in UNSAID {
            assert!(!said.contains(word), "{word}");
        }
        for word in [
            "haven't",
            "missed",
            "streak",
            "in a row",
            "visits",
            "times",
            "been away",
            "miss you",
        ] {
            assert!(!said.contains(word), "{word}");
        }
        assert!(!said.chars().any(|c| c.is_ascii_digit()));
    }

    #[test]
    fn greetings_fit_the_night_and_hold_all_evening() {
        let lines = Lines::bundled();
        let mut seen = std::collections::HashSet::new();
        for d in 1..=28 {
            let night = format!("2026-10-{d:02}");
            let t = tonight(&night);
            let g = lines.greeting(&t);
            assert_eq!(g, lines.greeting(&t));
            assert!(!g.contains("full") && !g.contains("thin Moon"), "{g}");
            seen.insert(g.to_owned());
        }
        assert!(seen.len() > 4, "{seen:?}");
        let full = Tonight {
            moon: "Full Moon",
            ..tonight("2026-10-26")
        };
        let any_full = (1..=28).any(|d| {
            let night = format!("2026-11-{d:02}");
            lines
                .greeting(&Tonight {
                    night: &night,
                    ..full
                })
                .contains("full")
        });
        assert!(any_full);
    }

    #[test]
    fn seasons_turn_for_each_hemisphere() {
        assert_eq!(season(1, 51.5), Some(Season::Winter));
        assert_eq!(season(1, -33.9), Some(Season::Summer));
        assert_eq!(season(10, -33.9), Some(Season::Spring));
        assert_eq!(season(10, 1.3), None);
        // The September equinox 2026 is on the 23rd.
        let after = midnight_utc(2026, 9, 25);
        let (kind, at) = turned(after, 51.5).expect("an equinox");
        assert_eq!(kind, TurnKind::Autumn);
        assert!((at - midnight_utc(2026, 9, 23)).abs() < DAY, "{at}");
        assert_eq!(turned(after, -33.9).map(|t| t.0), Some(TurnKind::Spring));
        assert_eq!(turned(midnight_utc(2026, 10, 10), 51.5), None);
        let lines = Lines::bundled();
        let line = lines.turn(kind, at, after, 0);
        assert!(line.contains("on Wednesday"), "{line}");
    }

    #[test]
    fn teaching_lines_keep_their_blanks() {
        let teach = Lines::bundled().teach;
        assert!(teach.things.contains("{n}") && teach.list.contains("{n}"));
        assert!(teach.setting_one.contains("{weight}"));
    }

    fn utc(_: UnixMs) -> i32 {
        0
    }

    fn arrival<'a>(sky: &'a Sky, night: &'a str, now: UnixMs) -> Arrival<'a> {
        Arrival {
            tonight: tonight(night),
            sky,
            observer: LONDON,
            now,
            zone: &utc,
            first: None,
            last: None,
            first_find: None,
            people: &[],
            seen: &[],
        }
    }

    #[test]
    fn after_a_while_away_the_hello_says_what_the_sky_did() {
        let sky = Sky::bundled();
        let lines = Lines::bundled();
        let now = midnight_utc(2026, 12, 14) + 21 * HOUR;
        let away = Arrival {
            last: Some("2026-08-14"),
            ..arrival(&sky, "2026-12-14", now)
        };
        let hello = lines.hello(&away);
        assert!(
            hello.starts_with("Hello. Since you were last here"),
            "{hello}"
        );
        let yesterday = Arrival {
            last: Some("2026-12-13"),
            ..arrival(&sky, "2026-12-14", now)
        };
        assert_eq!(lines.hello(&yesterday), lines.greeting(&yesterday.tonight));
    }

    #[test]
    fn the_year_coming_round_is_said_once_a_year() {
        let sky = Sky::bundled();
        let lines = Lines::bundled();
        let now = midnight_utc(2026, 12, 14) + 21 * HOUR;
        let people = [Person {
            name: "Sam".into(),
            mentions: vec![crate::journal::Mention {
                night: "2025-12-10".into(),
                question: String::new(),
            }],
            ..Person::default()
        }];
        let round = Arrival {
            first: Some("2025-12-10"),
            last: Some("2026-12-13"),
            people: &people,
            ..arrival(&sky, "2026-12-14", now)
        };
        match lines.one_more_thing(&round) {
            Some(More::Round { key, text }) => {
                assert_eq!(key, "round-1");
                assert!(text.contains("named Sam"), "{text}");
            }
            other => panic!("{other:?}"),
        }
        let found = Arrival {
            people: &[],
            first_find: Some("The Pleiades"),
            ..round
        };
        match lines.one_more_thing(&found) {
            Some(More::Round { text, .. }) => {
                assert!(text.contains("found the Pleiades"), "{text}")
            }
            other => panic!("{other:?}"),
        }
        let seen = ["round-1".to_owned()];
        let again = Arrival {
            seen: &seen,
            ..found
        };
        assert_eq!(lines.one_more_thing(&again), None);
    }

    #[test]
    fn an_equinox_is_marked_once_since_the_last_evening() {
        let sky = Sky::bundled();
        let lines = Lines::bundled();
        let now = midnight_utc(2026, 9, 25) + 21 * HOUR;
        let before = Arrival {
            last: Some("2026-09-20"),
            ..arrival(&sky, "2026-09-25", now)
        };
        assert!(matches!(lines.one_more_thing(&before), Some(More::Turn(_))));
        let since = Arrival {
            last: Some("2026-09-24"),
            ..arrival(&sky, "2026-09-25", now)
        };
        assert_eq!(lines.one_more_thing(&since), None);
    }

    #[test]
    fn something_has_risen_after_a_few_months() {
        let sky = Sky::bundled();
        let now = midnight_utc(2026, 12, 14) + 21 * HOUR;
        let then = midnight_utc(2026, 8, 14) + 21 * HOUR;
        let name = risen(&sky, LONDON, then, now, &[]).expect("something new");
        assert!(!name.is_empty());
        assert_ne!(
            risen(&sky, LONDON, then, now, std::slice::from_ref(&name)),
            Some(name)
        );
        assert_eq!(risen(&sky, LONDON, now - DAY, now, &[]), None);
    }
}
