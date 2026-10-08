//! What the user does: keys, the pointer, dragging and scrolling, and the
//! one way on that Space, Enter and Tab share; the two ways of looking;
//! quiet words at the foot of the screen.

use super::*;

/// A press shorter than this is a click, not a hold.
const CLICK_MS: UnixMs = 250;

/// The beat between a card going away, or a question being answered, and
/// the view turning to the next find by itself.
const MOVE_ON_MS: UnixMs = 1_600;

impl Game {
    pub(crate) fn input(&mut self, real: UnixMs) {
        self.last_input = real;
        // The user's steering now: nothing moves on by itself.
        self.move_on_was = self.move_on.take().is_some();
        // Whatever the user does shows at once, not at the next paced frame.
        self.urgent = true;
    }

    /// Looking around with the mouse, rather than the keyboard and the
    /// ring. Never by day, where there's no ring.
    pub(crate) fn free_look(&self) -> bool {
        self.free && !self.by_day()
    }

    /// F: between the guided way, with the keyboard and the ring, and free
    /// look, with the mouse, once the wisp has handed the sky over.
    pub(crate) fn toggle_look(&mut self, real: UnixMs) {
        if self.by_day() {
            return;
        }
        if !self.free && !self.handed_over {
            self.guide_not_yet(real);
            return;
        }
        if self.free && self.card.is_some() {
            self.dismiss_card(real);
        }
        let free = !self.free;
        self.free = free;
        self.catch = Catch::default();
        self.mouse_hold = None;
        self.ring_resting = false;
        self.free_since = real;
        self.status_for(
            if free {
                "Free look: drag to look around, and click anything that glows for a closer look"
            } else {
                "Guided: Space carries on, and the ring catches what's in it"
            }
            .into(),
            real,
            4_000,
        );
    }

    /// A little louder or quieter, and kept.
    pub(super) fn nudge_volume(&mut self, by: f64, real: UnixMs) {
        let v = (self.volume() + by).clamp(0.0, 1.0);
        self.journal.settings.volume = Some(v);
        if let Err(e) = self.journal.save_settings() {
            eprintln!("westering: couldn't save settings: {e}");
        }
        self.status(format!("Volume {}%", (v * 100.0).round()), real);
    }

    /// M: the next style of music, and after the last, none; then round
    /// again from the first.
    pub(crate) fn toggle_music(&mut self, real: UnixMs) {
        let settings = &mut self.journal.settings;
        let at = self
            .styles
            .iter()
            .position(|(id, _)| Some(id) == settings.style.as_ref())
            .unwrap_or(0);
        let line = if settings.quiet {
            settings.quiet = false;
            settings.style = self.styles.first().map(|s| s.0.clone());
            self.styles.first().map(|s| format!("Music: {}", s.1))
        } else if at + 1 < self.styles.len() {
            settings.style = Some(self.styles[at + 1].0.clone());
            Some(format!("Music: {}", self.styles[at + 1].1))
        } else {
            settings.quiet = true;
            None
        };
        if let Err(e) = self.journal.save_settings() {
            eprintln!("westering: couldn't save settings: {e}");
        }
        self.status(line.unwrap_or_else(|| "Music off".into()), real);
        if self.keeping() {
            self.wisp_gesture(westering_core::wisp::Gesture::Nod, real);
        }
    }

    /// A short word about a setting just changed: at the foot of the screen,
    /// or at the top when keeping company. A new one replaces the last
    /// without fading out and in again.
    pub(crate) fn status(&mut self, text: String, real: UnixMs) {
        self.status_for(text, real, 2_500);
    }

    /// A quiet word at the foot of the screen, kept up for `hold`.
    pub(crate) fn status_for(&mut self, text: String, real: UnixMs, hold: UnixMs) {
        let shown = match &self.hint {
            Some(h) if real - h.shown < h.hold => h.shown.min(real - 1_000),
            // Almost at once: it answers a key.
            _ => real - 800,
        };
        self.hint = Some(Timed {
            text,
            shown,
            hold: real - shown + hold,
        });
    }

    /// Whether that word is still up, fading or not.
    pub(crate) fn status_up(&self, real: UnixMs) -> bool {
        self.hint
            .as_ref()
            .is_some_and(|h| real - h.shown < 1_000 + h.hold + 1_800)
    }

    pub fn key_pressed(&mut self, key: gdk::Key, real: UnixMs) -> bool {
        self.input(real);
        if self.typing() {
            return false;
        }
        // Any key puts the table of keys away; the ones that only ever
        // carry on or put things away do no more than that.
        if !matches!(key, gdk::Key::question | gdk::Key::F1)
            && self.help_away(real)
            && matches!(
                key,
                gdk::Key::Escape
                    | gdk::Key::space
                    | gdk::Key::Return
                    | gdk::Key::KP_Enter
                    | gdk::Key::Tab
            )
        {
            return true;
        }
        self.releases.retain(|(k, _)| *k != key);
        if key == gdk::Key::space {
            if self.held.space {
                return true;
            }
            self.held.space = true;
        }
        let phase = self.session.phase();
        if self.winding() {
            match key {
                gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::space => self.wind_next(real),
                gdk::Key::Escape => self.wind_done(real),
                gdk::Key::k | gdk::Key::K => self.toggle_company(real),
                gdk::Key::m | gdk::Key::M => self.toggle_music(real),
                _ => {}
            }
            return true;
        }
        if self.keeping() {
            // In the background only a few keys mean anything.
            match key {
                gdk::Key::k | gdk::Key::K => self.toggle_company(real),
                gdk::Key::w | gdk::Key::W => {
                    self.stop_company(real);
                    if self.by_day() {
                        self.day_end(real);
                    } else {
                        self.wind_down(real);
                    }
                }
                gdk::Key::Return | gdk::Key::KP_Enter => return self.guide_next(real),
                gdk::Key::m | gdk::Key::M => self.toggle_music(real),
                gdk::Key::Up | gdk::Key::plus | gdk::Key::equal => self.nudge_volume(0.1, real),
                gdk::Key::Down | gdk::Key::minus => self.nudge_volume(-0.1, real),
                _ => return false,
            }
            return true;
        }
        if matches!(phase, Phase::Finale | Phase::LightsOut | Phase::Over)
            && matches!(key, gdk::Key::Escape | gdk::Key::q | gdk::Key::Q)
        {
            self.quit = true;
            return true;
        }
        if self.drawing.is_some() && self.drawing_key(key, real) {
            return true;
        }
        if self.placing()
            && matches!(
                key,
                gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::Escape
            )
        {
            self.place_weight(real);
            return true;
        }
        if let Some(prompt) = &self.talk.prompt {
            let chips = prompt.chips.len();
            if key == gdk::Key::Escape {
                self.skip_prompt(real);
                return true;
            }
            if let Some(n) = key.to_unicode().and_then(|c| c.to_digit(10))
                && (1..=chips as u32).contains(&n)
            {
                self.answer_chip(n as usize - 1, real);
                return true;
            }
        }
        if self.by_day() {
            return self.day_key(key, real);
        }
        let arrow = matches!(
            key,
            gdk::Key::Left | gdk::Key::Right | gdk::Key::Up | gdk::Key::Down
        );
        if arrow && self.card.is_some() {
            // Looking away from a card puts it down.
            self.end_tour(real);
            self.dismiss_card(real);
        }
        if arrow {
            self.ring_resting = false;
        }
        match key {
            gdk::Key::Left => self.held.left = true,
            gdk::Key::Right => self.held.right = true,
            gdk::Key::Up => self.held.up = true,
            gdk::Key::Down => self.held.down = true,
            gdk::Key::question | gdk::Key::F1 => self.guide_help(real),
            gdk::Key::w | gdk::Key::W => self.wind_down(real),
            gdk::Key::m | gdk::Key::M => self.toggle_music(real),
            gdk::Key::k | gdk::Key::K => self.toggle_company(real),
            gdk::Key::f | gdk::Key::F => self.toggle_look(real),
            gdk::Key::c | gdk::Key::C => {
                if self.hunting() && self.card.is_none() && self.talk.prompt.is_none() {
                    self.start_drawing(real);
                }
            }
            gdk::Key::l | gdk::Key::L => {
                if !matches!(phase, Phase::Finale | Phase::LightsOut | Phase::Over) {
                    self.request = Some(crate::talk::Request::Book(Some(self.night.clone())));
                }
            }
            gdk::Key::plus | gdk::Key::equal | gdk::Key::KP_Add => self.held.zoom_in = true,
            gdk::Key::minus | gdk::Key::KP_Subtract => self.held.zoom_out = true,
            gdk::Key::Shift_L | gdk::Key::Shift_R => self.held.fast = true,
            // Space held on what's in the ring catches it; otherwise, like
            // Enter and Tab, it carries on, when it's let go.
            gdk::Key::space => {
                self.space_since = Some(real);
                self.space_caught = false;
                if self.card.is_none()
                    && self.talk.prompt.is_none()
                    && !self.more_now()
                    && self.hunting()
                    && !self.free_look()
                {
                    self.catch.holding = true;
                    if self.catch.target.is_none() {
                        self.try_meteor(real);
                    }
                }
            }
            gdk::Key::Return | gdk::Key::KP_Enter => {
                if !self.carry_on(real) {
                    return false;
                }
            }
            gdk::Key::Tab => {
                self.back_to_skipped(real);
            }
            gdk::Key::Escape => {
                if self.card.is_some() {
                    self.end_tour(real);
                    self.dismiss_card(real);
                } else if matches!(phase, Phase::Arrival | Phase::Hunt) {
                    if real < self.esc_armed {
                        self.wind_down(real);
                    } else {
                        self.esc_armed = real + 4_000;
                        self.hint = Some(Timed {
                            text: "Press Esc again to wind down".into(),
                            shown: real,
                            hold: 3_000,
                        });
                    }
                }
            }
            _ => return false,
        }
        true
    }

    pub fn key_released(&mut self, key: gdk::Key, real: UnixMs) {
        self.input(real);
        self.releases.push((key, real));
    }

    pub(super) fn settle_releases(&mut self, real: UnixMs) {
        let due: Vec<(gdk::Key, UnixMs)> = self
            .releases
            .iter()
            .filter(|(_, at)| real - at > 45)
            .copied()
            .collect();
        self.releases.retain(|(_, at)| real - at <= 45);
        for (key, at) in due {
            self.release(key, at);
        }
    }

    /// The guided evening carries itself on: a beat after a card is put
    /// away, or a question is answered or let pass, the view turns to the
    /// next thing to find, as a tap of Space would. It waits for the wisp
    /// to finish and for anything being asked, and gives way to whatever
    /// the user does first.
    pub(crate) fn tick_move_on(&mut self, real: UnixMs) {
        let asking = matches!(
            self.talk.flow,
            Some(
                crate::talk::Flow::Question { .. }
                    | crate::talk::Flow::NameStar { .. }
                    | crate::talk::Flow::PlanWho { .. }
                    | crate::talk::Flow::PlanOutcome { .. }
                    | crate::talk::Flow::CourseCheck { .. }
                    | crate::talk::Flow::LookBack { .. }
            )
        );
        if std::mem::replace(&mut self.talk.asking, asking) && !asking {
            self.move_on = Some(real + MOVE_ON_MS);
        }
        let Some(at) = self.move_on else {
            return;
        };
        let guided = self.hunting() && !self.free_look() && !self.by_day() && !self.keeping();
        if !guided || self.card.is_some() || self.tour.is_some() || self.drawing.is_some() {
            self.move_on = None;
            return;
        }
        // Something's being asked, or about to be, or said: after that.
        if self.talk.prompt.is_some() || self.talk.pending.is_some() || self.wisp_busy(real) {
            self.move_on = Some(at.max(real + MOVE_ON_MS));
            return;
        }
        if real < at {
            return;
        }
        self.move_on = None;
        // Nothing left to turn to, or something's already in the ring.
        let in_ring = self.catch.target.is_some() && !self.ring_resting;
        if !self.session.all_found() && !in_ring {
            self.turn_to_next(real);
        }
    }

    /// The one way on, whatever's showing, a step at a time so nothing is
    /// missed: the wisp's next word first, then past a card or a story's
    /// page, then round to the next thing to find.
    pub(crate) fn carry_on(&mut self, real: UnixMs) -> bool {
        if self.more_now() {
            // Only hearing the wisp out: the evening still carries on
            // by itself once it's done, if it was about to.
            if self.move_on_was {
                self.move_on = Some(real + MOVE_ON_MS);
            }
            return self.guide_next(real);
        }
        if self.card.is_some() {
            self.dismiss_card(real);
            return true;
        }
        if self.talk.prompt.is_some() || !self.hunting() {
            return self.guide_next(real);
        }
        self.guide_next(real);
        // Something is already in the ring: don't swing away from it.
        if self.catch.target.is_some() && !self.free_look() && !self.ring_resting {
            self.status(
                "Hold Space, or press and hold on the ring, to catch it".into(),
                real,
            );
            return true;
        }
        self.turn_to_next(real);
        true
    }

    pub(super) fn release(&mut self, key: gdk::Key, at: UnixMs) {
        match key {
            gdk::Key::Left => self.held.left = false,
            gdk::Key::Right => self.held.right = false,
            gdk::Key::Up => self.held.up = false,
            gdk::Key::Down => self.held.down = false,
            gdk::Key::plus | gdk::Key::equal | gdk::Key::KP_Add => self.held.zoom_in = false,
            gdk::Key::minus | gdk::Key::KP_Subtract => self.held.zoom_out = false,
            gdk::Key::Shift_L | gdk::Key::Shift_R => self.held.fast = false,
            gdk::Key::space => {
                self.held.space = false;
                self.catch.holding = false;
                // Letting go carries on, however long the key was down,
                // unless the press was spent on the ring: one that caught
                // something has made a card, which mustn't be put away, and
                // one let go part-way is told to keep holding.
                if let Some(down) = self.space_since.take()
                    && !self.by_day()
                    && !self.winding()
                {
                    if std::mem::take(&mut self.space_caught) {
                    } else if self.catch.progress > 0.0 && at - down >= TAP_MS {
                        self.status("Keep Space held until the ring closes".into(), at);
                    } else {
                        self.carry_on(at);
                    }
                }
            }
            _ => {}
        }
    }

    pub fn drag_begin(&mut self, x: f64, y: f64, real: UnixMs) {
        self.input(real);
        // The guided way: a press held on the ring catches, as Space does.
        let (cx, cy) = (self.camera.width / 2.0, self.camera.height / 2.0);
        let on_ring = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt() < self.reticle_radius() + 12.0;
        if on_ring
            && self.hunting()
            && !self.free_look()
            && !self.ring_resting
            && self.card.is_none()
            && self.talk.prompt.is_none()
            && self.drawing.is_none()
            && self.tour.is_none()
            && !self.more_now()
        {
            self.mouse_hold = Some(real);
            self.catch.holding = true;
            if self.catch.target.is_none() {
                self.try_meteor(real);
            }
            return;
        }
        self.ring_resting = false;
        self.look = None;
        self.drag_from = Some((self.camera.az, self.camera.alt, 0.0, 0.0));
    }

    pub fn drag_update(&mut self, dx: f64, dy: f64, real: UnixMs) {
        self.input(real);
        // Moving off while holding the ring lets go, and looks around instead.
        if self.mouse_hold.is_some() && dx.abs() + dy.abs() > 8.0 {
            self.mouse_hold = None;
            self.catch.holding = false;
            self.ring_resting = false;
            self.look = None;
            self.drag_from = Some((self.camera.az, self.camera.alt, 0.0, 0.0));
        }
        if let Some((az, alt, _, _)) = self.drag_from {
            let ppd = self.camera.px_per_degree();
            let cos_alt = self.camera.alt.to_radians().cos().max(0.2);
            self.camera.az = az - dx / ppd / cos_alt;
            self.camera.alt = alt + dy / ppd;
            self.camera.update();
        }
    }

    /// Says whether the press was held on the ring, catching, rather
    /// than a click.
    pub fn drag_end(&mut self) -> bool {
        self.drag_from = None;
        let Some(since) = self.mouse_hold.take() else {
            return false;
        };
        self.catch.holding = false;
        self.last_real - since > CLICK_MS
    }

    pub fn scroll(&mut self, dy: f64, real: UnixMs) {
        self.input(real);
        if matches!(
            self.session.phase(),
            Phase::Hunt | Phase::Arrival | Phase::Dimming
        ) {
            self.look = None;
            self.let_go();
            self.camera.fov *= 1.12f64.powf(dy);
            self.camera.update();
        }
    }

    /// A click on something turns the view to it.
    pub fn click(&mut self, x: f64, y: f64, real: UnixMs) {
        self.input(real);
        if self.help_away(real) {
            return;
        }
        if self.winding() {
            self.wind_next(real);
            return;
        }
        if self.on_bubble(x, y) {
            self.guide_next(real);
            return;
        }
        if self.on_wisp(x, y) {
            // With more to say, a click hurries it on; otherwise it helps.
            if self.keeping() {
                if !self.guide_next(real) {
                    self.toggle_company(real);
                }
            } else if !self.guide_next(real) {
                self.guide_help(real);
            }
            return;
        }
        if self.by_day() {
            self.day_click(x, y, real);
            return;
        }
        if self.click_hovered(x, y, real) {
            return;
        }
        // Free look: a click on empty sky backs out. The guided way: it
        // carries on, as Space does, so the mouse alone will do.
        if self.card.is_some() {
            if self.free_look() {
                self.dismiss_card(real);
            } else {
                self.carry_on(real);
            }
            return;
        }
        if !self.hunting() {
            return;
        }
        let v = self.camera.unproject(x, y);
        let (alt, az) = alt_az(v);
        self.look = Some(Look {
            az,
            alt,
            fov: self.camera.fov,
            rate: 2.2,
        });
    }

    /// Keys, easing and the finale's camera.
    pub(super) fn steer(&mut self, dt: f64, tempo: f64) {
        let phase = self.session.phase();
        if phase == Phase::Finale && !self.finale_turned {
            let into = self.session.in_phase(self.last_real);
            if into >= self.session.timings.lapse
                && let Some(h) = &self.handoff
            {
                // After the lapse, drift round to face the hand-off.
                self.look = Some(Look {
                    az: h.az,
                    alt: h.alt.clamp(8.0, 60.0) + 6.0,
                    fov: 95.0,
                    rate: 0.3,
                });
                self.finale_turned = true;
            }
        }
        let free = matches!(
            phase,
            Phase::Arrival | Phase::Weights | Phase::Hunt | Phase::Dimming
        );
        let speed = self.camera.fov * 0.5 * if self.held.fast { 2.6 } else { 1.0 } * tempo.sqrt();
        let want = if free && self.catch.progress < 0.02 {
            (
                (self.held.right as i32 - self.held.left as i32) as f64 * speed,
                (self.held.up as i32 - self.held.down as i32) as f64 * speed,
            )
        } else {
            (0.0, 0.0)
        };
        let k = 1.0 - (-dt * 7.0).exp();
        self.pan.0 += (want.0 - self.pan.0) * k;
        self.pan.1 += (want.1 - self.pan.1) * k;
        if self.pan.0.abs() > 1e-4 || self.pan.1.abs() > 1e-4 {
            self.look = None;
            let cos_alt = self.camera.alt.to_radians().cos().max(0.2);
            self.camera.az += self.pan.0 * dt / cos_alt;
            self.camera.alt += self.pan.1 * dt;
        }
        if free {
            let zoom = self.held.zoom_out as i32 - self.held.zoom_in as i32;
            if zoom != 0 {
                self.camera.fov *= (1.0 + 0.9 * dt).powi(zoom);
            }
        }
        // Following something: keep it where it is in the view as the sky turns.
        if let Some(i) = self.track {
            let now = self.sky_now(self.last_real);
            let hz = horizon(self.observer, now);
            let prec = precession(now);
            match self.find_dir(i, now, &hz, &prec) {
                Some(v)
                    if self.pan.0.abs() < 1e-3
                        && self.pan.1.abs() < 1e-3
                        && self.drag_from.is_none() =>
                {
                    let (alt, az) = alt_az(v);
                    match &mut self.look {
                        Some(look) => {
                            look.az = az;
                            look.alt = alt;
                        }
                        None => {
                            self.camera.az = az;
                            self.camera.alt = alt;
                        }
                    }
                }
                _ => self.track = None,
            }
        }
        self.inspect_steer();
        if let Some(look) = self.look {
            let k = 1.0 - (-dt * look.rate * 2.0).exp();
            let daz = turn(self.camera.az, look.az);
            self.camera.az += daz * k;
            self.camera.alt += (look.alt - self.camera.alt) * k;
            self.camera.fov *= (look.fov / self.camera.fov).powf(k);
            if daz.abs() < 0.01
                && (look.alt - self.camera.alt).abs() < 0.01
                && (look.fov / self.camera.fov - 1.0).abs() < 0.001
            {
                self.look = None;
            }
        }
        if self.placing() {
            self.camera.alt = self.camera.alt.min(25.0);
        }
        self.camera.update();
    }
}
