//! How the wisp gets about: a small gathering crouch, a springy chase
//! towards wherever it's going, a wide loop when it arrives at something in
//! the sky to point it out, a wandering bob while it hovers that never quite
//! repeats, and a trail of embers behind it. Its body stretches as it flies
//! and wobbles when it lands.

use crate::view::Point;
use westering_core::ease::smoothstep;
use westering_core::time::UnixMs;

struct Ember {
    x: f64,
    y: f64,
    dx: f64,
    dy: f64,
    born: f64,
    life: f64,
    size: f32,
}

pub struct Flight {
    pub x: f64,
    pub y: f64,
    vx: f64,
    vy: f64,
    /// Where it's heading, and when it got there.
    goal: Option<(f64, f64)>,
    arrived: Option<f64>,
    /// A loop around what it's pointing at: centre, radius, start time.
    loop_round: Option<(f64, f64, f64, f64)>,
    embers: Vec<Ember>,
    last_ember: f64,
    seed: u64,
    pub home: bool,
    /// Until when it gathers itself before setting off, and which way it's
    /// about to go.
    gather_until: f64,
    heading: f64,
    /// How squashed (below 0) or stretched (above 0) it is, on a spring.
    squash: f64,
    squash_v: f64,
    /// Drifting rather than darting: a softer spring, a lower top speed
    /// and a sparser trail, for keeping someone company without fuss.
    pub gentle: bool,
}

/// Smooth noise: a gentle wander through -1 to 1 that never repeats, from
/// random slopes at whole numbers of `t`.
fn noise(t: f64, seed: u64) -> f64 {
    let slope = |i: i64| {
        let mut x = (i as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
            ^ seed.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        x ^= x >> 31;
        x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
        x ^= x >> 29;
        (x >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
    };
    let i = t.floor();
    let f = t - i;
    let (a, b) = (slope(i as i64) * f, slope(i as i64 + 1) * (f - 1.0));
    let u = smoothstep(f);
    (a + (b - a) * u) * 2.0
}

const SPRING: f64 = 16.0;
const DAMPING: f64 = 6.2;
const TOP_SPEED: f64 = 1300.0;
const LOOP_MS: f64 = 1100.0;
/// The crouch before it sets off from a standstill.
const GATHER_MS: f64 = 170.0;

impl Flight {
    pub fn new(x: f64, y: f64) -> Flight {
        Flight {
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            goal: None,
            arrived: None,
            loop_round: None,
            embers: Vec::new(),
            last_ember: 0.0,
            seed: 0x2545_f491_4f6c_dd1d,
            home: true,
            gather_until: 0.0,
            heading: 0.0,
            squash: 0.0,
            squash_v: 0.0,
            gentle: false,
        }
    }

    fn random(&mut self) -> f64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        (self.seed >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn speed(&self) -> f64 {
        (self.vx * self.vx + self.vy * self.vy).sqrt()
    }

    /// Leaning into the direction of travel, -1 to 1.
    pub fn lean(&self) -> f64 {
        (self.vx / 700.0).clamp(-1.0, 1.0)
    }

    /// Moves on by `dt` seconds towards `goal`, looping once around
    /// `pointing` the first time it arrives near it.
    pub fn step(
        &mut self,
        now: UnixMs,
        dt: f64,
        goal: (f64, f64),
        pointing: Option<(f64, f64)>,
        circle: bool,
        bob: bool,
    ) {
        let t = now as f64;
        if self.goal != Some(goal) {
            let moved = self
                .goal
                .is_none_or(|g| (g.0 - goal.0).abs() + (g.1 - goal.1).abs() > 40.0);
            if moved {
                self.arrived = None;
                self.loop_round = None;
                // From a standstill it looks where it's going and gathers
                // itself first.
                if self.speed() < 120.0 && self.goal.is_some() {
                    self.gather_until = t + GATHER_MS;
                    self.heading = ((goal.0 - self.x) / 120.0).clamp(-1.0, 1.0);
                }
            }
            self.goal = Some(goal);
        }
        let near = ((self.x - goal.0).powi(2) + (self.y - goal.1).powi(2)).sqrt() < 30.0;
        if near && self.arrived.is_none() {
            self.arrived = Some(t);
            // Landing: a soft squash that wobbles out.
            self.squash_v -= 0.9 * (self.speed() / 400.0).clamp(0.3, 1.0);
            if let Some((px, py)) = pointing.filter(|_| circle) {
                // Wide enough that the glow never covers what it's showing.
                let r = ((goal.0 - px).powi(2) + (goal.1 - py).powi(2))
                    .sqrt()
                    .clamp(100.0, 150.0);
                self.loop_round = Some((px, py, r, t));
            }
        }
        // Where it's aiming this moment: the goal, or a point on the loop.
        let (mut ax, mut ay) = goal;
        if let Some((cx, cy, r, start)) = self.loop_round {
            let f = (t - start) / LOOP_MS;
            if f < 1.0 {
                let begin = (goal.1 - cy).atan2(goal.0 - cx);
                let a = begin + f * std::f64::consts::TAU;
                ax = cx + r * a.cos();
                ay = cy + r * a.sin();
            } else {
                self.loop_round = None;
            }
        }
        if bob {
            ax += noise(t / 1500.0, 1) * 6.0 + noise(t / 610.0, 2) * 2.0;
            ay += noise(t / 1200.0, 3) * 5.0 + noise(t / 530.0, 4) * 1.5;
        }
        let gathering = t < self.gather_until;
        if gathering {
            (ax, ay) = (self.x, self.y + 4.0);
        }
        let (spring, damping, top) = if self.gentle {
            (SPRING * 0.3, DAMPING * 0.55, TOP_SPEED * 0.3)
        } else {
            (SPRING, DAMPING, TOP_SPEED)
        };
        let (fx, fy) = (
            spring * (ax - self.x) - damping * self.vx,
            spring * (ay - self.y) - damping * self.vy,
        );
        self.vx += fx * dt;
        self.vy += fy * dt;
        let speed = self.speed();
        if speed > top {
            self.vx *= top / speed;
            self.vy *= top / speed;
        }
        self.x += self.vx * dt;
        self.y += self.vy * dt;

        // Stretched along its way while it flies, squashed as it gathers.
        let speed = self.speed();
        let target = if gathering {
            -0.1
        } else if speed > 1.0 {
            0.13 * (speed / TOP_SPEED).min(1.0).sqrt() * (self.vy.abs() - self.vx.abs()) / speed
        } else {
            0.0
        };
        let pull = 320.0 * (target - self.squash) - 13.0 * self.squash_v;
        self.squash_v += pull * dt;
        self.squash = (self.squash + self.squash_v * dt).clamp(-0.25, 0.25);

        // Embers: a stream while it flies, a spark now and then while it
        // hovers, and none once it's home on its moss.
        let speed = self.speed();
        let every = match (speed > 60.0, self.gentle) {
            (true, false) => 14.0,
            (true, true) => 70.0,
            _ => 260.0,
        };
        if self.home && speed <= 60.0 {
            self.last_ember = t;
        }
        while t - self.last_ember > every {
            self.last_ember = if t - self.last_ember > 500.0 {
                t
            } else {
                self.last_ember + every
            };
            let (jx, jy) = (self.random() - 0.5, self.random() - 0.5);
            let life = 700.0 + self.random() * 600.0;
            let size = 1.3 + self.random() as f32 * 1.6;
            self.embers.push(Ember {
                x: self.x + jx * 10.0,
                y: self.y + 8.0 + jy * 8.0,
                dx: -self.vx * 0.08 + jx * 30.0,
                dy: -self.vy * 0.08 - 26.0 + jy * 20.0,
                born: t,
                life,
                size,
            });
        }
        for e in &mut self.embers {
            e.x += e.dx * dt;
            e.y += e.dy * dt;
            e.dx *= 1.0 - dt * 1.5;
        }
        self.embers.retain(|e| t - e.born < e.life);
    }

    /// Width and height factors for the body.
    pub fn body(&self) -> (f64, f64) {
        (1.0 - self.squash * 0.7, 1.0 + self.squash)
    }

    /// Where it's about to go, while it gathers itself to set off.
    pub fn heading(&self, now: UnixMs) -> Option<f64> {
        ((now as f64) < self.gather_until).then_some(self.heading)
    }

    /// Whether its body is still settling after a take-off or landing.
    pub fn wobbling(&self) -> bool {
        self.squash.abs() > 0.004 || self.squash_v.abs() > 0.02
    }

    pub fn has_embers(&self) -> bool {
        !self.embers.is_empty()
    }

    /// Settles straight onto a spot, without flying there.
    pub fn place(&mut self, x: f64, y: f64) {
        self.x = x;
        self.y = y;
        self.vx = 0.0;
        self.vy = 0.0;
    }

    /// The trail, as small glowing points.
    pub fn embers(&self, now: UnixMs, alpha: f32) -> Vec<Point> {
        let t = now as f64;
        self.embers
            .iter()
            .map(|e| {
                let f = ((t - e.born) / e.life).clamp(0.0, 1.0) as f32;
                // Gold to ember orange, dimming as it goes.
                let color = [1.0, 0.86 - 0.36 * f, 0.55 - 0.4 * f];
                Point {
                    x: e.x,
                    y: e.y,
                    radius: e.size * (1.0 - 0.6 * f),
                    color,
                    alpha: (1.0 - f) * 0.9 * alpha,
                    halo: 0.8 * (1.0 - f),
                }
            })
            .collect()
    }
}
