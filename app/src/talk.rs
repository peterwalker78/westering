//! The quiet conversation: setting weights down, what the sky asks, plans
//! and the odd check-back. The window shows whatever prompt is current and
//! hands the answers back here.

use crate::game::{Game, Look};
use std::cell::RefCell;
use westering_core::care::Care;
use westering_core::coords::{angles, apply, from_alt_az, horizon};
use westering_core::events::{Kind as EventKind, SkyEvent, upcoming};
use westering_core::finds::Target;
use westering_core::journal::{Answer, Asked, Check, Journal, Mention, NightWeight, Plan};
use westering_core::questions::{
    self, AnswerKind, Chosen, Context, Question, course_to_check, days_between, key_of,
    plan_nearby, plan_to_ask_about, weight_to_look_back,
};
use westering_core::time::UnixMs;

/// What the window shows at the foot of the sky.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Prompt {
    pub text: String,
    pub placeholder: String,
    pub chips: Vec<String>,
    pub entry: bool,
    pub hint: String,
    /// Offer names already used as the user types.
    pub names: bool,
    /// The key shown on a chip, when it isn't its number.
    pub chip_keys: Vec<String>,
}

/// How a page asks the window to go somewhere: None is back to the sky.
pub type Go = RefCell<Option<Box<dyn Fn(Option<Request>)>>>;

/// Something for the window to open.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    /// The logbook, at a night's page or a contents page.
    Book(Option<String>),
    /// Chart a course for a weight.
    Course(u32),
    Settings,
}

pub(crate) enum Flow {
    Weight {
        count: usize,
        offered: Vec<u32>,
    },
    Place {
        weight: u32,
        count: usize,
    },
    Question {
        chosen: Box<Chosen>,
        star: Option<u16>,
    },
    NameStar {
        name: String,
        hr: u16,
    },
    PlanWho {
        plan: u32,
    },
    PlanOutcome {
        plan: u32,
    },
    CourseCheck {
        course: u32,
    },
    LookBack {
        weight: u32,
    },
    DrawingName,
    /// Outside to look, or off to bed.
    Ending,
    /// Whether to wind down slowly, with breaths and a few thoughts.
    WindChoice,
}

/// A question waiting its turn: it shows a moment after the card.
pub(crate) enum Pending {
    Question {
        chosen: Box<Chosen>,
        star: Option<u16>,
    },
    PlanOutcome(u32),
    CourseCheck(u32),
    LookBack(u32),
}

#[derive(Default)]
pub struct Talk {
    pub(crate) flow: Option<Flow>,
    pub(crate) prompt: Option<Prompt>,
    pub(crate) serial: u64,
    pub(crate) pending: Option<(UnixMs, Pending)>,
    pub(crate) asked_now: Vec<String>,
    pub(crate) bank: Vec<Question>,
    pub(crate) events: Vec<SkyEvent>,
    pub(crate) care: Option<Care>,
    pub(crate) caring: bool,
    pub(crate) first_night: bool,
    pub(crate) checked_back: bool,
    /// Whether a question was up at the last tick, to notice it going.
    pub(crate) asking: bool,
}

const MAX_WEIGHTS: usize = 3;

/// How an old weight sits now.
const LOOKS: [&str; 4] = ["Lighter", "Much the same", "Heavier", "It's behind me"];
pub(crate) const LOOK_HEAVIER: usize = 2;
pub(crate) const LOOK_BEHIND: usize = 3;

/// "The Moon" becomes "the Moon"; names stay as they are.
pub(crate) fn lower_first(name: &str) -> String {
    match name.strip_prefix("The ") {
        Some(rest) => format!("the {rest}"),
        None => name.to_owned(),
    }
}

impl Talk {
    pub fn new(journal: &Journal, night: &str, events: Vec<SkyEvent>) -> Talk {
        Talk {
            bank: questions::bundled(),
            events,
            care: Some(Care::bundled()),
            first_night: journal.nights().iter().all(|n| n == night),
            ..Talk::default()
        }
    }
}

/// The sky's calendar for the next two months.
pub fn calendar(showers: &[westering_core::catalogues::Shower], now: UnixMs) -> Vec<SkyEvent> {
    upcoming(showers, now, 60)
}

impl Game {
    pub fn prompt(&self) -> (u64, Option<Prompt>) {
        (self.talk.serial, self.talk.prompt.clone())
    }

    pub(crate) fn set_prompt(&mut self, prompt: Option<Prompt>) {
        self.talk.prompt = prompt;
        self.talk.serial += 1;
        // Whatever the wisp was saying about the last one is over.
        self.guide_prompt_gone(self.last_real);
    }

    pub fn take_request(&mut self) -> Option<Request> {
        self.request.take()
    }

    pub fn journal(&self) -> &Journal {
        &self.journal
    }

    pub fn journal_mut(&mut self) -> &mut Journal {
        &mut self.journal
    }

    pub fn night(&self) -> &str {
        &self.night
    }

    /// Clears the journal and tonight's page with it.
    pub fn forget_everything(&mut self) -> std::io::Result<()> {
        self.journal.forget_everything()?;
        self.refresh();
        Ok(())
    }

    /// Picks up the journal again after something in it was deleted or
    /// restored: tonight's page, and what's been found tonight.
    pub fn refresh(&mut self) {
        self.page =
            self.journal
                .night(&self.night)
                .unwrap_or_else(|| westering_core::journal::Night {
                    key: self.night.clone(),
                    ..Default::default()
                });
        let night = self.night.clone();
        for (i, f) in self.finds.iter().enumerate() {
            self.caught[i] = self.journal.found_on(&f.id, &night);
        }
    }

    /// Replaces everything kept with a backup.
    pub fn restore(&mut self, text: &str) -> std::io::Result<usize> {
        let n = self.journal.restore(text)?;
        self.refresh();
        Ok(n)
    }

    /// A star's everyday name, if it has one.
    pub fn star_name(&self, hr: u16) -> Option<String> {
        self.sky.lists.star_name(hr).map(|s| s.name.clone())
    }

    /// Names already used that begin with what's typed.
    pub fn names_like(&self, prefix: &str) -> Vec<String> {
        self.journal
            .names_like(prefix)
            .into_iter()
            .filter(|n| !n.eq_ignore_ascii_case(prefix.trim()))
            .take(4)
            .map(str::to_owned)
            .collect()
    }

    pub(crate) fn typing(&self) -> bool {
        self.talk.prompt.as_ref().is_some_and(|p| p.entry)
    }

    /// The words of the weight being hung, while it's being hung.
    pub(crate) fn placing_words(&self) -> Option<String> {
        match self.talk.flow {
            Some(Flow::Place { weight, .. }) => self.journal.weight(weight).map(|w| w.text.clone()),
            _ => None,
        }
    }

    pub(crate) fn placing(&self) -> bool {
        matches!(self.talk.flow, Some(Flow::Place { .. }))
    }

    fn care_check(&mut self, text: &str) {
        if !self.talk.caring
            && let Some(care) = &self.talk.care
            && care.concerning(text)
        {
            self.talk.caring = true;
        }
    }

    // The weights.

    pub(crate) fn begin_weights(&mut self, real: UnixMs) {
        if !self.page.weights.is_empty() {
            self.finish_weights(real);
            return;
        }
        self.weight_prompt(0);
        self.guide_weights(real);
    }

    fn weight_prompt(&mut self, count: usize) {
        let tonight: Vec<u32> = self.page.weights.iter().map(|w| w.weight).collect();
        let offered: Vec<u32> = self
            .journal
            .open_weights(&self.night)
            .into_iter()
            .filter(|w| !tonight.contains(&w.id))
            .take(3)
            .map(|w| w.id)
            .collect();
        let mut chips: Vec<String> = offered
            .iter()
            .filter_map(|id| self.journal.weight(*id))
            .map(|w| format!("Still this: {}", w.text))
            .collect();
        chips.push(
            if count == 0 {
                "Nothing tonight"
            } else {
                "That's all"
            }
            .to_owned(),
        );
        let text = if count == 0 {
            "Is anything weighing on you tonight? Write it in a few words and it becomes a star low in the west of this sky. At the end of tonight's visit you'll watch it set: a way of putting it down for the night."
        } else {
            "It's waiting low in the west to set. Anything else on your mind, or is that all?"
        };
        self.talk.flow = Some(Flow::Weight { count, offered });
        self.set_prompt(Some(Prompt {
            text: text.into(),
            placeholder: "One thing, in a few words".into(),
            chips,
            entry: true,
            hint: "Enter to set it down · Esc when that's all".into(),
            names: false,
            ..Prompt::default()
        }));
    }

    fn start_placing(&mut self, weight: u32, count: usize, real: UnixMs) {
        self.talk.flow = Some(Flow::Place { weight, count });
        let words = self
            .journal
            .weight(weight)
            .map(|w| w.text.clone())
            .unwrap_or_default();
        // Said in full every time: what the glowing star is, where it goes,
        // and why.
        self.set_prompt(Some(Prompt {
            text: format!(
                "“{words}” is now the warm star in the ring. Move it low over the western horizon: stars there are the next to set, so at the end of tonight's visit you'll watch it go down."
            ),
            placeholder: String::new(),
            chips: vec!["Leave it here".into()],
            entry: false,
            hint: "Arrows move it · Enter leaves it here".into(),
            names: false,
            chip_keys: vec!["Enter".into()],
        }));
        self.look = Some(Look {
            az: 262.0,
            alt: 9.0,
            fov: 80.0,
            rate: 1.1,
        });
        self.guide_placing(real);
    }

    /// Fixes the weight being placed where the reticle is.
    pub(crate) fn place_weight(&mut self, real: UnixMs) {
        let Some(Flow::Place { weight, count }) = self.talk.flow.take() else {
            return;
        };
        let now = self.clock.sky(real);
        let hz = horizon(self.observer, now);
        let eq = apply(
            &crate::game::transpose(&hz),
            from_alt_az(self.camera.alt, self.camera.az),
        );
        let (ra, dec) = angles(eq);
        self.page.weights.push(NightWeight { weight, ra, dec });
        self.save_page_now();
        self.guide_placed();
        if count + 1 >= MAX_WEIGHTS {
            self.finish_weights(real);
        } else {
            self.weight_prompt(count + 1);
        }
    }

    pub(crate) fn finish_weights(&mut self, real: UnixMs) {
        self.hush();
        self.talk.flow = None;
        self.set_prompt(None);
        self.hint = None;
        if let Some(p) = self.session.weights_done(real) {
            self.entered(p, real);
        }
        self.face_first_find(real);
    }

    fn add_weight(&mut self, text: &str, count: usize, real: UnixMs) {
        self.care_check(text);
        let id = self.journal.add_weight(text, &self.night);
        self.save_weights_now();
        self.wisp_gesture(westering_core::wisp::Gesture::Nod, real);
        self.start_placing(id, count, real);
    }

    // What the sky asks.

    /// After something is caught: a check-back, or a question, a moment later.
    pub(crate) fn after_catch(&mut self, i: usize, real: UnixMs) {
        let today = self.night.clone();
        if !self.talk.checked_back
            && self.journal.settings.ask != westering_core::journal::Ask::Never
        {
            self.talk.checked_back = true;
            let recently = |journal: &Journal, id: &str| {
                journal
                    .asked
                    .iter()
                    .any(|a| a.question == id && days_between(&a.night, &today) < 30)
            };
            if let Some(plan) = plan_to_ask_about(&self.journal.plans, &today)
                && !recently(&self.journal, &format!("plan:{}", plan.id))
            {
                self.talk.pending = Some((real + 2_200, Pending::PlanOutcome(plan.id)));
                return;
            }
            if let Some(course) = course_to_check(&self.journal.courses, &today) {
                self.talk.pending = Some((real + 2_200, Pending::CourseCheck(course.id)));
                return;
            }
            if let Some(weight) =
                weight_to_look_back(&self.journal.weights, &self.journal.courses, &today)
            {
                self.talk.pending = Some((real + 2_200, Pending::LookBack(weight.id)));
                return;
            }
        }
        let find = self.finds[i].clone();
        let triggers = questions::triggers(&find, &self.sky);
        let star = match find.target {
            Target::Star(hr) => Some(hr),
            _ => None,
        };
        if let Some(chosen) = self.choose(&triggers) {
            self.talk.pending = Some((
                real + 2_500,
                Pending::Question {
                    chosen: Box::new(chosen),
                    star,
                },
            ));
        }
    }

    pub(crate) fn choose(&self, triggers: &[&str]) -> Option<Chosen> {
        let people: Vec<&westering_core::journal::Person> = self
            .journal
            .people
            .iter()
            .filter(|p| !p.mentions.is_empty())
            .collect();
        let ctx = Context {
            night: &self.night,
            ask: self.journal.settings.ask,
            first_night: self.talk.first_night,
            asked_now: &self.talk.asked_now,
            asked: &self.journal.asked,
            has_weights: !self.page.weights.is_empty(),
            people: &people,
            plans: &self.journal.plans,
            events: &self.talk.events,
            now: self.clock.sky(self.last_real),
            offset_s: self.offset_s,
        };
        questions::choose(&self.talk.bank, triggers, &ctx)
    }

    /// Shows a waiting question once its moment comes.
    pub(crate) fn tick_talk(&mut self, real: UnixMs) {
        // Free look is quiet: one thing is asked an evening at most, in a
        // pause after a close look, and the rest waits for the guided way.
        let free = self.free_look();
        if free && (self.free_asked || self.free_moment.is_none_or(|at| real < at)) {
            return;
        }
        let due = self
            .talk
            .pending
            .as_ref()
            .is_some_and(|(at, _)| real >= *at);
        // Waits for the card to be put away, so Space isn't typed into the answer.
        if !due
            || self.talk.prompt.is_some()
            || self.drawing.is_some()
            || self.card.is_some()
            || self.tour.is_some()
            || !self.hunting()
        {
            return;
        }
        let Some((_, pending)) = self.talk.pending.take() else {
            return;
        };
        if free {
            self.free_asked = true;
            self.free_moment = None;
        }
        match pending {
            Pending::Question { chosen, star } => {
                self.show_question(*chosen, star);
                self.guide_question(real);
            }
            Pending::PlanOutcome(id) => {
                let Some(plan) = self.journal.plans.iter().find(|p| p.id == id) else {
                    return;
                };
                let who = plan
                    .who
                    .as_deref()
                    .map(|w| format!(", with {w}"))
                    .unwrap_or_default();
                let text = format!("Your plan for {}{who}: how did it go?", plan.event);
                self.remember_asked(&format!("plan:{id}"));
                self.talk.flow = Some(Flow::PlanOutcome { plan: id });
                self.set_prompt(Some(Prompt {
                    text,
                    placeholder: "A line about it, if you like".into(),
                    chips: vec!["It happened".into(), "Didn't happen".into()],
                    entry: true,
                    hint: "Enter to keep it · Esc to let it pass".into(),
                    names: false,
                    chip_keys: Vec::new(),
                }));
            }
            Pending::LookBack(id) => {
                let Some(weight) = self.journal.weight(id) else {
                    return;
                };
                let text = weight.text.clone();
                let when = weight.nights.last().cloned().unwrap_or_default();
                // The night it was set down, as the sky was that night.
                let page = self.journal.night(&when);
                let date = {
                    let mut p = when.split('-').map(|x| x.parse::<u32>().unwrap_or(1));
                    let (_, m, d) = (p.next(), p.next().unwrap_or(1), p.next().unwrap_or(1));
                    format!(
                        "{d} {}",
                        westering_core::time::MONTHS[(m.clamp(1, 12) - 1) as usize]
                    )
                };
                let found = page
                    .as_ref()
                    .and_then(|p| p.finds.first())
                    .map(|f| format!(", the night you found {}", lower_first(f)))
                    .unwrap_or_default();
                self.remember_asked(&format!("look-back:{id}"));
                self.talk.flow = Some(Flow::LookBack { weight: id });
                self.set_prompt(Some(Prompt {
                    text: match westering_core::journey::sky_since(days_between(&when, &self.night)) {
                        Some(ago) => format!("{ago}, on {date}{found}, you wrote down “{text}” as something on your mind. How does it feel now?"),
                        None => format!("On {date}{found}, you wrote down “{text}” as something on your mind. How does it feel now?"),
                    },
                    placeholder: String::new(),
                    chips: LOOKS.iter().map(|s| (*s).to_owned()).collect(),
                    entry: false,
                    hint: "Esc to let it pass".into(),
                    names: false,
                    chip_keys: Vec::new(),
                }));
                self.guide_look_back(real);
            }
            Pending::CourseCheck(id) => {
                let Some(course) = self.journal.courses.iter().find(|c| c.id == id) else {
                    return;
                };
                let weight = self
                    .journal
                    .weight(course.weight)
                    .map(|w| w.text.clone())
                    .unwrap_or_default();
                self.remember_asked(&format!("course:{id}"));
                self.talk.flow = Some(Flow::CourseCheck { course: id });
                self.set_prompt(Some(Prompt {
                    text: format!(
                        "A while ago you charted a course for “{weight}”. How's it going?"
                    ),
                    placeholder: String::new(),
                    chips: vec![
                        "Going well".into(),
                        "Mixed".into(),
                        "Not yet".into(),
                        "Change the plan".into(),
                    ],
                    entry: false,
                    hint: "Esc to let it pass".into(),
                    names: false,
                    chip_keys: Vec::new(),
                }));
            }
        }
    }

    fn remember_asked(&mut self, id: &str) {
        self.talk.asked_now.push(id.to_owned());
        self.journal.asked.push(Asked {
            question: id.to_owned(),
            night: self.night.clone(),
        });
        if let Err(e) = self.journal.save_asked() {
            eprintln!("westering: couldn't save what was asked: {e}");
        }
    }

    pub(crate) fn show_question(&mut self, chosen: Chosen, star: Option<u16>) {
        self.remember_asked(&chosen.question.id.clone());
        let (placeholder, names) = match (chosen.question.answer, chosen.question.ask.as_deref()) {
            (AnswerKind::Name, _) | (AnswerKind::Plan, Some("who")) => ("A name", true),
            (AnswerKind::Plan, _) => ("What you'd do", false),
            (AnswerKind::Text, _) => ("A few words", false),
        };
        let text = chosen.text.clone();
        self.talk.flow = Some(Flow::Question {
            chosen: Box::new(chosen),
            star,
        });
        self.set_prompt(Some(Prompt {
            text,
            placeholder: placeholder.into(),
            chips: Vec::new(),
            entry: true,
            hint: "Enter to keep it · Esc to let it pass".into(),
            names,
            chip_keys: Vec::new(),
        }));
    }

    fn keep_answer(
        &mut self,
        chosen: &Chosen,
        text: &str,
        person: Option<&str>,
        star: Option<u16>,
    ) {
        self.page.answers.push(Answer {
            question: chosen.question.id.clone(),
            prompt: chosen.text.clone(),
            text: text.to_owned(),
            person: person.map(str::to_owned),
            star,
        });
        if let Some(name) = person {
            self.journal.person_mut(name).mentions.push(Mention {
                night: self.night.clone(),
                question: chosen.question.id.clone(),
            });
            self.save_people_now();
            self.wisp_moved(self.last_real);
        } else {
            self.wisp_gesture(westering_core::wisp::Gesture::Nod, self.last_real);
        }
        self.save_page_now();
    }

    fn make_plan(&mut self, what: &str, who: Option<&str>, event: &SkyEvent) -> u32 {
        let id = Journal::next_id(self.journal.plans.iter().map(|p| p.id));
        self.journal.plans.push(Plan {
            id,
            what: what.to_owned(),
            who: who.map(str::to_owned),
            date: key_of(event.at, self.offset_s),
            event: event.title.clone(),
            made: self.night.clone(),
            outcome: None,
            note: None,
        });
        self.page.plans.push(id);
        self.wisp_gesture(westering_core::wisp::Gesture::Glow, self.last_real);
        if let Some(name) = who {
            self.journal.person_mut(name).mentions.push(Mention {
                night: self.night.clone(),
                question: "plan".into(),
            });
            self.save_people_now();
        }
        if let Err(e) = self.journal.save_plans() {
            eprintln!("westering: couldn't save the plan: {e}");
        }
        self.save_page_now();
        id
    }

    /// Adds who a plan is with, once they're named.
    fn plan_with(&mut self, plan: u32, who: &str) {
        if let Some(p) = self.journal.plans.iter_mut().find(|p| p.id == plan) {
            p.who = Some(who.to_owned());
        }
        self.journal.person_mut(who).mentions.push(Mention {
            night: self.night.clone(),
            question: "plan".into(),
        });
        self.save_people_now();
        if let Err(e) = self.journal.save_plans() {
            eprintln!("westering: couldn't save the plan: {e}");
        }
        self.save_page_now();
    }

    fn done_talking(&mut self) {
        self.talk.flow = None;
        self.set_prompt(None);
    }

    pub fn answer_text(&mut self, text: &str, real: UnixMs) {
        let text = text.trim();
        let Some(flow) = self.talk.flow.take() else {
            return;
        };
        match flow {
            Flow::Weight { count, .. } => {
                if text.is_empty() {
                    self.finish_weights(real);
                } else {
                    self.add_weight(text, count, real);
                }
            }
            Flow::Question { chosen, star } => {
                if text.is_empty() {
                    self.done_talking();
                    return;
                }
                self.care_check(text);
                match (chosen.question.answer, chosen.question.ask.as_deref()) {
                    (AnswerKind::Plan, Some("who")) => {
                        let event = chosen.event.clone().expect("plans carry their event");
                        let what = match event.kind {
                            EventKind::FullMoon => "Seeing the full Moon".to_owned(),
                            _ => format!("Watching {}", event.title),
                        };
                        self.keep_answer(&chosen, text, Some(text), None);
                        let plan = self.make_plan(&what, None, &event);
                        self.plan_with(plan, text);
                        self.done_talking();
                    }
                    (AnswerKind::Plan, _) => {
                        let event = chosen.event.clone().expect("plans carry their event");
                        self.keep_answer(&chosen, text, None, None);
                        let plan = self.make_plan(text, None, &event);
                        self.talk.flow = Some(Flow::PlanWho { plan });
                        self.set_prompt(Some(Prompt {
                            text: "Anyone with you?".into(),
                            placeholder: "A name, or Esc".into(),
                            chips: Vec::new(),
                            entry: true,
                            hint: "Enter to keep it · Esc if it's just you".into(),
                            names: true,
                            chip_keys: Vec::new(),
                        }));
                    }
                    (AnswerKind::Name, _) => {
                        let person = chosen.person.clone();
                        let name = person.as_deref().unwrap_or(text);
                        self.keep_answer(&chosen, text, Some(name), star);
                        let pole = matches!(chosen.question.on.as_str(), "pole" | "pole-south");
                        match star.or(pole.then_some(if self.observer.lat >= 0.0 {
                            424
                        } else {
                            4730
                        })) {
                            Some(hr) if self.journal.person_on(hr).is_none() => {
                                self.talk.flow = Some(Flow::NameStar {
                                    name: name.to_owned(),
                                    hr,
                                });
                                self.set_prompt(Some(Prompt {
                                    text: "Their star, then?".into(),
                                    placeholder: String::new(),
                                    chips: vec!["Yes".into(), "Leave it".into()],
                                    entry: false,
                                    hint: String::new(),
                                    names: false,
                                    chip_keys: Vec::new(),
                                }));
                            }
                            _ => {
                                self.done_talking();
                                self.guide_answered(real);
                            }
                        }
                    }
                    (AnswerKind::Text, _) => {
                        let person = chosen.person.clone();
                        self.keep_answer(&chosen, text, person.as_deref(), None);
                        self.done_talking();
                        self.guide_answered(real);
                    }
                }
            }
            Flow::PlanWho { plan } => {
                if !text.is_empty() {
                    self.plan_with(plan, text);
                }
                self.done_talking();
            }
            Flow::PlanOutcome { plan } => {
                if let Some(p) = self.journal.plans.iter_mut().find(|p| p.id == plan) {
                    p.outcome = Some("went".into());
                    if !text.is_empty() {
                        p.note = Some(text.to_owned());
                    }
                }
                self.care_check(text);
                let _ = self.journal.save_plans();
                self.done_talking();
            }
            Flow::DrawingName => {
                self.name_drawing(text, real);
            }
            other => {
                self.talk.flow = Some(other);
            }
        }
    }

    pub fn answer_chip(&mut self, chip: usize, real: UnixMs) {
        let Some(flow) = self.talk.flow.take() else {
            return;
        };
        match flow {
            Flow::Weight { count, offered } => {
                if let Some(&id) = offered.get(chip) {
                    self.journal.bring_back(id, &self.night);
                    self.save_weights_now();
                    self.start_placing(id, count, real);
                } else {
                    self.finish_weights(real);
                }
            }
            Flow::Place { weight, count } => {
                self.talk.flow = Some(Flow::Place { weight, count });
                self.place_weight(real);
            }
            Flow::WindChoice => {
                self.done_talking();
                if chip == 0 {
                    self.start_breathing(real);
                } else {
                    self.ask_ending();
                }
            }
            Flow::Ending => {
                let ending = if chip == 0 {
                    crate::game::Ending::Outside
                } else {
                    crate::game::Ending::Bed
                };
                self.ending = Some(ending);
                self.session.ending_chosen(real);
                self.done_talking();
                self.guide_ending(ending, real);
            }
            Flow::NameStar { name, hr } => {
                if chip == 0 {
                    for p in &mut self.journal.people {
                        p.stars.retain(|s| *s != hr);
                    }
                    self.journal.person_mut(&name).stars.push(hr);
                    self.save_people_now();
                }
                self.done_talking();
            }
            Flow::PlanOutcome { plan } => {
                if let Some(p) = self.journal.plans.iter_mut().find(|p| p.id == plan) {
                    p.outcome = Some(if chip == 0 { "went" } else { "didnt" }.into());
                }
                let _ = self.journal.save_plans();
                self.done_talking();
            }
            Flow::LookBack { weight } => {
                let answer = LOOKS.get(chip).copied().unwrap_or(LOOKS[1]);
                let night = self.night.clone();
                if let Some(w) = self.journal.weights.iter_mut().find(|w| w.id == weight) {
                    w.looks.push(Check {
                        night,
                        answer: answer.into(),
                    });
                    if chip == LOOK_BEHIND {
                        w.sorted = true;
                    }
                }
                self.save_weights_now();
                self.done_talking();
                self.guide_looked_back(chip, real);
            }
            Flow::CourseCheck { course } => {
                let answer = ["Going well", "Mixed", "Not yet", "Change the plan"]
                    .get(chip)
                    .copied()
                    .unwrap_or("Mixed");
                let night = self.night.clone();
                let mut weight = None;
                if let Some(c) = self.journal.courses.iter_mut().find(|c| c.id == course) {
                    c.checks.push(Check {
                        night,
                        answer: answer.into(),
                    });
                    weight = Some(c.weight);
                }
                let _ = self.journal.save_courses();
                self.done_talking();
                if chip == 3
                    && let Some(w) = weight
                {
                    self.request = Some(Request::Course(w));
                }
            }
            other => {
                self.talk.flow = Some(other);
            }
        }
    }

    /// Asks how tonight ends, as the winding down begins.
    pub(crate) fn ask_ending(&mut self) {
        self.talk.flow = Some(Flow::Ending);
        self.set_prompt(Some(Prompt {
            text: "How does tonight end? If it's clear, a few minutes under the real sky first can be worth it: something that vast tends to put the day in proportion, and the dark is kinder to sleep than a screen.".into(),
            chips: vec!["A few minutes outside first".into(), "Straight to bed".into()],
            entry: false,
            // The first night, where it's all kept, said here rather than
            // later, once the screen has gone dark.
            hint: if self.talk.first_night {
                "Either way, the screen goes off. Everything from tonight is kept in your logbook: L opens it, any night".into()
            } else {
                "Either way, the screen goes off".into()
            },
            ..Prompt::default()
        }));
    }

    /// The last line when the night ends in bed: something from tonight
    /// worth taking with you, and goodnight.
    pub(crate) fn goodnight(&self) -> westering_core::finale::Handoff {
        let named = self.page.answers.iter().find_map(|a| a.person.clone());
        let planned = self
            .page
            .plans
            .last()
            .and_then(|id| self.journal.plans.iter().find(|p| p.id == *id));
        let line = if let Some(name) = named {
            format!("You thought of {name} tonight. Sleep well.")
        } else if let Some(plan) = planned {
            format!("Something to look forward to: {}. Sleep well.", plan.what)
        } else if !self.page.weights.is_empty() {
            "What you set down tonight has gone down with the sky. Sleep well.".to_owned()
        } else {
            "The sky will keep turning while you sleep. Goodnight.".to_owned()
        };
        westering_core::finale::Handoff {
            line,
            alt: 14.0,
            az: 268.0,
            id: None,
        }
    }

    pub fn skip_prompt(&mut self, real: UnixMs) {
        match self.talk.flow.take() {
            Some(Flow::Weight { .. }) => self.finish_weights(real),
            Some(Flow::DrawingName) => {
                self.drawing = None;
                self.done_talking();
            }
            Some(Flow::WindChoice) => {
                self.done_talking();
                self.ask_ending();
            }
            _ => self.done_talking(),
        }
    }

    /// A second line for the arrival: a plan close at hand, or a named star up.
    pub(crate) fn arrival_extra(&mut self, now: UnixMs) -> Option<String> {
        if let Some(line) = plan_nearby(&self.journal.plans, &self.night) {
            return Some(line);
        }
        let hz = horizon(self.observer, now);
        let night = self.night.clone();
        let shown_lately = |journal: &Journal, id: &str| {
            journal
                .asked
                .iter()
                .any(|a| a.question == id && days_between(&a.night, &night) < 7)
        };
        let up = self.journal.fixed_stars().into_iter().find_map(|p| {
            let hr = *p.stars.first()?;
            let idx = self.sky.stars.index_of(hr)?;
            let alt = westering_core::coords::alt_az(apply(&hz, self.star_dirs[idx])).0;
            (alt > 10.0).then(|| p.name.clone())
        })?;
        let id = format!("star-up:{up}");
        if shown_lately(&self.journal, &id) {
            return None;
        }
        self.journal.asked.push(Asked {
            question: id,
            night: self.night.clone(),
        });
        let _ = self.journal.save_asked();
        Some(format!("{up}'s star is up tonight."))
    }

    pub(crate) fn save_page_now(&self) {
        if let Err(e) = self.journal.save_night(&self.page) {
            eprintln!("westering: couldn't save tonight's page: {e}");
        }
    }

    fn save_weights_now(&self) {
        if let Err(e) = self.journal.save_weights() {
            eprintln!("westering: couldn't save the weights: {e}");
        }
    }

    fn save_people_now(&self) {
        if let Err(e) = self.journal.save_people() {
            eprintln!("westering: couldn't save names: {e}");
        }
    }
}
