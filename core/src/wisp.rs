//! The wisp: a flame-shaped sprite with a friendly face, resting on a tuft of
//! moss. It comes out at night to show someone the sky, and goes back to
//! sleep when the evening is over.
//!
//! This is the whole of it — its moods, its motion, its face and its moss —
//! drawn on a `Canvas`, so none of it depends on how it reaches the screen.
//! What it cannot know it is told: its mode, the frame's time, and whether
//! animation is wanted.
//!
//! Its colours come from `core/data/wisp.toml`; its face, motion and moss
//! are set by the constants below. It asks to be redrawn only as often as
//! what it is doing needs, and a still wisp (reduced motion) only when its
//! mode changes.

use std::f64::consts::{PI, TAU};

use crate::canvas::{Canvas, Paint, stop};
use crate::ease::smoothstep;
use crate::look::Look;
use crate::oklab::Rgb;

/// How the wisp feels about the moment, which sets its face.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Asleep.
    Away,
    /// Calm.
    Resting,
    /// Delighted.
    Nourishing,
    /// Curious.
    Holding,
}

/// The time of year where the user is, shown on the moss.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Season {
    Winter,
    Spring,
    Summer,
    Autumn,
}

impl Season {
    pub fn name(self) -> &'static str {
        match self {
            Season::Winter => "winter",
            Season::Spring => "spring",
            Season::Summer => "summer",
            Season::Autumn => "autumn",
        }
    }
}

/// Feelings that colour whatever mode it's in. Each eases in and out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Feel {
    /// Someone is writing: it keeps still, leans in and looks at them.
    pub listening: bool,
    /// Something warm has just been said: it glows and its cheeks colour.
    pub moved: bool,
    /// Keeping someone company through something heavy: softer, quieter.
    pub tender: bool,
    /// It's late: heavy lids and a slower breath.
    pub sleepy: bool,
}

/// Small things it does now and then, once through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gesture {
    /// A little hop on the spot.
    Hop,
    /// Tall and thin for a moment, eyes shut, then back.
    Stretch,
    /// A big slow yawn.
    Yawn,
    /// A small dip, as if to say yes.
    Nod,
    /// Looks down at its moss and bobs, as if tidying it.
    Tend,
    /// A warm swell of light.
    Glow,
}

impl Gesture {
    fn length(self) -> f64 {
        match self {
            Gesture::Hop => 700.0,
            Gesture::Stretch => 1700.0,
            Gesture::Yawn => 2000.0,
            Gesture::Nod => 520.0,
            Gesture::Tend => 2600.0,
            Gesture::Glow => 1100.0,
        }
    }
}

// Feel constants. Alternatives are noted beside some of them.
/// Body radius at rest, in CSS pixels: about 27 px across, which fits a
/// 56 px nook with its glow.
const BODY_RADIUS: f64 = 13.338;
/// A slow breath of about six seconds at rest.
const BREATH_MS: f64 = 6000.0;
/// How much the body swells with each breath: 0.1 made the face bounce.
const BREATH_DEPTH: f64 = 0.05;
/// How much the light swells with it.
const BREATH_GLOW: f64 = 0.07;
/// Awake and at ease, it bobs this far, in CSS pixels.
const BOB: f64 = 1.8;
/// The face at ease: eyes all but wide open, a broad smile, warm cheeks,
/// and brows lifted a little.
const EYES_OPEN: f64 = 0.982_857_142_857;
const SMILE: f64 = 0.941_666_666_667;
const CHEEKS: f64 = 0.93;
const BROW_LIFT: f64 = 0.36;
/// A blink lasts this long, and comes every 2.5-7 s.
const BLINK_MS: f64 = 170.0;
/// Eyes open less than this lose their brows, and a face happier than this
/// smiles with its eyes: either way a blink there can't be seen.
const BROWLESS: f64 = 0.05;
const BEAMING: f64 = 0.5;
/// Noticing a change of mode: a glance aside and a small hop, so the user
/// sees at once that the wisp registered it.
const NOTICE_MS: f64 = 1100.0;
const NOTICE_HOP: f64 = 3.0;
/// Eyebrows: how far above the eyes they sit, and how much a mood lifts or
/// lowers them. They carry most of what the face says, so a mood reads from
/// across the room rather than only close up.
const BROW_ABOVE: f64 = 0.36;
const BROW_TRAVEL: f64 = 0.16;
/// Brows stay level and only move up or down: tilting either end turns the
/// face sad (inner end up) or stern (inner end down), and it is never either.
const BROW_ARCH: f64 = 0.06;
/// At night the glow warms by this much and the breath slows by this
/// factor.
const NIGHT_WARMTH: f64 = 0.35;
const NIGHT_BREATH: f64 = 1.4;
const NIGHT_GLOW: Rgb = Rgb(0.96, 0.66, 0.36);
/// Frames per second by what the wisp is doing. Each frame per second costs
/// about 0.11% of a core. A dozing wisp breathes at 2; resting, 4; releasing
/// motes or notes, 6. Blinks, gestures and changes of mood get short bursts
/// of 20.
const DOZING_FPS: f64 = 2.0;
const RESTING_FPS: f64 = 4.0;
const LIVELY_FPS: f64 = 6.0;
const BURST_FPS: f64 = 20.0;
/// Nourishing: a mote now and then, not a fountain.
const MOTE_EVERY_MS: f64 = 1400.0;
/// Humming: a note drifts up now and then.
const NOTE_EVERY_MS: f64 = 4200.0;
/// Dozing: a small z drifts up now and then.
const Z_EVERY_MS: f64 = 3200.0;
/// The moss: the share of the nook's width its mound spans, and its floor
/// and height.
const MOSS_SPAN: f64 = 0.74;
const MOSS_FLOOR: f64 = 7.0;
const MOSS_HEIGHT: f64 = 8.0;
/// Moss colour: fresh green.
const MOSS_FRESH: Rgb = Rgb(0.49, 0.62, 0.34);
/// The seasons on the moss: winter's frost, a spring flower, an autumn leaf.
const FROST: Rgb = Rgb(0.86, 0.93, 1.0);
const PETAL: Rgb = Rgb(1.0, 0.96, 0.84);
const LEAF: Rgb = Rgb(0.86, 0.5, 0.2);
/// The face's dark ink.
const FACE_INK: Rgb = Rgb(0.17, 0.13, 0.10);
/// Warm cheeks: amber, never pink or red.
const BLUSH: Rgb = Rgb(0.95, 0.62, 0.35);

struct Particle {
    born: f64,
    life: f64,
    x: f64,
    y: f64,
    dx: f64,
    dy: f64,
    size: f64,
    kind: ParticleKind,
}

#[derive(Clone, Copy, PartialEq)]
enum ParticleKind {
    Mote,
    Z,
    /// A little note, drifting up while it hums along to music.
    Note,
}

/// Everything the wisp is doing, and everything it is about to do.
pub struct Wisp {
    light: Look,
    mode: Mode,
    /// 0 = normal, 1 = happily closed eyes; eased.
    happy: f64,
    /// 0 = awake, 1 = asleep; eased.
    sleep: f64,
    /// The mode the wisp last reacted to, and when it noticed the change.
    noticed: Mode,
    notice_start: f64,
    /// 0-1, eased: how calm or curious it looks.
    calm: f64,
    curious: f64,
    glance: f64,
    glance_target: f64,
    next_glance: f64,
    next_blink: f64,
    particles: Vec<Particle>,
    last_tick: Option<f64>,
    last_mote: f64,
    last_z: f64,
    seed: u64,
    width: f64,
    height: f64,
    /// Out of its nook: drawn in the middle of its canvas with no moss.
    flying: bool,
    /// Leaning into its flight, -1 to 1.
    lean: f64,
    /// Where it's looking when it has somewhere to look, -1 to 1.
    look: Option<f64>,
    /// Up (-1) or down (1), when it has somewhere to look.
    look_y: Option<f64>,
    glance_y: f64,
    feel: Feel,
    /// 0-1, eased toward `feel`.
    listening: f64,
    moved: f64,
    tender: f64,
    sleepy: f64,
    /// One breath, in milliseconds, and how far through one it is (0-1).
    breath_ms: f64,
    breath: f64,
    /// What it's doing, and when it began.
    gesture: Option<(Gesture, f64)>,
    /// Squashed or stretched by its flight: width and height factors.
    body: (f64, f64),
    season: Option<Season>,
    /// Breathing in time with someone: the breath's phase, 0 to 1.
    paced: Option<f64>,
    /// The breath's swell is left to whoever shows the picture, who scales
    /// it every frame; the picture itself is drawn at rest size.
    breath_outside: bool,
    /// The breath's pace as last drawn, slowed for sleep and night.
    breath_slow: f64,
    humming: bool,
    last_note: f64,
}

impl Wisp {
    /// A wisp asleep in a nook of this size.
    pub fn new(width: f64, height: f64) -> Wisp {
        Wisp {
            light: Look::bundled(),
            mode: Mode::Away,
            happy: 0.0,
            sleep: 1.0,
            noticed: Mode::Away,
            notice_start: f64::NEG_INFINITY,
            calm: 0.0,
            curious: 0.0,
            glance: 0.0,
            glance_target: 0.0,
            next_glance: 0.0,
            next_blink: 0.0,
            particles: Vec::new(),
            last_tick: None,
            last_mote: 0.0,
            last_z: 0.0,
            seed: 0x9e37_79b9_7f4a_7c15,
            width,
            height,
            flying: false,
            lean: 0.0,
            look: None,
            look_y: None,
            glance_y: 0.0,
            feel: Feel::default(),
            listening: 0.0,
            moved: 0.0,
            tender: 0.0,
            sleepy: 0.0,
            breath_ms: BREATH_MS,
            breath: 0.0,
            gesture: None,
            body: (1.0, 1.0),
            season: None,
            paced: None,
            breath_outside: false,
            breath_slow: 1.0,
            humming: false,
            last_note: 0.0,
        }
    }

    /// Breathes in time with a pace set from outside (0 to 1 through a
    /// breath, in over the first two fifths), or its own when none.
    pub fn breathe_with(&mut self, phase: Option<f64>) {
        self.paced = phase;
    }

    /// Leaves the breath's swell to the caller, who asks `breath_scale`
    /// each frame, so the picture needn't be drawn again for it.
    pub fn set_breath_outside(&mut self, outside: bool) {
        self.breath_outside = outside;
    }

    /// How big the wisp is with this moment's breath, as a factor near 1,
    /// running on from the last drawing without drawing again.
    pub fn breath_scale(&self, now: f64) -> f64 {
        let phase = match self.paced {
            Some(p) => p,
            None => {
                let since = self.last_tick.map_or(0.0, |t| (now - t).max(0.0));
                (self.breath + since / (self.breath_ms * self.breath_slow)).fract()
            }
        };
        let depth = if self.paced.is_some() {
            BREATH_DEPTH * 3.0
        } else {
            BREATH_DEPTH
        };
        1.0 + breathing(phase) * depth * (1.0 - self.sleep * 0.5)
    }

    /// Humming along to music: now and then a note drifts up.
    pub fn set_humming(&mut self, humming: bool) {
        self.humming = humming;
    }

    /// The time of year, for the moss: none where the year has no seasons
    /// to speak of.
    pub fn set_season(&mut self, season: Option<Season>) {
        self.season = season;
    }

    /// What it's feeling, over and above its mode.
    pub fn set_feel(&mut self, feel: Feel) {
        self.feel = feel;
    }

    /// How long one breath takes, in milliseconds.
    pub fn set_breath(&mut self, ms: f64) {
        self.breath_ms = ms.clamp(3000.0, 15000.0);
    }

    /// Squashed or stretched: width and height factors, near 1.
    pub fn set_body(&mut self, width: f64, height: f64) {
        self.body = (width.clamp(0.7, 1.4), height.clamp(0.7, 1.4));
    }

    /// Looking up (-1) or down (1), or wherever it likes.
    pub fn set_look_up(&mut self, look: Option<f64>) {
        self.look_y = look.map(|l| l.clamp(-1.0, 1.0));
    }

    /// Starts a gesture, unless it's in the middle of one.
    pub fn gesture(&mut self, gesture: Gesture, now: f64) {
        if !self.gesturing(now) {
            self.gesture = Some((gesture, now));
        }
    }

    /// Whether it's in the middle of a gesture.
    pub fn gesturing(&self, now: f64) -> bool {
        self.gesture
            .is_some_and(|(g, start)| now >= start && now - start < g.length())
    }

    /// Out of the nook and flying, leaning into it and looking somewhere.
    pub fn set_flight(&mut self, flying: bool, lean: f64, look: Option<f64>) {
        self.flying = flying;
        self.lean = lean.clamp(-1.0, 1.0);
        self.look = look.map(|l| l.clamp(-1.0, 1.0));
    }

    /// Just the moss, on a canvas `width` by `height`.
    pub fn draw_moss_alone(&self, width: f64, height: f64, now: f64, canvas: &mut dyn Canvas) {
        draw_moss(canvas, width, height, self.happy, now, true, self.season);
    }

    /// The nook has been given a new size.
    pub fn resize(&mut self, width: f64, height: f64) {
        self.width = width;
        self.height = height;
    }

    /// Sets its mode. Says whether that changed anything, so a platform
    /// knows whether it is worth a redraw.
    pub fn update(&mut self, mode: Mode) -> bool {
        let changed = self.mode != mode;
        self.mode = mode;
        changed
    }

    /// Draw one frame, at `now` milliseconds. `moving` is whether the
    /// platform allows animation. Returns how long until the next frame is
    /// wanted, or `None` to rest.
    pub fn draw(&mut self, now: f64, moving: bool, canvas: &mut dyn Canvas) -> Option<f64> {
        draw(self, now, moving, canvas)
    }
}

/// A breath at `phase` (0-1), from -1 (out) to 1 (in): in over the first
/// two fifths, a longer, softer out.
fn breathing(phase: f64) -> f64 {
    if phase < 0.4 {
        -(phase / 0.4 * PI).cos()
    } else {
        ((phase - 0.4) / 0.6 * PI).cos()
    }
}

fn wander(t: f64, seed: f64) -> f64 {
    (t * 0.00031 + seed).sin() * 0.5
        + (t * 0.00073 + seed * 2.1).sin() * 0.3
        + (t * 0.00137 + seed * 3.7).sin() * 0.2
}

/// A small xorshift, for blinks, glances and particles.
fn random(wisp: &mut Wisp) -> f64 {
    let mut x = wisp.seed;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    wisp.seed = x;
    (x >> 11) as f64 / (1u64 << 53) as f64
}

fn ease_toward(value: f64, target: f64, dt: f64, time_ms: f64) -> f64 {
    value + (target - value) * (1.0 - (-dt / time_ms).exp())
}

/// Draw one frame. Returns how long until the next one is wanted.
fn draw(wisp: &mut Wisp, now: f64, moving: bool, cr: &mut dyn Canvas) -> Option<f64> {
    let dt = wisp
        .last_tick
        .map_or(16.0, |last| (now - last).clamp(0.0, 250.0));
    wisp.last_tick = Some(now);

    let asleep = wisp.mode == Mode::Away;
    let nourishing = wisp.mode == Mode::Nourishing;
    let as_f64 = |b: bool| f64::from(u8::from(b));

    // A change of mode (or waking): notice it straight away.
    let mut just_noticed = false;
    if wisp.mode != wisp.noticed {
        let woke = wisp.noticed == Mode::Away;
        wisp.noticed = wisp.mode;
        if !asleep {
            wisp.notice_start = now;
            just_noticed = true;
            // Glance aside, toward whatever changed.
            wisp.glance_target = if woke { 0.0 } else { -1.0 };
            wisp.next_glance = now + NOTICE_MS;
        }
    }

    let targets = (
        as_f64(wisp.mode == Mode::Resting),
        as_f64(wisp.mode == Mode::Holding),
    );
    if moving {
        wisp.happy = ease_toward(wisp.happy, as_f64(nourishing), dt, 400.0);
        wisp.sleep = ease_toward(wisp.sleep, as_f64(asleep), dt, 900.0);
        wisp.calm = ease_toward(wisp.calm, targets.0, dt, 450.0);
        wisp.curious = ease_toward(wisp.curious, targets.1, dt, 450.0);
        let feel = wisp.feel;
        wisp.listening = ease_toward(wisp.listening, as_f64(feel.listening), dt, 500.0);
        // A warm moment arrives quickly and fades slowly.
        let moved_ms = if feel.moved { 300.0 } else { 1800.0 };
        wisp.moved = ease_toward(wisp.moved, as_f64(feel.moved), dt, moved_ms);
        wisp.tender = ease_toward(wisp.tender, as_f64(feel.tender), dt, 1200.0);
        wisp.sleepy = ease_toward(wisp.sleepy, as_f64(feel.sleepy), dt, 4000.0);
    } else {
        let feel = wisp.feel;
        wisp.listening = as_f64(feel.listening);
        wisp.moved = as_f64(feel.moved);
        wisp.tender = as_f64(feel.tender);
        wisp.sleepy = as_f64(feel.sleepy);
        wisp.happy = as_f64(nourishing);
        wisp.sleep = as_f64(asleep);
        (wisp.calm, wisp.curious) = targets;
    }

    let mut look = wisp.light;
    look.glow = look.glow.mix(NIGHT_GLOW, NIGHT_WARMTH + 0.3 * wisp.tender);
    look.core = look.core.mix(NIGHT_GLOW, NIGHT_WARMTH * 0.3);
    let height = wisp.height;
    let awake = 1.0 - wisp.sleep;

    // The moss it rests on is behind everything.
    let width = wisp.width;
    let home_x = width / 2.0;
    if !wisp.flying {
        draw_moss(cr, width, height, wisp.happy, now, moving, wisp.season);
    }

    // --- Where and how big ---------------------------------------------------
    let mut radius = BODY_RADIUS;
    let x = home_x;
    let mut y = if wisp.flying {
        height / 2.0
    } else {
        height - MOSS_FLOOR - MOSS_HEIGHT - radius * 1.15
    };
    let mut brightness = look.brightness;
    let mut sway = 0.0;
    let mut blink = 0.0;

    let since_notice = now - wisp.notice_start;
    let noticing = moving && (0.0..NOTICE_MS).contains(&since_notice);
    if noticing {
        y -= (since_notice / NOTICE_MS * PI).sin() * NOTICE_HOP;
    }

    // Squash and stretch, from flying and from gestures, about its base.
    let (mut body_w, mut body_h) = wisp.body;
    let mut face_happy = wisp.happy.max(wisp.moved * 0.85);
    let mut shut = 0.0;
    let mut yawn = 0.0;
    let mut warm = wisp.moved;
    if moving {
        // Still while it listens or keeps someone company.
        let still = 1.0 - 0.8 * wisp.listening.max(wisp.tender);
        let slow = (1.0 + wisp.sleep * 0.4) * NIGHT_BREATH * (1.0 + 0.15 * wisp.sleepy);
        // Kept as a running phase, so a changing pace never skips a beat.
        wisp.breath_slow = slow;
        wisp.breath = match wisp.paced {
            Some(p) => p,
            None => (wisp.breath + dt / (wisp.breath_ms * slow)).fract(),
        };
        // Swelling with the breath is the caller's, when it's asked for.
        let breath = if wisp.breath_outside {
            0.0
        } else {
            breathing(wisp.breath)
        };
        // Breathing with someone, it breathes deeper, so it's easy to follow.
        let depth = if wisp.paced.is_some() {
            BREATH_DEPTH * 3.0
        } else {
            BREATH_DEPTH
        };
        radius *= (1.0 + breath * depth) * (1.0 + 0.03 * wisp.listening);
        // The light swells with each breath too, so a slow breath is easy to
        // fall in with.
        brightness *= 1.0 + BREATH_GLOW * breath;
        y -= ((now / 2400.0 * TAU).sin() * 0.5 + 0.5) * BOB * awake * still;
        sway = wander(now * 1.7, 5.1) * radius * 0.22 * still + wisp.lean * radius * 0.6;

        if let Some((gesture, start)) = wisp.gesture {
            let p = (now - start) / gesture.length();
            if (0.0..1.0).contains(&p) {
                let bump = (p * PI).sin();
                match gesture {
                    Gesture::Hop => {
                        // Crouch, spring up, land soft.
                        let crouch = |q: f64| (1.0 - (q / 0.14 - 0.5).abs() * 2.0).max(0.0);
                        let squash = 0.12 * (crouch(p) + crouch(p - 0.84));
                        let air = ((p - 0.14) / 0.7).clamp(0.0, 1.0);
                        y -= (air * PI).sin() * 9.0;
                        body_w *= 1.0 + squash;
                        body_h *= 1.0 - squash + 0.06 * (air * PI).sin();
                    }
                    Gesture::Stretch => {
                        let s = smoothstep(bump * 1.4);
                        body_h *= 1.0 + 0.16 * s;
                        body_w *= 1.0 - 0.08 * s;
                        shut = s;
                        face_happy = face_happy.max(s);
                    }
                    Gesture::Yawn => {
                        let s = smoothstep(bump * 1.5);
                        body_h *= 1.0 + 0.07 * s;
                        yawn = s;
                        shut = s;
                    }
                    Gesture::Nod => {
                        y += bump * 3.0;
                        wisp.glance_y = wisp.glance_y.max(bump * 0.6);
                    }
                    Gesture::Tend => {
                        wisp.glance_y = ease_toward(wisp.glance_y, 1.0, dt, 200.0);
                        let bob = (p * PI * 4.0).sin().max(0.0) * bump;
                        y += bob * 2.5;
                        body_h *= 1.0 - 0.05 * bob;
                        body_w *= 1.0 + 0.04 * bob;
                    }
                    Gesture::Glow => {
                        warm = warm.max(bump);
                        radius *= 1.0 + 0.06 * bump;
                    }
                }
            } else if p >= 1.0 {
                wisp.gesture = None;
            }
        }

        // Blinks and glances.
        if now >= wisp.next_blink {
            wisp.next_blink = now + 2500.0 + random(wisp) * 4500.0;
        }
        let until_blink = wisp.next_blink - now;
        if until_blink < BLINK_MS {
            blink = 1.0 - ((until_blink / BLINK_MS) * 2.0 - 1.0).abs();
        }
        if now >= wisp.next_glance {
            wisp.next_glance = now + 2000.0 + random(wisp) * 3000.0;
            wisp.glance_target = (random(wisp) * 2.0 - 1.0) * (wisp.curious * 0.8);
        }
        if just_noticed && nourishing {
            for _ in 0..3 {
                spawn(wisp, now, x, y - radius, ParticleKind::Mote);
            }
            wisp.last_mote = now;
        }
        if let Some(look) = wisp.look {
            wisp.glance_target = look;
        } else if wisp.listening > 0.5 {
            // Looking out at whoever is writing.
            wisp.glance_target = 0.0;
        }
        wisp.glance = ease_toward(wisp.glance, wisp.glance_target, dt, 250.0);
        // Watching something, it leans towards it, and lifts a little to
        // look up.
        if wisp.look.is_some() {
            sway += wisp.glance * radius * 0.3;
        }
        y -= (-wisp.glance_y).max(0.0) * 3.0;
        let tending = wisp
            .gesture
            .is_some_and(|(g, _)| matches!(g, Gesture::Tend | Gesture::Nod));
        if !tending {
            wisp.glance_y = ease_toward(wisp.glance_y, wisp.look_y.unwrap_or(0.0), dt, 300.0);
        }

        // Particles.
        let glad = wisp.happy.max(wisp.moved);
        if glad > 0.5 && now - wisp.last_mote > MOTE_EVERY_MS * (0.6 + random(wisp) * 0.8) {
            wisp.last_mote = now;
            spawn(wisp, now, x, y - radius, ParticleKind::Mote);
        }
        if wisp.humming
            && awake > 0.5
            && now - wisp.last_note > NOTE_EVERY_MS * (0.7 + random(wisp) * 0.6)
        {
            wisp.last_note = now;
            spawn(
                wisp,
                now,
                x + radius * 0.9,
                y - radius * 1.1,
                ParticleKind::Note,
            );
        }
        if wisp.sleep > 0.8 && now - wisp.last_z > Z_EVERY_MS {
            wisp.last_z = now;
            spawn(
                wisp,
                now,
                x + radius * 0.9,
                y - radius * 0.9,
                ParticleKind::Z,
            );
        }
    }

    if nourishing {
        brightness = (brightness * 1.15).min(1.0);
    }
    brightness = (brightness * (1.0 + 0.25 * warm) * (1.0 - 0.12 * wisp.tender)).min(1.15);

    draw_particles(wisp, cr, now, look);
    // Squash and stretch about the base, so it stays on its feet.
    let base = y + radius;
    let shaped = (body_w - 1.0).abs() > 1e-3 || (body_h - 1.0).abs() > 1e-3;
    if shaped {
        cr.save();
        cr.translate(x, base);
        cr.scale(body_w, body_h);
        cr.translate(-x, -base);
    }
    draw_sprite(
        cr,
        Sprite {
            x,
            y,
            radius,
            sway,
            look,
            brightness,
        },
    );
    let open = EYES_OPEN
        * (1.0 - wisp.sleep)
        * (1.0 - 0.28 * wisp.tender)
        * (1.0 - 0.42 * wisp.sleepy * (1.0 - wisp.listening))
        * (1.0 - shut);
    let openness = open * (1.0 - blink);
    let happy = face_happy * awake;
    draw_face(
        cr,
        Face {
            x,
            y,
            radius,
            openness,
            happy,
            glance: wisp.glance,
            glance_y: wisp.glance_y,
            smile: SMILE
                .max(0.9 * wisp.calm * awake)
                .max(0.7 * wisp.listening)
                .min(1.0 - 0.45 * wisp.tender)
                * awake.max(0.4),
            brow: (0.7 * wisp.happy * awake
                + 0.6 * wisp.curious * awake
                + BROW_LIFT
                + 0.35 * wisp.listening
                + 0.25 * wisp.tender)
                .clamp(-1.0, 1.0)
                * (1.0 - wisp.sleep * 0.6),
            sleepy: wisp.sleep.max(yawn),
            yawn,
            blush: CHEEKS * (1.0 + 1.4 * warm + 0.5 * wisp.tender),
        },
    );
    if shaped {
        cr.restore();
    }

    if !moving {
        return None;
    }
    let settling = |v: f64| v > 0.02 && v < 0.98;
    let bursting = (blink > 0.0 && open > BROWLESS && happy <= BEAMING)
        || noticing
        || wisp.gesturing(now)
        || settling(wisp.listening)
        || settling(wisp.moved)
        || settling(wisp.tender)
        || settling(wisp.calm)
        || settling(wisp.curious)
        || settling(wisp.sleep)
        || settling(wisp.happy);
    let lively = wisp.particles.iter().any(|p| p.kind != ParticleKind::Z) || nourishing;
    let fps = if bursting {
        BURST_FPS
    } else if wisp.sleep > 0.9 {
        DOZING_FPS
    } else if lively {
        LIVELY_FPS
    } else {
        RESTING_FPS
    };
    let mut delay = 1000.0 / fps;
    // Wake in time for the next blink rather than stepping over it.
    if awake > 0.9 {
        let to_blink = wisp.next_blink - BLINK_MS - now;
        if to_blink > 0.0 {
            delay = delay.min(to_blink);
        }
    }
    Some(delay)
}

/// A soft mound of moss along the nook's floor, with a few tufts.
fn draw_moss(
    cr: &mut dyn Canvas,
    width: f64,
    height: f64,
    happy: f64,
    now: f64,
    moving: bool,
    season: Option<Season>,
) {
    let moss_left = width * (1.0 - MOSS_SPAN) / 2.0;
    let moss_right = width - moss_left;
    // Deep against the night sky; delight perks it up.
    let colour = MOSS_FRESH
        .mix(Rgb(0.1, 0.12, 0.08), 0.35)
        .mix(MOSS_FRESH, happy * 0.3);
    // Frost pales it in winter.
    let colour = if season == Some(Season::Winter) {
        colour.mix(FROST, 0.22)
    } else {
        colour
    };
    let floor = height - MOSS_FLOOR;
    let top = floor - MOSS_HEIGHT;

    cr.set_paint(Paint::solid(colour, 0.8));
    cr.move_to(moss_left, floor);
    cr.curve_to(
        moss_left + 16.0,
        top,
        moss_right - 16.0,
        top,
        moss_right,
        floor,
    );
    cr.close_path();
    cr.fill();

    // Cushions along the mound's top, plump and a little lighter.
    let cushion = colour.mix(Rgb(0.85, 0.92, 0.6), 0.18);
    cr.set_paint(Paint::solid(cushion, 0.85));
    for (at, size) in [
        (0.14, 3.2),
        (0.24, 4.2),
        (0.35, 3.6),
        (0.47, 4.6),
        (0.58, 3.8),
        (0.69, 4.3),
        (0.8, 3.4),
        (0.89, 2.8),
    ] {
        let cx = moss_left + (moss_right - moss_left) * at;
        // Sit each cushion on the curve of the mound.
        let t = at;
        let ground = floor - (top - floor).abs() * 0.75 * (4.0 * t * (1.0 - t));
        cr.save();
        cr.translate(cx, ground);
        cr.scale(size * 1.15, size);
        cr.arc(0.0, 0.0, 1.0, PI, TAU);
        cr.restore();
        cr.fill();
    }

    // A few short sprigs stand up from it; summer's stand taller.
    let lift = if season == Some(Season::Summer) {
        1.35
    } else {
        1.0
    };
    let stir = if moving {
        (now / 3100.0).sin() * 0.5
    } else {
        0.0
    };
    cr.set_paint(Paint::solid(cushion.mix(Rgb(0.9, 0.95, 0.7), 0.15), 0.9));
    cr.set_line_width(1.2);
    cr.set_round_ends(true);
    let mut tips = Vec::with_capacity(3);
    for (i, (at, tall)) in [(0.3, 4.0), (0.52, 5.0), (0.74, 3.5)]
        .into_iter()
        .enumerate()
    {
        let bx = moss_left + (moss_right - moss_left) * at;
        let by = floor - MOSS_HEIGHT * 0.9;
        let rise = tall * lift;
        let lean = stir * if i % 2 == 0 { 1.0 } else { -1.0 };
        cr.move_to(bx, by);
        cr.curve_to(
            bx,
            by - rise * 0.6,
            bx + lean * 0.4,
            by - rise * 0.9,
            bx + lean,
            by - rise,
        );
        cr.stroke();
        cr.disc(bx + lean, by - rise, 0.9 * lift);
        tips.push((bx + lean, by - rise));
    }

    match season {
        Some(Season::Winter) => {
            // Frost glinting on the cushions, one glint at a time.
            for (k, at) in [0.2, 0.33, 0.45, 0.61, 0.77, 0.86].into_iter().enumerate() {
                let t = at;
                let gx = moss_left + (moss_right - moss_left) * at;
                let gy = floor - (top - floor).abs() * 0.75 * (4.0 * t * (1.0 - t)) - 3.2;
                let glint = if moving {
                    ((now / 2300.0 + k as f64 * 1.7).sin() * 0.5 + 0.5).powi(3)
                } else {
                    0.3
                };
                cr.set_paint(Paint::solid(FROST, 0.35 + 0.55 * glint));
                cr.disc(gx, gy, 0.55 + 0.5 * glint);
            }
        }
        Some(Season::Spring) => {
            // A small flower on the middle sprig.
            let (fx, fy) = tips[1];
            cr.set_paint(Paint::solid(PETAL, 0.95));
            for k in 0..5 {
                let a = k as f64 / 5.0 * TAU - PI / 2.0;
                cr.disc(fx + a.cos() * 1.5, fy + a.sin() * 1.5, 1.1);
            }
            cr.set_paint(Paint::solid(Rgb(0.98, 0.78, 0.3), 1.0));
            cr.disc(fx, fy, 0.8);
        }
        Some(Season::Summer) => {
            // Seed heads, gold in the light.
            cr.set_paint(Paint::solid(Rgb(0.93, 0.8, 0.45), 0.9));
            for &(tx, ty) in &tips {
                cr.disc(tx, ty - 0.6, 1.3);
            }
        }
        Some(Season::Autumn) => {
            // One fallen leaf, resting against the mound.
            let (lx, ly) = (moss_right - 9.0, floor - 1.6);
            cr.save();
            cr.translate(lx, ly);
            cr.move_to(-4.2, 0.6);
            cr.curve_to(-2.0, -2.6, 2.4, -2.4, 4.4, -0.4);
            cr.curve_to(2.2, 1.8, -1.8, 2.0, -4.2, 0.6);
            cr.close_path();
            cr.set_paint(Paint::solid(LEAF, 0.92));
            cr.fill();
            cr.set_paint(Paint::solid(Rgb(0.55, 0.3, 0.12), 0.7));
            cr.set_line_width(0.5);
            cr.move_to(-4.6, 0.8);
            cr.line_to(3.6, -0.5);
            cr.stroke();
            cr.restore();
        }
        None => {}
    }
}

struct Sprite {
    x: f64,
    y: f64,
    radius: f64,
    sway: f64,
    look: Look,
    brightness: f64,
}

/// The flame-topped teardrop body and its glow.
fn draw_sprite(cr: &mut dyn Canvas, s: Sprite) {
    let Sprite {
        x,
        y,
        radius: r,
        sway,
        look,
        brightness,
    } = s;

    cr.set_adding(true);
    let halo = r * 2.4;
    cr.set_paint(Paint::glow(
        x,
        y,
        halo,
        &[
            stop(0.0, look.glow, 0.55 * brightness),
            stop(0.45, look.glow, 0.24 * brightness),
            stop(1.0, look.glow, 0.0),
        ],
    ));
    cr.disc(x, y, halo);
    cr.set_adding(false);

    // A dim wisp is smaller and softer, never muddy: the body stays lit.
    let lit = 0.55 + 0.45 * brightness;
    let warm = look.core.mix(look.glow, 0.3);
    cr.move_to(x - r, y);
    cr.curve_to(x - r, y + r * 1.05, x + r, y + r * 1.05, x + r, y);
    cr.curve_to(
        x + r,
        y - r * 0.75,
        x + r * 0.35 + sway,
        y - r * 1.15,
        x + r * 0.15 + sway * 1.6,
        y - r * 1.75,
    );
    cr.curve_to(
        x - r * 0.05 + sway,
        y - r * 1.2,
        x - r,
        y - r * 0.8,
        x - r,
        y,
    );
    cr.close_path();
    cr.set_paint(Paint::Radial {
        inner: (x - r * 0.25, y - r * 0.3, r * 0.1),
        outer: (x, y, r * 1.35),
        stops: &[
            stop(0.0, look.core, lit),
            stop(0.7, warm, lit),
            stop(1.0, warm, lit * 0.9),
        ],
    });
    cr.fill();
}

struct Face {
    x: f64,
    y: f64,
    radius: f64,
    /// 1 = wide open, 0 = closed.
    openness: f64,
    /// 1 = happily closed (^ ^).
    happy: f64,
    /// -1 to 1, where the eyes look.
    glance: f64,
    /// -1 up to 1 down.
    glance_y: f64,
    /// 1 = a small smile, 0 = a flat line.
    smile: f64,
    /// 1 = brows lifted and open, -1 = lowered and watchful.
    brow: f64,
    /// 1 = heavy-lidded, mouth fallen open a little: sleepy, never sad.
    sleepy: f64,
    /// 0-1, how far into a yawn.
    yawn: f64,
    blush: f64,
}

fn draw_face(cr: &mut dyn Canvas, f: Face) {
    let r = f.radius;
    // The ink is a little see-through, so the face sits in the light.
    let alpha = 0.9;
    let eye_y = f.y - r * 0.08 + f.glance_y * r * 0.14;
    let (eye_w, eye_h) = (r * 0.22, r * 0.38);
    cr.set_round_ends(true);

    for side in [-1.0, 1.0] {
        // Cheeks first, under the eyes.
        let cheek_x = f.x + side * r * 0.62;
        let cheek_y = f.y + r * 0.2;
        cr.set_paint(Paint::glow(
            cheek_x,
            cheek_y,
            r * 0.22,
            &[stop(0.0, BLUSH, 0.45 * f.blush), stop(1.0, BLUSH, 0.0)],
        ));
        cr.disc(cheek_x, cheek_y, r * 0.22);

        let eye_x = f.x + side * r * 0.4 + f.glance * r * 0.14;

        // The brow, a short stroke that lifts with a bright mood and lowers
        // with a heavy one, its outer end dipping as it goes.
        if f.openness > BROWLESS || f.happy > BEAMING {
            let brow_y = eye_y - r * BROW_ABOVE - r * BROW_TRAVEL * f.brow;
            // Arched while the mood is bright, flat while it is heavy.
            let arch = r * BROW_ARCH * f.brow.max(0.0);
            cr.set_paint(Paint::solid(FACE_INK, alpha * 0.8));
            cr.set_line_width(r * 0.062);
            cr.move_to(eye_x - eye_w * 0.85, brow_y);
            cr.curve_to(
                eye_x - eye_w * 0.3,
                brow_y - arch,
                eye_x + eye_w * 0.3,
                brow_y - arch,
                eye_x + eye_w * 0.85,
                brow_y,
            );
            cr.stroke();
        }

        if f.happy > BEAMING {
            // ^ ^
            cr.set_paint(Paint::solid(FACE_INK, alpha));
            cr.set_line_width(r * 0.09);
            cr.set_line_width(r * 0.11);
            cr.arc(
                eye_x,
                eye_y + eye_h * 0.24,
                eye_w * 0.78,
                PI * 1.1,
                PI * 1.9,
            );
            cr.stroke();
        } else if f.openness < 0.12 {
            // Closed and gently curved: asleep, or mid-blink.
            cr.set_paint(Paint::solid(FACE_INK, alpha));
            cr.set_line_width(r * 0.08);
            cr.arc(
                eye_x,
                eye_y - eye_h * 0.05,
                eye_w * 0.55,
                PI * 0.15,
                PI * 0.85,
            );
            cr.stroke();
        } else {
            // An oval with the lid lowered from the top.
            cr.save();
            let lid = eye_y - eye_h / 2.0 + eye_h * (1.0 - f.openness);
            cr.rectangle(eye_x - eye_w, lid, eye_w * 2.0, eye_h * 1.2);
            cr.clip();
            cr.save();
            cr.translate(eye_x, eye_y);
            cr.scale(eye_w / 2.0, eye_h / 2.0);
            cr.arc(0.0, 0.0, 1.0, 0.0, TAU);
            cr.restore();
            cr.set_paint(Paint::solid(FACE_INK, alpha));
            cr.fill();
            if f.openness > 0.5 {
                cr.set_paint(Paint::solid(Rgb(1.0, 1.0, 1.0), 0.9));
                cr.disc(eye_x - eye_w * 0.18, eye_y - eye_h * 0.2, eye_w * 0.2);
            }
            cr.restore();
        }
    }

    // The mouth: a smile, falling a little open when the wisp is sleepy.
    cr.set_paint(Paint::solid(FACE_INK, alpha * 0.9));
    let mouth_y = f.y + r * 0.32;
    if f.yawn > 0.05 {
        // A big round yawn.
        cr.save();
        cr.translate(f.x, mouth_y + r * 0.04 * f.yawn);
        cr.scale(r * (0.1 + 0.05 * f.yawn), r * (0.08 + 0.2 * f.yawn));
        cr.arc(0.0, 0.0, 1.0, 0.0, TAU);
        cr.restore();
        cr.fill();
    } else if f.happy > 0.5 {
        cr.arc(f.x, mouth_y - r * 0.14, r * 0.26, PI * 0.1, PI * 0.9);
        cr.close_path();
        cr.fill();
    } else if f.sleepy > 0.6 {
        // A small oval, as if halfway through a yawn.
        cr.save();
        cr.translate(f.x, mouth_y);
        cr.scale(r * 0.1, r * 0.13 * f.sleepy);
        cr.arc(0.0, 0.0, 1.0, 0.0, TAU);
        cr.restore();
        cr.fill();
    } else {
        cr.set_line_width(r * 0.085);
        let width = r * 0.15;
        let curve = r * 0.16 * f.smile;
        cr.move_to(f.x - width, mouth_y);
        cr.curve_to(
            f.x - width * 0.4,
            mouth_y + curve,
            f.x + width * 0.4,
            mouth_y + curve,
            f.x + width,
            mouth_y,
        );
        cr.stroke();
    }
}

fn spawn(wisp: &mut Wisp, now: f64, x: f64, y: f64, kind: ParticleKind) {
    let jitter = random(wisp) - 0.5;
    let spread = random(wisp) - 0.5;
    let (life, dx, dy, size) = match kind {
        ParticleKind::Mote => (900.0, spread * 8.0, -16.0, 1.4),
        ParticleKind::Z => (2600.0, 6.0, -14.0, 3.2),
        ParticleKind::Note => (3400.0, 7.0 + spread * 6.0, -18.0, 4.2),
    };
    wisp.particles.push(Particle {
        born: now,
        life,
        x: x + jitter * 4.0,
        y,
        dx,
        dy,
        size,
        kind,
    });
}

fn draw_particles(wisp: &mut Wisp, cr: &mut dyn Canvas, now: f64, look: Look) {
    wisp.particles.retain(|p| now - p.born < p.life);
    for p in &wisp.particles {
        let age = (now - p.born) / p.life;
        let px = p.x + p.dx * age;
        let py = p.y + p.dy * age;
        let fade = 1.0 - age;
        match p.kind {
            ParticleKind::Mote => {
                cr.set_paint(Paint::solid(look.core, 0.85 * fade));
                cr.disc(px, py, p.size * (1.0 - age * 0.5));
            }
            ParticleKind::Note => {
                // A quaver: a round head, a stem and a flag, swaying a little.
                let sway = (age * PI * 2.0).sin() * 1.5;
                let (hx, hy) = (px + sway, py);
                let s = p.size;
                cr.set_paint(Paint::solid(look.core, 0.7 * fade));
                cr.save();
                cr.translate(hx, hy);
                cr.scale(s * 0.5, s * 0.38);
                cr.arc(0.0, 0.0, 1.0, 0.0, TAU);
                cr.restore();
                cr.fill();
                cr.set_line_width(0.8);
                cr.set_round_ends(true);
                cr.move_to(hx + s * 0.45, hy);
                cr.line_to(hx + s * 0.45, hy - s * 1.7);
                cr.curve_to(
                    hx + s * 0.9,
                    hy - s * 1.3,
                    hx + s * 1.1,
                    hy - s * 1.0,
                    hx + s * 0.8,
                    hy - s * 0.6,
                );
                cr.stroke();
            }
            ParticleKind::Z => {
                cr.set_paint(Paint::solid(Rgb(0.85, 0.86, 0.83), 0.45 * fade));
                cr.set_line_width(1.1);
                cr.set_round_ends(true);
                let s = p.size * (1.0 + age * 0.5);
                cr.move_to(px, py);
                cr.line_to(px + s, py);
                cr.line_to(px, py + s);
                cr.line_to(px + s, py + s);
                cr.stroke();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    /// Notes every colour painted on it, to the nearest 8-bit step.
    #[derive(Default)]
    struct Palette(BTreeSet<String>);

    impl Palette {
        fn note(&mut self, Rgb(r, g, b): Rgb) {
            let byte = |c: f64| (c * 255.0).round() as u8;
            let hex = format!("#{:02x}{:02x}{:02x}", byte(r), byte(g), byte(b));
            self.0.insert(hex);
        }
    }

    impl Canvas for Palette {
        fn save(&mut self) {}
        fn restore(&mut self) {}
        fn translate(&mut self, _: f64, _: f64) {}
        fn scale(&mut self, _: f64, _: f64) {}
        fn move_to(&mut self, _: f64, _: f64) {}
        fn line_to(&mut self, _: f64, _: f64) {}
        fn curve_to(&mut self, _: f64, _: f64, _: f64, _: f64, _: f64, _: f64) {}
        fn arc(&mut self, _: f64, _: f64, _: f64, _: f64, _: f64) {}
        fn rectangle(&mut self, _: f64, _: f64, _: f64, _: f64) {}
        fn close_path(&mut self) {}
        fn new_path(&mut self) {}
        fn set_paint(&mut self, paint: Paint<'_>) {
            match paint {
                Paint::Solid { colour, .. } => self.note(colour),
                Paint::Radial { stops, .. } | Paint::Linear { stops, .. } => {
                    for s in stops {
                        self.note(s.colour);
                    }
                }
            }
        }
        fn set_line_width(&mut self, _: f64) {}
        fn set_round_ends(&mut self, _: bool) {}
        fn set_adding(&mut self, _: bool) {}
        fn fill(&mut self) {}
        fn stroke(&mut self) {}
        fn clip(&mut self) {}
    }

    /// The colours a still wisp paints with in `mode`: out flying, then on
    /// its moss in winter.
    fn still_colours(mode: Mode) -> Vec<String> {
        let mut palette = Palette::default();
        let mut wisp = Wisp::new(100.0, 100.0);
        wisp.update(mode);
        wisp.set_flight(true, 0.0, None);
        wisp.draw(0.0, false, &mut palette);
        wisp.set_flight(false, 0.0, None);
        wisp.resize(56.0, 56.0);
        wisp.set_season(Some(Season::Winter));
        wisp.draw(0.0, false, &mut palette);
        palette.0.into_iter().collect()
    }

    #[test]
    fn each_mode_keeps_the_wisps_colours() {
        // Its glow, core and face, and the frost on the moss.
        let wisp = [
            "#2b211a", "#dbedff", "#f29e59", "#f2ba66", "#fbdfb3", "#ffefd2",
        ];
        let moss = ["#728967", "#849a70", "#92a77a"];
        // Delight greens the moss a little; open eyes catch the light.
        let perked = ["#7b946e", "#8ba375", "#99af7e"];
        let glint = ["#ffffff"];
        let expect = |parts: &[&[&str]]| {
            let mut all: Vec<String> = parts.concat().iter().map(|c| c.to_string()).collect();
            all.sort();
            all
        };
        assert_eq!(still_colours(Mode::Away), expect(&[&wisp, &moss]));
        assert_eq!(
            still_colours(Mode::Resting),
            expect(&[&wisp, &moss, &glint])
        );
        assert_eq!(
            still_colours(Mode::Holding),
            expect(&[&wisp, &moss, &glint])
        );
        assert_eq!(still_colours(Mode::Nourishing), expect(&[&wisp, &perked]));
    }

    #[test]
    fn an_asleep_wisp_never_wakes_for_a_blink() {
        let mut palette = Palette::default();
        let mut wisp = Wisp::new(56.0, 56.0);
        wisp.update(Mode::Away);
        let mut now = 0.0;
        while now < 70_000.0 {
            let delay = wisp.draw(now, true, &mut palette).expect("moving");
            if now > 10_000.0 {
                assert!(delay >= 1000.0 / DOZING_FPS, "{delay} ms at {now}");
            }
            now += delay;
        }
    }
}
