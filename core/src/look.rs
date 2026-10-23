//! How the wisp looks, from `core/data/wisp.toml`.

use serde::Deserialize;

use crate::oklab::Rgb;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    toward: f64,
    warm: RawLook,
    cool: RawLook,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLook {
    core: String,
    glow: String,
    brightness: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub core: Rgb,
    pub glow: Rgb,
    pub brightness: f64,
}

impl Look {
    pub fn bundled() -> Look {
        parse(include_str!("../data/wisp.toml")).expect("core/data/wisp.toml is valid")
    }
}

fn parse(text: &str) -> Result<Look, String> {
    let file: File = toml::from_str(text).map_err(|e| e.to_string())?;
    if !(0.0..=1.0).contains(&file.toward) {
        return Err("toward must be between 0 and 1".into());
    }
    let (warm, cool) = (resolve(&file.warm)?, resolve(&file.cool)?);
    let t = file.toward;
    Ok(Look {
        core: warm.core.mix(cool.core, t),
        glow: warm.glow.mix(cool.glow, t),
        brightness: warm.brightness + (cool.brightness - warm.brightness) * t,
    })
}

fn resolve(raw: &RawLook) -> Result<Look, String> {
    let colour = |hex: &str| Rgb::from_hex(hex).ok_or_else(|| format!("{hex} must be #rrggbb"));
    if !(0.0..=1.0).contains(&raw.brightness) {
        return Err(format!(
            "brightness {} must be between 0 and 1",
            raw.brightness
        ));
    }
    Ok(Look {
        core: colour(&raw.core)?,
        glow: colour(&raw.glow)?,
        brightness: raw.brightness,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_look_lies_between_warm_and_cool() {
        let look = Look::bundled();
        assert!(look.brightness < 1.0 && look.brightness > 0.95);
        let warm = Rgb::from_hex("#f2c063").unwrap();
        let cool = Rgb::from_hex("#e6d39c").unwrap();
        assert!(
            look.glow.0 < warm.0 && look.glow.0 > cool.0,
            "{:?}",
            look.glow
        );
    }

    #[test]
    fn broken_looks_are_rejected() {
        let good = include_str!("../data/wisp.toml");
        assert!(parse(good).is_ok());
        assert!(parse(&good.replace("#f2c063", "gold")).is_err());
        assert!(parse(&good.replace("toward = 0.142857142857", "toward = 2.0")).is_err());
        assert!(parse(&good.replace("brightness = 0.95", "brightness = 1.5")).is_err());
    }
}
