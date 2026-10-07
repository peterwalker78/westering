//! The widget the sky is drawn on. It holds one finished frame at a time:
//! the dot field's texture, masked into round dots on the GPU, and a few
//! lines of text.

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, graphene, gsk, pango};
use std::cell::RefCell;

#[derive(Clone, Copy, PartialEq)]
pub enum Align {
    Left,
    Centre,
}

#[derive(Clone)]
pub struct Text {
    pub x: f64,
    pub y: f64,
    pub text: String,
    pub size: f64,
    pub bold: bool,
    pub alpha: f64,
    pub align: Align,
    /// Wrap width in pixels.
    pub wrap: Option<f64>,
    pub color: [f32; 3],
}

impl Text {
    pub fn new(x: f64, y: f64, text: impl Into<String>, size: f64, alpha: f64) -> Text {
        Text {
            x,
            y,
            text: text.into(),
            size,
            bold: false,
            alpha,
            align: Align::Left,
            wrap: None,
            color: [0.93, 0.94, 0.97],
        }
    }
    pub fn centred(mut self) -> Text {
        self.align = Align::Centre;
        self
    }
    pub fn bold(mut self) -> Text {
        self.bold = true;
        self
    }
    pub fn wrap(mut self, width: f64) -> Text {
        self.wrap = Some(width);
        self
    }
    pub fn color(mut self, color: [f32; 3]) -> Text {
        self.color = color;
        self
    }
}

/// A bright star or planet, drawn as a true point at its exact place rather
/// than on the lattice: a crisp core and a soft halo.
#[derive(Clone)]
pub struct Point {
    pub x: f64,
    pub y: f64,
    pub radius: f32,
    pub color: [f32; 3],
    pub alpha: f32,
    /// How strong the halo is, 0 to 1.
    pub halo: f32,
}

/// A soft line on the screen: a constellation's figure.
#[derive(Clone)]
pub struct Line {
    pub a: (f64, f64),
    pub b: (f64, f64),
    pub color: [f32; 3],
    pub alpha: f32,
}

/// The card for something just caught.
#[derive(Clone)]
pub struct CardView {
    pub x: f64,
    pub y: f64,
    /// A small line above the title: what kind of thing, and where.
    pub kicker: String,
    pub title: String,
    pub body: String,
    /// Keys and what they do, shown as key caps along the bottom.
    pub keys: Vec<(String, String)>,
    pub alpha: f64,
    pub width: f64,
    /// Smaller type and tighter spacing, for a small window.
    pub compact: bool,
    /// A photograph at the top, and whose it is.
    pub picture: Option<(gdk::Texture, String)>,
    /// A small line under the words.
    pub footnote: Option<String>,
    /// Centred up and down on `y`, rather than hanging from it.
    pub middle: bool,
}

/// A soft light round something worth a click, and a fine ring round the
/// one the pointer is on.
#[derive(Clone, Debug)]
pub struct Glow {
    pub x: f64,
    pub y: f64,
    /// How big the thing itself is on the screen.
    pub radius: f64,
    /// 0 to 1: how lit.
    pub strength: f64,
    /// 0 to 1: the ring's opacity.
    pub ring: f64,
}

/// A photograph of what's being looked at, set into the sky at its true size.
#[derive(Clone)]
pub struct Eyepiece {
    pub texture: gdk::Texture,
    pub x: f64,
    pub y: f64,
    pub radius: f64,
    /// Degrees clockwise, to sit the picture as the object sits in the sky.
    pub rotation: f64,
    /// A Moon or planet: an opaque disc, rather than light over the sky.
    pub disc: bool,
    pub credit: String,
    pub alpha: f64,
    /// Height over width.
    pub aspect: f64,
    /// Where the picture's middle is from the object, on the screen.
    pub offset: (f64, f64),
}

/// The strip along the top that says which way the view faces.
#[derive(Clone)]
pub struct Compass {
    /// Azimuth the view faces, degrees from north through east.
    pub heading: f64,
    pub alpha: f64,
    /// Along the bottom rather than the top: in a small window, where the
    /// top is taken by the evening's guide and the list.
    pub bottom: bool,
}

/// A crisp shape drawn over everything but the wisp: a bird, a butterfly,
/// a falling leaf. Strokes when open, filled when closed.
#[derive(Clone)]
pub struct Silhouette {
    pub path: Vec<(f64, f64)>,
    pub width: f32,
    pub color: [f32; 3],
    pub alpha: f32,
    pub filled: bool,
}

/// A small picture drawn over the sky: the wisp.
#[derive(Clone)]
pub struct Sprite {
    pub texture: gdk::Texture,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub alpha: f64,
}

/// Words in a speech bubble, standing on `bottom` from `x`.
#[derive(Clone)]
pub struct Bubble {
    pub x: f64,
    pub bottom: f64,
    pub text: String,
    pub width: f64,
    pub alpha: f64,
    /// A small tail down towards the wisp on its moss.
    pub tail: bool,
    /// The wisp has more to say: three dots at the foot, running, keyed
    /// by the time in seconds, and how to hear it.
    pub more: Option<(f64, &'static str)>,
}

/// The keys and what they do: a small, faint reminder in a corner, or,
/// when asked for, the whole table in the middle of the screen.
#[derive(Clone)]
pub struct Legend {
    /// Each row: its keys, drawn as caps, and what they do.
    pub rows: Vec<(Vec<String>, String)>,
    pub alpha: f64,
    /// The whole table: in the middle, under a heading, and easy to read.
    pub whole: bool,
}

#[derive(Default, Clone)]
pub struct Frame {
    pub legend: Option<Legend>,
    /// What's worth a click, lit as the pointer comes near: drawn smooth,
    /// over the dots, every frame.
    pub glows: Vec<Glow>,
    pub points: Vec<Point>,
    /// Living things moving about: drawn fresh every frame.
    pub silhouettes: Vec<Silhouette>,
    /// Points drawn over the photographs: the wisp's trail and what it marks.
    pub marks: Vec<Point>,
    pub lines: Vec<Line>,
    pub eyepiece: Option<Eyepiece>,
    pub card: Option<CardView>,
    pub compass: Option<Compass>,
    pub sprites: Vec<Sprite>,
    pub bubble: Option<Bubble>,
    pub texture: Option<gdk::Texture>,
    pub cols: usize,
    pub rows: usize,
    pub pitch: f32,
    pub background: [f32; 3],
    pub texts: Vec<Text>,
    /// Over everything: 0 is none, 1 is black.
    pub veil: f32,
}

fn font(widget: &gtk::Widget, size: f64, bold: bool) -> pango::FontDescription {
    let mut f = widget
        .pango_context()
        .font_description()
        .unwrap_or_default();
    f.set_absolute_size(size * pango::SCALE as f64);
    f.set_weight(if bold {
        pango::Weight::Semibold
    } else {
        pango::Weight::Normal
    });
    f
}

fn layout(
    widget: &gtk::Widget,
    text: &str,
    size: f64,
    bold: bool,
    wrap: Option<f64>,
) -> pango::Layout {
    let l = widget.create_pango_layout(Some(text));
    l.set_font_description(Some(&font(widget, size, bold)));
    if let Some(w) = wrap {
        l.set_width((w * pango::SCALE as f64) as i32);
        l.set_wrap(pango::WrapMode::WordChar);
    }
    l
}

fn text_at(snapshot: &gtk::Snapshot, l: &pango::Layout, x: f32, y: f32, rgba: gdk::RGBA) {
    snapshot.save();
    snapshot.translate(&graphene::Point::new(x, y));
    snapshot.append_layout(l, &rgba);
    snapshot.restore();
}

fn panel(snapshot: &gtk::Snapshot, rect: &graphene::Rect, radius: f32, alpha: f32) {
    snapshot.push_rounded_clip(&gsk::RoundedRect::from_rect(*rect, radius));
    snapshot.append_color(&gdk::RGBA::new(0.045, 0.055, 0.1, 0.82 * alpha), rect);
    snapshot.pop();
    let border = [gdk::RGBA::new(1.0, 1.0, 1.0, 0.07 * alpha); 4];
    snapshot.append_border(
        &gsk::RoundedRect::from_rect(*rect, radius),
        &[1.0; 4],
        &border,
    );
}

/// A key drawn as a small cap; returns its width.
fn keycap(
    widget: &gtk::Widget,
    snapshot: &gtk::Snapshot,
    key: &str,
    x: f32,
    y: f32,
    alpha: f32,
) -> f32 {
    let l = layout(widget, key, 12.0, true, None);
    let (w, h) = l.pixel_size();
    let rect = graphene::Rect::new(x, y, w as f32 + 12.0, h as f32 + 4.0);
    snapshot.push_rounded_clip(&gsk::RoundedRect::from_rect(rect, 5.0));
    snapshot.append_color(&gdk::RGBA::new(1.0, 1.0, 1.0, 0.1 * alpha), &rect);
    snapshot.pop();
    text_at(
        snapshot,
        &l,
        x + 6.0,
        y + 2.0,
        gdk::RGBA::new(0.95, 0.95, 0.98, 0.9 * alpha),
    );
    rect.width()
}

/// The keys, bottom right: a faint panel, a cap for each key and a word or
/// two for what it does.
fn draw_legend(widget: &gtk::Widget, snapshot: &gtk::Snapshot, w: f32, h: f32, l: &Legend) {
    let a = l.alpha as f32;
    let (pad, row_h, gap) = (10.0f32, 24.0f32, 8.0f32);
    let words: Vec<pango::Layout> = l
        .rows
        .iter()
        .map(|(_, what)| layout(widget, what, 12.0, false, None))
        .collect();
    // Measure the caps once so the words line up in a column.
    let caps_w = l
        .rows
        .iter()
        .map(|(keys, _)| {
            keys.iter()
                .map(|k| layout(widget, k, 12.0, true, None).pixel_size().0 as f32 + 12.0 + 4.0)
                .sum::<f32>()
        })
        .fold(0.0f32, f32::max);
    let words_w = words
        .iter()
        .map(|l| l.pixel_size().0 as f32)
        .fold(0.0f32, f32::max);
    // The whole table has a heading, more room round it, and full ink.
    let (pad, gap) = if l.whole { (18.0, 16.0) } else { (pad, gap) };
    let heading = l.whole.then(|| layout(widget, "KEYS", 11.0, true, None));
    let head_h = if l.whole { 26.0 } else { 0.0 };
    let width = pad * 2.0 + caps_w + gap + words_w;
    let height = pad * 2.0 + head_h + row_h * l.rows.len() as f32 - 4.0;
    let (x, y) = if l.whole {
        ((w - width) / 2.0, ((h - height) / 2.0).max(8.0))
    } else {
        (w - width - 16.0, h - height - 16.0)
    };
    let (backing, caps, ink) = if l.whole {
        (1.0, 1.0, 0.92)
    } else {
        (0.6, 0.8, 0.65)
    };
    if l.whole {
        // The sky steps back, and nothing shows through the table.
        snapshot.append_color(
            &gdk::RGBA::new(0.0, 0.0, 0.02, 0.45 * a),
            &graphene::Rect::new(0.0, 0.0, w, h),
        );
        let rect = graphene::Rect::new(x, y, width, height);
        snapshot.push_rounded_clip(&gsk::RoundedRect::from_rect(rect, 10.0));
        snapshot.append_color(&gdk::RGBA::new(0.045, 0.055, 0.1, a), &rect);
        snapshot.pop();
    }
    panel(
        snapshot,
        &graphene::Rect::new(x, y, width, height),
        10.0,
        backing * a,
    );
    if let Some(heading) = &heading {
        text_at(
            snapshot,
            heading,
            x + pad,
            y + pad - 2.0,
            gdk::RGBA::new(0.86, 0.88, 0.93, 0.5 * a),
        );
    }
    for (i, ((keys, _), said)) in l.rows.iter().zip(&words).enumerate() {
        let ry = y + pad + head_h + row_h * i as f32;
        let mut cx = x + pad;
        for k in keys {
            cx += keycap(widget, snapshot, k, cx, ry, caps * a) + 4.0;
        }
        text_at(
            snapshot,
            said,
            x + pad + caps_w + gap,
            ry + 2.0,
            gdk::RGBA::new(0.86, 0.88, 0.93, ink * a),
        );
    }
}

fn draw_card(widget: &gtk::Widget, snapshot: &gtk::Snapshot, c: &CardView) {
    let a = c.alpha as f32;
    let (pad, width) = (if c.compact { 12.0 } else { 18.0f32 }, c.width as f32);
    let (title_size, body_size) = if c.compact {
        (17.0, 13.0)
    } else {
        (21.0, 15.0)
    };
    let kicker = layout(
        widget,
        &c.kicker.to_uppercase(),
        11.0,
        true,
        Some((width - 2.0 * pad) as f64),
    );
    let title = layout(
        widget,
        &c.title,
        title_size,
        true,
        Some((width - 2.0 * pad) as f64),
    );
    let body = layout(
        widget,
        &c.body,
        body_size,
        false,
        Some((width - 2.0 * pad) as f64),
    );
    let (_, kh) = kicker.pixel_size();
    let (_, th) = title.pixel_size();
    let (_, bh) = body.pixel_size();
    let keys_h = if c.keys.is_empty() { 0.0 } else { 34.0 };
    let room = widget.height() as f32;
    // A picture across the top, as tall as its shape asks but never so tall
    // the words run off the screen.
    let inner = width - 2.0 * pad;
    let credit = c.picture.as_ref().map(|(_, who)| {
        layout(
            widget,
            &format!("Photograph: {who}"),
            10.5,
            false,
            Some(inner as f64),
        )
    });
    let footnote = c
        .footnote
        .as_ref()
        .map(|f| layout(widget, f, 10.5, false, Some(inner as f64)));
    let foot_h = footnote
        .as_ref()
        .map_or(0.0, |l| l.pixel_size().1 as f32 + 10.0);
    let text_h = pad + kh as f32 + 6.0 + th as f32 + 10.0 + bh as f32 + foot_h + keys_h + pad;
    let pic_h = c.picture.as_ref().map_or(0.0, |(t, _)| {
        let natural = inner * t.height() as f32 / t.width().max(1) as f32;
        natural
            .min(if c.compact { 130.0 } else { 240.0 })
            .min((room - text_h - 60.0).max(0.0))
    });
    let credit_h = credit
        .as_ref()
        .map_or(0.0, |l| l.pixel_size().1 as f32 + 6.0);
    let top_h = if pic_h > 20.0 {
        pic_h + credit_h + 10.0
    } else {
        0.0
    };
    let height = text_h + top_h;
    // Kept on the screen, however short the window.
    let top = if c.middle {
        c.y as f32 - height / 2.0
    } else {
        c.y as f32
    };
    let (x, y) = (c.x as f32, top.min(room - height - 8.0).max(8.0));
    panel(snapshot, &graphene::Rect::new(x, y, width, height), 14.0, a);
    let mut cy = y + pad;
    if top_h > 0.0
        && let Some((texture, _)) = &c.picture
    {
        // Filled to the width, trimmed top and bottom to fit.
        let frame = graphene::Rect::new(x + pad, cy, inner, pic_h);
        let natural = inner * texture.height() as f32 / texture.width().max(1) as f32;
        snapshot.push_rounded_clip(&gsk::RoundedRect::from_rect(frame, 8.0));
        snapshot.push_opacity(a as f64);
        snapshot.append_texture(
            texture,
            &graphene::Rect::new(x + pad, cy - (natural - pic_h) / 2.0, inner, natural),
        );
        snapshot.pop();
        snapshot.pop();
        cy += pic_h + 4.0;
        if let Some(l) = &credit {
            text_at(
                snapshot,
                l,
                x + pad,
                cy,
                gdk::RGBA::new(0.8, 0.83, 0.9, 0.5 * a),
            );
            cy += credit_h;
        }
        cy += 6.0;
    }
    text_at(
        snapshot,
        &kicker,
        x + pad,
        cy,
        gdk::RGBA::new(0.95, 0.8, 0.6, 0.75 * a),
    );
    cy += kh as f32 + 6.0;
    text_at(
        snapshot,
        &title,
        x + pad,
        cy,
        gdk::RGBA::new(0.98, 0.97, 0.94, a),
    );
    cy += th as f32 + 10.0;
    text_at(
        snapshot,
        &body,
        x + pad,
        cy,
        gdk::RGBA::new(0.86, 0.88, 0.93, 0.92 * a),
    );
    cy += bh as f32 + 16.0;
    if let Some(l) = &footnote {
        text_at(
            snapshot,
            l,
            x + pad,
            cy - 4.0,
            gdk::RGBA::new(0.8, 0.83, 0.9, 0.5 * a),
        );
        cy += foot_h;
    }
    let mut kx = x + pad;
    for (key, words) in &c.keys {
        kx += keycap(widget, snapshot, key, kx, cy, a) + 6.0;
        let l = layout(widget, words, 12.0, false, None);
        text_at(
            snapshot,
            &l,
            kx,
            cy + 2.0,
            gdk::RGBA::new(0.8, 0.83, 0.9, 0.7 * a),
        );
        kx += l.pixel_size().0 as f32 + 18.0;
    }
}

/// A line with round ends and a faint glow either side of it.
fn draw_line(snapshot: &gtk::Snapshot, l: &Line) {
    if l.alpha <= 0.004 {
        return;
    }
    let builder = gsk::PathBuilder::new();
    builder.move_to(l.a.0 as f32, l.a.1 as f32);
    builder.line_to(l.b.0 as f32, l.b.1 as f32);
    let path = builder.to_path();
    let [r, g, b] = l.color;
    for (width, share) in [(5.0, 0.22), (1.3, 1.0)] {
        let stroke = gsk::Stroke::new(width);
        stroke.set_line_cap(gsk::LineCap::Round);
        snapshot.append_stroke(&path, &stroke, &gdk::RGBA::new(r, g, b, l.alpha * share));
    }
}

fn draw_silhouette(snapshot: &gtk::Snapshot, s: &Silhouette) {
    if s.alpha <= 0.004 || s.path.len() < 2 {
        return;
    }
    let builder = gsk::PathBuilder::new();
    builder.move_to(s.path[0].0 as f32, s.path[0].1 as f32);
    for &(x, y) in &s.path[1..] {
        builder.line_to(x as f32, y as f32);
    }
    if s.filled {
        builder.close();
    }
    let path = builder.to_path();
    let [r, g, b] = s.color;
    let rgba = gdk::RGBA::new(r, g, b, s.alpha.min(1.0));
    if s.filled {
        snapshot.append_fill(&path, gsk::FillRule::Winding, &rgba);
    } else {
        let stroke = gsk::Stroke::new(s.width);
        stroke.set_line_cap(gsk::LineCap::Round);
        stroke.set_line_join(gsk::LineJoin::Round);
        snapshot.append_stroke(&path, &stroke, &rgba);
    }
}

fn draw_point(snapshot: &gtk::Snapshot, p: &Point) {
    let [r, g, b] = p.color;
    let (x, y) = (p.x as f32, p.y as f32);
    if p.halo > 0.01 {
        let reach = p.radius * 4.5;
        let stops = [
            gsk::ColorStop::new(0.0, gdk::RGBA::new(r, g, b, 0.32 * p.halo * p.alpha)),
            gsk::ColorStop::new(0.35, gdk::RGBA::new(r, g, b, 0.08 * p.halo * p.alpha)),
            gsk::ColorStop::new(1.0, gdk::RGBA::new(r, g, b, 0.0)),
        ];
        snapshot.append_radial_gradient(
            &graphene::Rect::new(x - reach, y - reach, 2.0 * reach, 2.0 * reach),
            &graphene::Point::new(x, y),
            reach,
            reach,
            0.0,
            1.0,
            &stops,
        );
    }
    let rect = graphene::Rect::new(x - p.radius, y - p.radius, 2.0 * p.radius, 2.0 * p.radius);
    snapshot.push_rounded_clip(&gsk::RoundedRect::from_rect(rect, p.radius));
    snapshot.append_color(&gdk::RGBA::new(r, g, b, p.alpha.min(1.0)), &rect);
    snapshot.pop();
}

fn draw_glow(snapshot: &gtk::Snapshot, g: &Glow) {
    let (x, y) = (g.x as f32, g.y as f32);
    let s = g.strength.clamp(0.0, 1.0) as f32;
    if s > 0.004 {
        // Warm at the heart, fading to nothing: wider and brighter the nearer.
        let core = g.radius.max(2.0) as f32;
        let reach = core + 12.0 + 30.0 * s;
        let warm = |a: f32| gdk::RGBA::new(1.0, 0.9, 0.72, a);
        let stops = [
            gsk::ColorStop::new(0.0, warm(0.6 * s)),
            gsk::ColorStop::new(core / reach, warm(0.38 * s)),
            gsk::ColorStop::new((core / reach + 1.0) / 2.0, warm(0.1 * s)),
            gsk::ColorStop::new(1.0, warm(0.0)),
        ];
        snapshot.append_radial_gradient(
            &graphene::Rect::new(x - reach, y - reach, 2.0 * reach, 2.0 * reach),
            &graphene::Point::new(x, y),
            reach,
            reach,
            0.0,
            1.0,
            &stops,
        );
    }
    let ring = g.ring.clamp(0.0, 1.0) as f32;
    if ring > 0.004 {
        // Settling inwards as it appears.
        let r = (g.radius.max(3.0) + 7.0 + 5.0 * (1.0 - g.ring)) as f32;
        let rect = graphene::Rect::new(x - r, y - r, 2.0 * r, 2.0 * r);
        let colour = gdk::RGBA::new(0.9, 0.93, 1.0, 0.55 * ring);
        snapshot.append_border(
            &gsk::RoundedRect::from_rect(rect, r),
            &[1.2; 4],
            &[colour; 4],
        );
    }
}

fn draw_eyepiece(_widget: &gtk::Widget, snapshot: &gtk::Snapshot, e: &Eyepiece) {
    let a = e.alpha as f32;
    let (x, y, r) = (
        (e.x + e.offset.0) as f32,
        (e.y + e.offset.1) as f32,
        e.radius as f32,
    );
    let rh = r * e.aspect as f32;
    let bounds = graphene::Rect::new(x - r, y - r, 2.0 * r, 2.0 * r);
    let picture = |snapshot: &gtk::Snapshot| {
        snapshot.save();
        snapshot.translate(&graphene::Point::new(x, y));
        snapshot.rotate(e.rotation as f32);
        snapshot.append_texture(&e.texture, &graphene::Rect::new(-r, -rh, 2.0 * r, 2.0 * rh));
        snapshot.restore();
    };
    let white = |alpha: f32| gdk::RGBA::new(1.0, 1.0, 1.0, alpha);
    if e.disc {
        // The disc itself, solid, with a soft limb.
        let d = crate::eyepiece::DISC as f32;
        let edge = [
            gsk::ColorStop::new(0.0, white(a)),
            gsk::ColorStop::new(d - 0.015, white(a)),
            gsk::ColorStop::new(d + 0.01, white(0.0)),
        ];
        snapshot.push_mask(gsk::MaskMode::Alpha);
        snapshot.append_radial_gradient(
            &bounds,
            &graphene::Point::new(x, y),
            r,
            r,
            0.0,
            1.0,
            &edge,
        );
        snapshot.pop();
        picture(snapshot);
        snapshot.pop();
    } else {
        // Light laid over the sky: dark parts of the picture let the sky
        // through, and the edges melt away.
        let edge = [
            gsk::ColorStop::new(0.0, white(a)),
            gsk::ColorStop::new(0.55, white(a)),
            gsk::ColorStop::new(0.98, white(0.0)),
        ];
        // The fade follows the picture's shape, turned with it.
        snapshot.push_mask(gsk::MaskMode::Alpha);
        snapshot.save();
        snapshot.translate(&graphene::Point::new(x, y));
        snapshot.rotate(e.rotation as f32);
        snapshot.append_radial_gradient(
            &graphene::Rect::new(-r, -rh, 2.0 * r, 2.0 * rh),
            &graphene::Point::new(0.0, 0.0),
            r,
            rh,
            0.0,
            1.0,
            &edge,
        );
        snapshot.restore();
        snapshot.pop();
        snapshot.push_mask(gsk::MaskMode::Luminance);
        picture(snapshot);
        snapshot.pop();
        picture(snapshot);
        snapshot.pop();
        snapshot.pop();
    }
}

fn draw_compass(
    widget: &gtk::Widget,
    snapshot: &gtk::Snapshot,
    width: f32,
    height: f32,
    c: &Compass,
) {
    let a = c.alpha as f32;
    let strip_h = 34.0f32;
    let top = if c.bottom {
        height - strip_h - 6.0
    } else {
        12.0
    };
    let strip_w = 600.0f32.min(width - 40.0);
    let x0 = (width - strip_w) / 2.0;
    panel(
        snapshot,
        &graphene::Rect::new(x0, top, strip_w, strip_h),
        10.0,
        0.55 * a,
    );
    // 160 degrees across the strip, the view's heading in the middle.
    let span = 160.0;
    let at = |az: f64| {
        let d = (az - c.heading + 540.0).rem_euclid(360.0) - 180.0;
        (d.abs() <= span / 2.0).then(|| x0 + strip_w / 2.0 + (d / span) as f32 * strip_w)
    };
    for step in (0..360).step_by(5) {
        let Some(x) = at(step as f64) else { continue };
        let edge = 1.0 - ((x - (x0 + strip_w / 2.0)).abs() / (strip_w / 2.0)).powi(2);
        let (len, strong) = if step % 45 == 0 {
            (0.0, 0.0)
        } else if step % 15 == 0 {
            (7.0, 0.35)
        } else {
            (4.0, 0.2)
        };
        if len > 0.0 {
            snapshot.append_color(
                &gdk::RGBA::new(0.85, 0.88, 0.95, strong * a * edge),
                &graphene::Rect::new(x - 0.5, top + strip_h - len - 5.0, 1.0, len),
            );
        }
        if step % 45 == 0 {
            let name = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"][step as usize / 45];
            let main = name.len() == 1;
            let l = layout(widget, name, if main { 15.0 } else { 11.5 }, main, None);
            let (lw, lh) = l.pixel_size();
            let colour = if name == "N" {
                gdk::RGBA::new(1.0, 0.78, 0.55, 0.95 * a * edge)
            } else {
                gdk::RGBA::new(0.92, 0.94, 0.98, (if main { 0.9 } else { 0.6 }) * a * edge)
            };
            text_at(
                snapshot,
                &l,
                x - lw as f32 / 2.0,
                top + (strip_h - lh as f32) / 2.0,
                colour,
            );
        }
    }
    // Where the view is pointing.
    let cx = x0 + strip_w / 2.0;
    snapshot.save();
    snapshot.translate(&graphene::Point::new(cx, top + strip_h + 1.0));
    snapshot.rotate(45.0);
    snapshot.append_color(
        &gdk::RGBA::new(1.0, 0.8, 0.55, 0.85 * a),
        &graphene::Rect::new(-4.0, -4.0, 8.0, 8.0),
    );
    snapshot.restore();
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct SkyView {
        pub frame: RefCell<Frame>,
        pub tile: RefCell<Option<(f32, i32, gdk::Texture)>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SkyView {
        const NAME: &'static str = "WesteringView";
        type Type = super::SkyView;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for SkyView {}

    impl WidgetImpl for SkyView {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();
            let (w, h) = (widget.width() as f32, widget.height() as f32);
            let frame = self.frame.borrow();
            let bg = frame.background;
            snapshot.append_color(
                &gdk::RGBA::new(bg[0], bg[1], bg[2], 1.0),
                &graphene::Rect::new(0.0, 0.0, w, h),
            );
            if let Some(texture) = &frame.texture {
                let pitch = frame.pitch;
                let scale = widget.scale_factor();
                let tile = {
                    let mut cached = self.tile.borrow_mut();
                    match &*cached {
                        Some((p, s, t)) if *p == pitch && *s == scale => t.clone(),
                        _ => {
                            let t = crate::field::dot_tile(pitch, scale as f64);
                            *cached = Some((pitch, scale, t.clone()));
                            t
                        }
                    }
                };
                let bounds = graphene::Rect::new(
                    0.0,
                    0.0,
                    frame.cols as f32 * pitch,
                    frame.rows as f32 * pitch,
                );
                let cell = graphene::Rect::new(0.0, 0.0, pitch, pitch);
                snapshot.push_mask(gsk::MaskMode::Alpha);
                snapshot.push_repeat(&bounds, Some(&cell));
                snapshot.append_texture(&tile, &cell);
                snapshot.pop();
                snapshot.pop();
                snapshot.append_scaled_texture(texture, gsk::ScalingFilter::Nearest, &bounds);
                snapshot.pop();
                // Living things are drawn in the same dots as everything else.
                if !frame.silhouettes.is_empty() {
                    snapshot.push_mask(gsk::MaskMode::Alpha);
                    snapshot.push_repeat(&bounds, Some(&cell));
                    snapshot.append_texture(&tile, &cell);
                    snapshot.pop();
                    snapshot.pop();
                    for s in &frame.silhouettes {
                        draw_silhouette(snapshot, s);
                    }
                    snapshot.pop();
                }
            }
            for p in &frame.points {
                draw_point(snapshot, p);
            }
            for l in &frame.lines {
                draw_line(snapshot, l);
            }
            for g in &frame.glows {
                draw_glow(snapshot, g);
            }
            if let Some(e) = &frame.eyepiece
                && e.alpha > 0.004
            {
                draw_eyepiece(widget.upcast_ref(), snapshot, e);
            }
            for p in &frame.marks {
                draw_point(snapshot, p);
            }
            for sprite in &frame.sprites {
                snapshot.push_opacity(sprite.alpha.clamp(0.0, 1.0));
                snapshot.append_texture(
                    &sprite.texture,
                    &graphene::Rect::new(
                        sprite.x as f32,
                        sprite.y as f32,
                        sprite.width as f32,
                        sprite.height as f32,
                    ),
                );
                snapshot.pop();
            }
            if let Some(b) = &frame.bubble
                && b.alpha > 0.004
            {
                let layout = widget.create_pango_layout(Some(&b.text));
                let mut font = widget
                    .pango_context()
                    .font_description()
                    .unwrap_or_default();
                font.set_absolute_size(15.0 * pango::SCALE as f64);
                layout.set_font_description(Some(&font));
                layout.set_width((b.width * pango::SCALE as f64) as i32);
                layout.set_wrap(pango::WrapMode::WordChar);
                let (lw, lh) = layout.pixel_size();
                let pad = 14.0f32;
                let dots = if b.more.is_some() { 14.0 } else { 0.0 };
                let rect = graphene::Rect::new(
                    b.x as f32,
                    b.bottom as f32 - lh as f32 - 2.0 * pad - dots,
                    (lw as f32).max(60.0) + 2.0 * pad,
                    lh as f32 + 2.0 * pad + dots,
                );
                let a = b.alpha as f32;
                snapshot.push_rounded_clip(&gsk::RoundedRect::from_rect(rect, 14.0));
                snapshot.append_color(&gdk::RGBA::new(0.05, 0.06, 0.11, 0.86 * a), &rect);
                snapshot.pop();
                // A small tail down towards the wisp.
                let tail =
                    graphene::Rect::new(b.x as f32 + 34.0, b.bottom as f32 - 7.0, 12.0, 12.0);
                if b.tail {
                    snapshot.save();
                    snapshot.translate(&graphene::Point::new(tail.x() + 6.0, tail.y() + 6.0));
                    snapshot.rotate(45.0);
                    snapshot.append_color(
                        &gdk::RGBA::new(0.05, 0.06, 0.11, 0.86 * a),
                        &graphene::Rect::new(-6.0, -6.0, 12.0, 12.0),
                    );
                    snapshot.restore();
                }
                snapshot.save();
                snapshot.translate(&graphene::Point::new(rect.x() + pad, rect.y() + pad));
                snapshot.append_layout(&layout, &gdk::RGBA::new(1.0, 0.96, 0.88, 0.95 * a));
                snapshot.restore();
                if let Some((t, how)) = b.more {
                    // Says so in words, beside three dots brightening in turn.
                    let more = widget.create_pango_layout(Some(how));
                    let mut small = widget
                        .pango_context()
                        .font_description()
                        .unwrap_or_default();
                    small.set_absolute_size(11.0 * pango::SCALE as f64);
                    more.set_font_description(Some(&small));
                    let (mw, mh) = more.pixel_size();
                    snapshot.save();
                    snapshot.translate(&graphene::Point::new(
                        rect.x() + rect.width() - pad - 28.0 - mw as f32,
                        rect.y() + rect.height() - 12.0 - mh as f32 / 2.0,
                    ));
                    snapshot.append_layout(&more, &gdk::RGBA::new(1.0, 0.9, 0.72, 0.55 * a));
                    snapshot.restore();
                    for k in 0..3 {
                        let phase = ((t * 1.6 - k as f64 * 0.22).rem_euclid(1.0)
                            * std::f64::consts::TAU)
                            .sin();
                        let lit = 0.35 + 0.5 * phase.max(0.0) as f32;
                        let cx = rect.x() + rect.width() - pad - 20.0 + k as f32 * 8.0;
                        let cy = rect.y() + rect.height() - 12.0;
                        let dot = graphene::Rect::new(cx - 2.2, cy - 2.2, 4.4, 4.4);
                        snapshot.push_rounded_clip(&gsk::RoundedRect::from_rect(dot, 2.2));
                        snapshot.append_color(&gdk::RGBA::new(1.0, 0.9, 0.72, lit * a), &dot);
                        snapshot.pop();
                    }
                }
            }
            if let Some(c) = &frame.compass {
                draw_compass(widget.upcast_ref(), snapshot, w, h, c);
            }
            if let Some(l) = &frame.legend
                && l.alpha > 0.004
                && !l.whole
            {
                draw_legend(widget.upcast_ref(), snapshot, w, h, l);
            }
            if let Some(c) = &frame.card
                && c.alpha > 0.004
            {
                draw_card(widget.upcast_ref(), snapshot, c);
            }
            for t in &frame.texts {
                if t.alpha <= 0.004 || t.text.is_empty() {
                    continue;
                }
                let layout = widget.create_pango_layout(Some(&t.text));
                let mut font = widget
                    .pango_context()
                    .font_description()
                    .unwrap_or_default();
                font.set_absolute_size(t.size * pango::SCALE as f64);
                font.set_weight(if t.bold {
                    pango::Weight::Semibold
                } else {
                    pango::Weight::Normal
                });
                layout.set_font_description(Some(&font));
                if let Some(wrap) = t.wrap {
                    layout.set_width((wrap * pango::SCALE as f64) as i32);
                    layout.set_wrap(pango::WrapMode::WordChar);
                    if t.align == Align::Centre {
                        layout.set_alignment(pango::Alignment::Center);
                    }
                }
                let (lw, _) = layout.pixel_size();
                let x = match (t.align, t.wrap) {
                    (Align::Left, _) => t.x,
                    // Pango centres wrapped lines within the wrap width itself.
                    (Align::Centre, Some(wrap)) => t.x - wrap / 2.0,
                    (Align::Centre, None) => t.x - lw as f64 / 2.0,
                };
                snapshot.save();
                snapshot.translate(&graphene::Point::new(x as f32, t.y as f32));
                let c = t.color;
                // A soft dark shadow keeps words clear of the dots behind.
                snapshot.push_shadow(&[gsk::Shadow::new(
                    gdk::RGBA::new(0.0, 0.01, 0.03, 0.7 * t.alpha as f32),
                    0.0,
                    1.0,
                    3.0,
                )]);
                snapshot.append_layout(&layout, &gdk::RGBA::new(c[0], c[1], c[2], t.alpha as f32));
                snapshot.pop();
                snapshot.restore();
            }
            // The whole table of keys goes over everything else on the sky.
            if let Some(l) = &frame.legend
                && l.alpha > 0.004
                && l.whole
            {
                draw_legend(widget.upcast_ref(), snapshot, w, h, l);
            }
            if frame.veil > 0.0 {
                snapshot.append_color(
                    &gdk::RGBA::new(0.0, 0.0, 0.0, frame.veil.min(1.0)),
                    &graphene::Rect::new(0.0, 0.0, w, h),
                );
            }
        }
    }
}

glib::wrapper! {
    pub struct SkyView(ObjectSubclass<imp::SkyView>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for SkyView {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl SkyView {
    pub fn show(&self, frame: Frame) {
        *self.imp().frame.borrow_mut() = frame;
        self.queue_draw();
    }
}

use gtk::glib;
