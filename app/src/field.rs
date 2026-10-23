//! The Braille dot field: a lattice of fine dots, each lit by whatever falls
//! on it. Light is added in linear units and tone-mapped once, into a small
//! texture with one texel per dot; the widget scales that up and masks it
//! with a round dot.
//!
//! Most of a frame is the slow-changing base layer, so its tones are kept:
//! a frame redoes only the dots something lit on top of it.

use gtk::{gdk, glib};

pub type Rgb = [f32; 3];

pub struct Field {
    pub cols: usize,
    pub rows: usize,
    /// Logical pixels between dot centres.
    pub pitch: f32,
    base: Vec<Rgb>,
    light: Vec<Rgb>,
    tone: Vec<u8>,
    /// Dots lit on top of the base since the frame began.
    touched: Vec<usize>,
    /// The base tone-mapped at `base_gain`; stale when that's `None`.
    base_tone: Vec<u8>,
    base_gain: Option<f32>,
    /// The base changed: the next frame starts from all of it.
    base_changed: bool,
}

/// Steps per unit of light in the tone curve's table.
const TONE_STEPS: f32 = 512.0;
const TONE_MAX: f32 = 8.0;

impl Field {
    pub fn new() -> Field {
        Field {
            cols: 0,
            rows: 0,
            pitch: 5.0,
            base: Vec::new(),
            light: Vec::new(),
            touched: Vec::new(),
            base_tone: Vec::new(),
            base_gain: None,
            base_changed: true,
            // Soft shoulder: faint light stays linear, bright light saturates gently.
            tone: (0..=(TONE_STEPS * TONE_MAX) as usize)
                .map(|i| ((1.0 - (-1.5 * i as f32 / TONE_STEPS).exp()) * 255.0 + 0.5) as u8)
                .collect(),
        }
    }

    /// Fits the lattice to a window; true if it changed.
    pub fn fit(&mut self, width: f64, height: f64, pitch: f32) -> bool {
        let cols = (width as f32 / pitch).ceil() as usize + 1;
        let rows = (height as f32 / pitch).ceil() as usize + 1;
        if cols == self.cols && rows == self.rows && pitch == self.pitch {
            return false;
        }
        self.cols = cols;
        self.rows = rows;
        self.pitch = pitch;
        let n = cols * rows;
        self.base = vec![[0.0; 3]; n];
        self.light = vec![[0.0; 3]; n];
        self.base_changed = true;
        self.base_gain = None;
        true
    }

    pub fn base_mut(&mut self) -> &mut [Rgb] {
        self.base_changed = true;
        self.base_gain = None;
        &mut self.base
    }

    /// Starts a frame from the slow-changing base layer.
    pub fn begin(&mut self) {
        if self.base_changed {
            self.light.copy_from_slice(&self.base);
            self.base_changed = false;
        } else {
            for &i in &self.touched {
                self.light[i] = self.base[i];
            }
        }
        self.touched.clear();
    }

    /// The light of the dot at `c`, `r`, if it's on the lattice, noted as
    /// touched this frame.
    fn cell(&mut self, c: isize, r: isize) -> Option<&mut Rgb> {
        if c < 0 || r < 0 || c as usize >= self.cols || r as usize >= self.rows {
            return None;
        }
        let i = r as usize * self.cols + c as usize;
        self.touched.push(i);
        Some(&mut self.light[i])
    }

    fn add_at(&mut self, c: isize, r: isize, color: Rgb, amount: f32) {
        let Some(cell) = self.cell(c, r) else {
            return;
        };
        cell[0] += color[0] * amount;
        cell[1] += color[1] * amount;
        cell[2] += color[2] * amount;
    }

    /// Light at a point, shared among the four nearest dots so that slow
    /// movement glides instead of hopping from dot to dot.
    pub fn splat(&mut self, x: f64, y: f64, color: Rgb, amount: f32) {
        let fx = x / self.pitch as f64 - 0.5;
        let fy = y / self.pitch as f64 - 0.5;
        let (c0, r0) = (fx.floor(), fy.floor());
        let (tx, ty) = ((fx - c0) as f32, (fy - r0) as f32);
        let (c0, r0) = (c0 as isize, r0 as isize);
        self.add_at(c0, r0, color, amount * (1.0 - tx) * (1.0 - ty));
        self.add_at(c0 + 1, r0, color, amount * tx * (1.0 - ty));
        self.add_at(c0, r0 + 1, color, amount * (1.0 - tx) * ty);
        self.add_at(c0 + 1, r0 + 1, color, amount * tx * ty);
    }

    /// A point of light: bright ones spill into a small cross of neighbours.
    /// Light on the single nearest dot: crisp, like a Braille character.
    pub fn dot(&mut self, x: f64, y: f64, color: Rgb, amount: f32) {
        let c = (x / self.pitch as f64 - 0.5).round() as isize;
        let r = (y / self.pitch as f64 - 0.5).round() as isize;
        self.add_at(c, r, color, amount);
    }

    /// Darkens the dot nearest a point by `factor`: a shadow falling on it.
    pub fn shade(&mut self, x: f64, y: f64, factor: f32) {
        let c = (x / self.pitch as f64 - 0.5).round() as isize;
        let r = (y / self.pitch as f64 - 0.5).round() as isize;
        let Some(cell) = self.cell(c, r) else {
            return;
        };
        for k in cell.iter_mut() {
            *k *= factor;
        }
    }

    pub fn star(&mut self, x: f64, y: f64, color: Rgb, amount: f32) {
        self.dot(x, y, color, amount.min(1.2));
        if amount > 1.0 {
            let p = self.pitch as f64;
            let spill = ((amount - 1.0) * 0.22).min(0.9);
            for (dx, dy) in [(p, 0.0), (-p, 0.0), (0.0, p), (0.0, -p)] {
                self.dot(x + dx, y + dy, color, spill);
            }
            if amount > 2.5 {
                let far = ((amount - 2.5) * 0.06).min(0.35);
                for (dx, dy) in [
                    (p, p),
                    (-p, p),
                    (p, -p),
                    (-p, -p),
                    (2.0 * p, 0.0),
                    (-2.0 * p, 0.0),
                    (0.0, 2.0 * p),
                    (0.0, -2.0 * p),
                ] {
                    self.dot(x + dx, y + dy, color, far);
                }
            }
        }
    }

    /// A soft round glow, `radius` in pixels. It fades to nothing before
    /// the edge of the square it's worked out over.
    pub fn glow(&mut self, x: f64, y: f64, radius: f64, color: Rgb, amount: f32) {
        let p = self.pitch as f64;
        let reach = (radius * 1.6 / p).ceil() as isize + 1;
        let (cc, cr) = (
            (x / p - 0.5).round() as isize,
            (y / p - 0.5).round() as isize,
        );
        for r in cr - reach..=cr + reach {
            for c in cc - reach..=cc + reach {
                let (px, py) = ((c as f64 + 0.5) * p, (r as f64 + 0.5) * p);
                let d2 = ((px - x).powi(2) + (py - y).powi(2)) / radius.max(0.5).powi(2);
                if d2 < 2.5 {
                    self.add_at(c, r, color, amount * (-d2 * 1.6).exp() as f32);
                }
            }
        }
    }

    /// A line of dots from one point to another.
    pub fn line(&mut self, a: (f64, f64), b: (f64, f64), color: Rgb, amount: f32) {
        let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        let steps = (len / (self.pitch as f64 * 0.7)).ceil().max(1.0) as usize;
        for i in 0..=steps {
            let t = i as f64 / steps as f64;
            self.splat(a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t, color, amount);
        }
    }

    /// Visits every dot of a disc, handing the offset from its centre in
    /// units of the radius; the callback returns the light for that dot.
    pub fn disc(
        &mut self,
        x: f64,
        y: f64,
        radius: f64,
        mut shade: impl FnMut(f64, f64) -> Option<(Rgb, f32)>,
    ) {
        let p = self.pitch as f64;
        let reach = (radius / p).ceil() as isize + 1;
        let (cc, cr) = (
            (x / p - 0.5).round() as isize,
            (y / p - 0.5).round() as isize,
        );
        for r in cr - reach..=cr + reach {
            for c in cc - reach..=cc + reach {
                let (px, py) = ((c as f64 + 0.5) * p, (r as f64 + 0.5) * p);
                let (u, v) = ((px - x) / radius, (py - y) / radius);
                if let Some((color, amount)) = shade(u, v) {
                    self.add_at(c, r, color, amount);
                }
            }
        }
    }

    /// Tone-maps the frame into a texture, scaling everything by `gain`.
    pub fn texture(&mut self, gain: f32) -> gdk::Texture {
        let (tone, scale) = (&self.tone, gain * TONE_STEPS);
        let map = |cell: &Rgb, out: &mut [u8; 3]| {
            for (o, &value) in out.iter_mut().zip(cell) {
                *o = tone[((value * scale) as usize).min(tone.len() - 1)];
            }
        };
        let whole = |cells: &[Rgb]| {
            let mut bytes = vec![0u8; cells.len() * 3];
            for (cell, out) in cells.iter().zip(bytes.as_chunks_mut::<3>().0) {
                map(cell, out);
            }
            bytes
        };
        // When most of the frame is lit on top, doing it all is quicker.
        let bytes = if self.touched.len() > self.light.len() / 4 {
            whole(&self.light)
        } else {
            if self.base_gain != Some(gain) {
                self.base_tone = whole(&self.base);
                self.base_gain = Some(gain);
            }
            let mut bytes = self.base_tone.clone();
            let out = bytes.as_chunks_mut::<3>().0;
            for &i in &self.touched {
                map(&self.light[i], &mut out[i]);
            }
            bytes
        };
        let bytes = glib::Bytes::from_owned(bytes);
        gdk::MemoryTexture::new(
            self.cols as i32,
            self.rows as i32,
            gdk::MemoryFormat::R8g8b8,
            &bytes,
            self.cols * 3,
        )
        .upcast()
    }
}

use gtk::prelude::Cast;

/// A single round dot, drawn once per scale factor and repeated across the
/// lattice as a mask.
pub fn dot_tile(pitch: f32, scale: f64) -> gdk::Texture {
    let size = ((pitch as f64 * scale).round() as usize).max(2);
    let radius = size as f64 * 0.41;
    let centre = size as f64 / 2.0;
    let mut bytes = Vec::with_capacity(size * size * 4);
    for y in 0..size {
        for x in 0..size {
            let d = ((x as f64 + 0.5 - centre).powi(2) + (y as f64 + 0.5 - centre).powi(2)).sqrt();
            let a = (radius + 0.5 - d).clamp(0.0, 1.0);
            let a = (a * 255.0) as u8;
            bytes.extend_from_slice(&[a, a, a, a]);
        }
    }
    gdk::MemoryTexture::new(
        size as i32,
        size as i32,
        gdk::MemoryFormat::R8g8b8a8Premultiplied,
        &glib::Bytes::from_owned(bytes),
        size * 4,
    )
    .upcast()
}

/// Smooth value noise on the unit sphere, for the Milky Way's texture.
pub fn noise3(x: f64, y: f64, z: f64) -> f64 {
    fn hash(i: i64, j: i64, k: i64) -> f64 {
        let mut h = (i.wrapping_mul(73_856_093)
            ^ j.wrapping_mul(19_349_663)
            ^ k.wrapping_mul(83_492_791)) as u64;
        h ^= h >> 13;
        h = h.wrapping_mul(0x5bd1_e995);
        h ^= h >> 15;
        (h & 0xffff) as f64 / 65_535.0
    }
    let (xi, yi, zi) = (x.floor(), y.floor(), z.floor());
    let (xf, yf, zf) = (x - xi, y - yi, z - zi);
    let s = westering_core::ease::smoothstep;
    let (u, v, w) = (s(xf), s(yf), s(zf));
    let (xi, yi, zi) = (xi as i64, yi as i64, zi as i64);
    let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;
    let c = |dx, dy, dz| hash(xi + dx, yi + dy, zi + dz);
    lerp(
        lerp(
            lerp(c(0, 0, 0), c(1, 0, 0), u),
            lerp(c(0, 1, 0), c(1, 1, 0), u),
            v,
        ),
        lerp(
            lerp(c(0, 0, 1), c(1, 0, 1), u),
            lerp(c(0, 1, 1), c(1, 1, 1), u),
            v,
        ),
        w,
    )
}
