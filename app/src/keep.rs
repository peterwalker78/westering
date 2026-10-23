//! Keeping someone company in the background. The evening holds where it
//! is, the sky carries on as it was, the music plays on, and every half
//! hour or so the wisp looks in with a word. If the window is out of sight
//! the music softens for a moment instead, and the word waits for when
//! it's seen. It works in a window of any size: everything but the sky,
//! the wisp and one quiet line steps aside.

use crate::game::Game;
use crate::guide::Aim;
use westering_core::company::{Company, Lines, Look};
use westering_core::coords::{angles, observe};
use westering_core::finale::whereabouts;
use westering_core::journey::risen;
use westering_core::session::Phase;
use westering_core::sky::see;
use westering_core::time::{UnixMs, clock};
use westering_core::wisp::Gesture;

/// How long the music softens when the wisp looks in.
const DIP_MS: UnixMs = 7_000;
/// How long a word stays up when it looks in: long enough to be seen by
/// someone who glances over now and then.
const HOLD_MS: UnixMs = 90_000;

pub struct Keep {
    pub(crate) company: Option<Company>,
    pub(crate) lines: Lines,
    /// 0-1, eased: how far the sky has settled into the background.
    pub(crate) mix: f64,
    /// A word waiting for the window to be seen.
    pending: Option<String>,
    dip_until: UnixMs,
    /// What's been said to have come up, so it isn't said twice.
    risen: Vec<String>,
    /// The sky's time when the company began.
    from: UnixMs,
    last: UnixMs,
    /// Asked for while the wisp was still saying hello: it starts as soon
    /// as the evening does.
    want: bool,
}

impl Keep {
    pub fn new() -> Keep {
        Keep {
            company: None,
            lines: Lines::bundled(),
            mix: 0.0,
            pending: None,
            dip_until: 0,
            risen: Vec::new(),
            from: 0,
            last: 0,
            want: false,
        }
    }
}

impl Game {
    pub(crate) fn keeping(&self) -> bool {
        self.keep.company.is_some()
    }

    /// K: keeps the user company, or brings the sky back.
    pub fn toggle_company(&mut self, real: UnixMs) {
        if self.keeping() {
            self.stop_company(real);
            let back = self.keep.lines.back.clone();
            self.say_at(Aim::Home, back, real, 4_000);
            return;
        }
        // Winding down, or at the night's end: K means company instead.
        if self.winding()
            || matches!(
                self.session.phase(),
                Phase::Dimming | Phase::Finale | Phase::LightsOut
            )
        {
            self.wind_cancel();
            self.talk.flow = None;
            self.set_prompt(None);
            self.ending = None;
            self.handoff = None;
            self.session.reopen(real);
        }
        // Still arriving: the hello can wait, and company begins as soon as
        // the evening does.
        if self.session.phase() == Phase::Arrival {
            self.keep.want = true;
            self.hush();
            return;
        }
        // From the menu it can start while something's being asked: that
        // just passes, as Esc would.
        if self.session.phase() == Phase::Weights && !self.placing() {
            self.finish_weights(real);
        }
        if self.session.phase() == Phase::Hunt && self.talk.prompt.is_some() {
            self.skip_prompt(real);
        }
        let free = self.session.phase() == Phase::Hunt
            && self.talk.prompt.is_none()
            && self.drawing.is_none()
            && !self.placing();
        if !free {
            let text = self.keep.lines.not_yet.clone();
            self.say(text, real, 6_000);
            return;
        }
        self.end_trail(real);
        self.card = None;
        self.caption = None;
        self.hint = None;
        self.session.hold(real);
        self.keep.company = Some(Company::new(self.clock.sky(real), &self.night));
        self.keep.from = self.sky_now(real);
        self.keep.risen.clear();
        self.hush();
        let start = self.keep.lines.start.clone();
        self.say_at(Aim::Home, start, real + 400, 12_000);
    }

    pub(crate) fn stop_company(&mut self, real: UnixMs) {
        if self.keep.company.take().is_some() {
            self.session.resume(real);
            self.keep.pending = None;
            self.hush();
        }
    }

    /// Looks in when it's time. `seen` is whether the window is on the
    /// screen: if not, the music softens and the word waits.
    pub fn company_tick(&mut self, real: UnixMs, seen: bool) {
        let dt = 1.0 - (-(real - self.keep.last).clamp(0, 500) as f64 / 1_500.0).exp();
        self.keep.last = real;
        let target = if self.keeping() { 1.0 } else { 0.0 };
        self.keep.mix += (target - self.keep.mix) * dt;
        if self.keep.want && matches!(self.session.phase(), Phase::Weights | Phase::Hunt) {
            self.keep.want = false;
            self.toggle_company(real);
        }
        if !self.keeping() {
            return;
        }
        if seen && let Some(text) = self.keep.pending.take() {
            self.say_at(Aim::Home, text, real + 600, HOLD_MS);
            self.wisp_gesture(Gesture::Glow, real);
            return;
        }
        let now = self.clock.sky(real);
        let hour = clock(now, self.offset_s(now));
        if !self.keep.company.as_ref().is_some_and(|c| c.due(now, hour)) {
            return;
        }
        let up = risen(
            &self.sky,
            self.observer,
            self.keep.from,
            now,
            &self.keep.risen,
        );
        // Someone named on more than one night, a different one each time.
        let people = self.journal.fixed_stars();
        let person = (!people.is_empty())
            .then(|| people[(now / 60_000) as usize % people.len()].name.clone());
        let by_day = self.by_day();
        let Some(company) = &mut self.keep.company else {
            return;
        };
        let Some(look) = company.tick(
            now,
            hour,
            up.as_deref(),
            person.as_deref(),
            by_day,
            &self.keep.lines,
        ) else {
            return;
        };
        let lines = &self.keep.lines;
        let text = match look {
            Look::Care(text) => text,
            Look::Late => lines.late.clone(),
            Look::Risen(name) => {
                self.keep.risen.push(name.clone());
                let place = self.place_of(&name, now);
                lines
                    .risen
                    .replace("{name}", &name)
                    .replace("{where}", &place)
            }
            Look::Closing => {
                let text = lines.closing.clone();
                self.stop_company(real);
                self.say_at(Aim::Home, text, real, 8_000);
                if self.by_day() {
                    self.day_end(real);
                } else {
                    self.wind_down(real);
                }
                return;
            }
        };
        self.keep.dip_until = real + DIP_MS;
        if seen {
            self.say_at(Aim::Home, text, real + 1_500, HOLD_MS);
            self.wisp_gesture(Gesture::Glow, real + 1_500);
        } else {
            self.keep.pending = Some(text);
        }
    }

    /// The music under company: a little lower, and softer for a moment
    /// when the wisp looks in.
    pub(crate) fn company_music(&self, real: UnixMs) -> f64 {
        if !self.keeping() {
            return 1.0;
        }
        let into = (real - (self.keep.dip_until - DIP_MS)) as f64 / DIP_MS as f64;
        let dip = if (0.0..1.0).contains(&into) {
            (into * std::f64::consts::PI).sin()
        } else {
            0.0
        };
        0.85 * (1.0 - 0.55 * dip)
    }

    /// Where a planet or named star is, in words.
    fn place_of(&self, name: &str, now: UnixMs) -> String {
        use westering_core::ephem::Body;
        let planet = [Body::Venus, Body::Jupiter, Body::Mars, Body::Saturn]
            .into_iter()
            .find(|b| b.name() == name)
            .map(|b| {
                let s = see(b, self.observer, now);
                (s.alt, s.az)
            });
        let star = || {
            let s = self.sky.stars.stars.iter().find(|s| {
                self.sky
                    .lists
                    .star_name(s.hr)
                    .is_some_and(|n| n.name == name)
            })?;
            let (ra, dec) = angles(s.dir);
            Some(observe(self.observer, now, ra, dec))
        };
        match planet.or_else(star) {
            Some((alt, az)) => whereabouts(alt, az),
            None => "in the sky".to_owned(),
        }
    }
}
