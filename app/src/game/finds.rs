//! Tonight's finds: where each is, what kind of thing, how to look for
//! it, the Tonight list, and turning the view to one, or back to one
//! skipped.

use super::*;

impl Game {
    /// Where find `i` is, as a horizon vector.
    pub(crate) fn find_dir(&self, i: usize, now: UnixMs, hz: &Mat3, prec: &Mat3) -> Option<Vec3> {
        match self.finds[i].target {
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
            Target::Hop(_) => self.stop_dir(self.hop_stop(i)?, hz, prec),
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
        match self.finds[i].target {
            Target::Body(Body::Moon) => "Our Moon",
            Target::Body(_) => "Planet",
            Target::Star(_) => "Star",
            Target::Showpiece(p) => match self.sky.lists.showpieces[p].kind {
                Kind::Cluster => "Star cluster",
                Kind::Galaxy => "Galaxy",
                Kind::Nebula => "Nebula",
                Kind::Double => "Double star",
                Kind::Star => "Star",
                Kind::Dark => "Dark cloud",
                Kind::Cloud => "Star cloud",
                Kind::Asterism => "Asterism",
            },
            Target::Meteor(_) => "Meteor",
            Target::Figure(_) => "Constellation",
            Target::Hop(_) => "Star-hop",
            Target::Story(_) => "Tonight's story",
            Target::MoonWalk => "Moon walk",
        }
    }

    /// What to look out for, to help find it.
    pub(crate) fn look_for(&self, i: usize) -> &'static str {
        match self.finds[i].target {
            Target::Body(Body::Moon) => "You can't miss it.",
            Target::Body(_) => "Look for a bright, steady light that doesn't twinkle.",
            Target::Star(_) => "Look for a single bright star.",
            Target::Showpiece(p) if self.sky.lists.showpieces[p].kind == Kind::Cloud => {
                "Look for the brightest patch of the Milky Way there."
            }
            Target::Showpiece(p) if self.sky.lists.showpieces[p].deep => {
                "Too faint for the eye alone: turn to it and zoom in close with +, and it will show."
            }
            Target::Showpiece(p) => match self.sky.lists.showpieces[p].kind {
                Kind::Cluster => "Look for a little knot of faint stars.",
                Kind::Asterism => "Look for a little pattern of stars.",
                Kind::Galaxy | Kind::Nebula => "Look for a faint smudge of light.",
                Kind::Double => "It looks like a single star, but close up it's two.",
                _ => "Look for a single star.",
            },
            Target::Meteor(_) => "Watch the sky, and press Space the moment one flies.",
            Target::Figure(_) => "Look for its shape; put the ring in the middle of it.",
            Target::Hop(_) => "Start from a star you know, and hop from star to star.",
            Target::Story(_) => {
                "Tonight's story starts at the star I'm showing you: hold Space when it's in the ring."
            }
            Target::MoonWalk if self.moon_find().is_some_and(|m| !self.caught[m]) => {
                "Catch the Moon first, then hold Space on it again to go closer."
            }
            Target::MoonWalk => "Hold Space on the Moon to go closer.",
        }
    }

    /// How narrow the view goes to show a find close up.
    pub(super) fn zoom_for(&self, i: usize) -> f64 {
        match self.finds[i].target {
            // A hop keeps the view wide enough to see the next step.
            Target::Hop(_) => self.catch.fov_before.unwrap_or(self.camera.fov),
            Target::MoonWalk => self.moon_walk_fov(),
            ref target => self.zoom_for_target(target),
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
        if self.caught[i] && self.photo_id(i).is_some() {
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

    /// Where the words about something go: clear of it, to its right.
    pub(crate) fn beside_centre(&self) -> f64 {
        self.beside(None, self.camera.fov)
    }

    pub(super) fn beside(&self, i: Option<usize>, fov: f64) -> f64 {
        const CARD: f64 = 400.0;
        let r = self.reticle_radius();
        let object = i.map(|i| self.apparent_radius(i, fov)).unwrap_or(0.0);
        // The Tonight list takes the right-hand edge during the hunt, except
        // while a photograph is up.
        let photo = i.is_some_and(|i| {
            self.photo_id(i).is_some() || self.finds[i].target == Target::MoonWalk
        });
        let list = if photo { 0.0 } else { 300.0 };
        let clear = r.max(object) + 34.0;
        let (w, cx) = (self.camera.width, self.camera.width / 2.0);
        let right = cx + clear;
        if right + CARD <= w - list {
            right
        } else {
            (cx - clear - CARD).max(24.0)
        }
    }

    pub(super) fn turn_to_next(&mut self, real: UnixMs) {
        let now = self.sky_now(real);
        let hz = horizon(self.observer, now);
        let prec = precession(now);
        let here = from_alt_az(self.camera.alt, self.camera.az);
        let next = (0..self.finds.len())
            .filter(|&i| !self.caught[i])
            .filter_map(|i| self.find_dir(i, now, &hz, &prec).map(|v| (i, v)))
            .filter(|(_, v)| alt_az(*v).0 > 0.0)
            .min_by(|a, b| {
                // Skip the one already under the reticle.
                let da = angle_between(here, a.1);
                let db = angle_between(here, b.1);
                let da = if da < 1.0 { 999.0 } else { da };
                let db = if db < 1.0 { 999.0 } else { db };
                da.total_cmp(&db)
            })
            .map(|(i, _)| i);
        match next {
            Some(i) => self.turn_to(i, real),
            None => {
                let meteor_left = (0..self.finds.len())
                    .any(|i| !self.caught[i] && matches!(self.finds[i].target, Target::Meteor(_)));
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
        self.ring_resting = false;
        if self.by_day() {
            if self.card.is_some() {
                self.dismiss_card(real);
            }
            self.day_open(i, real);
            return;
        }
        if i >= self.finds.len() || !self.hunting() {
            return;
        }
        self.input(real);
        if self.card.is_some() {
            self.dismiss_card(real);
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
            let line = format!("{name} has gone below the horizon for now.");
            if self.free_look() {
                self.status_for(line, real, 4_000);
            } else {
                self.say(line, real, 5_000);
            }
            return;
        }
        let fov = match self.finds[i].target {
            _ if self.caught[i] && self.photo_id(i).is_some() => self.zoom_for(i),
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
        self.track = Some(i);
        if !self.caught[i] {
            let hint = match self.hop_stop(i) {
                Some(stop) => stop.say.clone(),
                None => self.look_for(i).to_owned(),
            };
            let line = format!("{}. {hint}", self.finds[i].name);
            // Free look keeps the wisp out of it: a quiet word instead.
            if self.free_look() {
                self.status_for(line, real, 5_000);
            } else {
                self.say_at(crate::guide::Aim::Find(i), line, real, 7_000);
            }
        }
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
                    Target::Hop(h) if self.steps[i] > 0 => format!(
                        "step {} of {}, {whereabouts}",
                        self.steps[i] + 1,
                        self.sky.tours.hops[h].steps.len()
                    ),
                    _ => whereabouts,
                };
                Row {
                    name: self.finds[i].name.clone(),
                    kind: self.kind_word(i),
                    whereabouts,
                    found: self.caught[i],
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
        matches!(self.session.phase(), Phase::Hunt)
            && !self.keeping()
            && !self.finds.is_empty()
            && self.eye_alpha < 0.3
            && self.inspect.is_none()
    }

    /// Tab: back to the first thing on the list not yet seen, whether it
    /// was never reached or its card was put away before it could be read;
    /// with nothing missed, it carries on like Space.
    pub(super) fn back_to_skipped(&mut self, real: UnixMs) -> bool {
        if !self.hunting() || self.more_now() || self.talk.prompt.is_some() {
            return self.carry_on(real);
        }
        let now = self.sky_now(real);
        let hz = horizon(self.observer, now);
        let prec = precession(now);
        let missed = (0..self.finds.len()).find(|&i| {
            !self.viewed[i]
                && !matches!(self.finds[i].target, Target::Meteor(_))
                && self.card.as_ref().and_then(|c| c.find) != Some(i)
                && self
                    .find_dir(i, now, &hz, &prec)
                    .is_some_and(|v| alt_az(v).0 > 0.0)
        });
        let Some(i) = missed else {
            return self.carry_on(real);
        };
        self.turn_to(i, real);
        if self.caught[i] {
            self.show_find_card(i, real);
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
        let most = if late(now, self.offset_s) {
            LATE_FINDS
        } else {
            westering_core::finds::MOST
        };
        let fresh = tonight(&self.sky, self.observer, now, self.offset_s, &|id| {
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
            self.finds.push(find);
            self.caught.push(caught);
            self.viewed.push(caught);
            self.steps.push(0);
        }
        if added > 0 {
            self.session.add_finds(added);
            self.guide_darker(added, real);
        }
    }

    /// Turns the view to the first thing still to find.
    pub(crate) fn face_first_find(&mut self, real: UnixMs) {
        let now = self.clock.sky(real);
        let hz = horizon(self.observer, now);
        let prec = precession(now);
        let first = (0..self.finds.len())
            .filter(|&i| !self.caught[i])
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
