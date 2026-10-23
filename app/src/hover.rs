//! What's under the pointer. Moving the mouse over the sky names whatever
//! it's near, in a small line beside the pointer; a click on it tells more,
//! on a card. On the tour, one of tonight's finds not yet caught keeps its
//! secret: the click turns the view to it instead, so it can be caught. In
//! free look everything with a card glows wherever it is above the
//! horizon, however faint and however wide the view, and a click looks
//! closer.

use crate::game::{Card, Game, envelope};
use crate::view::{Glow, Text};
use westering_core::coords::{Mat3, alt_az, apply, from_alt_az, unit};
use westering_core::ephem::Body;
use westering_core::finds::Target;
use westering_core::sky::see;
use westering_core::time::{UnixMs, civil_date};

/// Something lit by the pointer's nearness, easing towards how lit it
/// should be.
pub(crate) struct GlowState {
    key: String,
    x: f64,
    y: f64,
    radius: f64,
    level: f64,
    target: f64,
}

/// Something the pointer is over.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Hovered {
    x: f64,
    y: f64,
    /// How big it is on the screen, for the ring round it.
    radius: f64,
    name: String,
    kind: String,
    /// What a click tells, if there's more to tell.
    more: Option<String>,
    /// One of tonight's finds, which a click turns to instead.
    find: Option<usize>,
    /// What a click in free look tells about, if it has anything to tell.
    subject: Option<Target>,
}

/// How close the pointer has to be, in pixels, beyond the thing's own size.
const REACH: f64 = 20.0;
/// Stars brighter than this answer to the pointer even without a name.
const NAMELESS: f32 = 4.5;
/// Stars drawn in a neighbour's figure, and the constellation they're
/// really in: nu Puppis in Carina's (the old ship Argo, drawn as one), and
/// delta and nu Ophiuchi in the Serpent's, which passes through Ophiuchus's
/// hands.
const BORROWED: [(u16, &str); 3] = [(2451, "Pup"), (6056, "Oph"), (6698, "Oph")];
/// In free look everything with more to tell glows this much, and more
/// within this many pixels of the pointer.
const GLOW_BASE: f64 = 0.3;
const GLOW_REACH: f64 = 260.0;
/// A hint fades once the pointer has been still this long.
const STILL_MS: UnixMs = 4_000;

impl Game {
    pub fn pointer_moved(&mut self, x: f64, y: f64, real: UnixMs) {
        if self
            .pointer
            .is_none_or(|(px, py, _)| (px - x).abs() + (py - y).abs() > 1.0)
        {
            self.pointer = Some((x, y, real));
            // Looking around with the pointer is being busy with the sky.
            self.last_input = real;
            self.urgent = true;
        }
    }

    pub fn pointer_left(&mut self) {
        self.pointer = None;
    }

    /// Whether hints make sense now: during the hunt, with nothing else
    /// asking for attention.
    fn hinting(&self) -> bool {
        matches!(
            self.session.phase(),
            westering_core::session::Phase::Weights
                | westering_core::session::Phase::Hunt
                | westering_core::session::Phase::Dimming
        ) && (self.card.is_none() || self.free_look())
            && self.drawing.is_none()
            && self.trail.is_none()
            && !self.placing()
    }

    /// Whether a click where the pointer is would do something, for the
    /// pointer's shape.
    pub fn clickable_at(&self, x: f64, y: f64, real: UnixMs) -> bool {
        if self.by_day() {
            return self.day_at(x, y).is_some();
        }
        if !self.hinting() {
            return false;
        }
        let now = self.sky_now(real);
        let hz = westering_core::coords::horizon(self.observer, now);
        let prec = westering_core::coords::precession(now);
        self.pick(x, y, now, &hz, &prec).is_some_and(|h| {
            if self.free_look() {
                self.has_closer_look(h.subject)
            } else {
                h.find.is_some() || h.more.is_some()
            }
        })
    }

    /// The nearest thing to a point on the screen, if anything's near.
    pub(crate) fn pick(
        &self,
        x: f64,
        y: f64,
        now: UnixMs,
        hz: &Mat3,
        prec: &Mat3,
    ) -> Option<Hovered> {
        self.near(x, y, now, hz, prec, REACH, true)
            .into_iter()
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, h)| h)
    }

    /// Whether free look has a closer look to give at something.
    fn has_closer_look(&self, subject: Option<Target>) -> bool {
        match subject {
            Some(Target::Body(_) | Target::Showpiece(_)) => true,
            Some(t @ Target::Figure(_)) => {
                self.find_of(t).is_some() || self.photos.pictures(&self.sky, &t)
            }
            // A star glows as one of tonight's finds, for its own photograph,
            // for something to tell about it, or for someone's name.
            Some(t @ Target::Star(hr)) => {
                self.find_of(t).is_some()
                    || self.photos.pictures(&self.sky, &t)
                    || self
                        .sky
                        .lists
                        .star_name(hr)
                        .is_some_and(|s| s.fact.is_some() || !s.more.is_empty())
                    || self.journal.person_on(hr).is_some()
            }
            _ => false,
        }
    }

    /// What's nearest the middle of the screen with a closer look to give,
    /// if anything's near enough.
    fn in_the_middle(&self, now: UnixMs, hz: &Mat3, prec: &Mat3) -> Option<Hovered> {
        let (cx, cy) = (self.camera.width / 2.0, self.camera.height / 2.0);
        self.near(cx, cy, now, hz, prec, self.reticle_radius(), false)
            .into_iter()
            .filter(|(_, h)| self.has_closer_look(h.subject))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, h)| h)
    }

    /// Free look: a closer look at what's in the middle. Says whether
    /// there was anything there.
    pub(crate) fn look_at_middle(&mut self, real: UnixMs) -> bool {
        let now = self.sky_now(real);
        let hz = westering_core::coords::horizon(self.observer, now);
        let prec = westering_core::coords::precession(now);
        self.in_the_middle(now, &hz, &prec)
            .is_some_and(|h| self.inspect(h.subject, h.x, real))
    }

    /// Everything within `reach` pixels of a point, beyond its own size,
    /// with how far; the stars with nothing more to tell only if `plain`.
    #[allow(clippy::too_many_arguments)]
    fn near(
        &self,
        x: f64,
        y: f64,
        now: UnixMs,
        hz: &Mat3,
        prec: &Mat3,
        reach: f64,
        plain: bool,
    ) -> Vec<(f64, Hovered)> {
        let cam = self.camera;
        let ppd = cam.px_per_degree();
        let mut all: Vec<(f64, Hovered)> = Vec::new();
        let mut consider = |sx: f64, sy: f64, radius: f64, h: Hovered| {
            let d = ((sx - x).powi(2) + (sy - y).powi(2)).sqrt() - radius;
            if d < reach {
                all.push((
                    d,
                    Hovered {
                        x: sx,
                        y: sy,
                        radius,
                        ..h
                    },
                ));
            }
        };
        let up = |v: [f64; 3]| alt_az(v).0 > 0.0;
        // Free look shows everything with a card, not only what the eye
        // could pick out at this zoom.
        let every = self.free_look();
        let year = civil_date(now, self.offset_s(now)).0;

        // Tonight's finds first: they're what the hunt is about.
        let mut taken: Vec<String> = Vec::new();
        for i in 0..self.finds.len() {
            if matches!(self.finds[i].target, Target::Meteor(_) | Target::Figure(_)) {
                continue;
            }
            let Some(v) = self.find_dir(i, now, hz, prec).filter(|v| up(*v)) else {
                continue;
            };
            let Some((sx, sy)) = cam.project(v) else {
                continue;
            };
            let f = &self.finds[i];
            taken.push(f.name.clone());
            let caught = self.finds[i].caught;
            consider(
                sx,
                sy,
                3.0,
                Hovered {
                    x: 0.0,
                    y: 0.0,
                    radius: 0.0,
                    name: f.name.clone(),
                    kind: self.kind_word(i).to_owned(),
                    more: caught.then(|| f.fact.clone()),
                    find: (!caught).then_some(i),
                    subject: Some(self.finds[i].target),
                },
            );
        }

        let limit = westering_core::sky::limiting_magnitude(see(Body::Sun, self.observer, now).alt);
        // The Moon and the planets.
        for body in [
            Body::Moon,
            Body::Mercury,
            Body::Venus,
            Body::Mars,
            Body::Jupiter,
            Body::Saturn,
            Body::Uranus,
            Body::Neptune,
        ] {
            if self.finds.iter().any(|f| f.target == Target::Body(body)) {
                continue;
            }
            let s = see(body, self.observer, now);
            // Uranus and Neptune only once the view gathers enough light.
            if s.alt < 0.0
                || (!every && s.position.magnitude > limit + crate::game::gather(cam.fov) + 0.5)
            {
                continue;
            }
            if let Some((sx, sy)) = cam.project(from_alt_az(s.alt, s.az)) {
                let radius = s.position.diameter / 7200.0 * ppd;
                let more = self
                    .sky
                    .lists
                    .bodies
                    .iter()
                    .find(|b| b.id == body.id())
                    .and_then(|b| b.facts.first().cloned());
                consider(
                    sx,
                    sy,
                    radius,
                    Hovered {
                        x: 0.0,
                        y: 0.0,
                        radius: 0.0,
                        name: {
                            // "the Moon" as a title reads "The Moon".
                            let n = body.name();
                            let mut c = n.chars();
                            c.next()
                                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                                .unwrap_or_default()
                        },
                        kind: Target::Body(body)
                            .kind_word(&self.sky.lists.showpieces)
                            .to_owned(),
                        more,
                        find: None,
                        subject: Some(Target::Body(body)),
                    },
                );
            }
        }

        // Any star bright enough to pick out: which figure it belongs to.
        let named: Vec<u16> = self.sky.lists.stars.iter().map(|s| s.hr).collect();
        for (k, star) in self.sky.stars.stars.iter().enumerate() {
            if !plain || star.mag > NAMELESS {
                break;
            }
            if named.contains(&star.hr) {
                continue;
            }
            let v = apply(hz, self.star_dirs[k]);
            if !up(v) {
                continue;
            }
            let Some((sx, sy)) = cam.project(v) else {
                continue;
            };
            if (sx - x).abs() > reach + 20.0 || (sy - y).abs() > reach + 20.0 {
                continue;
            }
            // A few figures borrow stars from their neighbours; those stars
            // are named for the constellation they're really in.
            let home = BORROWED
                .iter()
                .find(|(hr, _)| *hr == star.hr)
                .map(|(_, abbrev)| *abbrev);
            let figure = self
                .sky
                .figures
                .iter()
                .find(|f| match home {
                    Some(abbrev) => f.abbrev == abbrev,
                    None => f.edges.iter().any(|&(a, b)| a == star.hr || b == star.hr),
                })
                .map(|f| f.name.split(',').next().unwrap_or(&f.name).to_owned());
            let brightness = match star.mag {
                m if m < 1.5 => "One of the brightest stars",
                m if m < 3.0 => "A bright star",
                _ => "A star you can see from a town",
            };
            consider(
                sx,
                sy,
                1.5,
                Hovered {
                    x: 0.0,
                    y: 0.0,
                    radius: 0.0,
                    name: match &figure {
                        Some(f) => format!("A star in {f}"),
                        None => "A star".to_owned(),
                    },
                    kind: brightness.to_owned(),
                    more: None,
                    find: None,
                    subject: None,
                },
            );
        }

        // Named stars.
        for s in &self.sky.lists.stars {
            if taken.iter().any(|n| n == &s.name) {
                continue;
            }
            let Some(idx) = self.sky.stars.index_of(s.hr) else {
                continue;
            };
            let v = apply(hz, self.star_dirs[idx]);
            if !up(v) {
                continue;
            }
            if let Some((sx, sy)) = cam.project(v) {
                consider(
                    sx,
                    sy,
                    2.0,
                    Hovered {
                        x: 0.0,
                        y: 0.0,
                        radius: 0.0,
                        name: s.name.clone(),
                        kind: Target::Star(s.hr)
                            .kind_word(&self.sky.lists.showpieces)
                            .to_owned(),
                        more: s.describe(year),
                        find: None,
                        subject: Some(Target::Star(s.hr)),
                    },
                );
            }
        }

        // Clusters, galaxies and nebulae bright enough to show at this zoom.
        let reach = limit + crate::game::gather(cam.fov) + 0.5;
        for (pi, p) in self.sky.lists.showpieces.iter().enumerate() {
            if (!every && p.mag > reach) || taken.iter().any(|n| n == &p.name) {
                continue;
            }
            let v = apply(hz, apply(prec, unit(p.ra, p.dec)));
            if !up(v) {
                continue;
            }
            if let Some((sx, sy)) = cam.project(v) {
                let radius = (p.size / 120.0 * ppd).min(60.0);
                consider(
                    sx,
                    sy,
                    radius,
                    Hovered {
                        x: 0.0,
                        y: 0.0,
                        radius: 0.0,
                        name: p.name.clone(),
                        kind: Target::Showpiece(pi)
                            .kind_word(&self.sky.lists.showpieces)
                            .to_owned(),
                        more: Some(p.fact.clone()),
                        find: None,
                        subject: Some(Target::Showpiece(pi)),
                    },
                );
            }
        }

        // Tonight's weights, and stars carrying someone's name.
        for w in &self.page.weights {
            let v = apply(hz, unit(w.ra, w.dec));
            if let (Some((sx, sy)), Some(weight)) = (cam.project(v), self.journal.weight(w.weight))
            {
                consider(
                    sx,
                    sy,
                    3.0,
                    Hovered {
                        x: 0.0,
                        y: 0.0,
                        radius: 0.0,
                        name: weight.text.clone(),
                        kind: "Set down tonight".into(),
                        more: Some(
                            "Something you set down tonight. At the end of the visit you'll watch it set in the west."
                                .into(),
                        ),
                        find: None,
                        subject: None,
                    },
                );
            }
        }
        for person in &self.journal.people {
            for &hr in &person.stars {
                let Some(idx) = self.sky.stars.index_of(hr) else {
                    continue;
                };
                let v = apply(hz, self.star_dirs[idx]);
                if !up(v) {
                    continue;
                }
                if let Some((sx, sy)) = cam.project(v) {
                    consider(
                        sx,
                        sy,
                        2.0,
                        Hovered {
                            x: 0.0,
                            y: 0.0,
                            radius: 0.0,
                            name: format!("{}'s star", person.name),
                            kind: "A star with a name".into(),
                            more: None,
                            find: None,
                            subject: Some(Target::Star(hr)),
                        },
                    );
                }
            }
        }
        // Every constellation at its middle, in free look.
        if every {
            for (f, figure) in self.sky.figures.iter().enumerate() {
                let target = Target::Figure(f);
                if self.find_of(target).is_some() {
                    continue;
                }
                let Some(v) = self.target_dir(target, now, hz, prec).filter(|v| up(*v)) else {
                    continue;
                };
                if let Some((sx, sy)) = cam.project(v) {
                    consider(
                        sx,
                        sy,
                        8.0,
                        Hovered {
                            x: 0.0,
                            y: 0.0,
                            radius: 0.0,
                            name: figure.name.clone(),
                            kind: target.kind_word(&self.sky.lists.showpieces).to_owned(),
                            more: None,
                            find: None,
                            subject: Some(target),
                        },
                    );
                }
            }
        }
        // Tonight's constellations, at their middles.
        for i in 0..self.finds.len() {
            let Target::Figure(f) = self.finds[i].target else {
                continue;
            };
            let Some(v) = self.find_dir(i, now, hz, prec).filter(|v| up(*v)) else {
                continue;
            };
            if let Some((sx, sy)) = cam.project(v) {
                let caught = self.finds[i].caught;
                consider(
                    sx,
                    sy,
                    8.0,
                    Hovered {
                        x: 0.0,
                        y: 0.0,
                        radius: 0.0,
                        name: self.sky.figures[f].name.clone(),
                        kind: self.kind_word(i).to_owned(),
                        more: caught.then(|| self.finds[i].fact.clone()),
                        find: (!caught).then_some(i),
                        subject: Some(self.finds[i].target),
                    },
                );
            }
        }
        all
    }

    /// Looks round, on the frames the sky is drawn: what the pointer is on,
    /// and in free look everything with more to tell, glowing softly, more
    /// near the pointer and most in the middle, where Space looks.
    pub(crate) fn hover_scan(&mut self, now: UnixMs, hz: &Mat3, prec: &Mat3) {
        for g in &mut self.glowing {
            g.target = 0.0;
        }
        let pointer = self
            .pointer
            .filter(|_| self.hinting())
            .map(|(x, y, _)| (x, y));
        // Not while looking at something close up: nothing competes with it.
        if self.free_look() && self.hinting() && self.inspecting().is_none() {
            let middle = self.in_the_middle(now, hz, prec).map(|h| h.subject);
            let (cx, cy) = (self.camera.width / 2.0, self.camera.height / 2.0);
            for (_, h) in self.near(cx, cy, now, hz, prec, f64::INFINITY, false) {
                if !self.has_closer_look(h.subject) || !self.camera.on_screen(h.x, h.y, 40.0) {
                    continue;
                }
                let close = pointer.map_or(0.0, |(px, py)| {
                    let d = ((h.x - px).powi(2) + (h.y - py).powi(2)).sqrt() - h.radius;
                    (1.0 - d.max(0.0) / GLOW_REACH).clamp(0.0, 1.0).powf(1.3)
                });
                let mut target = GLOW_BASE + (1.0 - GLOW_BASE) * close;
                if middle == Some(h.subject) {
                    target = target.max(0.7);
                }
                let key = format!("{:?} {}", h.subject, h.name);
                match self.glowing.iter_mut().find(|g| g.key == key) {
                    Some(g) => {
                        (g.x, g.y, g.radius, g.target) = (h.x, h.y, h.radius, target);
                    }
                    None => self.glowing.push(GlowState {
                        key,
                        x: h.x,
                        y: h.y,
                        radius: h.radius,
                        level: 0.0,
                        target,
                    }),
                }
            }
        }
        let looking_at = self.inspecting().map(|i| i.subject);
        self.hovered = pointer
            .and_then(|(px, py)| self.pick(px, py, now, hz, prec))
            .filter(|h| h.subject.is_none() || h.subject != looking_at);
    }

    /// The glows, the ring round what the pointer is on and the words
    /// beside it, eased every frame so nothing flickers or pops.
    pub(crate) fn hover_overlay(&mut self, real: UnixMs, dt: f64) -> (Vec<Glow>, Vec<Text>) {
        let mut glows = Vec::new();
        let mut out = Vec::new();
        let (rise, fall) = (1.0 - (-dt / 0.12).exp(), 1.0 - (-dt / 0.35).exp());
        for g in &mut self.glowing {
            let k = if g.target > g.level { rise } else { fall };
            g.level += (g.target - g.level) * k;
        }
        self.glowing.retain(|g| g.target > 0.0 || g.level > 0.01);
        let shown = self.pointer.is_some() && self.hinting();
        let hovered = self.hovered.clone().filter(|_| shown);
        for g in &self.glowing {
            glows.push(Glow {
                x: g.x,
                y: g.y,
                radius: g.radius,
                strength: g.level,
                ring: 0.0,
            });
        }
        let free = self.free_look();
        let Some(h) = hovered else {
            self.ring_level += (0.0 - self.ring_level) * fall;
            return (glows, out);
        };
        // What's in the ring already has its own label.
        if h.find.is_some() && h.find == self.ring_target() {
            return (glows, out);
        }
        let since = self.pointer.map_or(real, |p| p.2);
        // Up a moment after the pointer settles, gone a while after it stops;
        // at once in free look, where pointing is the way to find things.
        let a = if free {
            envelope(real - since, 90, STILL_MS * 3, 1_200)
        } else {
            envelope(real - since - 150, 200, STILL_MS, 1_200)
        };
        let k = if a > self.ring_level { rise } else { fall };
        self.ring_level += (a - self.ring_level) * k;
        if self.ring_level > 0.004 {
            glows.push(Glow {
                x: h.x,
                y: h.y,
                radius: h.radius,
                strength: 0.0,
                ring: self.ring_level,
            });
        }
        if a <= 0.0 {
            return (glows, out);
        }
        // Beside the pointer, kept on the screen.
        let (px, py) = self.pointer.map_or((h.x, h.y), |p| (p.0, p.1));
        let (w, hgt) = (self.camera.width, self.camera.height);
        // Clear of the Tonight list on the right when it's showing.
        let right = if self.show_tonight() { w - 310.0 } else { w };
        let x = if px + 280.0 > right {
            px - 270.0
        } else {
            px + 18.0
        };
        let y = (py + 14.0).min(hgt - 60.0);
        out.push(Text::new(x, y, h.name.clone(), 14.0, 0.92 * a).bold());
        let action = match (h.find.is_some(), h.more.is_some()) {
            _ if free && self.has_closer_look(h.subject) => {
                format!("{} · click to look closer", h.kind)
            }
            _ if free => h.kind.clone(),
            (true, _) => format!("{} · on tonight's list · click to turn to it", h.kind),
            (false, true) => format!("{} · click for more", h.kind),
            (false, false) => h.kind.clone(),
        };
        out.push(Text::new(x, y + 19.0, action, 12.0, 0.6 * a).color([0.82, 0.86, 0.95]));
        (glows, out)
    }

    /// A click on something named: more about it, or a turn to one of
    /// tonight's finds. Says whether it was on something.
    pub(crate) fn click_hovered(&mut self, x: f64, y: f64, real: UnixMs) -> bool {
        if !self.hinting() {
            return false;
        }
        let now = self.sky_now(real);
        let hz = westering_core::coords::horizon(self.observer, now);
        let prec = westering_core::coords::precession(now);
        let Some(h) = self.pick(x, y, now, &hz, &prec) else {
            return false;
        };
        // Free look: a closer look, telling all there is about it. Nothing
        // here is caught or ticked off the list.
        if self.free_look() {
            return self.has_closer_look(h.subject) && self.inspect(h.subject, x, real);
        }
        if let Some(i) = h.find {
            // The tour turns to it, for the ring.
            self.turn_to(i, real);
            return true;
        }
        let Some(body) = h.more else {
            return false;
        };
        self.card = Some(Card {
            footnote: None,
            picture: None,
            find: None,
            x: self.beside_centre(),
            kicker: h.kind,
            title: h.name,
            body,
            shown: real,
        });
        // Look at it, without zooming.
        let v = self.camera.unproject(h.x, h.y);
        let (alt, az) = alt_az(v);
        self.look = Some(crate::game::Look {
            az,
            alt,
            fov: self.camera.fov,
            rate: 1.8,
        });
        if let Some(t) = self.on_tour_mut() {
            t.track = None;
        }
        true
    }
}
