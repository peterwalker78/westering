//! The finds that take a few minutes: tonight's story, told a card at a time
//! while the view moves round the stars in it; a star-hop, found one step
//! at a time; and a walk across the Moon's lit face, with the wisp flying
//! from place to place.

use crate::game::{Card, Game, Look, dot3};
use crate::guide::Aim;
use westering_core::coords::{Mat3, Vec3, alt_az, angles, apply, from_alt_az, unit};
use westering_core::ephem::Body;
use westering_core::finds::Target;
use westering_core::sky::see;
use westering_core::time::UnixMs;
use westering_core::tours::Stop;

/// What a card of a tour looks at.
#[derive(Clone, Debug)]
pub(crate) enum Focus {
    Nothing,
    Sky(Stop),
    /// A place on the Moon, by index into the features.
    Moon(usize),
}

pub(crate) struct Page {
    kicker: String,
    title: String,
    body: String,
    focus: Focus,
}

pub(crate) struct Tour {
    pub(crate) find: usize,
    pages: Vec<Page>,
    pub(crate) at: usize,
}

impl Tour {
    pub(crate) fn focus(&self) -> &Focus {
        &self.pages[self.at].focus
    }

    pub(crate) fn on_last_page(&self) -> bool {
        self.at + 1 == self.pages.len()
    }
}

/// Moon walks are drawn this share of the view's height across.
const MOON_FILL: f64 = 0.62;

impl Game {
    /// Where a hop step or a story line points, as a horizon vector.
    pub(crate) fn stop_dir(&self, stop: &Stop, hz: &Mat3, prec: &Mat3) -> Option<Vec3> {
        match (stop.hr, &stop.showpiece) {
            (Some(hr), _) => self
                .sky
                .stars
                .index_of(hr)
                .map(|idx| apply(hz, self.star_dirs[idx])),
            (None, Some(id)) => self
                .sky
                .lists
                .showpieces
                .iter()
                .find(|p| &p.id == id)
                .map(|p| apply(hz, apply(prec, unit(p.ra, p.dec)))),
            _ => None,
        }
    }

    /// A stop's place on the sky, for the wisp to fly to.
    fn stop_aim(&self, stop: &Stop) -> Option<Aim> {
        let dir = match (stop.hr, &stop.showpiece) {
            (Some(hr), _) => self.sky.stars.get(hr)?.dir,
            (None, Some(id)) => {
                let p = self.sky.lists.showpieces.iter().find(|p| &p.id == id)?;
                unit(p.ra, p.dec)
            }
            _ => return None,
        };
        let (ra, dec) = angles(dir);
        Some(Aim::Sky(ra, dec))
    }

    /// The Moon's find on tonight's list.
    pub(crate) fn moon_find(&self) -> Option<usize> {
        self.finds
            .iter()
            .position(|f| f.target == Target::Body(Body::Moon))
    }

    /// The field of view that makes the Moon fill most of the height.
    pub(crate) fn moon_walk_fov(&self) -> f64 {
        let d = see(Body::Moon, self.observer, self.clock.sky(self.last_real))
            .position
            .diameter
            / 3600.0;
        let share = MOON_FILL * self.camera.height / self.camera.width;
        (d / share).clamp(crate::camera::FOV_NARROWEST, 2.4)
    }

    /// Where a place on the Moon is on the screen, and the size of the
    /// Moon's disc there.
    pub(crate) fn moon_spot(&self, feature: usize) -> Option<(f64, f64, f64)> {
        let now = self.sky_now(self.last_real);
        let seen = see(Body::Moon, self.observer, now);
        let v = from_alt_az(seen.alt, seen.az);
        let cam = self.camera;
        let (x, y) = cam.project(v)?;
        // Which way celestial north points on the screen.
        let hz = westering_core::coords::horizon(self.observer, now);
        let pole = apply(&hz, [0.0, 0.0, 1.0]);
        let along = dot3(pole, v);
        let t = [
            pole[0] - along * v[0],
            pole[1] - along * v[1],
            pole[2] - along * v[2],
        ];
        let n = dot3(t, t).sqrt().max(1e-9);
        let nudge = [
            v[0] + t[0] / n * 1e-5,
            v[1] + t[1] / n * 1e-5,
            v[2] + t[2] / n * 1e-5,
        ];
        let (nx, ny) = cam
            .project(nudge)
            .map_or((0.0, -1.0), |(a, b)| (a - x, b - y));
        let north = nx.atan2(-ny);
        let radius = seen.position.diameter / 7200.0 * cam.px_per_degree();
        let (fx, fy) = westering_core::tours::on_disc(&self.sky.tours.moon[feature]);
        // On a north-up picture Crisium is to the right; turn with the sky.
        let (px, py) = (fx * radius, -fy * radius);
        let (s, c) = north.sin_cos();
        Some((x + px * c - py * s, y + px * s + py * c, radius))
    }

    /// Whether a find can be caught yet: the Moon walk waits for the Moon.
    pub(crate) fn catchable(&self, i: usize) -> bool {
        match self.finds[i].target {
            Target::MoonWalk => self.moon_find().is_none_or(|m| self.caught[m]),
            _ => true,
        }
    }

    /// How far into a hop someone has got, for the find's direction.
    pub(crate) fn hop_stop(&self, i: usize) -> Option<&Stop> {
        let Target::Hop(h) = self.finds[i].target else {
            return None;
        };
        let steps = &self.sky.tours.hops[h].steps;
        steps.get(self.steps[i].min(steps.len() - 1))
    }

    /// A step of a hop found: on to the next, or the hop is done.
    pub(crate) fn hop_step(&mut self, i: usize, real: UnixMs) -> bool {
        let Target::Hop(h) = self.finds[i].target else {
            return false;
        };
        let len = self.sky.tours.hops[h].steps.len();
        if self.steps[i] + 1 >= len {
            return false;
        }
        self.steps[i] += 1;
        self.space_caught = true;
        self.catch.progress = 0.0;
        self.catch.target = None;
        self.catch.holding = false;
        self.catch.fov_before = None;
        self.guide.last_progress = real;
        let line = self.sky.tours.hops[h].steps[self.steps[i]].say.clone();
        let step = format!("{} of {}. {line}", self.steps[i] + 1, len);
        self.say_at(Aim::Ring, step, real, 14_000);
        true
    }

    /// Starts a find that's told a page at a time, if it is one.
    pub(crate) fn begin_tour(&mut self, i: usize, real: UnixMs) -> bool {
        let pages = match self.finds[i].target {
            Target::Story(s) => {
                let story = &self.sky.tours.stories[s];
                let mut pages: Vec<Page> = story
                    .lines
                    .iter()
                    .map(|l| Page {
                        kicker: "Tonight's story".into(),
                        title: story.title.clone(),
                        body: l.say.clone(),
                        focus: if l.hr.is_some() || l.showpiece.is_some() {
                            Focus::Sky(l.clone())
                        } else {
                            Focus::Nothing
                        },
                    })
                    .collect();
                pages.push(Page {
                    kicker: "Tonight's story".into(),
                    title: "And closer to home".into(),
                    body: story.mirror.clone(),
                    focus: Focus::Nothing,
                });
                pages
            }
            Target::MoonWalk => {
                let stops = self.moon_stops.clone();
                let n = stops.len();
                let mut pages = vec![Page {
                    kicker: "A walk on the Moon".into(),
                    title: "The edge of night".into(),
                    body: self.sky.tours.moon_intro.clone(),
                    focus: Focus::Nothing,
                }];
                for (k, &f) in stops.iter().enumerate() {
                    let feature = &self.sky.tours.moon[f];
                    pages.push(Page {
                        kicker: format!("On the Moon · {} of {n}", k + 1),
                        title: feature.name.clone(),
                        body: feature.fact.clone(),
                        focus: Focus::Moon(f),
                    });
                }
                pages.push(Page {
                    kicker: "A walk on the Moon".into(),
                    title: "Back home".into(),
                    body: self.sky.tours.moon_closing.clone(),
                    focus: Focus::Nothing,
                });
                pages
            }
            _ => return false,
        };
        if matches!(self.finds[i].target, Target::Story(_)) {
            // The story moves the view about; don't hold it on one star.
            self.track = None;
        }
        self.tour = Some(Tour {
            find: i,
            pages,
            at: 0,
        });
        self.show_page(real);
        true
    }

    fn show_page(&mut self, real: UnixMs) {
        let Some(tour) = &self.tour else {
            return;
        };
        let i = tour.find;
        let page = &tour.pages[tour.at];
        let (kicker, title, body, focus) = (
            page.kicker.clone(),
            page.title.clone(),
            page.body.clone(),
            page.focus.clone(),
        );
        let walk = matches!(self.finds[i].target, Target::MoonWalk);
        let x = if walk {
            self.beside_moon()
        } else {
            self.beside_centre()
        };
        self.card = Some(Card {
            footnote: None,
            picture: None,
            find: None,
            x,
            kicker,
            title,
            body,
            shown: real,
        });
        match focus {
            Focus::Sky(stop) => {
                let now = self.sky_now(real);
                let hz = westering_core::coords::horizon(self.observer, now);
                let prec = westering_core::coords::precession(now);
                if let Some(v) = self.stop_dir(&stop, &hz, &prec) {
                    let (alt, az) = alt_az(v);
                    if alt > 2.0 {
                        self.look = Some(Look {
                            az,
                            alt,
                            fov: self.camera.fov.clamp(40.0, 70.0),
                            rate: 1.1,
                        });
                        if let Some(aim) = self.stop_aim(&stop) {
                            self.hush();
                            self.fly(aim, real, 9_000);
                        }
                    }
                }
            }
            Focus::Moon(f) => {
                self.hush();
                self.fly(Aim::Moon(f), real, 60_000);
            }
            Focus::Nothing => self.fly(Aim::Card, real, 3_000),
        }
    }

    /// Where the card goes during a Moon walk: clear of the disc.
    fn beside_moon(&self) -> f64 {
        let r = MOON_FILL * self.camera.height / 2.0;
        let (w, cx) = (self.camera.width, self.camera.width / 2.0);
        let right = cx + r + 30.0;
        if right + 400.0 <= w - 10.0 {
            right
        } else {
            (w - 410.0).max(10.0)
        }
    }

    /// On to the next page, if there is one.
    pub(crate) fn turn_page(&mut self, real: UnixMs) -> bool {
        let Some(tour) = &mut self.tour else {
            return false;
        };
        if tour.at + 1 < tour.pages.len() {
            tour.at += 1;
            self.show_page(real);
            return true;
        }
        self.end_tour(real);
        false
    }

    /// Leaves a tour, where it has got to.
    pub(crate) fn end_tour(&mut self, real: UnixMs) {
        if let Some(tour) = self.tour.take() {
            // The story's over: the ring waits until the view moves on,
            // rather than coming back over what it left on screen.
            self.ring_resting = true;
            if let Target::MoonWalk = self.finds[tour.find].target {
                let visited: Vec<String> = tour
                    .pages
                    .iter()
                    .take(tour.at + 1)
                    .filter_map(|p| match p.focus {
                        Focus::Moon(f) => Some(format!("moon:{}", self.sky.tours.moon[f].name)),
                        _ => None,
                    })
                    .collect();
                for id in visited {
                    if let Err(e) = self.journal.mark_found(&id, &self.night) {
                        eprintln!("westering: couldn't save what was found: {e}");
                    }
                }
                // Back out to see the Moon in its sky.
                self.catch.fov_before = Some(30.0);
            }
            self.after_catch(tour.find, real);
        }
    }

    /// The trail of a hop so far, and a ring round the place on the Moon
    /// being talked about.
    pub(crate) fn draw_tours(&mut self, real: UnixMs, hz: &Mat3, prec: &Mat3) {
        let cam = self.camera;
        for i in 0..self.finds.len() {
            let Target::Hop(h) = self.finds[i].target else {
                continue;
            };
            let reached = if self.caught[i] {
                self.sky.tours.hops[h].steps.len()
            } else {
                self.steps[i] + 1
            };
            let points: Vec<Option<(f64, f64)>> = self.sky.tours.hops[h].steps[..reached]
                .iter()
                .map(|s| self.stop_dir(s, hz, prec).and_then(|v| cam.project(v)))
                .collect();
            let alpha = if self.caught[i] { 0.12 } else { 0.3 };
            for pair in points.windows(2) {
                if let [Some(a), Some(b)] = pair {
                    // Stop short of the stars themselves.
                    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                    let l = (dx * dx + dy * dy).sqrt();
                    if l < 20.0 {
                        continue;
                    }
                    let k = 9.0 / l;
                    self.field.line(
                        (a.0 + dx * k, a.1 + dy * k),
                        (b.0 - dx * k, b.1 - dy * k),
                        crate::game::WARM,
                        alpha,
                    );
                }
            }
        }
        if let Some(Focus::Moon(f)) = self.tour.as_ref().map(|t| t.focus().clone())
            && let Some((x, y, radius)) = self.moon_spot(f)
        {
            let km = self.sky.tours.moon[f].diameter.unwrap_or(40.0);
            let r = (km / 3474.8 * radius).clamp(9.0, radius * 0.5) + 6.0;
            let t = real as f64 / 1000.0;
            for k in 0..28 {
                let a = k as f64 / 28.0 * std::f64::consts::TAU + t * 0.25;
                self.marks.push(crate::view::Point {
                    x: x + r * a.cos(),
                    y: y + r * a.sin(),
                    radius: 1.2,
                    color: [1.0, 0.82, 0.55],
                    alpha: 0.55,
                    halo: 0.3,
                });
            }
        }
    }
}
