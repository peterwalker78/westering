//! The ring and what it catches: holding on a find, meteors, the card
//! that tells about it, and putting the card away.

use super::*;

impl Game {
    pub(crate) fn reticle_radius(&self) -> f64 {
        (self.camera.width.min(self.camera.height) * 0.055).max(26.0)
    }

    pub(super) fn try_meteor(&mut self, real: UnixMs) {
        let Some(i) = (0..self.finds.len())
            .find(|&i| !self.caught[i] && matches!(self.finds[i].target, Target::Meteor(_)))
        else {
            return;
        };
        let alive = self
            .meteors
            .iter()
            .any(|m| real >= m.start && real <= m.start + m.duration + 400);
        if alive {
            self.catch.holding = false;
            self.caught_one(i, real);
        }
    }

    pub(crate) fn caught_one(&mut self, i: usize, real: UnixMs) {
        self.caught[i] = true;
        self.space_caught = true;
        if !matches!(self.finds[i].target, Target::Meteor(_)) {
            self.track = Some(i);
        }
        self.session.found_one(real);
        let find = self.finds[i].clone();
        if let Err(e) = self.journal.mark_found(&find.id, &self.night) {
            eprintln!("westering: couldn't save what was found: {e}");
        }
        if !self.page.finds.contains(&find.name) {
            self.page.finds.push(find.name.clone());
        }
        self.save_page();
        if self.begin_tour(i, real) {
            // A story or a walk is its own showing: nothing to come back to.
            self.viewed[i] = true;
            self.guide_tour_began(real);
            return;
        }
        if let Target::Figure(f) = self.finds[i].target {
            self.reveal = Some((f, real));
        }
        // Free look: a close look, with all there is to tell; questions
        // wait for the guided way.
        if self.free_look() {
            // Whatever it brings to mind waits for a pause.
            self.after_catch(i, real);
            let x = self.pointer.map_or(self.camera.width / 2.0, |p| p.0);
            if !self.inspect(Subject::Find(i), x, real) {
                self.show_find_card(i, real);
            }
            return;
        }
        self.show_find_card(i, real);
        self.after_catch(i, real);
        self.guide_caught(real);
    }

    /// The card for find `i`: what it is, something true about it, and its
    /// photograph if it has one for the card.
    pub(super) fn show_find_card(&mut self, i: usize, real: UnixMs) {
        let find = self.finds[i].clone();
        let x = self.beside(Some(i), self.zoom_for(i));
        let kicker = self.kind_word(i).to_owned();
        let mut body = find.fact;
        if matches!(self.finds[i].target, Target::Hop(_))
            && (!self.journal.settings.seen.iter().any(|k| k == "hop-done")
                || westering_core::finds::stable_hash((0, 0, 0), &self.night).is_multiple_of(3))
        {
            body = format!("{body}\n\n{}", self.sky.tours.hop_closing);
            if !self.journal.settings.seen.iter().any(|k| k == "hop-done") {
                self.journal.settings.seen.push("hop-done".into());
                if let Err(e) = self.journal.save_settings() {
                    eprintln!("westering: couldn't save settings: {e}");
                }
            }
        }
        self.card = Some(Card {
            footnote: None,
            picture: self.card_picture(i),
            find: Some(i),
            x,
            kicker,
            title: find.name,
            body,
            shown: real,
        });
    }

    pub(crate) fn dismiss_card(&mut self, real: UnixMs) {
        let read = self
            .card
            .as_ref()
            .is_some_and(|c| real - c.shown >= 2 * VIEWED_MS);
        // Up long enough to read counts as seen; put away sooner, Tab can
        // bring it back.
        if let Some((i, shown)) = self.card.as_ref().and_then(|c| Some((c.find?, c.shown))) {
            if real - shown >= VIEWED_MS {
                self.viewed[i] = true;
            } else if !self.told_tab && !self.free_look() {
                self.told_tab = true;
                self.status("Tab brings back anything you skipped".into(), real);
            }
        }
        if self.tour.is_some() && self.turn_page(real) {
            return;
        }
        self.card = None;
        self.guide_card_gone(real);
        if self.by_day() {
            if self.session.all_found() {
                self.guide_all_found(real);
            }
            self.day_card_gone(real);
            return;
        }
        if let Some(fov) = self.catch.fov_before.take() {
            self.look = Some(Look {
                az: self.camera.az,
                alt: self.camera.alt,
                fov,
                rate: 1.4,
            });
        }
        self.catch.progress = 0.0;
        self.catch.target = None;
        if self.free_look() {
            // Back out to where the view was, and the wisp home. After a
            // card that was read, a waiting question may be offered, once
            // an evening, when the view has settled.
            self.end_inspect(real);
            if read && !self.free_asked && self.talk.pending.is_some() {
                self.free_moment = Some(real + 1_800);
            }
            return;
        }
        if self.session.all_found() && self.hunting() {
            self.guide_all_found(real);
        }
    }

    pub(super) fn update_catch(
        &mut self,
        dt: f64,
        tempo: f64,
        real: UnixMs,
        now: UnixMs,
        hz: &Mat3,
        prec: &Mat3,
    ) {
        if !self.hunting() || self.card.is_some() {
            return;
        }
        // The ring is the guided way's, and rests after a story or a walk
        // until the view moves on.
        if self.free_look() || self.ring_resting {
            self.catch = Catch::default();
            return;
        }
        let r = self.reticle_radius();
        let (cx, cy) = (self.camera.width / 2.0, self.camera.height / 2.0);
        let near = (0..self.finds.len())
            .filter(|&i| !self.caught[i] && self.catchable(i))
            .filter_map(|i| {
                let v = self.find_dir(i, now, hz, prec)?;
                if alt_az(v).0 < -0.5 {
                    return None;
                }
                let (x, y) = self.camera.project(v)?;
                let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
                (d < r).then_some((i, d))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i);
        if self.catch.progress <= 0.0 {
            self.catch.target = near;
        }
        let Some(i) = self.catch.target else {
            self.catch.progress = 0.0;
            return;
        };
        if self.catch.holding {
            if self.catch.fov_before.is_none() {
                self.catch.fov_before = Some(self.camera.fov);
            }
            let hold = if matches!(self.finds[i].target, Target::Hop(_)) {
                0.8
            } else {
                1.4
            };
            self.catch.progress += dt / (hold / tempo.max(0.3));
            if let Some(v) = self.find_dir(i, now, hz, prec) {
                let (alt, az) = alt_az(v);
                let p = smoothstep(self.catch.progress);
                let zoom = self.zoom_for(i);
                let from = self.catch.fov_before.unwrap_or(self.camera.fov);
                self.look = Some(Look {
                    az,
                    alt,
                    fov: from * (zoom / from).powf(p),
                    rate: 3.0,
                });
            }
            if self.catch.progress >= 1.0 {
                self.catch.progress = 1.0;
                self.catch.holding = false;
                if !self.hop_step(i, real) {
                    self.caught_one(i, real);
                }
            }
        } else if self.catch.progress > 0.0 {
            self.catch.progress = (self.catch.progress - dt * 1.8).max(0.0);
            if self.catch.progress == 0.0 {
                if let Some(fov) = self.catch.fov_before.take() {
                    self.look = Some(Look {
                        az: self.camera.az,
                        alt: self.camera.alt,
                        fov,
                        rate: 1.4,
                    });
                }
                self.catch.target = None;
            }
        }
    }

    pub(super) fn spawn_meteors(&mut self, real: UnixMs, now: UnixMs, hz: &Mat3) {
        self.meteors.retain(|m| real < m.start + m.duration + 600);
        let Some(shower) = self.finds.iter().find_map(|f| match f.target {
            Target::Meteor(s) => Some(s),
            _ => None,
        }) else {
            return;
        };
        if real < self.next_meteor || !matches!(self.session.phase(), Phase::Hunt | Phase::Dimming)
        {
            return;
        }
        let s = &self.sky.lists.showers[shower];
        let radiant = apply(hz, apply(&precession(now), unit(s.ra, s.dec)));
        let tempo = self.session.tempo(real);
        let wait = 9_000.0 + self.random() * 22_000.0;
        self.next_meteor = real + (wait / tempo) as UnixMs;
        // A direction at right angles to the radiant, then out along it.
        let seed = [
            self.random() - 0.5,
            self.random() - 0.5,
            self.random() - 0.5,
        ];
        let along = dot3(seed, radiant);
        let p = [
            seed[0] - along * radiant[0],
            seed[1] - along * radiant[1],
            seed[2] - along * radiant[2],
        ];
        let n = dot3(p, p).sqrt().max(1e-9);
        let p = [p[0] / n, p[1] / n, p[2] / n];
        let a = (12.0 + self.random() * 40.0).to_radians();
        let from = [
            radiant[0] * a.cos() + p[0] * a.sin(),
            radiant[1] * a.cos() + p[1] * a.sin(),
            radiant[2] * a.cos() + p[2] * a.sin(),
        ];
        if alt_az(from).0 < 8.0 {
            return;
        }
        let toward = [
            p[0] * a.cos() - radiant[0] * a.sin(),
            p[1] * a.cos() - radiant[1] * a.sin(),
            p[2] * a.cos() - radiant[2] * a.sin(),
        ];
        let length = (7.0 + self.random() * 14.0).to_radians();
        let brightness = 0.8 + self.random() as f32 * 0.9;
        let duration = 650 + (self.random() * 500.0) as UnixMs;
        self.meteors.push(Meteor {
            start: real,
            duration,
            from,
            toward,
            length,
            brightness,
        });
    }

    /// The card as drawn this frame, rising in, or fading and sinking as
    /// it's put away.
    pub(crate) fn card_view(&mut self, real: UnixMs) -> Option<crate::view::CardView> {
        let rise = |age: UnixMs| (1.0 - smoothstep(age as f64 / 450.0)) * 14.0;
        let compact = compact(self.camera.width, self.camera.height);
        let width = (self.camera.width - 32.0).min(400.0);
        let picture = self
            .card
            .as_ref()
            .and_then(|c| c.picture.clone())
            .and_then(|id| {
                let who = self.photos.credit(&id)?.credit.clone();
                Some((self.photos.card_texture(&id)?, who))
            });
        let card = self.card.as_ref().map(|c| crate::view::CardView {
            picture: picture.clone(),
            footnote: c.footnote.clone(),
            middle: self.inspect.is_some() && !compact,
            // In a small window it takes the middle.
            x: if compact {
                (self.camera.width - width) / 2.0
            } else {
                c.x
            },
            width,
            compact,
            y: if self.inspect.is_some() && !compact {
                self.camera.height / 2.0
            } else {
                (self.camera.height / 2.0 - 70.0).max(70.0)
            } + rise(real - c.shown),
            kicker: c.kicker.clone(),
            title: c.title.clone(),
            body: c.body.clone(),
            keys: match &self.tour {
                Some(t) if t.on_last_page() => vec![("Space".into(), "carry on".into())],
                Some(_) => vec![
                    ("Space".into(), "go on".into()),
                    ("Esc".into(), "stop here".into()),
                ],
                None if self.free_look() => vec![("Esc".into(), "zoom back out".into())],
                None => vec![("Space".into(), "carry on".into())],
            },
            alpha: envelope(real - c.shown, 350, 600_000, 800),
        });
        // A card put away fades and sinks rather than vanishing.
        let card = match (card, self.shown_card.take()) {
            (Some(c), _) => {
                self.leaving_card = None;
                self.shown_card = Some(c.clone());
                Some(c)
            }
            (None, Some(last)) => {
                self.leaving_card = Some((last, real));
                None
            }
            (None, None) => None,
        };
        card.or_else(|| {
            let (c, at) = self.leaving_card.as_ref()?;
            let f = (real - at) as f64 / 260.0;
            if f >= 1.0 {
                self.leaving_card = None;
                return None;
            }
            let mut c = c.clone();
            c.alpha *= 1.0 - smoothstep(f);
            c.y += smoothstep(f) * 10.0;
            Some(c)
        })
    }
}
