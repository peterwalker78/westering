//! Tonight's finds: where each is, what kind of thing, how to look for
//! it, the Tonight list, and turning the view to one, or back to one
//! skipped.

use super::*;

/// One of tonight's finds, and how far it has got.
#[derive(Clone)]
pub(crate) struct TonightFind {
    pub(crate) find: Find,
    pub(crate) caught: bool,
    /// Seen: caught with the card up long enough to read, or a story begun.
    pub(crate) viewed: bool,
    /// How many steps of a hop have been found.
    pub(crate) steps: usize,
}

impl TonightFind {
    pub(crate) fn new(find: Find, caught: bool) -> TonightFind {
        TonightFind {
            find,
            caught,
            // Anything caught earlier tonight was seen then.
            viewed: caught,
            steps: 0,
        }
    }
}

impl std::ops::Deref for TonightFind {
    type Target = Find;

    fn deref(&self) -> &Find {
        &self.find
    }
}

impl Game {
    /// Which of tonight's finds `target` is, if it's one.
    pub(crate) fn find_of(&self, target: Target) -> Option<usize> {
        self.finds.iter().position(|f| f.target == target)
    }

    /// Where find `i` is, as a horizon vector.
    pub(crate) fn find_dir(&self, i: usize, now: UnixMs, hz: &Mat3, prec: &Mat3) -> Option<Vec3> {
        self.target_dir(self.finds[i].target, now, hz, prec)
    }

    /// Where something in the sky is, as a horizon vector.
    pub(crate) fn target_dir(
        &self,
        target: Target,
        now: UnixMs,
        hz: &Mat3,
        prec: &Mat3,
    ) -> Option<Vec3> {
        match target {
            Target::Body(body) => {
                let s = see(body, self.observer, now);
                Some(from_alt_az(s.alt, s.az))
            }
            Target::Showpiece(p) => {
                let piece = &self.sky.lists.showpieces[p];
                Some(apply(hz, apply(prec, unit(piece.ra, piece.dec))))
            }
            Target::Star(hr) => self
                .sky
                .stars
                .index_of(hr)
                .map(|idx| apply(hz, self.star_dirs[idx])),
            Target::Meteor(_) => None,
            Target::Figure(f) => self.sky.figures[f]
                .centre(&self.sky.stars)
                .map(|(c, _)| apply(hz, apply(prec, c))),
            Target::Hop(_) => self.stop_dir(self.hop_stop(self.find_of(target)?)?, hz, prec),
            Target::Story(s) => {
                let anchor = self.sky.tours.stories[s].anchor;
                self.sky
                    .stars
                    .index_of(anchor)
                    .map(|idx| apply(hz, self.star_dirs[idx]))
            }
            Target::MoonWalk => {
                let s = see(Body::Moon, self.observer, now);
                Some(from_alt_az(s.alt, s.az))
            }
        }
    }

    /// What kind of thing find `i` is, in a word or two.
    pub(crate) fn kind_word(&self, i: usize) -> &'static str {
        self.finds[i].target.kind_word(&self.sky.lists.showpieces)
    }

    /// What to look out for, to help find it.
    pub(crate) fn look_for(&self, i: usize) -> &'static str {
        let moon_first = self.moon_find().is_some_and(|m| !self.finds[m].caught);
        self.finds[i]
            .target
            .look_for(&self.sky.lists.showpieces, moon_first)
    }

    /// How narrow the view goes to show a find close up.
    pub(super) fn zoom_for(&self, i: usize) -> f64 {
        match self.finds[i].target {
            // A hop keeps the view wide enough to see the next step.
            Target::Hop(_) => self
                .on_tour()
                .and_then(|t| t.catch.fov_before)
                .unwrap_or(self.camera.fov),
            Target::MoonWalk => self.moon_walk_fov(),
            ref target => self
                .framing(i)
                .map_or_else(|| self.zoom_for_target(target), |f| f.fov),
        }
    }

    /// How narrow the view goes to show something close up.
    pub(super) fn zoom_for_target(&self, target: &Target) -> f64 {
        match *target {
            // Close enough for the disc to fill about a quarter of the view.
            Target::Body(body) => {
                let d = see(body, self.observer, self.clock.sky(self.last_real))
                    .position
                    .diameter
                    / 3600.0;
                // Saturn is framed by its rings, 2.27 times as wide as the planet;
                // Jupiter wide enough to show its inner moons beside it.
                let share = match body {
                    Body::Saturn => 0.24 / 2.27,
                    Body::Jupiter => 0.07,
                    _ => 0.24,
                };
                (d / share).clamp(crate::camera::FOV_NARROWEST, 2.4)
            }
            Target::Showpiece(p) => {
                let id = &self.sky.lists.showpieces[p].id;
                match self.photos.credit(id).and_then(|c| c.view) {
                    Some(view) => view,
                    None => (self.photo_degrees(p) * 2.6).clamp(0.4, 40.0),
                }
            }
            Target::Star(_) => 8.0,
            Target::Meteor(_) => self.camera.fov,
            Target::Figure(f) => self.sky.figures[f]
                .centre(&self.sky.stars)
                .map_or(60.0, |(_, reach)| (reach * 3.0).clamp(20.0, 100.0)),
            Target::Story(_) => 50.0,
            Target::Hop(_) | Target::MoonWalk => self.camera.fov,
        }
    }

    /// How big find `i` looks, in pixels, at a field of view.
    pub(super) fn apparent_radius(&self, i: usize, fov: f64) -> f64 {
        if let Target::MoonWalk = self.finds[i].target {
            return self
                .moon_find()
                .map_or(0.0, |m| self.apparent_radius(m, fov));
        }
        if self.finds[i].caught && self.photo_id(i).is_some() {
            // Keep the card on the screen beside a picture that fills the view.
            return self
                .photo_half(i, fov)
                .min(self.camera.width.min(self.camera.height) * 0.3);
        }
        let ppd = (self.camera.width / 2.0) / (2.0 * (fov.to_radians() / 4.0).tan())
            * std::f64::consts::PI
            / 180.0;
        let degrees = match self.finds[i].target {
            Target::Body(body) => {
                see(body, self.observer, self.clock.sky(self.last_real))
                    .position
                    .diameter
                    / 7200.0
            }
            Target::Showpiece(p) => self.sky.lists.showpieces[p].size / 120.0,
            Target::Figure(f) => self.sky.figures[f]
                .centre(&self.sky.stars)
                .map_or(0.0, |(_, reach)| reach),
            _ => 0.0,
        };
        degrees * ppd
    }

    /// Where the words about something go: clear of it, to its right,
    /// unless the Tonight list is there.
    pub(crate) fn beside_centre(&self) -> f64 {
        const CARD: f64 = 400.0;
        let clear = self.reticle_radius() + 34.0;
        let (w, cx) = (self.camera.width, self.camera.width / 2.0);
        let right = cx + clear;
        if right + CARD <= w - 300.0 {
            right
        } else {
            (cx - clear - CARD).max(24.0)
        }
    }

    /// Puts tonight's list in the order to see things in: one way round
    /// the sky from where the view opens.
    pub(super) fn lay_out(&mut self, now: UnixMs) {
        let hz = horizon(self.observer, now);
        let prec = precession(now);
        let dirs: Vec<_> = (0..self.finds.len())
            .map(|i| {
                self.find_dir(i, now, &hz, &prec)
                    .filter(|v| alt_az(*v).0 > 0.0)
            })
            .collect();
        let here = from_alt_az(self.camera.alt, self.camera.az);
        let order = westering_core::route::route(here, &dirs);
        self.finds = order.iter().map(|&i| self.finds[i].clone()).collect();
    }

    /// The next thing down the list that's still to find and is up, other
    /// than the one already under the reticle.
    pub(super) fn turn_to_next(&mut self, real: UnixMs) {
        let now = self.sky_now(real);
        let hz = horizon(self.observer, now);
        let prec = precession(now);
        let here = from_alt_az(self.camera.alt, self.camera.az);
        let up: Vec<(usize, f64)> = (0..self.finds.len())
            .filter(|&i| !self.finds[i].caught)
            .filter_map(|i| self.find_dir(i, now, &hz, &prec).map(|v| (i, v)))
            .filter(|(_, v)| alt_az(*v).0 > 0.0)
            .map(|(i, v)| (i, angle_between(here, v)))
            .collect();
        let next = up
            .iter()
            .find(|(_, apart)| *apart >= 1.0)
            .or(up.first())
            .map(|(i, _)| *i);
        match next {
            Some(i) => self.turn_to(i, real),
            None => {
                let meteor_left = (0..self.finds.len()).any(|i| {
                    !self.finds[i].caught && matches!(self.finds[i].target, Target::Meteor(_))
                });
                // Nowhere to turn to: say so, so the key isn't met with
                // silence.
                if meteor_left {
                    self.guide_meteor_left(real);
                    self.status("No meteor just then. Keep watching".into(), real);
                } else if self.session.all_found() {
                    self.status(
                        "That's all of tonight's sky. W winds down when you're ready".into(),
                        real,
                    );
                } else {
                    self.status(
                        "What's left isn't up just now. W winds down when you're ready".into(),
                        real,
                    );
                }
            }
        }
    }

    /// Turns the view towards find `i`, and says what to look for.
    pub fn turn_to(&mut self, i: usize, real: UnixMs) {
        if let Some(t) = self.on_tour_mut() {
            t.ring_resting = false;
        }
        if self.by_day() {
            if self.card.is_some() {
                self.dismiss_card(real);
            }
            self.day_open(i, real);
            return;
        }
        if i >= self.finds.len() || !self.touring() {
            return;
        }
        self.input(real);
        if self.card.is_some() {
            self.dismiss_card(real);
        }
        // Turned to on purpose: the view stays with it.
        if let Some(t) = self.on_tour_mut() {
            t.move_on = None;
        }
        if let Target::Meteor(_) = self.finds[i].target {
            self.guide_meteor_left(real);
            return;
        }
        let now = self.sky_now(real);
        let hz = horizon(self.observer, now);
        let prec = precession(now);
        let Some(v) = self.find_dir(i, now, &hz, &prec) else {
            return;
        };
        let (alt, az) = alt_az(v);
        if alt < 0.0 {
            let name = self.finds[i].name.clone();
            self.say(
                format!("{name} has gone below the horizon for now."),
                real,
                5_000,
            );
            return;
        }
        let fov = match self.finds[i].target {
            _ if self.finds[i].caught && self.photo_id(i).is_some() => self.zoom_for(i),
            // Too faint for the eye: close enough in for it to show.
            Target::Showpiece(p) if self.sky.lists.showpieces[p].deep => {
                let limit = limiting_magnitude(see(Body::Sun, self.observer, now).alt);
                let short = self.sky.lists.showpieces[p].mag + 0.6 - limit;
                if short > 0.0 {
                    (60.0 / 10f64.powf(short / 2.5)).clamp(1.5, 60.0)
                } else {
                    self.camera.fov.max(60.0)
                }
            }
            Target::Figure(_) => self.zoom_for(i).max(self.camera.fov.min(90.0)),
            _ => self.camera.fov.max(60.0),
        };
        self.look = Some(Look {
            az,
            alt,
            fov,
            rate: 1.6,
        });
        if let Some(t) = self.on_tour_mut() {
            t.track = Some(i);
        }
        let line = (!self.finds[i].caught).then(|| {
            let hint = match self.hop_stop(i) {
                Some(stop) => stop.say.clone(),
                None => self.look_for(i).to_owned(),
            };
            format!("{}. {hint}", self.finds[i].name)
        });
        self.guide_turned(line.map(|l| (crate::guide::Aim::Find(i), l)), real);
    }

    /// Tonight's finds for the list: what, where, and whether found.
    pub fn tonight_rows(&self, real: UnixMs) -> Vec<Row> {
        if self.by_day() {
            return self.day_rows();
        }
        let now = self.sky_now(real);
        let hz = horizon(self.observer, now);
        let prec = precession(now);
        (0..self.finds.len())
            .map(|i| {
                let whereabouts = match self.finds[i].target {
                    Target::Meteor(_) => "anywhere, any moment".to_owned(),
                    _ => match self.find_dir(i, now, &hz, &prec).map(alt_az) {
                        Some((alt, _)) if alt < 0.0 => "below the horizon now".to_owned(),
                        Some((alt, az)) => westering_core::finale::whereabouts(alt, az),
                        None => String::new(),
                    },
                };
                let whereabouts = match self.finds[i].target {
                    Target::Hop(h) if self.finds[i].steps > 0 => format!(
                        "step {} of {}, {whereabouts}",
                        self.finds[i].steps + 1,
                        self.sky.tours.hops[h].steps.len()
                    ),
                    _ => whereabouts,
                };
                Row {
                    name: self.finds[i].name.clone(),
                    kind: self.kind_word(i),
                    whereabouts,
                    found: self.finds[i].caught,
                }
            })
            .collect()
    }

    pub fn show_tonight(&self) -> bool {
        // In a small window a card needs the room.
        if self.card.is_some() && compact(self.camera.width, self.camera.height) {
            return false;
        }
        if self.by_day() {
            return self.day_list_shows();
        }
        // A card at night needs the room too: the find takes the space
        // beside it. Free look has no list.
        self.touring()
            && self.card.is_none()
            && !self.keeping()
            && !self.finds.is_empty()
            && self.eye_alpha < 0.3
    }

    /// Tab: back to the first thing on the list not yet seen, whether it
    /// was never reached or its card was put away before it could be read;
    /// with nothing missed, it carries on like Space. Once the whole list
    /// is found there's nothing to go back for: it says so instead.
    pub(super) fn back_to_skipped(&mut self, real: UnixMs) -> bool {
        if !self.hunting() || self.more_now() || self.talk.prompt.is_some() {
            return self.carry_on(real);
        }
        let done = (0..self.finds.len())
            .all(|i| self.finds[i].caught || matches!(self.finds[i].target, Target::Meteor(_)));
        if done && self.card.is_none() {
            return self.carry_on(real);
        }
        let now = self.sky_now(real);
        let hz = horizon(self.observer, now);
        let prec = precession(now);
        let missed = (0..self.finds.len()).find(|&i| {
            !self.finds[i].viewed
                && !matches!(self.finds[i].target, Target::Meteor(_))
                && self.card.as_ref().and_then(|c| c.find) != Some(i)
                && self
                    .find_dir(i, now, &hz, &prec)
                    .is_some_and(|v| alt_az(v).0 > 0.0)
        });
        let Some(i) = missed.filter(|_| !done) else {
            return self.carry_on(real);
        };
        self.turn_to(i, real);
        if self.finds[i].caught {
            self.show_find_card(i, real);
            // Said, so going back doesn't seem to come from nowhere.
            let name = self.finds[i].name.clone();
            self.status_for(
                format!("Back to {name}: its card went by before it could be read"),
                real,
                6_000,
            );
        }
        true
    }

    /// Twilight: every couple of minutes, what's bright enough to find is
    /// worked out again, and anything newly out joins the list.
    pub(super) fn more_as_it_darkens(&mut self, real: UnixMs) {
        if real < self.next_look_again {
            return;
        }
        self.next_look_again = real + 2 * westering_core::time::MINUTE;
        let now = self.sky_now(real);
        if !matches!(self.session.phase(), Phase::Weights | Phase::Hunt)
            || westering_core::sky::sun_altitude(self.observer, now) < -18.0
        {
            return;
        }
        let most = if late(now, self.offset_s(now)) {
            LATE_FINDS
        } else {
            westering_core::finds::MOST
        };
        let fresh = tonight(&self.sky, self.observer, now, self.offset_s(now), &|id| {
            self.journal.times_found_before(id, &self.night)
        });
        let mut added = 0;
        for find in fresh {
            if self.finds.len() >= most {
                break;
            }
            if self.finds.iter().any(|f| f.id == find.id) {
                continue;
            }
            let caught = self.journal.found_on(&find.id, &self.night);
            added += usize::from(!caught);
            self.finds.push(TonightFind::new(find, caught));
        }
        if added > 0 {
            self.session.more_to_find();
            self.guide_darker(added, real);
        }
    }

    /// Turns the view to the first thing still to find.
    pub(crate) fn face_first_find(&mut self, real: UnixMs) {
        let now = self.clock.sky(real);
        let hz = horizon(self.observer, now);
        let prec = precession(now);
        let first = (0..self.finds.len())
            .filter(|&i| !self.finds[i].caught)
            .find_map(|i| self.find_dir(i, now, &hz, &prec));
        if let Some(v) = first {
            let (alt, az) = alt_az(v);
            self.look = Some(Look {
                az,
                alt: alt.clamp(15.0, 62.0),
                fov: 95.0,
                rate: 1.0,
            });
        }
    }
}
