//! The shape of one visit. Every visit runs the same arc: the sky arrives,
//! the weights are set down, the hunt, the dimming, the finale and lights
//! out. It is a pure state machine: the caller passes the time in.

use crate::time::{HOUR, MINUTE, UnixMs};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Arrival,
    Weights,
    Hunt,
    Dimming,
    Finale,
    LightsOut,
    Over,
}

#[derive(Clone, Copy, Debug)]
pub struct Timings {
    pub arrival: UnixMs,
    /// How long the sky lingers once everything is found, if nobody chooses
    /// to wind down first; the guide offers well before then.
    pub after_last_find: UnixMs,
    /// The hunt winds down by itself after this long.
    pub hunt_most: UnixMs,
    /// How long into the hunt winding down is first offered: a visit is
    /// meant to be a quarter of an hour, not the whole list.
    pub offer: UnixMs,
    pub dimming: UnixMs,
    /// The time-lapse through the rest of the night.
    pub lapse: UnixMs,
    /// The last line, held before the lights go out.
    pub hold: UnixMs,
    pub lights_out: UnixMs,
    /// How far the time-lapse carries the sky.
    pub lapse_sky: UnixMs,
    /// How long the whole visit takes to slow to its calmest.
    pub slowing: UnixMs,
}

impl Timings {
    pub const STANDARD: Timings = Timings {
        arrival: 4_500,
        after_last_find: 6 * MINUTE,
        hunt_most: 25 * MINUTE,
        offer: 12 * MINUTE,
        dimming: 40_000,
        lapse: 22_000,
        hold: 14_000,
        lights_out: 25_000,
        lapse_sky: 4 * HOUR,
        slowing: 18 * MINUTE,
    };

    /// Everything but the sky's own time-lapse shortened, for trying the arc
    /// out quickly.
    pub fn quick(factor: i64) -> Timings {
        let s = Timings::STANDARD;
        let f = factor.max(1);
        Timings {
            arrival: s.arrival,
            after_last_find: s.after_last_find / f,
            hunt_most: s.hunt_most / f,
            offer: s.offer / f,
            dimming: s.dimming / f,
            lapse: s.lapse,
            hold: s.hold / f.min(2),
            lights_out: s.lights_out / f,
            lapse_sky: s.lapse_sky,
            slowing: s.slowing / f,
        }
    }
}

/// How dim the sky is once the dimming's done, as a fraction of full.
pub const DIM: f64 = 0.4;
/// How dim it has gone by the time the lights go out: it only ever gets
/// darker from the dimming on.
pub const LAST: f64 = 0.28;
/// The least wait between choosing how tonight ends and the finale.
const ENDING_BEAT: UnixMs = 5_000;
/// The slowest the sky's motion gets, as a fraction of the start.
pub const CALMEST: f64 = 0.45;

pub struct Session {
    pub timings: Timings,
    started: UnixMs,
    phase: Phase,
    phase_since: UnixMs,
    ask_weights: bool,
    finds: usize,
    found: usize,
    all_found_at: Option<UnixMs>,
    /// Held still since then: the evening waits while the wisp keeps
    /// someone company in the background.
    held: Option<UnixMs>,
    /// When the dimming began. The screen keeps dimming by the clock
    /// while the evening's held for breaths and a line to think over.
    dim_from: UnixMs,
}

fn smoothstep(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

impl Session {
    pub fn new(now: UnixMs, timings: Timings, finds: usize, ask_weights: bool) -> Session {
        Session {
            timings,
            started: now,
            phase: Phase::Arrival,
            phase_since: now,
            ask_weights,
            finds,
            found: 0,
            all_found_at: None,
            held: None,
            dim_from: now,
        }
    }

    /// Holds the evening where it is: nothing moves on until `resume`.
    pub fn hold(&mut self, now: UnixMs) {
        if self.held.is_none() {
            self.held = Some(now);
        }
    }

    /// Carries on from where it was held, as if no time had passed.
    pub fn resume(&mut self, now: UnixMs) {
        if let Some(at) = self.held.take() {
            let gap = now - at;
            self.started += gap;
            self.phase_since += gap;
            if let Some(found) = &mut self.all_found_at {
                *found += gap;
            }
        }
    }

    pub fn held(&self) -> bool {
        self.held.is_some()
    }

    /// Back to looking up, from winding down or its ending: the user has
    /// chosen something else.
    pub fn reopen(&mut self, now: UnixMs) {
        self.held = None;
        if !matches!(self.phase, Phase::Arrival | Phase::Weights | Phase::Hunt) {
            self.phase = Phase::Hunt;
            self.phase_since = now;
            self.all_found_at = None;
        }
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// Time spent in the current phase.
    pub fn in_phase(&self, now: UnixMs) -> UnixMs {
        self.held.unwrap_or(now) - self.phase_since
    }

    fn enter(&mut self, phase: Phase, now: UnixMs) -> Option<Phase> {
        self.phase = phase;
        self.phase_since = now;
        if phase == Phase::Dimming {
            self.dim_from = now;
        }
        Some(phase)
    }

    /// Moves on by itself where the arc does; returns the new phase.
    pub fn tick(&mut self, now: UnixMs) -> Option<Phase> {
        if self.held.is_some() {
            return None;
        }
        let t = self.timings;
        let here = self.in_phase(now);
        match self.phase {
            Phase::Arrival if here >= t.arrival => {
                let next = if self.ask_weights {
                    Phase::Weights
                } else {
                    Phase::Hunt
                };
                self.enter(next, now)
            }
            Phase::Hunt => {
                let done = self
                    .all_found_at
                    .is_some_and(|at| now - at >= t.after_last_find);
                if done || here >= t.hunt_most {
                    self.enter(Phase::Dimming, now)
                } else {
                    None
                }
            }
            Phase::Dimming if here >= t.dimming => self.enter(Phase::Finale, now),
            Phase::Finale if here >= t.lapse + t.hold => self.enter(Phase::LightsOut, now),
            Phase::LightsOut if here >= t.lights_out => self.enter(Phase::Over, now),
            _ => None,
        }
    }

    pub fn weights_done(&mut self, now: UnixMs) -> Option<Phase> {
        (self.phase == Phase::Weights)
            .then(|| self.enter(Phase::Hunt, now))
            .flatten()
    }

    /// More to find than there was: the sky has darkened and more is out.
    pub fn add_finds(&mut self, n: usize) {
        self.finds += n;
        if self.found < self.finds {
            self.all_found_at = None;
        }
    }

    pub fn found_one(&mut self, now: UnixMs) {
        self.found += 1;
        if self.found >= self.finds && self.all_found_at.is_none() {
            self.all_found_at = Some(now);
        }
    }

    pub fn all_found(&self) -> bool {
        self.found >= self.finds
    }

    /// The user has had enough of the hunt.
    /// How tonight ends has been chosen: the finale follows as soon as the
    /// screen has finished dimming, a few seconds on at the least.
    pub fn ending_chosen(&mut self, now: UnixMs) {
        if self.phase != Phase::Dimming || self.held.is_some() {
            return;
        }
        let t = self.timings;
        let natural = self.phase_since + t.dimming;
        let soonest = (self.dim_from + t.dimming).max(now + ENDING_BEAT);
        self.phase_since = natural.min(soonest) - t.dimming;
    }

    pub fn finish(&mut self, now: UnixMs) -> Option<Phase> {
        match self.phase {
            Phase::Arrival | Phase::Weights | Phase::Hunt => self.enter(Phase::Dimming, now),
            _ => None,
        }
    }

    /// How quickly things move: 1 at the start, easing to `CALMEST`.
    pub fn tempo(&self, now: UnixMs) -> f64 {
        let x = (self.held.unwrap_or(now) - self.started) as f64 / self.timings.slowing as f64;
        let base = 1.0 - (1.0 - CALMEST) * smoothstep(x);
        match self.phase {
            Phase::Finale | Phase::LightsOut | Phase::Over => CALMEST.min(base),
            _ => base,
        }
    }

    /// How bright the sky is drawn, 0 to 1.
    pub fn brightness(&self, now: UnixMs) -> f64 {
        let t = self.timings;
        let x = |len: UnixMs| self.in_phase(now) as f64 / len.max(1) as f64;
        match self.phase {
            Phase::Arrival => smoothstep(x(t.arrival)),
            Phase::Weights | Phase::Hunt => 1.0,
            Phase::Dimming => {
                let dimmed = (now - self.dim_from) as f64 / t.dimming.max(1) as f64;
                1.0 - (1.0 - DIM) * smoothstep(dimmed)
            }
            Phase::Finale => DIM - (DIM - LAST) * smoothstep(x(t.lapse + t.hold)),
            Phase::LightsOut => LAST * (1.0 - smoothstep(x(t.lights_out))),
            Phase::Over => 0.0,
        }
    }

    /// How far through the time-lapse, 0 to 1.
    pub fn lapse_progress(&self, now: UnixMs) -> f64 {
        match self.phase {
            Phase::Finale => smoothstep(self.in_phase(now) as f64 / self.timings.lapse as f64),
            Phase::LightsOut | Phase::Over => 1.0,
            _ => 0.0,
        }
    }

    /// How far ahead of the clock the sky is drawn.
    pub fn lapse(&self, now: UnixMs) -> UnixMs {
        (self.lapse_progress(now) * self.timings.lapse_sky as f64) as UnixMs
    }

    /// Whether the last line should be showing.
    pub fn last_line(&self, now: UnixMs) -> bool {
        match self.phase {
            Phase::Finale => self.in_phase(now) >= self.timings.lapse,
            Phase::LightsOut => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_until(s: &mut Session, from: UnixMs, to: UnixMs) -> Vec<(UnixMs, Phase)> {
        let mut out = Vec::new();
        let mut t = from;
        while t <= to {
            if let Some(p) = s.tick(t) {
                out.push((t, p));
            }
            t += 100;
        }
        out
    }

    #[test]
    fn a_whole_visit_runs_its_arc_and_ends() {
        let t = Timings::STANDARD;
        let mut s = Session::new(0, t, 2, true);
        assert_eq!(s.brightness(0), 0.0);
        run_until(&mut s, 0, 10_000);
        assert_eq!(s.phase(), Phase::Weights, "waits for the weights");
        s.weights_done(10_000);
        s.found_one(20_000);
        s.found_one(30_000);
        let changes = run_until(&mut s, 30_000, 30 * MINUTE);
        let phases: Vec<Phase> = changes.iter().map(|c| c.1).collect();
        assert_eq!(
            phases,
            [Phase::Dimming, Phase::Finale, Phase::LightsOut, Phase::Over]
        );
        assert_eq!(changes[0].0, 30_000 + t.after_last_find);
        assert_eq!(s.brightness(30 * MINUTE), 0.0);
    }

    #[test]
    fn the_hunt_winds_down_by_itself() {
        let mut s = Session::new(0, Timings::STANDARD, 5, false);
        run_until(&mut s, 0, 10_000);
        assert_eq!(s.phase(), Phase::Hunt);
        let changes = run_until(&mut s, 10_000, 30 * MINUTE);
        assert_eq!(changes[0].1, Phase::Dimming);
        assert!(changes[0].0 >= 25 * MINUTE);
    }

    #[test]
    fn the_sky_slows_and_dims_but_never_jumps() {
        let mut s = Session::new(0, Timings::quick(10), 1, false);
        let mut last_b = 0.0;
        let mut last_tempo = 1.0;
        let mut t = 0;
        s.found_one(5_000);
        while s.phase() != Phase::Over {
            s.tick(t);
            let b = s.brightness(t);
            let tempo = s.tempo(t);
            if t > 5_000 {
                assert!(
                    (b - last_b).abs() < 0.02,
                    "brightness jumped at {t}: {last_b} to {b}"
                );
            }
            assert!(tempo <= last_tempo + 1e-9, "tempo rose at {t}");
            last_b = b;
            last_tempo = tempo;
            t += 50;
        }
        assert!(s.lapse(t) >= Timings::STANDARD.lapse_sky - 1);
    }

    #[test]
    fn the_screen_dims_through_the_breaths_and_goes_on_once_chosen() {
        let t = Timings::STANDARD;
        let mut s = Session::new(0, t, 5, false);
        run_until(&mut s, 0, 10_000);
        s.finish(10_000);
        // Held for the breaths and a line to think over: still dimming.
        s.hold(12_000);
        assert!((s.brightness(10_000 + t.dimming) - DIM).abs() < 1e-9);
        s.resume(90_000);
        assert_eq!(s.phase(), Phase::Dimming);
        // Chosen: the finale follows a few seconds on, not a whole dim later.
        s.ending_chosen(91_000);
        let changes = run_until(&mut s, 91_000, 100_000);
        assert_eq!(changes[0], (91_000 + ENDING_BEAT, Phase::Finale));
        // And it only ever gets darker from there.
        let mut last = s.brightness(changes[0].0);
        for at in (changes[0].0..changes[0].0 + 60_000).step_by(100) {
            s.tick(at);
            let b = s.brightness(at);
            assert!(b <= last + 1e-9, "brightened at {at}");
            last = b;
        }
    }

    #[test]
    fn a_held_evening_waits_and_carries_on_where_it_was() {
        let mut s = Session::new(0, Timings::STANDARD, 5, false);
        run_until(&mut s, 0, 10_000);
        assert_eq!(s.phase(), Phase::Hunt);
        s.hold(20 * MINUTE);
        let tempo = s.tempo(20 * MINUTE);
        assert!(run_until(&mut s, 20 * MINUTE, 3 * HOUR).is_empty());
        assert_eq!(s.tempo(3 * HOUR), tempo);
        s.resume(3 * HOUR);
        assert_eq!(s.phase(), Phase::Hunt);
        let changes = run_until(&mut s, 3 * HOUR, 4 * HOUR);
        assert!(changes[0].0 >= 3 * HOUR + 5 * MINUTE, "{changes:?}");
    }

    #[test]
    fn more_to_find_keeps_the_evening_going() {
        let mut s = Session::new(0, Timings::STANDARD, 1, false);
        run_until(&mut s, 0, 10_000);
        s.found_one(20_000);
        assert!(s.all_found());
        s.add_finds(2);
        assert!(!s.all_found());
        let changes = run_until(&mut s, 20_000, 20 * MINUTE);
        assert!(changes.is_empty(), "{changes:?}");
    }

    #[test]
    fn winding_down_is_offered_well_before_the_hunt_ends_itself() {
        for t in [Timings::STANDARD, Timings::quick(10)] {
            assert!(t.offer * 2 <= t.hunt_most, "{t:?}");
        }
        assert_eq!(Timings::quick(10).offer * 10, Timings::STANDARD.offer);
    }

    #[test]
    fn leaving_early_still_goes_through_the_ending() {
        let mut s = Session::new(0, Timings::STANDARD, 4, false);
        run_until(&mut s, 0, 10_000);
        assert_eq!(s.finish(10_000), Some(Phase::Dimming));
        assert_eq!(s.finish(11_000), None, "no skipping the ending");
    }
}
