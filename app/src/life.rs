//! Life in the daytime scene: a tree and far woods, grass in the
//! foreground, and things that fly: small birds crossing, geese passing in
//! spring and autumn, swifts screaming round in summer, butterflies and bees
//! over the ground, midges under the tree, leaves coming down in autumn, and
//! a robin on the stone in winter. Everything that moves is worked out from
//! the clock alone, so it costs nothing to keep and never repeats in a way
//! anyone would notice.

use crate::field::noise3;
use crate::game::Game;
use crate::view::Silhouette;
use std::f64::consts::{PI, TAU};
use westering_core::ease::smoothstep;
use westering_core::time::UnixMs;
use westering_core::wisp::Season;

const INK: [f32; 3] = [0.1, 0.11, 0.14];

/// The top of the far hills at `x`, for a horizon at `hy`: anything in the
/// sky lower than this is behind them.
pub(crate) fn hill_at(x: f64, hy: f64) -> f64 {
    hy - 8.0 - 24.0 * noise3(x / 260.0, 0.5, 3.3)
}

/// A small stable random number for a thing and a salt, 0 to 1.
fn hash(k: u64, salt: u64) -> f64 {
    let mut x = k
        .wrapping_mul(0x9e37_79b9_7f4a_7c15)
        .wrapping_add(salt.wrapping_mul(0xbf58_476d_1ce4_e5b9));
    x ^= x >> 31;
    x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^= x >> 29;
    (x >> 11) as f64 / (1u64 << 53) as f64
}

/// Moves and turns a shape drawn round its own middle.
fn place(shape: &[(f64, f64)], x: f64, y: f64, angle: f64) -> Vec<(f64, f64)> {
    let (s, c) = angle.sin_cos();
    shape
        .iter()
        .map(|&(px, py)| (x + px * c - py * s, y + px * s + py * c))
        .collect()
}

fn stroke(path: Vec<(f64, f64)>, width: f32, color: [f32; 3], alpha: f32) -> Silhouette {
    Silhouette {
        path,
        width,
        color,
        alpha,
        filled: false,
    }
}

fn fill(path: Vec<(f64, f64)>, color: [f32; 3], alpha: f32) -> Silhouette {
    Silhouette {
        path,
        width: 0.0,
        color,
        alpha,
        filled: true,
    }
}

/// A bird seen from below, wings at `flap` (-1 down to 1 up).
fn bird(x: f64, y: f64, size: f64, flap: f64, alpha: f32) -> Silhouette {
    let s = size;
    let tip = -flap * s * 0.55;
    stroke(
        vec![
            (x - s, y + tip),
            (x - s * 0.45, y - s * 0.12 + tip * 0.3),
            (x, y + s * 0.12),
            (x + s * 0.45, y - s * 0.12 + tip * 0.3),
            (x + s, y + tip),
        ],
        // Thick enough to fill whole dots.
        3.0,
        INK,
        alpha,
    )
}

impl Game {
    fn season_now(&self) -> Option<Season> {
        self.day.as_ref().and_then(|d| d.season)
    }

    /// The near tree: where its foot is, and how tall it stands.
    fn tree(&self) -> (f64, f64, f64) {
        let (w, h, hy) = (self.camera.width, self.camera.height, self.horizon_y());
        (w * 0.15, hy + (h - hy) * 0.26, (h - hy) * 0.62)
    }

    fn hill(&self, x: f64) -> f64 {
        hill_at(x, self.horizon_y())
    }

    /// Paints the still parts of the scene into the base layer.
    pub(crate) fn day_scenery(&mut self, light: f32) {
        let season = self.season_now();
        let (w, h, hy) = (self.camera.width, self.camera.height, self.horizon_y());
        let (tx, ty, th) = self.tree();
        let far: Vec<(f64, f64, f64)> = (0..6)
            .map(|k| {
                let x = w * (0.34 + 0.12 * k as f64) + (hash(k, 1) - 0.5) * 60.0;
                let r = 7.0 + 7.0 * hash(k, 2);
                (x, self.hill(x) - r * 0.4, r)
            })
            .collect();
        let tufts: Vec<(f64, f64, f64)> = (0..44)
            .map(|k| {
                let d = 0.5 + 0.5 * hash(k, 3);
                (w * hash(k, 4), hy + (h - hy) * d, 5.0 + 9.0 * d)
            })
            .collect();
        let (cols, rows, pitch) = (self.field.cols, self.field.rows, self.field.pitch as f64);
        let base = self.field.base_mut();
        let mut set = |x: f64, y: f64, colour: [f32; 3], amount: f32| {
            let c = (x / pitch - 0.5).round();
            let r = (y / pitch - 0.5).round();
            if c < 0.0 || r < 0.0 || c as usize >= cols || r as usize >= rows {
                return;
            }
            let a = amount * light;
            base[r as usize * cols + c as usize] = [colour[0] * a, colour[1] * a, colour[2] * a];
        };
        let line = |a: (f64, f64),
                    b: (f64, f64),
                    colour: [f32; 3],
                    amount: f32,
                    set: &mut dyn FnMut(f64, f64, [f32; 3], f32)| {
            let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
            let steps = (len / (pitch * 0.6)).ceil().max(1.0) as usize;
            for i in 0..=steps {
                let t = i as f64 / steps as f64;
                set(a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t, colour, amount);
            }
        };

        // Far woods along the hills.
        // Hazy with distance: a little bluer and paler than the tree.
        let wood = if season == Some(Season::Winter) {
            [0.46, 0.44, 0.44]
        } else {
            [0.3, 0.46, 0.42]
        };
        for &(fx, fy, r) in &far {
            // Each clump is a few crowns side by side, ragged at the edges.
            let crowns = [(-0.7, 0.25, 0.7), (0.0, 0.0, 1.0), (0.75, 0.2, 0.75)];
            let mut y = fy - r * 1.2;
            while y < fy + r {
                let mut x = fx - r * 1.6;
                while x < fx + r * 1.6 {
                    let q = crowns
                        .iter()
                        .map(|&(cx, cy, s)| {
                            ((x - fx - cx * r).powi(2) + (y - fy - cy * r).powi(2)).sqrt() / (r * s)
                        })
                        .fold(f64::MAX, f64::min);
                    let rough = noise3(x / 2.5, y / 2.5, 4.4);
                    if (q < 0.8 || (q < 1.05 && rough > 0.5)) && y < self_hill(x, hy) {
                        set(x, y, wood, (0.7 + 0.15 * rough) as f32);
                    }
                    x += pitch;
                }
                y += pitch;
            }
        }

        // Grass in the foreground.
        let blade = match season {
            Some(Season::Winter) => [0.62, 0.68, 0.58],
            Some(Season::Autumn) => [0.74, 0.72, 0.4],
            _ => [0.52, 0.8, 0.36],
        };
        for (k, &(gx, gy, tall)) in tufts.iter().enumerate() {
            for b in 0..4 {
                let lean = (b as f64 - 1.5) * 2.2 + (hash(k as u64, b + 7) - 0.5) * 3.0;
                let top = (gx + lean, gy - tall * (0.7 + 0.3 * hash(k as u64, b + 11)));
                line((gx + b as f64 - 1.5, gy), top, blade, 0.7, &mut set);
            }
        }

        // The near tree: a trunk, and its crown for the season.
        let trunk = [0.42, 0.32, 0.22];
        let top = ty - th * 0.55;
        let mut y = top;
        while y <= ty {
            let widen = 1.0 + 0.5 * ((y - top) / (ty - top)).powi(3);
            let half = th * 0.035 * widen;
            let mut x = tx - half;
            while x <= tx + half {
                set(x, y, trunk, if x > tx { 0.45 } else { 0.6 });
                x += pitch;
            }
            y += pitch;
        }
        let (cx, cy, rc) = (tx, ty - th * 0.74, th * 0.34);
        if season == Some(Season::Winter) {
            // Bare branches.
            for k in 0..6 {
                let a = -PI / 2.0 + (k as f64 - 2.5) * 0.36;
                let start = (tx, top + th * 0.05 * (k % 2) as f64);
                let end = (start.0 + a.cos() * rc * 1.1, start.1 + a.sin() * rc * 1.1);
                line(start, end, trunk, 0.6, &mut set);
                for side in [-0.5, 0.5] {
                    let b = a + side;
                    let mid = (
                        start.0 + (end.0 - start.0) * 0.6,
                        start.1 + (end.1 - start.1) * 0.6,
                    );
                    let twig = (mid.0 + b.cos() * rc * 0.45, mid.1 + b.sin() * rc * 0.45);
                    line(mid, twig, trunk, 0.5, &mut set);
                }
            }
        } else {
            let blobs = [
                (0.0, 0.0),
                (-0.55, 0.15),
                (0.55, 0.12),
                (-0.3, -0.42),
                (0.32, -0.38),
                (0.0, 0.32),
            ];
            let mut y = cy - rc * 1.2;
            while y < cy + rc * 1.0 {
                let mut x = cx - rc * 1.4;
                while x < cx + rc * 1.4 {
                    let n = noise3(x / 9.0, y / 9.0, 2.2);
                    let q = blobs
                        .iter()
                        .map(|&(bx, by)| {
                            ((x - cx - bx * rc).powi(2) + (y - cy - by * rc).powi(2)).sqrt()
                                / (rc * 0.6 * (0.8 + 0.4 * n))
                        })
                        .fold(f64::MAX, f64::min);
                    // Solid within, leaves scattered at the edge.
                    let rough = noise3(x / 2.2, y / 2.2, 6.1);
                    if q < 0.85 || (q < 1.15 && rough > 0.52) {
                        let fine = noise3(x / 3.0, y / 3.0, 7.7);
                        let colour = match season {
                            Some(Season::Spring) if fine > 0.7 => [1.0, 0.82, 0.88],
                            Some(Season::Spring) => [0.5, 0.76, 0.38],
                            Some(Season::Summer) => [0.34, 0.62, 0.28],
                            Some(Season::Autumn) => {
                                if n < 0.4 {
                                    [0.9, 0.36, 0.12]
                                } else if n < 0.62 {
                                    [0.94, 0.6, 0.16]
                                } else {
                                    [0.96, 0.8, 0.3]
                                }
                            }
                            _ => [0.34, 0.62, 0.3],
                        };
                        // Lit from above, shaded underneath.
                        let shade = 1.0 + 0.2 * (-(y - cy) / rc).max(0.0)
                            - 0.3 * ((y - cy) / rc).max(0.0)
                            - 0.12 * ((x - cx) / rc).max(0.0);
                        set(x, y, colour, (0.85 * shade * (0.8 + 0.3 * fine)) as f32);
                    }
                    x += pitch;
                }
                y += pitch;
            }
        }

        fn self_hill(x: f64, hy: f64) -> f64 {
            hy - 8.0 - 24.0 * noise3(x / 260.0, 0.5, 3.3) + 4.0
        }
    }

    /// How far a point is hidden behind the near tree, 0 to 1: its crown
    /// in leaf, or its trunk.
    fn tree_cover(&self, x: f64, y: f64) -> f64 {
        let (tx, ty, th) = self.tree();
        let top = ty - th * 0.55;
        if (x - tx).abs() < th * 0.05 && (top..=ty).contains(&y) {
            return 1.0;
        }
        if self.season_now() == Some(Season::Winter) {
            return 0.0;
        }
        let (cx, cy, rc) = (tx, ty - th * 0.74, th * 0.34);
        let q = [
            (0.0, 0.0),
            (-0.55, 0.15),
            (0.55, 0.12),
            (-0.3, -0.42),
            (0.32, -0.38),
            (0.0, 0.32),
        ]
        .iter()
        .map(|&(bx, by)| {
            ((x - cx - bx * rc).powi(2) + (y - cy - by * rc).powi(2)).sqrt() / (rc * 0.6)
        })
        .fold(f64::MAX, f64::min);
        1.0 - smoothstep((q - 0.75) / 0.4)
    }

    /// A small bird crossing, if one is: where it is now.
    fn small_bird(&self, k: u64, t: f64) -> Option<(f64, f64, f64, f64)> {
        let (w, hy) = (self.camera.width, self.horizon_y());
        let period = 34.0 + 13.0 * k as f64 + 9.0 * hash(k, 20);
        let phase = ((t + 17.0 * k as f64) / period).fract();
        let crossing = 0.55;
        if phase > crossing {
            return None;
        }
        let u = phase / crossing;
        let lap = ((t + 17.0 * k as f64) / period).floor() as u64;
        let rightwards = hash(k, lap) > 0.5;
        let x = if rightwards {
            -40.0 + (w + 80.0) * u
        } else {
            w + 40.0 - (w + 80.0) * u
        };
        let height = 0.18 + 0.5 * hash(k + 3, lap);
        // Small birds fly in bounds: a few beats, then wings shut, dipping.
        let bound = (u * 9.0 * PI).sin();
        let y = hy * height + bound * 5.0;
        let beating = (t * 0.9 + k as f64).fract() < 0.6;
        let flap = if beating {
            (t * 15.0 + k as f64).sin()
        } else {
            -0.2
        };
        let size = 5.0 + 3.0 * (1.0 - height);
        Some((x, y, size, flap))
    }

    /// Where a bird is flying now, for the wisp to watch.
    pub(crate) fn day_bird(&self, real: UnixMs) -> Option<(f64, f64)> {
        let t = real as f64 / 1000.0;
        (0..3).find_map(|k| {
            self.small_bird(k, t)
                .map(|(x, y, _, _)| (x, y))
                .filter(|&(x, y)| self.tree_cover(x, y) < 0.5)
        })
    }

    /// Everything alive, this moment.
    pub(crate) fn day_life(&self, real: UnixMs) -> Vec<Silhouette> {
        let Some(day) = &self.day else {
            return Vec::new();
        };
        let t = real as f64 / 1000.0;
        let season = day.season;
        let (w, h, hy) = (self.camera.width, self.camera.height, self.horizon_y());
        let fade = (1.0 - self.day_fade(real)) as f32;
        let mut out = Vec::new();

        // Small birds.
        let birds = if season == Some(Season::Winter) { 2 } else { 3 };
        for k in 0..birds {
            if let Some((x, y, size, flap)) = self.small_bird(k, t) {
                let seen = (1.0 - self.tree_cover(x, y)) as f32;
                out.push(bird(x, y, size, flap, 0.85 * fade * seen));
            }
        }

        // Geese going over in a V, spring and autumn.
        if matches!(season, Some(Season::Spring | Season::Autumn)) {
            let period = 140.0;
            let phase = (t / period).fract();
            if phase < 0.4 {
                let u = phase / 0.4;
                let lap = (t / period).floor() as u64;
                let rightwards = hash(9, lap) > 0.5;
                let x0 = if rightwards {
                    -120.0 + (w + 240.0) * u
                } else {
                    w + 120.0 - (w + 240.0) * u
                };
                let dir = if rightwards { -1.0 } else { 1.0 };
                let y0 = hy * (0.14 + 0.1 * hash(10, lap));
                for i in 0..7u32 {
                    let rank = i.div_ceil(2) as f64;
                    let side = if i % 2 == 0 { 1.0 } else { -1.0 };
                    let x = x0 + dir * rank * 16.0;
                    let y = y0 + side * rank * 9.0;
                    let flap = (t * 4.5 + i as f64 * 0.7).sin();
                    let seen = (1.0 - self.tree_cover(x, y)) as f32;
                    out.push(bird(x, y, 7.5, flap, 0.8 * fade * seen));
                }
            }
        }

        // Swifts, fast and high, in summer.
        if season == Some(Season::Summer) {
            for k in 0..2 {
                let a = t * (0.55 + 0.1 * k as f64) + k as f64 * 2.0;
                let x = w * 0.5 + (w * 0.35) * a.sin();
                let y = hy * (0.35 + 0.1 * k as f64) + hy * 0.15 * (2.0 * a).sin();
                let (vx, vy) = (a.cos(), 2.0 * (2.0 * a).cos() * 0.4);
                let angle = vy.atan2(vx);
                let s = 8.0;
                let scythe = [
                    (-s * 0.2, -s),
                    (s * 0.15, -s * 0.35),
                    (s * 0.2, 0.0),
                    (s * 0.15, s * 0.35),
                    (-s * 0.2, s),
                ];
                let seen = (1.0 - self.tree_cover(x, y)) as f32;
                out.push(stroke(
                    place(&scythe, x, y, angle),
                    3.0,
                    INK,
                    0.85 * fade * seen,
                ));
            }
        }

        // What's on the wing goes by the month: butterflies from spring into
        // autumn, bees a little longer, midges in summer; all year round
        // where there's no winter.
        let m = day.month;
        let always = season.is_none();
        // Butterflies and bees go to roost as the light goes, well before
        // dark; midges carry on into dusk.
        let daylight = smoothstep((day.today.sun_alt - 2.0) / 8.0) as f32;
        // Butterflies over the grass, wandering and fluttering.
        if (always || (4..=10).contains(&m)) && daylight > 0.0 {
            for k in 0..2u64 {
                let x = w * (0.3 + 0.45 * hash(k, 30))
                    + w * 0.12 * (t * 0.23 + k as f64).sin()
                    + 30.0 * (t * 0.61 + k as f64 * 3.0).sin();
                let y = hy
                    + (h - hy) * (0.45 + 0.2 * hash(k, 31))
                    + 25.0 * (t * 0.37 + k as f64).sin()
                    + 8.0 * (t * 2.3 + k as f64).sin();
                let open = (t * 11.0 + k as f64).sin().abs();
                let colour = if k == 0 {
                    [0.98, 0.97, 0.9]
                } else {
                    [0.96, 0.62, 0.22]
                };
                for side in [-1.0, 1.0] {
                    let o = open * side;
                    let wing = vec![
                        (x, y),
                        (x + 11.0 * o, y - 8.5),
                        (x + 12.5 * o, y + 1.5),
                        (x + 6.0 * o, y + 8.5),
                    ];
                    out.push(fill(wing, colour, 0.92 * fade * daylight));
                }
            }
        }
        if (always || (3..=10).contains(&m)) && daylight > 0.0 {
            // Bees working round the finds on the ground.
            for i in 0..day.finds.len() {
                if !matches!(day.finds[i], crate::day::Find::Ground(_)) {
                    continue;
                }
                let Some((fx, fy)) = self.day_spot(i) else {
                    continue;
                };
                if fy < hy {
                    continue;
                }
                let k = i as u64;
                let a = t * (1.7 + 0.4 * hash(k, 40)) + k as f64;
                let x = fx + 34.0 * a.cos() + 4.0 * (t * 23.0 + k as f64).sin();
                let y = fy - 20.0 + 14.0 * (a * 1.3).sin() + 3.0 * (t * 19.0).cos();
                let body: Vec<(f64, f64)> = (0..8)
                    .map(|j| {
                        let b = j as f64 / 8.0 * TAU;
                        (x + 4.0 * b.cos(), y + 2.8 * b.sin())
                    })
                    .collect();
                out.push(fill(body, [0.28, 0.22, 0.08], 0.9 * fade * daylight));
                let buzz = (t * 40.0).sin() * 2.0;
                out.push(stroke(
                    vec![
                        (x - 4.5, y - 4.0 - buzz),
                        (x, y - 1.5),
                        (x + 4.5, y - 4.0 - buzz),
                    ],
                    2.2,
                    [0.95, 0.97, 1.0],
                    0.7 * fade * daylight,
                ));
            }
        }

        // Midges dancing under the tree on summer days.
        if always || (5..=9).contains(&m) {
            let (tx, ty, th) = self.tree();
            let (cx, cy) = (tx + th * 0.28, ty - th * 0.25);
            for k in 0..12u64 {
                let x = cx + 16.0 * (t * (1.1 + hash(k, 50)) + k as f64).sin();
                let y = cy + 12.0 * (t * (1.7 + hash(k, 51)) + 2.0 * k as f64).cos();
                out.push(fill(
                    vec![(x - 1.7, y), (x, y - 1.7), (x + 1.7, y), (x, y + 1.7)],
                    INK,
                    0.6 * fade,
                ));
            }
        }

        // Leaves coming down in autumn.
        if season == Some(Season::Autumn) {
            let (tx, ty, th) = self.tree();
            let colours = [[0.92, 0.5, 0.14], [0.86, 0.32, 0.1], [0.95, 0.76, 0.26]];
            for k in 0..5u64 {
                let period = 7.0 + 4.0 * hash(k, 60);
                let phase = ((t + 3.1 * k as f64) / period).fract();
                let lap = ((t + 3.1 * k as f64) / period).floor() as u64;
                let start = tx + th * 0.34 * (hash(k, lap) * 2.0 - 1.0);
                let fall = th * (0.55 + 0.35 * hash(k + 1, lap));
                let x = start + 14.0 * (phase * TAU * 1.5).sin() + phase * 30.0;
                let y = ty - th * 0.7 + fall * phase;
                let angle = (phase * TAU * 2.0).sin() * 0.9;
                let leaf = [(-5.0, 0.0), (0.0, -3.2), (5.0, 0.0), (0.0, 3.2)];
                let a = if phase > 0.85 {
                    ((1.0 - phase) / 0.15) as f32
                } else {
                    1.0
                };
                out.push(fill(
                    place(&leaf, x, y, angle),
                    colours[(k % 3) as usize],
                    0.95 * a * fade,
                ));
            }
        }

        // A robin on the stone in winter, bobbing now and then.
        if season == Some(Season::Winter) {
            let (sx, sy, sh) = self.stone();
            let bob = if (t * 0.4).fract() < 0.08 { 1.5 } else { 0.0 };
            let (x, y) = (sx, sy - sh - 3.0 + bob);
            let body: Vec<(f64, f64)> = (0..10)
                .map(|j| {
                    let b = j as f64 / 10.0 * TAU;
                    (x + 5.0 * b.cos(), y + 4.0 * b.sin())
                })
                .collect();
            out.push(fill(body, [0.3, 0.24, 0.18], 0.95 * fade));
            out.push(fill(
                (0..8)
                    .map(|j| {
                        let b = j as f64 / 8.0 * TAU;
                        (x + 3.8 + 2.4 * b.cos(), y - 3.5 + 2.3 * b.sin())
                    })
                    .collect(),
                [0.3, 0.24, 0.18],
                0.95 * fade,
            ));
            out.push(fill(
                (0..8)
                    .map(|j| {
                        let b = j as f64 / 8.0 * TAU;
                        (x + 3.0 + 2.2 * b.cos(), y + 0.5 + 2.4 * b.sin())
                    })
                    .collect(),
                [0.9, 0.42, 0.18],
                0.95 * fade,
            ));
            out.push(stroke(
                vec![(x - 4.5, y), (x - 8.5, y - 3.0)],
                2.0,
                [0.3, 0.24, 0.18],
                0.95 * fade,
            ));
        }
        out
    }
}
