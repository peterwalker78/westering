//! Photographs of what's been found. Close in on something already found and
//! a real photograph of it takes over from the dots, at its true size on the
//! sky and turned as it sits there tonight. The Moon and Mercury wear
//! tonight's phase.

use gtk::{gdk, glib, prelude::*};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Clone, Debug, Deserialize)]
pub struct Credit {
    pub id: String,
    pub title: String,
    pub credit: String,
    pub licence: String,
    pub source: String,
    /// Degrees north is turned clockwise from straight up in the file.
    #[serde(default)]
    pub north: Option<f64>,
    /// How wide the picture is on the sky, degrees, when it isn't simply
    /// the object's own size.
    #[serde(default)]
    pub degrees: Option<f64>,
    /// Where the object sits in the picture, as shares of its width and
    /// height, when it isn't in the middle.
    #[serde(default)]
    pub centre: Option<[f64; 2]>,
    /// How wide a view shows it best, degrees.
    #[serde(default)]
    pub view: Option<f64>,
    /// What the picture shows that the eye wouldn't: false colour, a long
    /// exposure, light we can't see, marks the telescope makes.
    #[serde(default)]
    pub caption: Option<String>,
    /// Shown in the card rather than laid over the sky: a constellation's
    /// wide field, or a star's close-up.
    #[serde(default)]
    pub card_only: bool,
    /// The constellation a wide-field photograph shows, by abbreviation.
    #[serde(default)]
    pub constellation: Option<String>,
}

#[derive(Deserialize)]
struct Credits {
    image: Vec<Credit>,
}

/// How a disc is lit: which way the Sun is in the picture, and how far round.
#[derive(Clone, Copy, PartialEq)]
pub struct Phase {
    /// Towards the Sun, in the picture's own frame (y down), unit length.
    pub sun: (f64, f64),
    /// The phase angle, degrees: 0 full, 180 new.
    pub angle: f64,
}

pub struct Photos {
    dir: Option<PathBuf>,
    credits: Vec<Credit>,
    cache: Option<(String, gdk::Texture)>,
    /// The picture in the card showing, kept separately.
    card: Option<(String, gdk::Texture)>,
}

fn folder() -> Option<PathBuf> {
    let mut places = vec![PathBuf::from("/app/share/westering/images")];
    if let Ok(exe) = std::env::current_exe()
        && let Some(prefix) = exe.parent().and_then(|p| p.parent())
    {
        places.push(prefix.join("share/westering/images"));
    }
    places.push(PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/data/images"
    )));
    places
        .into_iter()
        .find(|p| p.join("credits.toml").is_file())
}

/// The share of the picture's width the disc of a Moon or planet fills.
pub const DISC: f64 = 0.88;

impl Photos {
    pub fn load() -> Photos {
        let dir = folder();
        let credits = dir
            .as_ref()
            .and_then(|d| std::fs::read_to_string(d.join("credits.toml")).ok())
            .and_then(|text| toml::from_str::<Credits>(&text).ok())
            .map(|c| c.image)
            .unwrap_or_default();
        Photos {
            dir,
            credits,
            cache: None,
            card: None,
        }
    }

    pub fn credits(&self) -> &[Credit] {
        &self.credits
    }

    pub fn credit(&self, id: &str) -> Option<&Credit> {
        self.credits.iter().find(|c| c.id == id)
    }

    /// The photograph for `id`, lit to `phase` if given. Kept while it's the
    /// one being looked at.
    pub fn texture(&mut self, id: &str, phase: Option<Phase>) -> Option<gdk::Texture> {
        self.credit(id)?;
        // Round the lighting so small drifts of the sky don't redo the work.
        let key = match phase {
            Some(p) => format!(
                "{id}:{:.0}:{:.0}",
                p.angle / 2.0,
                p.sun.1.atan2(p.sun.0).to_degrees() / 3.0
            ),
            None => id.to_owned(),
        };
        if let Some((k, t)) = &self.cache
            && *k == key
        {
            return Some(t.clone());
        }
        let path = self.dir.as_ref()?.join(format!("{id}.jpg"));
        let photo = gdk::Texture::from_filename(&path).ok()?;
        let texture = match phase {
            Some(p) => lit(&photo, p),
            None => photo,
        };
        self.cache = Some((key, texture.clone()));
        Some(texture)
    }
}

impl Photos {
    /// The picture for a card, by the photograph's id.
    pub fn card_texture(&mut self, id: &str) -> Option<gdk::Texture> {
        if let Some((k, t)) = &self.card
            && k == id
        {
            return Some(t.clone());
        }
        self.credit(id)?;
        let path = self.dir.as_ref()?.join(format!("{id}.jpg"));
        let texture = gdk::Texture::from_filename(&path).ok()?;
        self.card = Some((id.to_owned(), texture.clone()));
        Some(texture)
    }

    /// The card-only photograph of a constellation, by abbreviation.
    pub fn of_constellation(&self, abbrev: &str) -> Option<&Credit> {
        self.credits
            .iter()
            .find(|c| c.constellation.as_deref() == Some(abbrev))
    }
}

/// Darkens the part of a disc turned away from the Sun, leaving a trace of
/// earthshine, with a soft terminator.
fn lit(photo: &gdk::Texture, phase: Phase) -> gdk::Texture {
    let (w, h) = (photo.width() as usize, photo.height() as usize);
    let stride = w * 4;
    let mut data = vec![0u8; stride * h];
    photo.download(&mut data, stride);
    let (cx, cy) = (w as f64 / 2.0, h as f64 / 2.0);
    let radius = w.min(h) as f64 / 2.0 * DISC;
    let angle = phase.angle.to_radians();
    let (si, ci) = (angle.sin(), angle.cos());
    for y in 0..h {
        for x in 0..w {
            let u = (x as f64 + 0.5 - cx) / radius;
            let v = (y as f64 + 0.5 - cy) / radius;
            let rho2 = u * u + v * v;
            if rho2 > 1.08 {
                continue;
            }
            let z = (1.0 - rho2.min(1.0)).sqrt();
            let light = (u * phase.sun.0 + v * phase.sun.1) * si + z * ci;
            let t = (light * 7.0 + 0.5).clamp(0.0, 1.0);
            let day = t * t * (3.0 - 2.0 * t);
            let keep = (day + (1.0 - day) * 0.05) as f32;
            let i = y * stride + x * 4;
            for c in &mut data[i..i + 3] {
                *c = (*c as f32 * keep) as u8;
            }
        }
    }
    gdk::MemoryTexture::new(
        w as i32,
        h as i32,
        gdk::MemoryFormat::B8g8r8a8Premultiplied,
        &glib::Bytes::from_owned(data),
        stride,
    )
    .upcast()
}

#[cfg(test)]
mod tests {
    use super::*;
    use westering_core::sky::Sky;

    /// Anything that can be found or clicked has a photograph to show:
    /// its own, or for a named star its constellation's.
    #[test]
    fn everything_findable_is_pictured() {
        let photos = Photos::load();
        let dir = photos.dir.clone().expect("the photographs' folder");
        assert!(photos.credits.len() > 200, "credits.toml didn't load");
        for c in &photos.credits {
            let file = dir.join(format!("{}.jpg", c.id));
            assert!(file.is_file(), "no photograph on disk for {}", c.id);
        }
        let sky = Sky::bundled();
        let mut without = Vec::new();
        for figure in &sky.figures {
            if photos.of_constellation(&figure.abbrev).is_none() {
                without.push(figure.name.clone());
            }
        }
        for piece in &sky.lists.showpieces {
            if photos.credit(&piece.id).is_none() {
                without.push(piece.name.clone());
            }
        }
        for body in &sky.lists.bodies {
            if photos.credit(&body.id).is_none() {
                without.push(body.id.clone());
            }
        }
        for star in &sky.lists.stars {
            if photos.credit(&star.name.to_lowercase()).is_none()
                && photos.of_constellation(&star.constellation).is_none()
            {
                without.push(star.name.clone());
            }
        }
        assert!(without.is_empty(), "nothing to show for: {without:?}");
    }

    /// Every photograph says whose it is, under what licence, and where
    /// it came from.
    #[test]
    fn every_photograph_is_credited() {
        for c in Photos::load().credits() {
            assert!(!c.credit.trim().is_empty(), "{} has no credit", c.id);
            assert!(c.source.starts_with("https://"), "{} has no source", c.id);
            assert!(
                c.licence.starts_with("CC") || c.licence.starts_with("Public domain"),
                "{}: {}",
                c.id,
                c.licence
            );
        }
    }
}
