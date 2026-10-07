//! Free look's cards. A click on anything tells all there is: every fact
//! kept about it, where it is now, how bright and how big, and its
//! photograph when there is one. The wisp keeps out of it.

use super::*;

/// What a click in free look is about.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Subject {
    /// One of tonight's finds.
    Find(usize),
    Body(Body),
    /// A showpiece or a deep-sky object, by index.
    Piece(usize),
    /// A named star, by HR number.
    Star(u16),
    /// Something with nothing more to tell.
    Plain,
}

/// The most paragraphs a card holds, so it stays on the screen.
const MOST_PARAGRAPHS: usize = 5;

impl Game {
    /// Everything to say about a subject: kicker, title, the paragraphs, and
    /// a photograph's id if there is one.
    pub(crate) fn detail(
        &self,
        subject: Subject,
        now: UnixMs,
    ) -> Option<(String, String, String, Option<String>)> {
        let hz = horizon(self.observer, now);
        let prec = precession(now);
        let year = civil_date(now, self.offset_s).0;
        let place = |v: Vec3| {
            let (alt, az) = alt_az(v);
            westering_core::finale::whereabouts(alt, az)
        };
        let photo = |id: &str| self.photos.credit(id).map(|_| id.to_owned());
        let (kicker, title, mut paragraphs, picture) = match subject {
            Subject::Find(i) => {
                let find = &self.finds[i];
                let here = self.find_dir(i, now, &hz, &prec).map(place);
                match find.target {
                    Target::Body(b) => {
                        let mut all = vec![find.fact.clone()];
                        all.extend(self.body_facts(b));
                        (
                            self.kind_word(i).to_owned(),
                            find.name.clone(),
                            all,
                            photo(b.id()),
                        )
                            .with_place(here)
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
                        let figure = &self.sky.figures[f];
                        let mut all = vec![find.fact.clone()];
                        if let Some(note) =
                            self.sky.notes.iter().find(|n| n.abbrev == figure.abbrev)
                        {
                            all.push(note.fact.clone());
                            all.extend(note.more.iter().cloned());
                        }
                        let pic = self
                            .photos
                            .of_constellation(&figure.abbrev)
                            .map(|c| c.id.clone());
                        ("Constellation".to_owned(), figure.name.clone(), all, pic).with_place(here)
                    }
                    _ => return None,
                }
            }
            Subject::Body(b) => {
                let s = see(b, self.observer, now);
                let name = {
                    let n = b.name();
                    let mut c = n.chars();
                    c.next()
                        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                        .unwrap_or_default()
                };
                let kind = if b == Body::Moon {
                    "Our Moon"
                } else {
                    "Planet"
                };
                (kind.to_owned(), name, self.body_facts(b), photo(b.id()))
                    .with_place(Some(westering_core::finale::whereabouts(s.alt, s.az)))
            }
            Subject::Piece(p) => {
                let piece = &self.sky.lists.showpieces[p];
                let v = apply(&hz, apply(&prec, unit(piece.ra, piece.dec)));
                let (k, t, all, pic) = self.piece_detail(p);
                (k, t, all, pic).with_place(Some(place(v)))
            }
            Subject::Star(hr) => {
                let v = self
                    .sky
                    .stars
                    .index_of(hr)
                    .map(|idx| apply(&hz, self.star_dirs[idx]));
                let (k, t, all, pic) = self.star_detail(hr, year)?;
                (k, t, all, pic).with_place(v.map(place))
            }
            Subject::Plain => return None,
        };
        paragraphs.dedup();
        paragraphs.truncate(MOST_PARAGRAPHS);
        Some((kicker, title, paragraphs.join("\n\n"), picture))
    }

    fn body_facts(&self, b: Body) -> Vec<String> {
        self.sky
            .lists
            .bodies
            .iter()
            .find(|f| f.id == b.id())
            .map(|f| f.facts.clone())
            .unwrap_or_default()
    }

    fn piece_detail(&self, p: usize) -> (String, String, Vec<String>, Option<String>) {
        let piece = &self.sky.lists.showpieces[p];
        let mut all = vec![piece.fact.clone()];
        all.extend(piece.more.iter().cloned());
        // How bright and how big, when there's a brightness to give.
        let mut numbers = Vec::new();
        if piece.mag < 30.0 {
            numbers.push(format!("magnitude {:.1}", piece.mag));
        }
        if piece.size >= 1.0 {
            numbers.push(format!("about {:.0} arcminutes across", piece.size));
        }
        if !numbers.is_empty() {
            let line = numbers.join(", ");
            let mut c = line.chars();
            all.push(
                c.next()
                    .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                    .unwrap_or_default()
                    + ".",
            );
        }
        let kind = match piece.kind {
            Kind::Cluster => "Star cluster",
            Kind::Galaxy => "Galaxy",
            Kind::Nebula => "Nebula",
            Kind::Double => "Double star",
            Kind::Star => "Star",
            Kind::Dark => "Dark cloud",
            Kind::Cloud => "Star cloud",
            Kind::Asterism => "Asterism",
        };
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
            all.push(format!("Magnitude {:.1}.", star.mag));
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
        Some(("Star".to_owned(), named.name.clone(), all, pic))
    }

    /// Shows a subject's card with its left edge at `x`. A photograph the
    /// sky itself is showing isn't shown again on the card.
    fn show_detail(&mut self, subject: Subject, x: f64, real: UnixMs, in_sky: bool) -> bool {
        let now = self.sky_now(real);
        let Some((kicker, title, mut body, picture)) = self.detail(subject, now) else {
            return false;
        };
        // The photograph in the sky: what it shows that an eye wouldn't,
        // and whose it is.
        let credit = picture
            .as_deref()
            .filter(|_| in_sky)
            .and_then(|id| self.photos.credit(id))
            .cloned();
        if let Some(caption) = credit.as_ref().and_then(|c| c.caption.clone()) {
            body = format!("{body}\n\n{caption}");
        }
        self.card = Some(Card {
            footnote: credit.map(|c| format!("Photograph: {}", c.credit)),
            picture: picture.filter(|_| !in_sky),
            find: match subject {
                Subject::Find(i) => Some(i),
                _ => None,
            },
            x,
            kicker,
            title,
            body,
            // A beat after the view begins to close in.
            shown: real + CARD_BEAT_MS,
        });
        true
    }

    /// What a subject is, as a find's target.
    fn subject_target(&self, subject: Subject) -> Option<Target> {
        Some(match subject {
            Subject::Find(i) => self.finds[i].target.clone(),
            Subject::Body(b) => Target::Body(b),
            Subject::Piece(p) => Target::Showpiece(p),
            Subject::Star(hr) => Target::Star(hr),
            Subject::Plain => return None,
        })
    }

    fn subject_dir(&self, subject: Subject, now: UnixMs, hz: &Mat3, prec: &Mat3) -> Option<Vec3> {
        match subject {
            Subject::Find(i) => self.find_dir(i, now, hz, prec),
            Subject::Body(b) => {
                let s = see(b, self.observer, now);
                Some(from_alt_az(s.alt, s.az))
            }
            Subject::Piece(p) => {
                let piece = &self.sky.lists.showpieces[p];
                Some(apply(hz, apply(prec, unit(piece.ra, piece.dec))))
            }
            Subject::Star(hr) => self
                .sky
                .stars
                .index_of(hr)
                .map(|idx| apply(hz, self.star_dirs[idx])),
            Subject::Plain => None,
        }
    }

    /// The photograph the sky shows of a subject close up, if it has one.
    pub(super) fn inspect_eye(&self, subject: Subject) -> Option<Eye> {
        let in_sky = |id: &str| self.photos.credit(id).is_some_and(|c| !c.card_only);
        match subject {
            Subject::Find(i) => self.photo_id(i).map(|_| Eye::Find(i)),
            Subject::Piece(p) => in_sky(&self.sky.lists.showpieces[p].id).then_some(Eye::Piece(p)),
            Subject::Body(b) => in_sky(b.id()).then_some(Eye::Body(b)),
            Subject::Star(_) | Subject::Plain => None,
        }
    }

    /// Pixels per degree at a field of view.
    fn ppd_at(&self, fov: f64) -> f64 {
        (self.camera.width / 2.0) / (2.0 * (fov.to_radians() / 4.0).tan()) * std::f64::consts::PI
            / 180.0
    }

    /// Free look's click: the view glides in until the thing fills its
    /// side of the screen, photograph and all, with its card beside it and
    /// the wisp between the two. Esc glides back out to where it was.
    pub(crate) fn inspect(&mut self, subject: Subject, clicked_x: f64, real: UnixMs) -> bool {
        if self.inspect.as_ref().is_some_and(|i| i.subject == subject) {
            return true;
        }
        let now = self.sky_now(real);
        let (hz, prec) = (horizon(self.observer, now), precession(now));
        let Some(target) = self.subject_target(subject) else {
            return false;
        };
        if self.subject_dir(subject, now, &hz, &prec).is_none() {
            return false;
        }
        let cam = self.camera;
        let (w, h) = (cam.width, cam.height);
        let small = compact(w, h);
        let eye = self.inspect_eye(subject);
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
            .inspect
            .as_ref()
            .map_or((cam.az, cam.alt, cam.fov), |i| i.before);
        self.track = None;
        self.free_moment = None;
        self.inspect = Some(Inspect {
            subject,
            before,
            fov,
            at_x: ox,
            steering: true,
            arrived: false,
        });
        if !self.show_detail(subject, card_x, real, eye.is_some()) {
            self.inspect = None;
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

    /// Steers the view to what's being looked at: out a little first if
    /// it's far, then in, so it glides across rather than lurching. Once
    /// there it follows as the sky turns. Any look round by hand lets go.
    pub(super) fn inspect_steer(&mut self) {
        let by_hand = self.held.zoom_in
            || self.held.zoom_out
            || self.pan.0.abs() > 1e-3
            || self.pan.1.abs() > 1e-3
            || self.drag_from.is_some();
        let Some(ins) = &mut self.inspect else {
            return;
        };
        if by_hand {
            ins.steering = false;
        }
        if !ins.steering {
            return;
        }
        let (subject, final_fov, at_x, arrived) = (ins.subject, ins.fov, ins.at_x, ins.arrived);
        let now = self.sky_now(self.last_real);
        let (hz, prec) = (horizon(self.observer, now), precession(now));
        let Some(v) = self.subject_dir(subject, now, &hz, &prec) else {
            return;
        };
        let (alt, az) = alt_az(v);
        let here = from_alt_az(self.camera.alt, self.camera.az);
        let apart = angle_between(here, v);
        let fov = final_fov.max((apart * 2.4).min(crate::camera::FOV_WIDEST * 0.8));
        let cos_alt = alt.to_radians().cos().max(0.2);
        let goal_az = az - (at_x - self.camera.width / 2.0) / (self.ppd_at(fov) * cos_alt);
        let off = (turn(self.camera.az, goal_az).abs() * cos_alt)
            .max((alt - self.camera.alt).abs())
            / self.camera.fov
            + (fov / self.camera.fov).ln().abs();
        if !arrived {
            self.look = Some(Look {
                az: goal_az,
                alt,
                fov,
                rate: 1.4,
            });
            if off < 0.003
                && let Some(i) = &mut self.inspect
            {
                i.arrived = true;
            }
        } else if off > 0.02 {
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
        if let Some(i) = &mut self.inspect {
            i.steering = false;
        }
    }

    /// Back out to the view from before the click, and the wisp home.
    pub(crate) fn end_inspect(&mut self, real: UnixMs) {
        if let Some(ins) = self.inspect.take() {
            self.backing_out = real + 2_500;
            let (az, alt, fov) = ins.before;
            self.look = Some(Look {
                az,
                alt,
                fov,
                rate: 1.3,
            });
        }
        self.wisp_home(real);
    }
}

/// Something clicked in free look, looked at close up.
pub(crate) struct Inspect {
    pub(crate) subject: Subject,
    /// The view before the first click, azimuth, altitude and field, to go
    /// back to.
    before: (f64, f64, f64),
    /// How narrow the view closes in to.
    fov: f64,
    /// Where across the screen the thing sits once there.
    at_x: f64,
    /// Whether the view is still steered to it.
    steering: bool,
    arrived: bool,
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
