//! How a find is shown once the view arrives: its card beside it, and the
//! find in the room the card leaves. A constellation, or a star with its
//! constellation, is shown whole, its stars named.

use super::*;

/// How wide a card is, and how far it keeps from the window's edge.
const CARD: f64 = 400.0;
const EDGE: f64 = 24.0;
/// Clear of the top bar and the ground.
const TOP: f64 = 90.0;
const FOOT: f64 = 60.0;

/// Where a figure is framed: how wide the view goes, and where on the
/// screen the find sits.
pub(crate) struct Framing {
    pub(crate) fov: f64,
    pub(crate) at: (f64, f64),
}

impl Game {
    /// The constellation figure a star is drawn in: its own constellation's
    /// when it's named, otherwise the first that joins it up.
    pub(crate) fn figure_of(&self, hr: u16) -> Option<usize> {
        let joins =
            |f: &westering_core::figures::Figure| f.edges.iter().any(|&(a, b)| a == hr || b == hr);
        let own = self
            .sky
            .lists
            .star_name(hr)
            .map(|s| s.constellation.as_str());
        self.sky
            .figures
            .iter()
            .position(|f| Some(f.abbrev.as_str()) == own && joins(f))
            .or_else(|| self.sky.figures.iter().position(joins))
    }

    /// The figure being looked at: a constellation caught, the one a
    /// caught star is in, or the one a star-hop is on.
    pub(crate) fn subject_figure(&self) -> Option<usize> {
        let i = self.track?;
        match self.finds[i].target {
            Target::Figure(f) if self.caught[i] => Some(f),
            Target::Star(hr) if self.caught[i] => self.figure_of(hr),
            Target::Hop(_) if !self.caught[i] => self.figure_of(self.subject_star()?),
            _ => None,
        }
    }

    /// The star being looked at: a star caught, or a star-hop's next stop.
    fn subject_star(&self) -> Option<u16> {
        let i = self.track?;
        match self.finds[i].target {
            Target::Star(hr) if self.caught[i] => Some(hr),
            Target::Hop(_) if !self.caught[i] => self.hop_stop(i)?.hr,
            _ => None,
        }
    }

    /// Names on the figure that's up: its named stars, the one being looked
    /// at clearest, and any planet among them, which isn't one of its stars.
    pub(crate) fn pattern_names(&self, now: UnixMs, hz: &Mat3, brightness: f64) -> Vec<Text> {
        let mut out = Vec::new();
        let a = self.pattern_alpha * brightness;
        let Some(edges) = self.pattern.as_ref().filter(|_| a > 0.02) else {
            return out;
        };
        let subject = self.subject_star();
        // The ring names what's in it already.
        let ringed = self.card.is_none() && self.catch.target.is_some();
        let mut hrs: Vec<u16> = edges.iter().flat_map(|&(p, q)| [p, q]).collect();
        hrs.sort_unstable();
        hrs.dedup();
        let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for hr in hrs {
            let Some(idx) = self.sky.stars.index_of(hr) else {
                continue;
            };
            let v = apply(hz, self.star_dirs[idx]);
            let Some((x, y)) = self.camera.project(v).filter(|_| alt_az(v).0 > 0.0) else {
                continue;
            };
            (x0, x1, y0, y1) = (x0.min(x), x1.max(x), y0.min(y), y1.max(y));
            let Some(named) = self.sky.lists.star_name(hr) else {
                continue;
            };
            let text = if subject == Some(hr) {
                if ringed {
                    continue;
                }
                Text::new(x + 10.0, y + 5.0, named.name.clone(), 15.0, 0.9 * a).bold()
            } else {
                Text::new(x + 9.0, y + 4.0, named.name.clone(), 12.5, 0.55 * a)
            };
            out.push(text);
        }
        for body in Body::PLANETS {
            let s = see(body, self.observer, now);
            if s.alt < 0.0 || s.position.magnitude > 6.0 {
                continue;
            }
            if let Some((x, y)) = self.camera.project(from_alt_az(s.alt, s.az))
                && (x0 - 20.0..x1 + 20.0).contains(&x)
                && (y0 - 20.0..y1 + 20.0).contains(&y)
            {
                out.push(Text::new(
                    x + 10.0,
                    y + 5.0,
                    format!("{}, a planet passing through", body.name()),
                    12.5,
                    0.6 * a,
                ));
            }
        }
        out
    }

    /// The figure a card is about, drawn as it sits now: a constellation,
    /// or the one a star is in when there's no photograph of the star.
    pub(crate) fn card_chart(&self) -> Option<crate::view::Chart> {
        let card = self.card.as_ref()?;
        let target = match (card.find, self.inspect.as_ref().map(|i| i.subject)) {
            (Some(i), _) => self.finds[i].target.clone(),
            (None, Some(Subject::Star(hr))) => Target::Star(hr),
            _ => return None,
        };
        let (f, mark) = match target {
            Target::Figure(f) => (f, None),
            Target::Star(hr) if self.star_picture(hr).is_none() => (self.figure_of(hr)?, Some(hr)),
            _ => return None,
        };
        let plane = self.figure_plane(f);
        let mag = |hr: u16| self.sky.stars.get(hr).map_or(6.0, |s| s.mag as f64);
        // The brightest few with names are named, and the marked one.
        let mut named: Vec<u16> = plane
            .iter()
            .map(|p| p.0)
            .filter(|&hr| self.sky.lists.star_name(hr).is_some())
            .collect();
        named.sort_by(|a, b| mag(*a).total_cmp(&mag(*b)));
        named.truncate(4);
        let stars = plane
            .iter()
            .map(|&(hr, x, y)| crate::view::ChartStar {
                x,
                y,
                mag: mag(hr),
                name: (named.contains(&hr) || mark == Some(hr))
                    .then(|| self.sky.lists.star_name(hr).map(|s| s.name.clone()))
                    .flatten(),
                marked: mark == Some(hr),
            })
            .collect();
        let place = |hr: u16| plane.iter().position(|p| p.0 == hr);
        let lines = self.sky.figures[f]
            .edges
            .iter()
            .filter_map(|&(a, b)| Some((place(a)?, place(b)?)))
            .collect();
        Some(crate::view::Chart { stars, lines })
    }

    /// A figure's stars as they sit on the screen with the view centred on
    /// it, in units of the projection's scale: x right, y down. The centre
    /// is at the origin.
    pub(crate) fn figure_plane(&self, f: usize) -> Vec<(u16, f64, f64)> {
        let now = self.sky_now(self.last_real);
        let (hz, prec) = (horizon(self.observer, now), precession(now));
        let figure = &self.sky.figures[f];
        let Some((centre, _)) = figure.centre(&self.sky.stars) else {
            return Vec::new();
        };
        let (alt, az) = alt_az(apply(&hz, apply(&prec, centre)));
        let (w, h) = (self.camera.width, self.camera.height);
        let mut cam = Camera::new(az, alt, 60.0);
        cam.set_size(w, h);
        let k = cam.px_per_degree().to_degrees();
        figure
            .stars()
            .into_iter()
            .filter_map(|hr| {
                let idx = self.sky.stars.index_of(hr)?;
                let (x, y) = cam.project(apply(&hz, self.star_dirs[idx]))?;
                Some((hr, (x - w / 2.0) / k, (y - h / 2.0) / k))
            })
            .collect()
    }

    /// A constellation find framed whole in the room beside its card; a
    /// star find in its constellation, where it sits in it.
    pub(crate) fn framing(&self, i: usize) -> Option<Framing> {
        let (f, star) = match self.finds[i].target {
            Target::Figure(f) => (f, None),
            Target::Star(hr) => (self.figure_of(hr)?, Some(hr)),
            _ => return None,
        };
        let plane = self.figure_plane(f);
        let (x0, x1, y0, y1) = plane.iter().fold(
            (f64::MAX, f64::MIN, f64::MAX, f64::MIN),
            |(x0, x1, y0, y1), &(_, x, y)| (x0.min(x), x1.max(x), y0.min(y), y1.max(y)),
        );
        if x0 > x1 {
            return None;
        }
        let (w, h) = (self.camera.width, self.camera.height);
        let left = if compact(w, h) {
            EDGE
        } else {
            EDGE + CARD + 40.0
        };
        let (right, top, bottom) = (w - 32.0, TOP, h - FOOT);
        // A little short of the room: the projection stretches away from
        // the middle.
        let k =
            ((right - left) / (x1 - x0).max(1e-4)).min((bottom - top) / (y1 - y0).max(1e-4)) * 0.88;
        let fov = (4.0 * ((w / 2.0) / (2.0 * k)).atan())
            .to_degrees()
            .clamp(crate::camera::FOV_NARROWEST, 100.0);
        let k = (w / 2.0) / (2.0 * (fov.to_radians() / 4.0).tan());
        let (sx, sy) = match star {
            Some(hr) => plane.iter().find(|p| p.0 == hr).map(|p| (p.1, p.2))?,
            None => (0.0, 0.0),
        };
        let at = (
            (left + right) / 2.0 + k * (sx - (x0 + x1) / 2.0),
            (top + bottom) / 2.0 + k * (sy - (y0 + y1) / 2.0),
        );
        Some(Framing { fov, at })
    }

    /// Where find `i`'s card goes, and where the find sits while it's up,
    /// if not in the middle: with room, the card goes to its right;
    /// otherwise the card goes to the left and the find to the middle of
    /// the room left over.
    pub(super) fn card_place(&self, i: usize) -> (f64, Option<(f64, f64)>) {
        let (w, h) = (self.camera.width, self.camera.height);
        if let Some(framing) = self.framing(i) {
            return (EDGE, (!compact(w, h)).then_some(framing.at));
        }
        let clear = self
            .reticle_radius()
            .max(self.apparent_radius(i, self.zoom_for(i)))
            + 34.0;
        if compact(w, h) || w / 2.0 + clear + CARD <= w - EDGE {
            return (w / 2.0 + clear, None);
        }
        (EDGE, Some(((EDGE + CARD + 34.0 + w - EDGE) / 2.0, h / 2.0)))
    }

    /// Where the find whose card is up is held on the screen, if it's
    /// held anywhere but the middle.
    pub(crate) fn framed_at(&self) -> Option<(f64, f64)> {
        if self.inspect.is_some() || self.tour.is_some() {
            return None;
        }
        let i = self.card.as_ref()?.find?;
        if self.track != Some(i) || !self.caught[i] {
            return None;
        }
        self.card_place(i).1
    }

    /// Keeps the find in the room its card leaves, after the view has
    /// turned to follow it.
    pub(super) fn hold_beside_card(&mut self) {
        let Some((x, y)) = self.framed_at() else {
            return;
        };
        let (w, h) = (self.camera.width, self.camera.height);
        let ppd = |fov: f64| (w / 2.0) / (2.0 * (fov.to_radians() / 4.0).tan()) * PI / 180.0;
        let (dx, dy) = (x - w / 2.0, y - h / 2.0);
        let (az, alt, fov) = match &mut self.look {
            Some(look) => (&mut look.az, &mut look.alt, look.fov),
            None => (&mut self.camera.az, &mut self.camera.alt, self.camera.fov),
        };
        let p = ppd(fov);
        *az -= dx / (p * alt.to_radians().cos().max(0.2));
        *alt += dy / p;
    }
}

use std::f64::consts::PI;
