//! Free look's cards. A click on anything tells all there is: every fact
//! kept about it, where it is now, how bright and how big, and its
//! photograph when there is one. The wisp keeps out of it.

use super::*;

/// The most paragraphs a card holds, so it stays on the screen.
const MOST_PARAGRAPHS: usize = 5;

impl Game {
    /// Everything to say about something: kicker, title, the paragraphs,
    /// and a photograph's id if there is one.
    pub(crate) fn detail(
        &self,
        target: Target,
        now: UnixMs,
    ) -> Option<(String, String, String, Option<String>)> {
        let hz = horizon(self.observer, now);
        let prec = precession(now);
        let year = civil_date(now, self.offset_s(now)).0;
        let place = |v: Vec3| {
            let (alt, az) = alt_az(v);
            westering_core::finale::whereabouts(alt, az)
        };
        let (kicker, title, mut paragraphs, picture) = match (self.find_of(target), target) {
            (Some(i), _) => {
                let find = &self.finds[i];
                let here = self.find_dir(i, now, &hz, &prec).map(place);
                match find.target {
                    Target::Body(b) => {
                        let (_, _, facts, pic) = self.body_detail(b, now);
                        let mut all = vec![find.fact.clone()];
                        all.extend(facts);
                        (self.kind_word(i).to_owned(), find.name.clone(), all, pic).with_place(here)
                    }
                    Target::Showpiece(p) => {
                        let (k, t, all, pic) = self.piece_detail(p);
                        (k, t, all, pic).with_place(here)
                    }
                    Target::Star(hr) => {
                        let (k, t, all, pic) = self.star_detail(hr, year)?;
                        (k, t, all, pic).with_place(here)
                    }
                    Target::Figure(f) => {
                        let (_, t, facts, pic) = self.figure_detail(f);
                        let mut all = vec![find.fact.clone()];
                        all.extend(facts);
                        (self.kind_word(i).to_owned(), t, all, pic).with_place(here)
                    }
                    _ => return None,
                }
            }
            (None, Target::Body(b)) => {
                let s = see(b, self.observer, now);
                self.body_detail(b, now)
                    .with_place(Some(westering_core::finale::whereabouts(s.alt, s.az)))
            }
            (None, Target::Figure(f)) => {
                let v = self.target_dir(target, now, &hz, &prec);
                self.figure_detail(f).with_place(v.map(place))
            }
            (None, Target::Showpiece(p)) => {
                let piece = &self.sky.lists.showpieces[p];
                let v = apply(&hz, apply(&prec, unit(piece.ra, piece.dec)));
                let (k, t, all, pic) = self.piece_detail(p);
                (k, t, all, pic).with_place(Some(place(v)))
            }
            (None, Target::Star(hr)) => {
                let v = self
                    .sky
                    .stars
                    .index_of(hr)
                    .map(|idx| apply(&hz, self.star_dirs[idx]));
                let (k, t, all, pic) = self.star_detail(hr, year)?;
                (k, t, all, pic).with_place(v.map(place))
            }
            (None, _) => return None,
        };
        paragraphs.dedup();
        paragraphs.truncate(MOST_PARAGRAPHS);
        Some((kicker, title, paragraphs.join("\n\n"), picture))
    }

    fn body_detail(&self, b: Body, now: UnixMs) -> (String, String, Vec<String>, Option<String>) {
        let mut all = self
            .sky
            .lists
            .bodies
            .iter()
            .find(|f| f.id == b.id())
            .map(|f| f.facts.clone())
            .unwrap_or_default();
        let s = see(b, self.observer, now);
        all.extend(numbers(
            Some(s.position.magnitude),
            s.position.diameter / 60.0,
        ));
        let name = {
            let n = b.name();
            let mut c = n.chars();
            c.next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        };
        let kind = Target::Body(b).kind_word(&self.sky.lists.showpieces);
        let pic = self.photos.credit(b.id()).map(|c| c.id.clone());
        (kind.to_owned(), name, all, pic)
    }

    /// A constellation's card: what's kept about it, and the named stars
    /// to look for in it, brightest first.
    fn figure_detail(&self, f: usize) -> (String, String, Vec<String>, Option<String>) {
        let figure = &self.sky.figures[f];
        let mut all = Vec::new();
        if let Some(note) = self.sky.notes.iter().find(|n| n.abbrev == figure.abbrev) {
            all.push(note.fact.clone());
            all.extend(note.more.iter().cloned());
        }
        let mut named: Vec<(f32, &str)> = self
            .sky
            .lists
            .stars
            .iter()
            .filter(|s| s.constellation == figure.abbrev)
            .filter_map(|s| Some((self.sky.stars.get(s.hr)?.mag, s.name.as_str())))
            .collect();
        named.sort_by(|a, b| a.0.total_cmp(&b.0));
        let names: Vec<&str> = named.iter().take(3).map(|n| n.1).collect();
        match names.as_slice() {
            [] => {}
            [one] => all.push(format!("Look for {one}, its brightest named star.")),
            [first @ .., last] => all.push(format!(
                "Look for {} and {last}, its brightest named stars.",
                first.join(", ")
            )),
        }
        let kind = Target::Figure(f).kind_word(&self.sky.lists.showpieces);
        let pic = self
            .photos
            .of_constellation(&figure.abbrev)
            .map(|c| c.id.clone());
        (kind.to_owned(), figure.name.clone(), all, pic)
    }

    fn piece_detail(&self, p: usize) -> (String, String, Vec<String>, Option<String>) {
        let piece = &self.sky.lists.showpieces[p];
        let mut all = vec![piece.fact.clone()];
        all.extend(piece.more.iter().cloned());
        // How bright and how big, when there's a brightness to give.
        all.extend(numbers((piece.mag < 30.0).then_some(piece.mag), piece.size));
        let kind = Target::Showpiece(p).kind_word(&self.sky.lists.showpieces);
        let pic = self.photos.credit(&piece.id).map(|c| c.id.clone());
        (kind.to_owned(), piece.name.clone(), all, pic)
    }

    fn star_detail(
        &self,
        hr: u16,
        year: i32,
    ) -> Option<(String, String, Vec<String>, Option<String>)> {
        let named = self.sky.lists.star_name(hr)?;
        let mut all: Vec<String> = named.describe(year).into_iter().collect();
        all.extend(named.more.iter().cloned());
        if let Some(star) = self.sky.stars.get(hr) {
            all.extend(numbers(Some(star.mag as f64), 0.0));
        }
        // Its own photograph, however that's shown, before its
        // constellation's.
        let own = self
            .photos
            .credit(&named.name.to_lowercase())
            .map(|c| c.id.clone());
        let (pic, wide) = match (own, self.star_picture(hr)) {
            (Some(id), _) => (Some(id), None),
            (None, Some((id, wide))) => (Some(id), wide),
            (None, None) => (None, None),
        };
        // Room is kept for the line saying whose photograph it is.
        if let Some(wide) = wide {
            all.dedup();
            all.truncate(MOST_PARAGRAPHS - 1);
            all.push(wide);
        }
        let kind = Target::Star(hr).kind_word(&self.sky.lists.showpieces);
        Some((kind.to_owned(), named.name.clone(), all, pic))
    }

    /// Shows a subject's card with its left edge at `x`. A photograph the
    /// sky itself is showing isn't shown again on the card; either way the
    /// card says what the picture shows that an eye wouldn't.
    fn show_detail(&mut self, target: Target, x: f64, real: UnixMs, in_sky: bool) -> bool {
        let now = self.sky_now(real);
        let Some((kicker, title, mut body, picture)) = self.detail(target, now) else {
            return false;
        };
        let credit = picture
            .as_deref()
            .and_then(|id| self.photos.credit(id))
            .cloned();
        if let Some(caption) = credit.as_ref().and_then(|c| c.caption.clone()) {
            body = format!("{body}\n\n{caption}");
        }
        // A photograph on the card carries its own credit.
        self.card = Some(Card {
            footnote: credit
                .filter(|_| in_sky)
                .map(|c| format!("Photograph: {}", c.credit)),
            picture: picture.filter(|_| !in_sky),
            find: self.find_of(target),
            x,
            kicker,
            title,
            body,
            // A beat after the view begins to close in.
            shown: real + CARD_BEAT_MS,
        });
        true
    }

    /// The photograph the sky shows of something close up, if it has one.
    pub(super) fn inspect_eye(&self, target: Target) -> Option<Target> {
        self.eye_photo(target).map(|_| target)
    }

    /// Pixels per degree at a field of view.
    fn ppd_at(&self, fov: f64) -> f64 {
        (self.camera.width / 2.0) / (2.0 * (fov.to_radians() / 4.0).tan()) * std::f64::consts::PI
            / 180.0
    }

    /// Free look's click: the view glides in until the thing fills its
    /// side of the screen, photograph and all, with its card beside it and
    /// the wisp between the two. Esc glides back out to where it was.
    pub(crate) fn inspect(
        &mut self,
        subject: Option<Target>,
        clicked_x: f64,
        real: UnixMs,
    ) -> bool {
        let Some(target) = subject else {
            return false;
        };
        if self.inspecting().is_some_and(|i| i.subject == target) {
            return true;
        }
        let now = self.sky_now(real);
        let (hz, prec) = (horizon(self.observer, now), precession(now));
        if self.target_dir(target, now, &hz, &prec).is_none() {
            return false;
        }
        let cam = self.camera;
        let (w, h) = (cam.width, cam.height);
        let small = compact(w, h);
        let eye = self.inspect_eye(target);
        let mut fov = self.zoom_for_target(&target);
        let half = |g: &Game, fov: f64| match (&target, eye) {
            (_, Some(_)) => g.target_half(&target, fov),
            (Target::Figure(f), None) => g.sky.figures[*f]
                .centre(&g.sky.stars)
                .map_or(0.0, |(_, reach)| reach * g.ppd_at(fov)),
            _ => 18.0,
        };
        // Side by side: the thing, room for the wisp, then the card; the
        // view widens a little if the thing would crowd them.
        let room = (w - 2.0 * MARGIN - GAP - CARD_W).max(120.0) / 2.0;
        let most = room.min(h * 0.4);
        let mut hp = half(self, fov);
        // A photograph fills its side of the screen; anything else keeps
        // its own zoom unless it would crowd the card.
        let want = if eye.is_some() { most * 0.92 } else { most };
        if hp > most || (eye.is_some() && hp > 1.0) {
            fov = (fov * hp / want).clamp(crate::camera::FOV_NARROWEST, crate::camera::FOV_WIDEST);
            hp = half(self, fov);
        }
        let hp = hp.min(most);
        let total = 2.0 * hp + GAP + CARD_W;
        let left = ((w - total) / 2.0).max(MARGIN);
        // The thing stays on the side it was clicked on.
        let (ox, card_x, wisp_x) = if clicked_x <= w / 2.0 {
            (
                left + hp,
                left + 2.0 * hp + GAP,
                left + 2.0 * hp + GAP / 2.0,
            )
        } else {
            let right = w - left;
            (
                right - hp,
                right - 2.0 * hp - GAP - CARD_W,
                right - 2.0 * hp - GAP / 2.0,
            )
        };
        let ox = if small { w / 2.0 } else { ox };
        let before = self
            .inspecting()
            .map_or((cam.az, cam.alt, cam.fov), |i| i.before);
        let Looking::Free(free) = &mut self.looking else {
            return false;
        };
        free.back = None;
        free.inspect = Some(Inspect {
            subject: target,
            before,
            from: (cam.az, cam.alt, cam.fov),
            started: real,
            fov,
            at_x: ox,
            steering: true,
            arrived: false,
        });
        if !self.show_detail(target, card_x, real, eye.is_some()) {
            if let Some(slot) = self.inspect_slot() {
                *slot = None;
            }
            return false;
        }
        // The wisp keeps it company between the two, a little above the
        // middle, looking at what's shown.
        if !self.calm() {
            self.fly(
                crate::guide::Aim::Hang(wisp_x, (h / 2.0 - 70.0).max(80.0), ox, h / 2.0),
                real,
                HANG_MS,
            );
        }
        true
    }

    /// Steers the view to what's being looked at, gliding there in the
    /// same time whatever it is and however far, then following it as the
    /// sky turns; or back out after a close look. Any look round by hand
    /// lets go.
    pub(super) fn inspect_steer(&mut self) {
        let by_hand = self.held.zoom_in
            || self.held.zoom_out
            || self.pan.0.abs() > 1e-3
            || self.pan.1.abs() > 1e-3
            || self.drag_from.is_some();
        if by_hand {
            self.let_go();
        }
        let real = self.last_real;
        let Looking::Free(free) = &mut self.looking else {
            self.hold_beside_card();
            return;
        };
        let Some(ins) = &mut free.inspect else {
            if let Some(back) = free.back {
                let s = back.progress(real);
                if s >= 1.0 {
                    free.back = None;
                }
                self.look = None;
                (self.camera.az, self.camera.alt, self.camera.fov) = glide(back.from, back.to, s);
            }
            return;
        };
        if !ins.steering {
            return;
        }
        let (subject, fov, at_x, from, started) =
            (ins.subject, ins.fov, ins.at_x, ins.from, ins.started);
        let s = ((real - started) as f64 / GLIDE_MS as f64).clamp(0.0, 1.0);
        let arrived = ins.arrived;
        if s >= 1.0 {
            ins.arrived = true;
        }
        let now = self.sky_now(real);
        let (hz, prec) = (horizon(self.observer, now), precession(now));
        let Some(v) = self.target_dir(subject, now, &hz, &prec) else {
            return;
        };
        let (alt, az) = alt_az(v);
        let cos_alt = alt.to_radians().cos().max(0.2);
        let goal_az = az - (at_x - self.camera.width / 2.0) / (self.ppd_at(fov) * cos_alt);
        if !arrived {
            self.look = None;
            (self.camera.az, self.camera.alt, self.camera.fov) =
                glide(from, (goal_az, alt, fov), s);
        } else if (turn(self.camera.az, goal_az).abs() * cos_alt).max((alt - self.camera.alt).abs())
            / self.camera.fov
            > 0.02
        {
            // Following the sky round, slowly.
            self.look = Some(Look {
                az: goal_az,
                alt,
                fov,
                rate: 0.6,
            });
        }
    }

    /// Stops steering the view, for a look round by hand.
    pub(super) fn let_go(&mut self) {
        if let Looking::Free(free) = &mut self.looking {
            free.back = None;
            if let Some(i) = &mut free.inspect {
                i.steering = false;
            }
        }
    }

    /// Where free look keeps its close look.
    fn inspect_slot(&mut self) -> Option<&mut Option<Inspect>> {
        match &mut self.looking {
            Looking::Free(f) => Some(&mut f.inspect),
            Looking::Tour(_) => None,
        }
    }

    /// Back out to the view from before the click, and the wisp home.
    pub(crate) fn end_inspect(&mut self, real: UnixMs) {
        if let Looking::Free(free) = &mut self.looking
            && let Some(ins) = free.inspect.take()
        {
            self.backing_out = real + GLIDE_MS;
            self.look = None;
            free.back = Some(Glide {
                from: (self.camera.az, self.camera.alt, self.camera.fov),
                to: ins.before,
                started: real,
            });
        }
        self.wisp_home(real);
    }
}

/// Something clicked in free look, looked at close up.
pub(crate) struct Inspect {
    pub(crate) subject: Target,
    /// The view before the first click, azimuth, altitude and field, to go
    /// back to.
    before: (f64, f64, f64),
    /// The view at the click, and when, for the glide in.
    from: (f64, f64, f64),
    started: UnixMs,
    /// How narrow the view closes in to.
    fov: f64,
    /// Where across the screen the thing sits once there.
    at_x: f64,
    /// Whether the view is still steered to it.
    steering: bool,
    arrived: bool,
}

/// The view going back out after a close look: from where, to where and
/// since when.
#[derive(Clone, Copy)]
pub(crate) struct Glide {
    from: (f64, f64, f64),
    to: (f64, f64, f64),
    started: UnixMs,
}

impl Glide {
    fn progress(&self, real: UnixMs) -> f64 {
        ((real - self.started) as f64 / GLIDE_MS as f64).clamp(0.0, 1.0)
    }
}

/// How long a close look takes to glide in, and back out: the same for
/// the Moon as for a galaxy on the far side of the sky.
const GLIDE_MS: UnixMs = 1_800;

/// Where a glide between two views is, azimuth, altitude and field, `s`
/// of the way along. Far apart, it widens on the way, so where it was and
/// where it's going both stay in view.
fn glide(from: (f64, f64, f64), to: (f64, f64, f64), s: f64) -> (f64, f64, f64) {
    let ((az0, alt0, f0), (az1, alt1, f1)) = (from, to);
    let apart = angle_between(from_alt_az(alt0, az0), from_alt_az(alt1, az1));
    let wide = (apart * 1.2)
        .min(crate::camera::FOV_WIDEST * 0.8)
        .max(f0)
        .max(f1);
    let e = smoothstep(s);
    let ln = (1.0 - e).powi(2) * f0.ln() + 2.0 * e * (1.0 - e) * wide.ln() + e * e * f1.ln();
    (az0 + turn(az0, az1) * e, alt0 + (alt1 - alt0) * e, ln.exp())
}

/// The card's width, the room between it and the thing for the wisp, and
/// the margin at the screen's edges, when looking at something close up.
const CARD_W: f64 = 400.0;
const GAP: f64 = 100.0;
const MARGIN: f64 = 32.0;
/// How long after the click the card rises in.
const CARD_BEAT_MS: UnixMs = 250;
/// How long the wisp stays beside a card at most: as long as it's up.
const HANG_MS: UnixMs = 60 * 60_000;

/// How bright and how big, as one line: `size` in arcminutes, left out
/// under one.
fn numbers(mag: Option<f64>, size: f64) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(m) = mag {
        parts.push(format!("magnitude {m:.1}"));
    }
    if size >= 1.0 {
        parts.push(format!("about {size:.0} arcminutes across"));
    } else if size * 60.0 >= 1.0 && mag.is_some_and(|m| m < 7.0) && size > 0.0 {
        parts.push(format!("{:.0} arcseconds across", size * 60.0));
    }
    let line = parts.join(", ");
    let mut c = line.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str() + ".")
}

/// Adds where it is tonight to the kicker.
trait WithPlace {
    fn with_place(self, place: Option<String>) -> Self;
}

impl WithPlace for (String, String, Vec<String>, Option<String>) {
    fn with_place(self, place: Option<String>) -> Self {
        let (kicker, title, all, pic) = self;
        let kicker = match place {
            Some(p) => format!("{kicker} · {p}"),
            None => kicker,
        };
        (kicker, title, all, pic)
    }
}
