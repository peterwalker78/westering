//! The hand-kept lists: showpieces worth finding, named stars with a line
//! about each, and the year's meteor showers.

use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct Showpiece {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub ra: f64,
    pub dec: f64,
    /// Integrated magnitude; a dark nebula has none.
    #[serde(default = "no_light")]
    pub mag: f64,
    /// Apparent size in arcminutes; zero for a point.
    pub size: f64,
    pub fact: String,
    /// More to say on later visits.
    #[serde(default)]
    pub more: Vec<String>,
    /// From the longer list of fainter things, best seen zoomed in.
    #[serde(skip)]
    pub deep: bool,
}

impl Showpiece {
    /// What to say about it the `n`th time it's found, counting from zero.
    pub fn fact_for(&self, n: usize) -> String {
        say_for(&self.fact, &self.more, n)
    }
}

/// The first line, then each of the others in turn, then round again.
pub fn say_for(first: &str, more: &[String], n: usize) -> String {
    match n % (more.len() + 1) {
        0 => first.to_owned(),
        k => more[k - 1].clone(),
    }
}

fn no_light() -> f64 {
    99.0
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Cluster,
    Galaxy,
    Nebula,
    Double,
    Star,
    Dark,
    /// A rich patch of the Milky Way seen through a gap in nearer dust.
    Cloud,
    /// Stars that only look like a group from here: a chance line-up.
    Asterism,
}

#[derive(Clone, Debug, Deserialize)]
pub struct NamedStar {
    pub hr: u16,
    pub name: String,
    pub bayer: String,
    /// The constellation it belongs to, by abbreviation.
    pub constellation: String,
    #[serde(default)]
    pub fact: Option<String>,
    /// How far away, when it's known well enough to say.
    #[serde(default)]
    pub light_years: Option<f64>,
    #[serde(default)]
    pub more: Vec<String>,
}

/// A distance rounded down to a number that reads easily: 104 is "100",
/// 436 is "430", 1,180 is "1,100".
fn round_down(n: f64) -> String {
    let n = n.floor() as u64;
    let digits = n.to_string().len() as u32;
    let unit = if digits <= 2 {
        10
    } else {
        10u64.pow(digits - 2)
    };
    let r = (n / unit * unit).max(1);
    crate::finds::thousands(r)
}

/// How long starlight takes to arrive, and when what you see tonight left.
pub fn light_words(light_years: f64, this_year: i32) -> String {
    let left = this_year as f64 - light_years;
    if light_years < 20.0 {
        let years = if light_years.fract() >= 0.25 && light_years < 10.0 {
            format!("{light_years:.1}")
        } else {
            format!("{}", light_years.round())
        };
        return format!(
            "It's one of our nearest stars, yet its light still takes {years} years to get here: the light you see from it now set off in {}.",
            left.round()
        );
    }
    let year = if light_years > 300.0 {
        format!("around {}", (left / 10.0).round() * 10.0)
    } else {
        format!("in {}", left.round())
    };
    let over = if light_years.fract() > 0.0 || !(light_years.floor() as u64).is_multiple_of(10) {
        "over"
    } else {
        "about"
    };
    format!(
        "It's so far away that its light takes {over} {} years to get here: the light you see from it now left {year}.",
        round_down(light_years)
    )
}

impl NamedStar {
    /// Its line for the card, the light's journey worked out for `this_year`.
    pub fn describe(&self, this_year: i32) -> Option<String> {
        let light = self.light_years.map(|ly| light_words(ly, this_year));
        match (&self.fact, light) {
            (Some(f), Some(l)) => Some(format!("{f} {l}")),
            (Some(f), None) => Some(f.clone()),
            (None, Some(l)) => Some(l),
            (None, None) => None,
        }
    }

    /// Its line the `n`th time it's found: the first time, the usual one.
    pub fn describe_for(&self, this_year: i32, n: usize) -> Option<String> {
        let first = self.describe(this_year)?;
        Some(say_for(&first, &self.more, n))
    }
}

/// A few more lines about the Moon or a planet, for later visits.
#[derive(Clone, Debug, Deserialize)]
pub struct BodyFacts {
    pub id: String,
    pub facts: Vec<String>,
}

#[derive(Deserialize)]
struct BodyFile {
    body: Vec<BodyFacts>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Shower {
    pub id: String,
    pub name: String,
    /// The Sun's ecliptic longitude at the peak.
    pub peak_sol: f64,
    /// Month and day, "MM-DD".
    pub start: String,
    pub end: String,
    pub ra: f64,
    pub dec: f64,
    pub zhr: u32,
    /// Entry speed, km/s.
    pub speed: u32,
}

impl Shower {
    /// Whether `(month, day)` falls in the activity window, which may run
    /// over the new year.
    pub fn active_on(&self, month: u32, day: u32) -> bool {
        let parse = |s: &str| -> (u32, u32) {
            let (m, d) = s.split_once('-').unwrap_or(("1", "1"));
            (m.parse().unwrap_or(1), d.parse().unwrap_or(1))
        };
        let (start, end, today) = (parse(&self.start), parse(&self.end), (month, day));
        if start <= end {
            start <= today && today <= end
        } else {
            today >= start || today <= end
        }
    }
}

#[derive(Deserialize)]
struct ShowpieceFile {
    showpiece: Vec<Showpiece>,
}
#[derive(Deserialize)]
struct StarFile {
    star: Vec<NamedStar>,
}
#[derive(Deserialize)]
struct ShowerFile {
    shower: Vec<Shower>,
}

pub struct Catalogues {
    /// The showpieces, then the longer deep-sky list (marked `deep`).
    pub showpieces: Vec<Showpiece>,
    pub bodies: Vec<BodyFacts>,
    pub stars: Vec<NamedStar>,
    pub showers: Vec<Shower>,
}

impl Catalogues {
    pub fn bundled() -> Catalogues {
        let mut showpieces =
            toml::from_str::<ShowpieceFile>(include_str!("../data/showpieces.toml"))
                .expect("showpieces.toml")
                .showpiece;
        let deep = toml::from_str::<ShowpieceFile>(include_str!("../data/deep-sky.toml"))
            .expect("deep-sky.toml")
            .showpiece;
        showpieces.extend(deep.into_iter().map(|p| Showpiece { deep: true, ..p }));
        Catalogues {
            showpieces,
            bodies: toml::from_str::<BodyFile>(include_str!("../data/planet-facts.toml"))
                .expect("planet-facts.toml")
                .body,
            stars: toml::from_str::<StarFile>(include_str!("../data/star-names.toml"))
                .expect("star-names.toml")
                .star,
            showers: toml::from_str::<ShowerFile>(include_str!("../data/showers.toml"))
                .expect("showers.toml")
                .shower,
        }
    }

    pub fn star_name(&self, hr: u16) -> Option<&NamedStar> {
        self.stars.iter().find(|s| s.hr == hr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stars::Catalogue;

    #[test]
    fn the_lists_load_and_agree_with_the_catalogue() {
        let c = Catalogues::bundled();
        assert!(c.showpieces.len() >= 30);
        assert!(c.stars.len() >= 60);
        assert!(c.showers.len() >= 10);
        let cat = Catalogue::bundled();
        for s in &c.stars {
            assert!(
                cat.get(s.hr).is_some(),
                "{} (HR {}) not in the catalogue",
                s.name,
                s.hr
            );
        }
        let polaris = c
            .stars
            .iter()
            .find(|s| s.name == "Polaris")
            .expect("Polaris");
        assert_eq!(polaris.hr, 424);
    }

    #[test]
    fn starlight_is_explained_in_plain_words() {
        assert_eq!(
            light_words(104.0, 2026),
            "It's so far away that its light takes over 100 years to get here: the light you see from it now left in 1922."
        );
        assert_eq!(
            light_words(436.0, 2026),
            "It's so far away that its light takes over 430 years to get here: the light you see from it now left around 1590."
        );
        assert!(light_words(8.6, 2026).contains("8.6 years"));
        assert!(light_words(8.6, 2026).contains("set off in 2017"));
    }

    #[test]
    fn showers_can_span_the_new_year() {
        let c = Catalogues::bundled();
        let q = c
            .showers
            .iter()
            .find(|s| s.id == "quadrantids")
            .expect("Quadrantids");
        assert!(q.active_on(1, 3));
        assert!(q.active_on(12, 30));
        assert!(!q.active_on(6, 1));
    }
}
