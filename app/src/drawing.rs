//! Drawing your own constellations, and the marks the user leaves on the
//! sky: weights hanging in the west, stars given a person's name, and the
//! shapes drawn on earlier nights.

use crate::camera::Camera;
use crate::field::Rgb;
use crate::game::{Game, Look, Timed, hills};
use crate::talk::{Flow, Pending, Prompt};
use crate::view::{Line, Text};
use gtk::gdk;
use westering_core::coords::{Mat3, Vec3, alt_az, apply, unit};
use westering_core::finds::Target;
use westering_core::journal::Drawing as Drawn;
use westering_core::session::Phase;
use westering_core::time::UnixMs;

/// The constellation being drawn.
pub struct Drawing {
    pub edges: Vec<(u16, u16)>,
    pub current: Option<u16>,
    pub anchor: Option<u16>,
}

const DRAWABLE: f32 = 4.8;
const WEIGHT: Rgb = [1.0, 0.62, 0.36];
const DRAWN: Rgb = [1.0, 0.86, 0.66];
const FIGURE: Rgb = [0.66, 0.78, 1.0];
const NAMED: Rgb = [1.0, 0.82, 0.6];
/// How long a revealed constellation stays up after a catch or a drawing.
const REVEAL_MS: UnixMs = 14_000;

impl Game {
    fn star_screen(&self, hr: u16, hz: &Mat3) -> Option<(f64, f64)> {
        let idx = self.sky.stars.index_of(hr)?;
        let v = apply(hz, self.star_dirs[idx]);
        if alt_az(v).0 < 0.0 {
            return None;
        }
        self.camera.project(v)
    }

    fn star_dir(&self, hr: u16, hz: &Mat3) -> Option<Vec3> {
        let idx = self.sky.stars.index_of(hr)?;
        Some(apply(hz, self.star_dirs[idx]))
    }

    /// Bright stars on screen, with where they are.
    fn drawable(&self, hz: &Mat3) -> Vec<(u16, f64, f64)> {
        let cam = &self.camera;
        self.sky
            .stars
            .stars
            .iter()
            .take_while(|s| s.mag <= DRAWABLE)
            .filter_map(|s| {
                let (x, y) = self.star_screen(s.hr, hz)?;
                cam.on_screen(x, y, -20.0).then_some((s.hr, x, y))
            })
            .collect()
    }

    pub(crate) fn start_drawing(&mut self, real: UnixMs) {
        let now = self.clock.sky(real);
        let hz = westering_core::coords::horizon(self.observer, now);
        let (cx, cy) = (self.camera.width / 2.0, self.camera.height / 2.0);
        let reach = self.camera.width.min(self.camera.height) * 0.3;
        let nearest = self
            .drawable(&hz)
            .into_iter()
            .map(|(hr, x, y)| (hr, ((x - cx).powi(2) + (y - cy).powi(2)).sqrt()))
            .filter(|(_, d)| *d < reach)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let Some((hr, _)) = nearest else {
            self.hint = Some(Timed {
                text: "Look somewhere with a few bright stars, then press C".into(),
                shown: real,
                hold: 4_000,
            });
            return;
        };
        self.drawing = Some(Drawing {
            edges: Vec::new(),
            current: Some(hr),
            anchor: None,
        });
        self.guide_drawing(real);
    }

    /// Keys while drawing. Returns false for keys it doesn't use.
    pub(crate) fn drawing_key(&mut self, key: gdk::Key, real: UnixMs) -> bool {
        match key {
            gdk::Key::Left | gdk::Key::h => self.step((-1.0, 0.0), real),
            gdk::Key::Right | gdk::Key::l => self.step((1.0, 0.0), real),
            gdk::Key::Up | gdk::Key::k => self.step((0.0, -1.0), real),
            gdk::Key::Down | gdk::Key::j => self.step((0.0, 1.0), real),
            gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::space => self.join(),
            gdk::Key::BackSpace => self.undo(),
            gdk::Key::c | gdk::Key::C | gdk::Key::Escape => self.finish_drawing(real),
            _ => return false,
        }
        true
    }

    fn step(&mut self, dir: (f64, f64), real: UnixMs) {
        let now = self.clock.sky(real);
        let hz = westering_core::coords::horizon(self.observer, now);
        let Some(current) = self.drawing.as_ref().and_then(|d| d.current) else {
            return;
        };
        let Some((x0, y0)) = self.star_screen(current, &hz) else {
            return;
        };
        let best = self
            .drawable(&hz)
            .into_iter()
            .filter(|(hr, _, _)| *hr != current)
            .filter_map(|(hr, x, y)| {
                let (dx, dy) = (x - x0, y - y0);
                let dist = (dx * dx + dy * dy).sqrt();
                if dist < 4.0 {
                    return None;
                }
                let along = (dx * dir.0 + dy * dir.1) / dist;
                (along > 0.5).then_some((hr, dist * (2.0 - along)))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((hr, _)) = best {
            if let Some(d) = &mut self.drawing {
                d.current = Some(hr);
            }
            // Keep the chosen star comfortably in view.
            if let Some((x, y)) = self.star_screen(hr, &hz) {
                let (w, h) = (self.camera.width, self.camera.height);
                if x < w * 0.2 || x > w * 0.8 || y < h * 0.2 || y > h * 0.8 {
                    let (alt, az) = alt_az(self.star_dir(hr, &hz).expect("on screen"));
                    self.look = Some(Look {
                        az,
                        alt,
                        fov: self.camera.fov,
                        rate: 1.5,
                    });
                }
            }
        }
    }

    fn join(&mut self) {
        let Some(d) = &mut self.drawing else { return };
        let Some(current) = d.current else { return };
        match d.anchor {
            Some(a) if a != current => {
                if !d
                    .edges
                    .iter()
                    .any(|&(x, y)| (x, y) == (a, current) || (y, x) == (a, current))
                {
                    d.edges.push((a, current));
                }
                d.anchor = Some(current);
            }
            _ => d.anchor = Some(current),
        }
    }

    fn undo(&mut self) {
        let Some(d) = &mut self.drawing else { return };
        if d.edges.pop().is_some() {
            d.anchor = d.edges.last().map(|e| e.1);
        } else {
            d.anchor = None;
        }
    }

    fn finish_drawing(&mut self, real: UnixMs) {
        let empty = self.drawing.as_ref().is_none_or(|d| d.edges.is_empty());
        self.hush();
        if empty {
            self.drawing = None;
            return;
        }
        if let Some(d) = &mut self.drawing {
            d.current = None;
            d.anchor = None;
        }
        let _ = real;
        self.talk.flow = Some(Flow::DrawingName);
        self.set_prompt(Some(Prompt {
            text: "Name it.".into(),
            placeholder: "Anything you like".into(),
            chips: Vec::new(),
            entry: true,
            hint: "Enter to keep it · Esc to let it go".into(),
            names: false,
            chip_keys: Vec::new(),
        }));
    }

    pub(crate) fn name_drawing(&mut self, name: &str, real: UnixMs) {
        let Some(drawing) = self.drawing.take() else {
            return;
        };
        let name = name.trim();
        if name.is_empty() {
            self.talk.flow = None;
            self.set_prompt(None);
            return;
        }
        let mut mine: Vec<u16> = drawing.edges.iter().flat_map(|&(a, b)| [a, b]).collect();
        mine.sort_unstable();
        mine.dedup();
        let best = self
            .sky
            .figures
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let stars = f.stars();
                (
                    i,
                    mine.iter().filter(|s| stars.contains(s)).count(),
                    stars.len(),
                )
            })
            .max_by_key(|&(_, overlap, _)| overlap)
            .filter(|&(_, overlap, _)| overlap >= 2 && overlap * 2 >= mine.len());
        let reveals = best.map(|(i, _, _)| self.sky.figures[i].name.clone());
        self.journal.drawings.push(Drawn {
            name: name.to_owned(),
            edges: drawing.edges.clone(),
            night: self.night.clone(),
            reveals: reveals.clone(),
        });
        if let Err(e) = self.journal.save_drawings() {
            eprintln!("westering: couldn't save the drawing: {e}");
        }
        self.page.drawings.push(match &reveals {
            Some(f) => format!("{name}, part of {f}"),
            None => name.to_owned(),
        });
        self.save_page_now();
        self.talk.flow = None;
        self.set_prompt(None);
        let body = match best {
            Some((i, overlap, total)) if overlap == total => {
                self.reveal = Some((i, real));
                format!(
                    "Your {name} is {}, near enough. People have drawn those stars together for thousands of years.",
                    self.sky.figures[i].name
                )
            }
            Some((i, _, _)) => {
                self.reveal = Some((i, real));
                format!("Your {name} is part of {}.", self.sky.figures[i].name)
            }
            None => "Drawn tonight. It will rise and set with the real sky.".to_owned(),
        };
        self.card = Some(crate::game::Card {
            footnote: None,
            picture: None,
            find: None,
            x: self.beside_centre(),
            kicker: "Your constellation".into(),
            title: name.to_owned(),
            body,
            shown: real,
        });
        if reveals.is_some()
            && let Some(chosen) = self.choose(&["reveal"])
        {
            self.talk.pending = Some((
                real + 3_000,
                Pending::Question {
                    chosen: Box::new(chosen),
                    star: None,
                },
            ));
        }
    }

    /// Weights, named stars, drawings and a revealed constellation.
    pub(crate) fn draw_marks(&mut self, real: UnixMs, hz: &Mat3) {
        let cam: Camera = self.camera;
        let t = real as f64 / 1000.0;
        // The weights hang where they were put.
        let weights: Vec<Vec3> = self
            .page
            .weights
            .iter()
            .map(|w| apply(hz, unit(w.ra, w.dec)))
            .collect();
        for (k, v) in weights.iter().enumerate() {
            let (alt, az) = alt_az(*v);
            if alt < hills(az) {
                continue;
            }
            if let Some((x, y)) = cam.project(*v) {
                let pulse = 0.85 + 0.15 * (t * 0.7 + k as f64).sin();
                self.points.push(crate::view::Point {
                    x,
                    y,
                    radius: 2.8,
                    color: WEIGHT,
                    alpha: 0.9,
                    halo: 0.75 * pulse as f32,
                });
            }
        }
        if self.placing() {
            let (cx, cy) = (cam.width / 2.0, cam.height / 2.0);
            let pulse = 0.8 + 0.2 * (t * 1.6).sin();
            // The weight being hung: a warm star, big enough to see what it is.
            self.points.push(crate::view::Point {
                x: cx,
                y: cy,
                radius: 4.2,
                color: WEIGHT,
                alpha: 0.95,
                halo: pulse as f32,
            });
        }
        // Stars that carry a person's name wear a faint ring.
        let named: Vec<u16> = self
            .journal
            .people
            .iter()
            .flat_map(|p| p.stars.clone())
            .collect();
        for hr in named {
            if let Some((x, y)) = self.star_screen(hr, hz) {
                ring(self, x, y, 9.0, NAMED, 0.09);
            }
        }
        // Earlier drawings stay in the sky, faint.
        let past: Vec<(u16, u16)> = self
            .journal
            .drawings
            .iter()
            .flat_map(|d| d.edges.clone())
            .collect();
        for (a, b) in past {
            if let (Some(p), Some(q)) = (self.star_screen(a, hz), self.star_screen(b, hz)) {
                self.field.line(p, q, DRAWN, 0.05);
            }
        }
        // A revealed constellation is shown by `pattern_lines` for a while.
        if self.reveal.is_some_and(|(_, at)| real - at > REVEAL_MS) {
            self.reveal = None;
        }
        if let Some(d) = &self.drawing {
            let edges = d.edges.clone();
            let (current, anchor) = (d.current, d.anchor);
            for (a, b) in edges {
                if let (Some(p), Some(q)) = (self.star_screen(a, hz), self.star_screen(b, hz)) {
                    self.field.line(p, q, DRAWN, 0.28);
                }
            }
            if let Some((x, y)) = anchor.and_then(|a| self.star_screen(a, hz)) {
                ring(self, x, y, 8.0, DRAWN, 0.25);
            }
            if let Some((x, y)) = current.and_then(|c| self.star_screen(c, hz)) {
                let pulse = 0.7 + 0.3 * (t * 3.0).sin();
                ring(self, x, y, 12.0, [0.9, 0.95, 1.0], 0.45 * pulse as f32);
            }
        }
    }

    /// The star pattern to show now: the real constellation a drawing turned
    /// out to be part of, one just caught or still being looked at, or the
    /// one tonight's story is told in.
    fn wanted_pattern(&self) -> Option<Vec<(u16, u16)>> {
        if let Some((f, _)) = self.reveal {
            return Some(self.sky.figures[f].edges.clone());
        }
        if let Some(tour) = &self.tour
            && let Target::Story(s) = self.finds[tour.find].target
        {
            let story = &self.sky.tours.stories[s];
            if !story.pattern {
                return None;
            }
            if !story.figure.is_empty() {
                return Some(story.figure.iter().map(|&[a, b]| (a, b)).collect());
            }
            let anchor = story.anchor;
            return self
                .sky
                .figures
                .iter()
                .find(|f| f.edges.iter().any(|&(a, b)| a == anchor || b == anchor))
                .map(|f| f.edges.clone());
        }
        match self.track.map(|i| (i, &self.finds[i].target)) {
            Some((i, Target::Figure(f))) if self.caught[i] => {
                Some(self.sky.figures[*f].edges.clone())
            }
            _ => None,
        }
    }

    /// The pattern's lines, breathing slowly in and out and never bright.
    pub(crate) fn pattern_lines(&mut self, real: UnixMs, hz: &Mat3, dt: f64) -> Vec<Line> {
        let want = self.wanted_pattern();
        if want != self.pattern && self.pattern_alpha < 0.03 {
            self.pattern = want.clone();
        }
        let target = if want.is_some() && want == self.pattern {
            1.0
        } else {
            0.0
        };
        self.pattern_alpha += (target - self.pattern_alpha) * (1.0 - (-dt / 0.9).exp());
        let Some(edges) = self.pattern.clone() else {
            return Vec::new();
        };
        // About one breath every six and a half seconds.
        let t = real as f64 / 1000.0;
        let breath = 0.5 - 0.5 * (t * std::f64::consts::TAU / 6.5).cos();
        let alpha = (self.pattern_alpha
            * (0.09 + 0.13 * breath)
            * self.session.brightness(real).max(0.4)) as f32;
        if alpha < 0.004 {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (a, b) in edges {
            if let (Some(p), Some(q)) = (self.star_screen(a, hz), self.star_screen(b, hz)) {
                // Stop short of the stars, so they stand clear of the lines.
                let (dx, dy) = (q.0 - p.0, q.1 - p.1);
                let len = (dx * dx + dy * dy).sqrt();
                if len < 20.0 {
                    continue;
                }
                let k = 8.0 / len;
                out.push(Line {
                    a: (p.0 + dx * k, p.1 + dy * k),
                    b: (q.0 - dx * k, q.1 - dy * k),
                    color: FIGURE,
                    alpha,
                });
            }
        }
        out
    }

    /// Tonight's plans: a warm mark on the horizon below where each
    /// event happens, or where it will come up, with the plan's words and
    /// what the mark is for.
    pub(crate) fn plan_mark_frame(&mut self, now: UnixMs, real: UnixMs) -> Vec<Text> {
        let mut out = Vec::new();
        let t = real as f64 / 1000.0;
        for mark in self.plan_marks.clone() {
            let says = &self.guide.lines().plan;
            let (alt, az) = mark.place.seen(self.observer, now);
            let (az, why) = if alt > 0.0 {
                let say = if mark.shower {
                    &says.shower_up
                } else {
                    &says.up
                };
                (az, say.replace("{name}", &mark.name))
            } else if let Some((at, az)) = mark.rise {
                let say = if mark.shower {
                    &says.shower_later
                } else {
                    &says.rises
                };
                // To the nearest ten minutes: it's "about", and easier read.
                let at = (at + 300_000) / 600_000 * 600_000;
                let time = westering_core::finds::civil_time(at, self.offset_s);
                let why = say.replace("{name}", &mark.name).replace("{time}", &time);
                (az, why)
            } else {
                // Down, and not up again before morning: nothing to point at.
                continue;
            };
            let on_horizon = westering_core::coords::from_alt_az(hills(az) + 1.2, az);
            let Some((x, y)) = self.camera.project(on_horizon) else {
                continue;
            };
            if !self.camera.on_screen(x, y, 20.0) {
                continue;
            }
            let pulse = 0.8 + 0.2 * (t * 0.9).sin();
            self.points.push(crate::view::Point {
                x,
                y,
                radius: 3.2,
                color: [1.0, 0.78, 0.45],
                alpha: 0.9,
                halo: pulse as f32,
            });
            // Above the mark, clear of the compass letters, and kept on the
            // sky: not under tonight's list, nor off the edge.
            let (w, h) = (self.camera.width, self.camera.height);
            let list = if crate::game::compact(w, h) || self.session.phase() != Phase::Hunt {
                16.0
            } else {
                300.0
            };
            if x > w - list {
                continue;
            }
            let half = |text: &str, size: f64| text.chars().count() as f64 * size * 0.27;
            let wide = half(&mark.words, 13.0).max(half(&why, 12.0)) + 16.0;
            let cx = x.clamp(wide, (w - list - wide).max(wide));
            let warm = [1.0, 0.86, 0.66];
            out.push(
                Text::new(cx, y - 64.0, mark.words, 13.0, 0.85)
                    .centred()
                    .color(warm),
            );
            out.push(
                Text::new(cx, y - 45.0, why, 12.0, 0.6)
                    .centred()
                    .color(warm),
            );
        }
        out
    }

    /// Words for marks near the middle of the view.
    pub(crate) fn mark_labels(&self, hz: &Mat3, brightness: f64) -> Vec<Text> {
        let mut out = Vec::new();
        let cam = &self.camera;
        let (cx, cy) = (cam.width / 2.0, cam.height / 2.0);
        let reach = self.reticle_radius() * 2.2;
        let near = |x: f64, y: f64| {
            let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
            (d < reach).then(|| 1.0 - d / reach)
        };
        // In the finale every weight keeps its words, so you see what's setting.
        let setting = self.session.phase() == westering_core::session::Phase::Finale;
        for w in &self.page.weights {
            let v = apply(hz, unit(w.ra, w.dec));
            if alt_az(v).0 < -1.0 {
                continue;
            }
            if let Some((x, y)) = cam.project(v)
                && let Some(a) = near(x, y).or(setting.then_some(0.85))
                && let Some(weight) = self.journal.weight(w.weight)
            {
                // Setting, it keeps its own light while the sky dims round it.
                let (size, alpha) = if setting {
                    (15.0, 0.8)
                } else {
                    (13.0, 0.55 * a * brightness)
                };
                out.push(
                    Text::new(x + 12.0, y + 6.0, weight.text.clone(), size, alpha)
                        .color([1.0, 0.85, 0.7]),
                );
            }
        }
        for p in &self.journal.people {
            for &hr in &p.stars {
                if let Some((x, y)) = self.star_screen(hr, hz)
                    && let Some(a) = near(x, y)
                {
                    out.push(
                        Text::new(
                            x + 14.0,
                            y - 18.0,
                            format!("{}'s star", p.name),
                            13.0,
                            0.5 * a * brightness,
                        )
                        .color([1.0, 0.88, 0.72]),
                    );
                }
            }
        }
        out
    }
}

fn ring(game: &mut Game, x: f64, y: f64, radius: f64, color: Rgb, amount: f32) {
    for k in 0..12 {
        let a = k as f64 / 12.0 * std::f64::consts::TAU;
        game.field
            .splat(x + radius * a.cos(), y + radius * a.sin(), color, amount);
    }
}
