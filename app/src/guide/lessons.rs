//! What the wisp says as it shows the way: the first evening's lessons,
//! each said once, and the few words that come back on any evening, on
//! arriving, as it ends, or when someone seems lost.

use super::{Aim, number};
use crate::game::{Ending, Game, Looking, late};
use crate::talk::{Flow, LOOK_BEHIND, LOOK_HEAVIER};
use westering_core::finds::Target;
use westering_core::journey::{Arrival, More, TeachLines, Tonight, season};
use westering_core::questions::AnswerKind;
use westering_core::session::Phase;
use westering_core::time::{UnixMs, civil_date, clock, weekday};

/// How long someone can look around without finding anything before the
/// wisp offers to help.
const STUCK_MS: UnixMs = 35_000;

impl Game {
    fn teach(&self) -> &TeachLines {
        &self.guide.lines.teach
    }

    pub(crate) fn guide_arrival(&mut self, real: UnixMs) {
        let now = self.sky_now(real);
        let (_, month, _) = civil_date(now, self.offset_s(now));
        self.guide.wisp.set_season(season(month, self.observer.lat));
        if !self.seen("hello") {
            self.mark_seen("hello");
            let teach = self.teach();
            let (hello, why, parts) = (teach.hello.clone(), teach.why.clone(), teach.parts.clone());
            self.say_at(Aim::Near(0.36, 0.42), hello, real + 1_200, 7_000);
            self.say_at(Aim::Near(0.4, 0.4), why, real + 1_200, 9_000);
            self.say_at(Aim::Evening, parts, real + 1_200, 9_000);
            return;
        }
        let (hello, more) = self.arrival_words(real);
        let n = self.to_find();
        let teach = self.teach();
        let things = match n {
            0 => String::new(),
            1 => format!(" {}", teach.one_thing),
            n => format!(" {}", teach.things.replace("{n}", &number(n))),
        };
        let text = if n == 0 && !self.finds.is_empty() {
            format!("{hello} {}", teach.found_already)
        } else if self.finds.is_empty() {
            format!("{hello} {}", teach.still_light)
        } else if late(now, self.offset_s(now)) {
            format!("{hello}{things} {}", teach.late)
        } else {
            format!("{hello}{things}")
        };
        self.say_at(Aim::Near(0.3, 0.5), text, real + 1_200, 6_000);
        match more {
            Some(More::Round { key, text }) => {
                self.mark_seen(&key);
                self.say_at(Aim::Near(0.4, 0.4), text, real + 1_200, 10_000);
            }
            Some(More::Turn(text)) => {
                self.say_at(Aim::Near(0.4, 0.4), text, real + 1_200, 9_000);
            }
            Some(More::Star { text, ra, dec }) => {
                self.say_at(Aim::Sky(ra, dec), text, real + 1_200, 8_000);
            }
            None => {}
        }
    }

    /// The evening before this one, and the first, from the logbook.
    fn earlier_nights(&self) -> (Option<String>, Option<String>) {
        let earlier: Vec<String> = self
            .journal
            .nights()
            .into_iter()
            .filter(|k| *k < self.night)
            .collect();
        (earlier.first().cloned(), earlier.last().cloned())
    }

    /// Tonight's hello, and at most one thing more worth saying.
    fn arrival_words(&self, real: UnixMs) -> (String, Option<More>) {
        let now = self.sky_now(real);
        let offset = self.offset_s(now);
        let (hour, _) = clock(now, offset);
        let (_, month, _) = civil_date(now, offset);
        let (last, first) = self.earlier_nights();
        let first_find = first
            .as_deref()
            .and_then(|f| self.journal.night(f))
            .and_then(|p| p.finds.first().cloned());
        let zone = |at| self.offset_s(at);
        let arrival = Arrival {
            tonight: Tonight {
                night: &self.night,
                hour,
                weekday: weekday(now, offset),
                moon: &self.page.moon,
                month,
                lat: self.observer.lat,
            },
            sky: &self.sky,
            observer: self.observer,
            now,
            zone: &zone,
            first: first.as_deref(),
            last: last.as_deref(),
            first_find: first_find.as_deref(),
            people: &self.journal.people,
            seen: &self.journal.settings.seen,
        };
        let lines = &self.guide.lines;
        (lines.hello(&arrival), lines.one_more_thing(&arrival))
    }

    pub(crate) fn guide_weights(&mut self, real: UnixMs) {
        let line = self.teach().weights.clone();
        self.say_once("weights", Aim::Prompt, line, real, 30_000);
    }

    pub(crate) fn guide_placing(&mut self, real: UnixMs) {
        // The prompt says what to do; the wisp only says why, the first time.
        let line = self.teach().placing.clone();
        self.say_once("placing", Aim::Ring, line, real, 12_000);
    }

    pub(crate) fn guide_placed(&mut self) {
        self.hush();
    }

    pub(crate) fn guide_hunt(&mut self, real: UnixMs) {
        self.guide.last_progress = real;
        let n = self.to_find();
        if n == 0 || self.seen("hunt") {
            return;
        }
        self.mark_seen("hunt");
        let teach = self.teach();
        let what = if n == 1 {
            teach.list_one.clone()
        } else {
            teach.list.replace("{n}", &number(n))
        };
        let (compass, point) = (teach.compass.clone(), teach.point.clone());
        self.say_at(Aim::List, what, real + 500, 9_000);
        self.say_at(Aim::Compass, compass, real + 500, 7_000);
        self.say_at(Aim::Near(0.5, 0.35), point, real + 500, 8_000);
    }

    pub(crate) fn guide_caught(&mut self, real: UnixMs) {
        self.guide.cheer_until = real + 4_000;
        self.guide.last_progress = real;
        // Over to the card, pleased.
        self.fly(Aim::Card, real, 4_000);
        let card = self.teach().card.clone();
        if self.say_once("caught", Aim::Card, card, real + 600, 9_000) {
            return;
        }
        let caught = self.finds.iter().filter(|f| f.caught).count();
        if caught >= 2 {
            let draw = self.teach().draw.clone();
            self.say_once("draw", Aim::Near(0.5, 0.35), draw, real + 600, 9_000);
        }
    }

    /// A story or a walk has begun: the wisp goes along, and keeps its tips
    /// for later.
    pub(crate) fn guide_tour_began(&mut self, real: UnixMs) {
        self.guide.cheer_until = real + 3_000;
        self.guide.last_progress = real;
        self.hush();
    }

    /// The list is done. By night the wisp says one line and hands the sky
    /// over to free look.
    pub(crate) fn guide_all_found(&mut self, real: UnixMs) {
        if self.by_day() {
            let done = self.guide.lines.day.done.clone();
            self.say_at(Aim::Near(0.4, 0.4), done, real + 400, 9_000);
            return;
        }
        self.guide.cheer_until = real + 5_000;
        self.set_looking(Looking::Free(Default::default()), real);
        let line = self.guide.lines.free.handover.clone();
        self.say_at(Aim::Near(0.5, 0.35), line, real, 9_000);
    }

    /// More has come out as the sky darkened: news for the list, so not
    /// in free look.
    pub(crate) fn guide_darker(&mut self, n: usize, real: UnixMs) {
        if self.free_look() {
            return;
        }
        let news = &self.guide.lines.news;
        let text = if n == 1 {
            news.darker_one.clone()
        } else {
            news.darker_many.replace("{n}", &number(n).to_lowercase())
        };
        // News, not a matter for the wisp to fly out about.
        self.status_for(text, real, 6_000);
    }

    pub(crate) fn guide_meteor_left(&mut self, real: UnixMs) {
        let line = self.teach().meteor.clone();
        self.say_at(Aim::Near(0.5, 0.3), line, real, 8_000);
    }

    pub(crate) fn guide_question(&mut self, real: UnixMs) {
        let (plan, name) = match &self.talk.flow {
            Some(Flow::Question { chosen, .. }) => (
                chosen.question.answer == AnswerKind::Plan,
                chosen.question.answer == AnswerKind::Name,
            ),
            _ => (false, false),
        };
        let teach = self.teach();
        let (plans, names, question) = (
            teach.plans.clone(),
            teach.names.clone(),
            teach.question.clone(),
        );
        if plan && self.say_once("plans", Aim::Prompt, plans, real + 400, 11_000) {
            return;
        }
        if name
            && self.seen("question")
            && self.say_once("names", Aim::Prompt, names, real + 400, 8_000)
        {
            return;
        }
        self.say_once("question", Aim::Prompt, question, real + 400, 12_000);
    }

    /// After something's been written down, where it all goes.
    pub(crate) fn guide_answered(&mut self, real: UnixMs) {
        let line = self.teach().logbook.clone();
        self.say_once("logbook", Aim::Near(0.3, 0.55), line, real + 800, 11_000);
    }

    pub(crate) fn guide_look_back(&mut self, real: UnixMs) {
        let line = self.teach().look_back.clone();
        self.say_once("look-back", Aim::Prompt, line, real + 400, 11_000);
    }

    /// Tonight's end chosen: what happens now.
    pub(crate) fn guide_ending(&mut self, ending: Ending, real: UnixMs) {
        let teach = self.teach();
        let line = match ending {
            Ending::Outside => teach.outside.clone(),
            Ending::Bed => teach.bed.clone(),
        };
        self.say_at(Aim::Near(0.4, 0.4), line, real + 300, 9_000);
    }

    /// After an old weight has been looked at again. Every answer is met
    /// the same way, so none of them is the right one: a glow, and a word
    /// about where it's kept.
    pub(crate) fn guide_looked_back(&mut self, chip: usize, real: UnixMs) {
        self.wisp_moved(real);
        let teach = self.teach();
        let kept = if chip == LOOK_BEHIND {
            teach.kept_behind.clone()
        } else {
            teach.kept_beside.clone()
        };
        let course = teach.course.clone();
        self.status_for(kept, real, 6_000);
        if chip == LOOK_HEAVIER {
            self.say_once(
                "course-offer",
                Aim::Near(0.3, 0.55),
                course,
                real + 500,
                10_000,
            );
        }
    }

    pub(crate) fn guide_drawing(&mut self, real: UnixMs) {
        let line = self.teach().drawing.clone();
        self.say_at(Aim::Ring, line, real, 600_000);
    }

    pub(crate) fn guide_phase(&mut self, phase: Phase, real: UnixMs) {
        match phase {
            Phase::Dimming => {
                let line = self.teach().dimming.clone();
                self.say_at(Aim::Prompt, line, real + 600, 10_000);
            }
            Phase::Finale => {
                let first = self
                    .page
                    .weights
                    .first()
                    .and_then(|w| self.journal.weight(w.weight))
                    .map(|w| w.text.clone());
                let teach = self.teach();
                if let Some(first) = first {
                    let line = teach.setting_one.replace("{weight}", &first);
                    self.say_at(Aim::Weights, line, real + 800, 9_000);
                } else if self.page.weights.is_empty() {
                    let line = teach.turning.clone();
                    self.say_at(Aim::Near(0.4, 0.45), line, real + 800, 6_000);
                } else {
                    let line = teach.setting.clone();
                    self.say_at(Aim::Weights, line, real + 800, 9_000);
                }
            }
            Phase::LightsOut => {
                let line = self.teach().goodnight.clone();
                self.say_at(Aim::Home, line, real, 5_000);
            }
            _ => {}
        }
    }

    /// The tour's own words, unasked: what the ring is for, the first time
    /// something's in it, and on the very first night a nudge if nothing at
    /// all has happened for a while.
    pub(super) fn guide_nudge(&mut self, touring: bool, real: UnixMs) {
        if !touring {
            return;
        }
        if self.ring_target().is_some() {
            let line = self.teach().ring.clone();
            self.say_once("ring", Aim::Ring, line, real, 8_000);
            return;
        }
        // Once, and otherwise the wisp waits to be asked.
        if self.to_find() > 0
            && self.talk.first_night
            && self.guide.last_nudge == 0
            && real - self.guide.last_progress.max(self.last_input) > STUCK_MS
        {
            self.guide.last_nudge = real;
            let only_meteor = (0..self.finds.len())
                .filter(|&i| !self.finds[i].caught)
                .all(|i| matches!(self.finds[i].target, Target::Meteor(_)));
            if only_meteor {
                self.guide_meteor_left(real);
            } else {
                let line = self.teach().lost.clone();
                self.say_at(Aim::List, line, real, 8_000);
            }
        }
    }
}
