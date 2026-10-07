//! By day. While the Sun is up the view looks down at the earth instead of
//! up at the stars: a stretch of ground under a daylit sky, drawn in the
//! same dots. What's there is true where the user is today: the Sun where
//! it really is, a standing stone casting this minute's shadow, the Moon if
//! it's out by day, how much the day has grown or shrunk, and a few small
//! things on the ground this time of year. It's short: a look, perhaps one
//! small question about the day ahead, and back to the day, with a nudge to
//! step outside.

use crate::field::noise3;
use crate::game::{Card, Game, Row, smoothstep, turn};
use crate::guide::Aim;
use crate::view::{Frame, Point, Text};
use gtk::gdk;
use westering_core::coords::{Observer, compass};
use westering_core::ephem::Body;
use westering_core::finale::whereabouts;
use westering_core::ground::{self, Ground, Shape, Thing, Today};
use westering_core::journey::season;
use westering_core::questions::{Chosen, days_between};
use westering_core::sky::{crossing, see};
use westering_core::time::{DAY, MINUTE, UnixMs, civil_date, clock};
use westering_core::wisp::Season;

/// The Sun has to be this high for a visit to be a day visit.
pub const SUN_UP: f64 = 1.0;
/// How long the ending lasts before the window closes.
const ENDING_MS: UnixMs = 16_000;
/// How much richer than their mix the day's colours are drawn.
const SATURATION: f32 = 1.45;
/// A day visit ends by itself after this long.
const LONGEST_MS: UnixMs = 20 * MINUTE;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Find {
    Sun,
    Shadow,
    Moon,
    Year,
    Ground(usize),
}

pub struct Day {
    pub today: Today,
    pub things: Vec<Thing>,
    pub finds: Vec<Find>,
    pub found: Vec<bool>,
    pub ending: Option<UnixMs>,
    pub handoff: String,
    asked: bool,
    /// What the base layer was drawn for: its size and the minute.
    stamp: Option<(usize, usize, i64)>,
    /// Which way the view faces: towards the Sun's side of the sky.
    facing: f64,
    pub(crate) season: Option<Season>,
    /// The month as it would be in the north, for what's about: the same
    /// in both hemispheres once shifted.
    pub(crate) month: u32,
    began: UnixMs,
}

pub fn is_day(observer: Observer, now: UnixMs) -> bool {
    see(Body::Sun, observer, now).alt > SUN_UP
}

impl Day {
    pub fn new(ground: &Ground, observer: Observer, now: UnixMs, offset_s: i32, key: &str) -> Day {
        let today = ground::today(observer, now, offset_s);
        let (_, month, _) = civil_date(now, offset_s);
        let things: Vec<Thing> = ground
            .today(month, observer.lat, key)
            .into_iter()
            .cloned()
            .collect();
        let mut finds = vec![Find::Sun];
        if today.shadow.is_some() {
            finds.push(Find::Shadow);
        }
        if today.moon.is_some() {
            finds.push(Find::Moon);
        }
        if today.change.is_some() {
            finds.push(Find::Year);
        }
        finds.extend((0..things.len()).map(Find::Ground));
        Day {
            found: vec![false; finds.len()],
            today,
            things,
            finds,
            ending: None,
            handoff: String::new(),
            asked: false,
            stamp: None,
            facing: if observer.lat >= 0.0 { 180.0 } else { 0.0 },
            season: season(month, observer.lat),
            month: if observer.lat < 0.0 {
                (month + 5) % 12 + 1
            } else {
                month
            },
            began: 0,
        }
    }
}

/// A shape's outline as strokes in a unit box, y downwards.
fn outline(shape: Shape) -> Vec<Vec<(f64, f64)>> {
    let ring = |cx: f64, cy: f64, rx: f64, ry: f64, from: f64, to: f64, n: usize| {
        (0..=n)
            .map(|k| {
                let a = from + (to - from) * k as f64 / n as f64;
                (cx + rx * a.cos(), cy + ry * a.sin())
            })
            .collect::<Vec<_>>()
    };
    use std::f64::consts::{PI, TAU};
    match shape {
        Shape::Leaf => {
            let side = |s: f64| {
                (0..=10)
                    .map(|k| {
                        let t = k as f64 / 10.0;
                        (s * (t * PI).sin() * 0.45, -0.9 + 1.6 * t)
                    })
                    .collect::<Vec<_>>()
            };
            vec![side(1.0), side(-1.0), vec![(0.0, -0.8), (0.0, 1.0)]]
        }
        Shape::Acorn => vec![
            ring(0.0, 0.25, 0.42, 0.6, 0.0, PI, 12),
            vec![(-0.5, -0.1), (0.5, -0.1)],
            ring(0.0, -0.1, 0.5, 0.35, PI, TAU, 10),
            vec![(0.0, -0.45), (0.1, -0.7)],
        ],
        Shape::Seed => vec![
            ring(-0.35, 0.2, 0.3, 0.3, 0.0, TAU, 10),
            ring(0.35, 0.25, 0.3, 0.3, 0.0, TAU, 10),
            ring(0.0, -0.25, 0.3, 0.3, 0.0, TAU, 10),
            vec![(0.0, -0.55), (0.2, -0.95)],
        ],
        Shape::Mushroom => vec![
            ring(0.0, 0.0, 0.8, 0.6, PI, TAU, 14),
            vec![(-0.8, 0.0), (0.8, 0.0)],
            vec![(-0.2, 0.0), (-0.25, 0.9), (0.25, 0.9), (0.2, 0.0)],
        ],
        Shape::Feather => vec![
            vec![(-0.5, 0.9), (0.5, -0.9)],
            ring(0.0, 0.0, 0.35, 0.9, PI * 0.9, PI * 1.9, 10)
                .into_iter()
                .map(|(x, y)| (x * 0.7 + y * 0.45, y * 0.9 - x * 0.2))
                .collect(),
        ],
        Shape::Flower => {
            let mut out: Vec<Vec<(f64, f64)>> = (0..5)
                .map(|k| {
                    let a = k as f64 / 5.0 * TAU - PI / 2.0;
                    ring(a.cos() * 0.45, a.sin() * 0.45, 0.3, 0.3, 0.0, TAU, 9)
                })
                .collect();
            out.push(ring(0.0, 0.0, 0.14, 0.14, 0.0, TAU, 6));
            out
        }
        Shape::Web => {
            let mut out: Vec<Vec<(f64, f64)>> = (0..8)
                .map(|k| {
                    let a = k as f64 / 8.0 * TAU;
                    vec![(0.0, 0.0), (a.cos() * 0.95, a.sin() * 0.95)]
                })
                .collect();
            for r in [0.35, 0.65, 0.9] {
                out.push(ring(0.0, 0.0, r, r, 0.0, TAU, 8));
            }
            out
        }
        Shape::Bird => vec![
            ring(-0.45, 0.2, 0.45, 0.35, PI * 1.05, PI * 1.95, 8),
            ring(0.45, 0.2, 0.45, 0.35, PI * 1.05, PI * 1.95, 8),
        ],
        Shape::Snowflake => {
            let mut out = Vec::new();
            for k in 0..6 {
                let a = k as f64 / 6.0 * TAU;
                let (c, s) = (a.cos(), a.sin());
                out.push(vec![(0.0, 0.0), (c * 0.95, s * 0.95)]);
                let (bx, by) = (c * 0.6, s * 0.6);
                for turn in [0.6, -0.6] {
                    let b = a + turn;
                    out.push(vec![(bx, by), (bx + b.cos() * 0.25, by + b.sin() * 0.25)]);
                }
            }
            out
        }
        Shape::Bud => vec![
            vec![(-0.9, 0.7), (0.9, 0.3)],
            ring(0.0, -0.1, 0.25, 0.45, PI * 0.5, PI * 2.5, 12),
        ],
        Shape::Stone => vec![ring(0.0, 0.2, 0.85, 0.5, 0.0, TAU, 16)],
        Shape::Drop => vec![{
            let mut d = ring(0.0, 0.3, 0.5, 0.5, -PI * 0.15, PI * 1.15, 12);
            d.push((0.0, -0.9));
            d.push(d[0]);
            d
        }],
        Shape::Tree => vec![
            vec![(-0.12, 0.95), (-0.12, 0.2)],
            vec![(0.12, 0.95), (0.12, 0.2)],
            ring(0.0, -0.3, 0.7, 0.55, 0.0, TAU, 16),
        ],
        Shape::Worm => vec![
            (0..=16)
                .map(|k| {
                    let t = k as f64 / 16.0;
                    (-0.9 + 1.8 * t, (t * TAU * 1.2).sin() * 0.2)
                })
                .collect(),
        ],
        Shape::Bee => vec![
            ring(0.0, 0.2, 0.55, 0.35, 0.0, TAU, 14),
            vec![(-0.15, -0.1), (-0.15, 0.5)],
            vec![(0.15, -0.1), (0.15, 0.5)],
            ring(-0.2, -0.35, 0.25, 0.3, 0.0, TAU, 8),
            ring(0.25, -0.35, 0.25, 0.3, 0.0, TAU, 8),
        ],
        Shape::Ant => {
            let mut out = vec![
                ring(-0.6, 0.0, 0.18, 0.15, 0.0, TAU, 7),
                ring(-0.1, 0.0, 0.18, 0.15, 0.0, TAU, 7),
                ring(0.5, 0.0, 0.3, 0.22, 0.0, TAU, 9),
            ];
            for x in [-0.25, -0.1, 0.05] {
                out.push(vec![(x - 0.2, -0.4), (x, 0.0), (x - 0.2, 0.4)]);
            }
            out
        }
        Shape::Moss => (0..5)
            .map(|k| ring(-0.8 + 0.4 * k as f64, 0.3, 0.2, 0.3, PI, TAU, 6))
            .chain([vec![(-1.0, 0.3), (1.0, 0.3)]])
            .collect(),
        Shape::Cloud => vec![
            ring(-0.45, 0.1, 0.35, 0.3, PI, TAU, 8),
            ring(0.05, -0.05, 0.45, 0.45, PI, TAU, 10),
            ring(0.55, 0.12, 0.3, 0.28, PI, TAU, 8),
            vec![(-0.8, 0.1), (0.85, 0.12)],
        ],
    }
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

impl Game {
    pub(crate) fn by_day(&self) -> bool {
        self.day.is_some()
    }

    fn day_ref(&self) -> &Day {
        self.day.as_ref().expect("a day visit")
    }

    /// Where the horizon sits: a third of the way down, so the ground has
    /// most of the view.
    pub(crate) fn horizon_y(&self) -> f64 {
        self.camera.height * 0.34
    }

    /// Where something in the sky sits on the screen, if it's on the side
    /// the view faces.
    fn day_sky(&self, alt: f64, az: f64) -> Option<(f64, f64)> {
        let (w, hy) = (self.camera.width, self.horizon_y());
        let rel = turn(self.day_ref().facing, az);
        if rel.abs() > 95.0 {
            return None;
        }
        // Facing south, east is on the left: turning clockwise is rightwards.
        let x = w / 2.0 + rel / 95.0 * (w / 2.0 - 30.0);
        // Below the horizon it's below the horizon: the hills hide it.
        let y = hy - alt.clamp(-20.0, 75.0) / 75.0 * (hy - 40.0);
        Some((x, y))
    }

    /// The standing stone's foot, and its height on the screen.
    pub(crate) fn stone(&self) -> (f64, f64, f64) {
        let (w, h, hy) = (self.camera.width, self.camera.height, self.horizon_y());
        (w * 0.58, hy + (h - hy) * 0.4, (h - hy) * 0.16)
    }

    /// Where find `i` is on the screen.
    pub(crate) fn day_spot(&self, i: usize) -> Option<(f64, f64)> {
        let day = self.day_ref();
        let (w, h, hy) = (self.camera.width, self.camera.height, self.horizon_y());
        match *day.finds.get(i)? {
            Find::Sun => self.day_sky(day.today.sun_alt, day.today.sun_az),
            Find::Moon => {
                let m = day.today.moon.as_ref()?;
                self.day_sky(m.alt, m.az)
            }
            Find::Shadow => {
                let (x, y, _) = self.stone();
                Some((x, y))
            }
            Find::Year => Some((w * 0.14, hy - 20.0)),
            Find::Ground(k) => {
                let aloft = matches!(day.things[k].shape, Shape::Bird | Shape::Cloud);
                if aloft {
                    Some((w * (0.26 + 0.2 * k as f64), hy * 0.4))
                } else {
                    let spots = [(0.3, 0.64), (0.82, 0.6), (0.5, 0.86)];
                    let (fx, fy) = spots[k % spots.len()];
                    Some((w * fx, hy + (h - hy) * (fy - 0.34) / 0.66))
                }
            }
        }
    }

    fn day_name(&self, i: usize) -> String {
        let day = self.day_ref();
        let cards = &self.ground.day;
        match day.finds[i] {
            Find::Sun => cards.sun.name.clone(),
            Find::Shadow => cards.shadow.name.clone(),
            Find::Moon => cards.moon.name.clone(),
            Find::Year => cards.year.name.clone(),
            Find::Ground(k) => day.things[k].name.clone(),
        }
    }

    fn day_id(&self, i: usize) -> String {
        let day = self.day_ref();
        match day.finds[i] {
            Find::Sun => "day:sun".into(),
            Find::Shadow => "day:shadow".into(),
            Find::Moon => "day:moon".into(),
            Find::Year => "day:year".into(),
            Find::Ground(k) => format!("day:{}", day.things[k].id),
        }
    }

    pub(crate) fn day_rows(&self) -> Vec<Row> {
        let day = self.day_ref();
        (0..day.finds.len())
            .map(|i| {
                let t = &day.today;
                let (kind, place) = match day.finds[i] {
                    Find::Sun => {
                        let side = if self.day_spot(i).is_some() {
                            whereabouts(t.sun_alt, t.sun_az)
                        } else {
                            format!("{}, behind you", whereabouts(t.sun_alt, t.sun_az))
                        };
                        ("In the sky", side)
                    }
                    Find::Shadow => (
                        "On the ground",
                        format!("pointing {}", compass(t.shadow_az)),
                    ),
                    Find::Moon => (
                        "In the sky",
                        t.moon
                            .as_ref()
                            .map(|m| whereabouts(m.alt, m.az))
                            .unwrap_or_default(),
                    ),
                    Find::Year => (
                        "Today",
                        t.length.map(ground::length_words).unwrap_or_default(),
                    ),
                    Find::Ground(k) => ("Around now", day.things[k].place.clone()),
                };
                Row {
                    name: self.day_name(i),
                    kind,
                    whereabouts: place,
                    found: day.found[i],
                }
            })
            .collect()
    }

    /// The card for find `i`, a new fact on each later visit.
    fn day_card_text(&self, i: usize) -> (String, String) {
        let day = self.day_ref();
        let t = &day.today;
        let cards = &self.ground.day;
        let times = self
            .journal
            .times_found_before(&self.day_id(i), &self.night);
        let say =
            |first: &str, more: &[String]| westering_core::catalogues::say_for(first, more, times);
        match day.finds[i] {
            Find::Sun => {
                let fact = say(&cards.sun.fact, &cards.sun.more)
                    .replace("{where}", &whereabouts(t.sun_alt, t.sun_az));
                (
                    "IN THE SKY".into(),
                    format!("{fact}\n\n{}", cards.sun.mirror),
                )
            }
            Find::Shadow => {
                let fact = say(&cards.shadow.fact, &cards.shadow.more)
                    .replace("{length}", ground::shadow_words(t.shadow.unwrap_or(1.0)))
                    .replace("{toward}", compass(t.shadow_az));
                (
                    "ON THE GROUND".into(),
                    format!("{fact}\n\n{}", cards.shadow.mirror),
                )
            }
            Find::Moon => {
                let (place, phase) = t
                    .moon
                    .as_ref()
                    .map(|m| (whereabouts(m.alt, m.az), m.phase.to_lowercase()))
                    .unwrap_or_default();
                let fact = say(&cards.moon.fact, &cards.moon.more)
                    .replace("{where}", &place)
                    .replace("{phase}", &phase);
                (
                    "IN THE SKY".into(),
                    format!("{fact}\n\n{}", cards.moon.mirror),
                )
            }
            Find::Year => {
                let change = t.change.unwrap_or(0);
                let fact = say(&cards.year.fact, &cards.year.more)
                    .replace("{change}", &ground::change_words(change))
                    .replace(
                        "{daylight}",
                        &t.length.map(ground::length_words).unwrap_or_default(),
                    );
                let mirror = if change >= 0 {
                    &cards.year.longer
                } else {
                    &cards.year.shorter
                };
                ("TODAY".into(), format!("{fact}\n\n{mirror}"))
            }
            Find::Ground(k) => {
                let thing = &day.things[k];
                let fact = say(&thing.fact, &thing.more);
                (
                    format!("AROUND NOW, {}", thing.place.to_uppercase()),
                    format!("{fact}\n\n{}", thing.mirror),
                )
            }
        }
    }

    /// Shows find `i`: its card, with the wisp beside it.
    pub(crate) fn day_open(&mut self, i: usize, real: UnixMs) {
        let Some(day) = &self.day else {
            return;
        };
        if i >= day.finds.len() || day.ending.is_some() || self.keeping() {
            return;
        }
        self.input(real);
        let (kicker, body) = self.day_card_text(i);
        // Left of the middle, so the wisp beside it stays clear of the list.
        let x = (self.camera.width / 2.0 - 240.0).max(20.0);
        self.card = Some(Card {
            footnote: None,
            picture: None,
            find: None,
            x,
            kicker,
            title: self.day_name(i),
            body,
            shown: real,
        });
        let first = !self.day_ref().found[i];
        if first {
            let id = self.day_id(i);
            let name = self.day_name(i);
            if let Some(day) = &mut self.day {
                day.found[i] = true;
            }
            self.session.found_one(real);
            if let Err(e) = self.journal.mark_found(&id, &self.night) {
                eprintln!("westering: couldn't keep today's find: {e}");
            }
            self.page.finds.push(name);
            self.save_page_now();
            self.guide_caught(real);
        }
        // Over to it, rather than to the card.
        if let Some((sx, sy)) = self.day_spot(i) {
            self.fly(Aim::Spot(sx, sy), real, 4_000);
        }
    }

    /// After a card is put away: now and then one small question about
    /// the day ahead, after the second find.
    pub(crate) fn day_card_gone(&mut self, real: UnixMs) {
        let Some(day) = &self.day else {
            return;
        };
        let found = day.found.iter().filter(|f| **f).count();
        if day.asked || found < 2 || self.talk.first_night {
            return;
        }
        if self.journal.settings.ask == westering_core::journal::Ask::Never {
            return;
        }
        if let Some(day) = &mut self.day {
            day.asked = true;
        }
        let today = self.night.clone();
        let fresh: Vec<_> = self
            .talk
            .bank
            .iter()
            .filter(|q| q.on == "day")
            .filter(|q| {
                !self
                    .journal
                    .asked
                    .iter()
                    .any(|a| a.question == q.id && days_between(&a.night, &today) < 30)
            })
            .cloned()
            .collect();
        if fresh.is_empty() {
            return;
        }
        let q = fresh[westering_core::finds::stable_hash((0, 0, 9), &today) as usize % fresh.len()]
            .clone();
        let chosen = Chosen {
            text: q.text.clone(),
            question: q,
            person: None,
            event: None,
        };
        self.show_question(chosen, None);
        self.guide_question(real);
    }

    pub(crate) fn day_click(&mut self, x: f64, y: f64, real: UnixMs) -> bool {
        if let Some(i) = self.day_at(x, y) {
            if self.card.is_some() {
                self.dismiss_card(real);
            }
            self.day_open(i, real);
            return true;
        }
        if self.card.is_some() {
            self.dismiss_card(real);
            return true;
        }
        false
    }

    /// The find under a point, if any.
    pub(crate) fn day_at(&self, x: f64, y: f64) -> Option<usize> {
        let day = self.day.as_ref()?;
        if day.ending.is_some() || self.keeping() {
            return None;
        }
        (0..day.finds.len())
            .filter(|&i| day.finds[i] != Find::Year)
            .filter_map(|i| {
                let (sx, sy) = self.day_spot(i)?;
                let d = ((sx - x).powi(2) + (sy - y).powi(2)).sqrt();
                (d < 42.0).then_some((i, d))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    pub(crate) fn day_key(&mut self, key: gdk::Key, real: UnixMs) -> bool {
        match key {
            // Space, Enter and Tab all carry on: the wisp's next word, then
            // past a card, then to the next find.
            gdk::Key::Tab | gdk::Key::space | gdk::Key::Return | gdk::Key::KP_Enter
                if self.more_now() =>
            {
                self.guide_next(real);
            }
            gdk::Key::Tab | gdk::Key::space | gdk::Key::Return | gdk::Key::KP_Enter
                if self.card.is_some() =>
            {
                self.dismiss_card(real);
            }
            gdk::Key::Tab | gdk::Key::space | gdk::Key::Return | gdk::Key::KP_Enter => {
                self.guide_next(real);
                let day = self.day_ref();
                let n = day.finds.len();
                let from = self
                    .card
                    .as_ref()
                    .and_then(|c| (0..n).find(|&i| self.day_name(i) == c.title))
                    .map_or(0, |i| i + 1);
                let next = (0..n)
                    .map(|k| (from + k) % n)
                    .find(|&i| !day.found[i])
                    .unwrap_or(from % n);
                if self.card.is_some() {
                    self.dismiss_card(real);
                }
                self.day_open(next, real);
            }

            gdk::Key::Escape => {
                if self.card.is_some() {
                    self.dismiss_card(real);
                }
            }
            gdk::Key::w | gdk::Key::W => self.day_end(real),
            gdk::Key::question | gdk::Key::F1 => self.guide_help(real),
            gdk::Key::k | gdk::Key::K => self.toggle_company(real),
            gdk::Key::m | gdk::Key::M => self.toggle_music(real),
            gdk::Key::l | gdk::Key::L => {
                self.request = Some(crate::talk::Request::Book(Some(self.night.clone())));
            }
            _ => return false,
        }
        true
    }

    /// The first moments: what the wisp says on arriving by day.
    pub(crate) fn day_arrival(&mut self, real: UnixMs) {
        let lines = &self.guide.lines().day;
        let say = if !self.seen("day-hello") {
            lines.first.clone()
        } else {
            let pick = westering_core::finds::stable_hash((0, 0, 4), &self.night) as usize
                % lines.hello.len();
            vec![lines.hello[pick].clone()]
        };
        self.mark_seen("day-hello");
        for (k, text) in say.into_iter().enumerate() {
            let aim = if k == 0 {
                Aim::Near(0.36, 0.5)
            } else {
                Aim::Near(0.4, 0.5)
            };
            self.say_at(aim, text, real + 1_200, 8_000);
        }
        let now = self.clock.sky(real);
        let (_, month, d) = civil_date(now, self.offset_s);
        let mut line = format!(
            "{} {} {} · the Sun is up",
            westering_core::time::weekday(now, self.offset_s),
            d,
            westering_core::time::MONTHS[month as usize - 1]
        );
        if let Some(length) = self.day_ref().today.length {
            line.push_str(&format!(" · {}", ground::length_words(length)));
        }
        self.caption = Some(crate::game::Timed {
            text: line,
            shown: real + 900,
            hold: 8_000,
        });
        if let Some(day) = &mut self.day {
            day.began = real;
        }
    }

    /// The evening proper begins: say where the list is, the first time.
    pub(crate) fn day_hunt(&mut self, real: UnixMs) {
        if !self.seen("day-list") {
            self.mark_seen("day-list");
            let text = self.guide.lines().day.list.clone();
            self.say_at(Aim::List, text, real + 500, 8_000);
        }
    }

    /// Back to the day: the last line points outside, then the window goes.
    pub(crate) fn day_end(&mut self, real: UnixMs) {
        let Some(day) = &self.day else {
            return;
        };
        if day.ending.is_some() {
            return;
        }
        self.stop_company(real);
        self.card = None;
        self.caption = None;
        self.talk.flow = None;
        self.talk.pending = None;
        self.set_prompt(None);
        self.hush();
        let handoff = self.day_handoff(real);
        let ending = self.guide.lines().day.ending.clone();
        if let Some(day) = &mut self.day {
            day.ending = Some(real);
            day.handoff = handoff;
        }
        // Home to its moss, clear of the last words.
        self.fly(Aim::Home, real, 0);
        self.say_at(Aim::Home, ending, real + 7_000, 6_000);
    }

    fn day_handoff(&self, real: UnixMs) -> String {
        let day = self.day_ref();
        let t = &day.today;
        let lines = &self.guide.lines().day;
        let now = self.clock.sky(real);
        match t.shadow {
            Some(ratio) if t.sun_alt > 6.0 => lines
                .outside
                .replace("{where}", &whereabouts(t.sun_alt, t.sun_az))
                .replace("{length}", ground::shadow_words(ratio))
                .replace("{toward}", compass(t.shadow_az)),
            _ => {
                let dark = crossing(now, DAY, -6.0, false, |at| {
                    see(Body::Sun, self.observer, at).alt
                });
                let (h, m) = clock(dark.unwrap_or(now + 40 * MINUTE), self.offset_s);
                lines.dusk.replace("{time}", &format!("{h:02}:{m:02}"))
            }
        }
    }

    /// How far through the ending, 0 to 1: the scene fades over its last
    /// few seconds.
    pub(crate) fn day_fade(&self, real: UnixMs) -> f64 {
        match self.day.as_ref().and_then(|d| d.ending) {
            Some(at) => smoothstep((real - at - (ENDING_MS - 5_000)) as f64 / 5_000.0),
            None => 0.0,
        }
    }

    /// The day's own arc: arrival, a look, and an end, by choice or after a
    /// while. The Sun's place is worked out afresh every few minutes, so a
    /// long stay sees it move and the shadow turn.
    pub(crate) fn day_logic(&mut self, real: UnixMs) {
        if self.session.phase() == westering_core::session::Phase::Arrival
            && !self.wisp_busy(real)
            && let Some(westering_core::session::Phase::Hunt) = self.session.tick(real)
        {
            self.day_hunt(real);
        }
        let day = self.day_ref();
        if let Some(at) = day.ending {
            if real - at > ENDING_MS {
                self.quit = true;
            }
        } else if !self.keeping() && day.began > 0 && real - day.began > LONGEST_MS {
            self.day_end(real);
        }
        let now = self.clock.sky(real);
        let minute = now / (5 * MINUTE);
        if self.day_ref().stamp.is_some_and(|s| s.2 != minute) {
            let fresh = ground::today(self.observer, now, self.offset_s);
            if let Some(day) = &mut self.day {
                let moon = fresh.moon.clone().or(day.today.moon.take());
                day.today = ground::Today { moon, ..fresh };
            }
        }
    }

    /// One frame of the day.
    pub(crate) fn day_frame(&mut self, real: UnixMs) -> Frame {
        let (w, h) = (self.camera.width, self.camera.height);
        let pitch = if w > 2200.0 { 3.2 } else { 2.8 };
        let resized = self.field.fit(w, h, pitch);
        let minute = self.clock.sky(real) / (5 * MINUTE);
        let stamp = (self.field.cols, self.field.rows, minute);
        if resized || self.day_ref().stamp != Some(stamp) {
            self.day_base();
            if let Some(day) = &mut self.day {
                day.stamp = Some(stamp);
            }
        }
        self.field.begin();
        self.day_draw(real);
        let arrive = self.session.brightness(real);
        let gain = arrive * (1.0 - self.day_fade(real));
        let texture = self.field.texture(gain as f32);
        let mut texts = self.words(real, gain.max(0.6));
        if let Some(i) = self.pointer.and_then(|(x, y, _)| self.day_at(x, y))
            && self.card.is_none()
            && let Some((sx, sy)) = self.day_spot(i)
        {
            texts.push(Text::new(sx + 22.0, sy + 6.0, self.day_name(i), 13.0, 0.75));
        }
        if let Some(at) = self.day_ref().ending {
            let a = smoothstep((real - at - 800) as f64 / 2_000.0) * (1.0 - self.day_fade(real));
            texts.push(
                Text::new(w / 2.0, h * 0.42, self.day_ref().handoff.clone(), 22.0, a)
                    .centred()
                    .wrap((w * 0.75).min(760.0))
                    .color([1.0, 0.95, 0.86]),
            );
        }
        self.sprite_brightness = gain.max(0.3);
        let (sprites, bubble, embers) = self.guide_frame(real, gain.max(0.3));
        let card = self.card_view(real);
        crate::view::Frame {
            legend: self.help_table(real),
            glows: Vec::new(),
            points: std::mem::take(&mut self.points),
            silhouettes: self.day_life(real),
            marks: embers,
            sprites,
            bubble,
            card,
            texture: Some(texture),
            cols: self.field.cols,
            rows: self.field.rows,
            pitch: self.field.pitch,
            background: [0.05, 0.06, 0.07],
            texts,
            ..Default::default()
        }
    }

    /// The slow layer: sky, far hills and the ground, lit for the hour and
    /// coloured for the season.
    fn day_base(&mut self) {
        let (h, hy) = (self.camera.height, self.horizon_y());
        let day = self.day_ref();
        let sun_alt = day.today.sun_alt;
        // Dimming towards dusk, for a long stay.
        let light = (0.25 + 0.75 * ((sun_alt + 6.0) / 36.0).clamp(0.0, 1.0)) as f32;
        let low = (1.0 - sun_alt / 14.0).clamp(0.0, 1.0) as f32;
        let grass = match day.season {
            Some(Season::Spring) => [0.42, 0.7, 0.3],
            Some(Season::Summer) => [0.4, 0.62, 0.24],
            Some(Season::Autumn) => [0.52, 0.6, 0.28],
            Some(Season::Winter) => [0.5, 0.58, 0.5],
            None => [0.38, 0.64, 0.3],
        };
        let soil = [0.46, 0.36, 0.24];
        let (cols, rows, pitch) = (self.field.cols, self.field.rows, self.field.pitch as f64);
        let base = self.field.base_mut();
        for r in 0..rows {
            let y = (r as f64 + 0.5) * pitch;
            for c in 0..cols {
                let x = (c as f64 + 0.5) * pitch;
                let hill = hy - 8.0 - 24.0 * noise3(x / 260.0, 0.5, 3.3);
                let (colour, amount) = if y < hill {
                    let t = (y / hy) as f32;
                    let sky = mix([0.36, 0.58, 1.0], [0.78, 0.88, 1.0], t);
                    let sky = mix(sky, [1.0, 0.74, 0.48], low * t * t * 0.6);
                    (sky, 0.62 + 0.3 * t)
                } else if y < hy {
                    let n = noise3(x / 40.0, y / 12.0, 5.0) as f32;
                    (
                        mix([0.42, 0.56, 0.62], [0.5, 0.64, 0.56], n),
                        0.62 + 0.1 * n,
                    )
                } else {
                    let d = ((y - hy) / (h - hy)).clamp(0.0, 1.0);
                    let near = 0.3 + d;
                    let n = noise3(x * 0.035 / near, (y - hy) * 0.09 / (near * near), 1.7) as f32;
                    let fine = noise3(x * 0.3, y * 0.3, 9.1) as f32;
                    let colour = mix(grass, soil, smoothstep(n as f64 - 0.3) as f32 * 0.7);
                    (colour, 0.42 + 0.3 * d as f32 + 0.14 * n + 0.1 * fine)
                };
                base[r * cols + c] = [
                    colour[0] * amount * light,
                    colour[1] * amount * light,
                    colour[2] * amount * light,
                ];
            }
        }
        self.day_scenery(light);
        // Daylight colours, a little richer than the mixing leaves them.
        for cell in self.field.base_mut() {
            let grey = (cell[0] + cell[1] + cell[2]) / 3.0;
            for k in cell.iter_mut() {
                *k = (grey + (*k - grey) * SATURATION).max(0.0);
            }
        }
    }

    /// What moves or matters: the Sun, the Moon, drifting clouds, the stone
    /// and its shadow, and the things on the ground.
    fn day_draw(&mut self, real: UnixMs) {
        let t = real as f64 / 1000.0;
        let (w, hy) = (self.camera.width, self.horizon_y());
        let day = self.day.as_ref().expect("a day visit");
        let today = day.today.clone();

        // A few soft clouds, drifting slowly.
        for k in 0..3 {
            let speed = 3.0 + k as f64;
            let x = (k as f64 * 0.37 * w + t * speed).rem_euclid(w + 300.0) - 150.0;
            let y = hy * (0.25 + 0.2 * k as f64);
            for j in 0..4 {
                self.field.glow(
                    x + j as f64 * 34.0,
                    y + (j % 2) as f64 * 6.0,
                    30.0,
                    [1.0, 1.0, 1.0],
                    0.14,
                );
            }
        }
        // The Sun, and its glow, only where there's sky: a low Sun sinks
        // behind the hills rather than shining through them.
        if let Some((x, y)) = self.day_sky(today.sun_alt, today.sun_az) {
            let hy = self.horizon_y();
            for (radius, colour, amount) in [
                (80.0, [1.0, 0.92, 0.75], 0.4f32),
                (14.0, [1.0, 0.98, 0.92], 4.0),
            ] {
                let reach = radius * 1.6;
                self.field.disc(x, y, reach, |u, v| {
                    let (px, py) = (x + u * reach, y + v * reach);
                    let d2 = (u * u + v * v) * 1.6 * 1.6;
                    (d2 < 2.5 && py < crate::life::hill_at(px, hy))
                        .then(|| (colour, amount * (-d2 * 1.6).exp() as f32))
                });
            }
        }
        if let Some(m) = &today.moon
            && let Some((x, y)) = self.day_sky(m.alt, m.az)
        {
            // Lit on the Sun's side, pale against the blue.
            let sun_right = self
                .day_sky(today.sun_alt, today.sun_az)
                .map_or(m.age < 0.5, |(sx, _)| sx > x);
            let k = (m.age * std::f64::consts::TAU).cos();
            let hy = self.horizon_y();
            self.field.disc(x, y, 11.0, |u, v| {
                if u * u + v * v > 1.0 || y + v * 11.0 >= crate::life::hill_at(x + u * 11.0, hy) {
                    return None;
                }
                let edge = k * (1.0 - v * v).max(0.0).sqrt();
                let u = if sun_right { u } else { -u };
                let lit = u > edge;
                Some(([0.95, 0.96, 1.0], if lit { 0.75 } else { 0.08 }))
            });
        }

        // The stone, and this minute's shadow falling from its foot.
        let (sx, sy, sh) = self.stone();
        let p = self.field.pitch as f64;
        // Dot centres in a box, row by row.
        let dots = |x0: f64, y0: f64, x1: f64, y1: f64| {
            let (c0, c1) = ((x0 / p - 0.5).floor() as i64, (x1 / p - 0.5).ceil() as i64);
            let (r0, r1) = ((y0 / p - 0.5).floor() as i64, (y1 / p - 0.5).ceil() as i64);
            (r0..=r1).flat_map(move |r| {
                (c0..=c1).map(move |c| ((c as f64 + 0.5) * p, (r as f64 + 0.5) * p))
            })
        };
        let hw = sh * 0.13;
        if let Some(ratio) = today.shadow {
            let rel = turn(day.facing, today.shadow_az).to_radians();
            let len = (ratio * sh).min(sh * 5.0);
            // Ahead is up the screen, foreshortened; clockwise is rightwards.
            let (dx, dy) = (rel.sin() * len, -rel.cos() * len * 0.38);
            // The stone's footprint, an oval on the ground, swept along the
            // shadow: a dot is in shadow if some point of the sweep covers it.
            let (ax, ay) = (hw, hw * 0.38);
            let a = (dx / ax).powi(2) + (dy / ay).powi(2);
            let (x0, x1) = (sx.min(sx + dx) - hw, sx.max(sx + dx) + hw);
            let (y0, y1) = (sy.min(sy + dy) - ay, sy.max(sy + dy) + ay);
            for (px, py) in dots(x0, y0, x1, y1) {
                let (qx, qy) = (px - sx, py - sy);
                let b = -2.0 * (qx * dx / (ax * ax) + qy * dy / (ay * ay));
                let c = (qx / ax).powi(2) + (qy / ay).powi(2);
                let t = if a > 1e-9 {
                    (-b / (2.0 * a)).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                if a * t * t + b * t + c <= 1.0 {
                    // Crisp at the foot, softer towards the tip.
                    self.field.shade(px, py, (0.4 + 0.35 * t) as f32);
                }
            }
        }
        // An upright stone with a rounded top, standing on its footprint.
        for (px, py) in dots(sx - hw, sy - sh, sx + hw, sy) {
            let off = ((px - sx) / hw).abs();
            if off > 1.0 || py > sy {
                continue;
            }
            let top = sy - sh + hw * 0.9 * (1.0 - (1.0 - off * off).max(0.0).sqrt());
            if py >= top {
                let side = if px > sx { 0.8 } else { 1.0 };
                self.field.dot(px, py, [0.86, 0.84, 0.8], 0.9 * side);
            }
        }

        // The things on the ground, breathing gently until found.
        let hovered = self.pointer.and_then(|(x, y, _)| self.day_at(x, y));
        let n = day.finds.len();
        for i in 0..n {
            let Find::Ground(k) = self.day_ref().finds[i] else {
                continue;
            };
            let Some((x, y)) = self.day_spot(i) else {
                continue;
            };
            let found = self.day_ref().found[i];
            let shape = self.day_ref().things[k].shape;
            let pulse = 0.75 + 0.25 * (t * 1.3 + i as f64).sin() as f32;
            let (colour, amount) = if found {
                ([1.0, 0.8, 0.5], 1.4)
            } else {
                ([1.0, 0.98, 0.9], 1.1 * pulse)
            };
            let size = 24.0;
            let amount = if hovered == Some(i) {
                amount * 1.4
            } else {
                amount
            };
            // Keeping company, the finds step aside with everything else.
            let amount = amount * (1.0 - self.keep.mix as f32);
            if amount < 0.01 {
                continue;
            }
            self.field.glow(x, y, 28.0, colour, 0.1 * amount);
            for stroke in outline(shape) {
                for pair in stroke.windows(2) {
                    let a = (x + pair[0].0 * size, y + pair[0].1 * size);
                    let b = (x + pair[1].0 * size, y + pair[1].1 * size);
                    self.field.line(a, b, colour, amount);
                }
            }
        }
        // A soft ring round whatever the pointer is over.
        if let Some(i) = hovered
            && let Some((x, y)) = self.day_spot(i)
        {
            for k in 0..14 {
                let a = k as f64 / 14.0 * std::f64::consts::TAU + t * 0.6;
                self.points.push(Point {
                    x: x + 30.0 * a.cos(),
                    y: y + 30.0 * a.sin(),
                    radius: 1.3,
                    color: [1.0, 0.85, 0.6],
                    alpha: 0.6,
                    halo: 0.5,
                });
            }
        }
    }

    /// How loud the music is while the day ends.
    pub(crate) fn day_music(&self, real: UnixMs) -> f64 {
        match self.day.as_ref().and_then(|d| d.ending) {
            Some(at) => 1.0 - smoothstep((real - at - 6_000) as f64 / (ENDING_MS - 6_000) as f64),
            None => 1.0,
        }
    }

    /// Whether the Tonight list shows, by day.
    pub(crate) fn day_list_shows(&self) -> bool {
        self.day.as_ref().is_some_and(|d| d.ending.is_none())
            && !self.keeping()
            && self.session.phase() == westering_core::session::Phase::Hunt
    }
}
