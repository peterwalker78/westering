//! The longer things a night can hold: star-hops that teach the way from a
//! star you know to one you don't, stories told round the stars involved,
//! a walk across the Moon's lit face, and Algol's eclipses.

use crate::finds::stable_hash;
use crate::time::UnixMs;
use serde::Deserialize;

/// Something a hop or a story points at: a star by HR number, or a showpiece.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Stop {
    pub hr: Option<u16>,
    pub showpiece: Option<String>,
    pub say: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Hop {
    pub id: String,
    pub title: String,
    pub hemisphere: String,
    pub steps: Vec<Stop>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Story {
    pub id: String,
    pub title: String,
    /// The star that has to be well up for the story to be told.
    pub anchor: u16,
    /// The thought it leaves you with.
    pub mirror: String,
    pub lines: Vec<Stop>,
    /// The lines to draw while it's told, when the story is about a shape
    /// within a constellation (the Plough, not all of the Great Bear) or
    /// across several (the Summer Triangle). Empty: the constellation its
    /// anchor belongs to.
    #[serde(default)]
    pub figure: Vec<[u16; 2]>,
    /// False for a story with no shape to draw: a cluster, a galaxy.
    #[serde(default = "yes")]
    pub pattern: bool,
}

fn yes() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize)]
pub struct MoonFeature {
    pub name: String,
    pub kind: String,
    /// Selenographic, degrees; east (towards Mare Crisium) is positive.
    pub lat: f64,
    pub lon: f64,
    pub diameter: Option<f64>,
    pub fact: String,
}

pub struct Trails {
    pub hops: Vec<Hop>,
    /// Said when a hop is done.
    pub hop_closing: String,
    /// The Moon walk's first and last words.
    pub moon_intro: String,
    pub moon_closing: String,
    pub stories: Vec<Story>,
    pub moon: Vec<MoonFeature>,
}

#[derive(Deserialize)]
struct HopFile {
    closing: String,
    hop: Vec<Hop>,
}

#[derive(Deserialize)]
struct StoryFile {
    story: Vec<Story>,
}

#[derive(Deserialize)]
struct MoonFile {
    intro: String,
    closing: String,
    feature: Vec<MoonFeature>,
}

impl Trails {
    pub fn bundled() -> Trails {
        let hops = toml::from_str::<HopFile>(include_str!("../data/hops.toml"))
            .expect("core/data/hops.toml is valid");
        let moon = toml::from_str::<MoonFile>(include_str!("../data/moon-features.toml"))
            .expect("core/data/moon-features.toml is valid");
        Trails {
            hops: hops.hop,
            hop_closing: hops.closing,
            stories: toml::from_str::<StoryFile>(include_str!("../data/stories.toml"))
                .expect("core/data/stories.toml is valid")
                .story,
            moon: moon.feature,
            moon_intro: moon.intro,
            moon_closing: moon.closing,
        }
    }
}

pub const SYNODIC_DAYS: f64 = 29.530_589;

/// The selenographic longitude the Sun stands over, for the Moon's age in
/// days: behind the Moon at new, over the middle of the near side at full.
pub fn subsolar_longitude(age_days: f64) -> f64 {
    (180.0 - age_days / SYNODIC_DAYS * 360.0 + 540.0).rem_euclid(360.0) - 180.0
}

/// How high the Sun stands over a feature, as the sine of its altitude:
/// below zero it's night there, and just above zero the shadows are long.
pub fn sunlight(feature: &MoonFeature, age_days: f64) -> f64 {
    let s = subsolar_longitude(age_days);
    feature.lat.to_radians().cos() * (feature.lon - s).to_radians().cos()
}

/// Where a feature sits on the Moon's disc as we see it, in radii: x
/// towards Mare Crisium, y towards the Moon's north.
pub fn on_disc(feature: &MoonFeature) -> (f64, f64) {
    let (lat, lon) = (feature.lat.to_radians(), feature.lon.to_radians());
    (lat.cos() * lon.sin(), lat.sin())
}

/// Tonight's walk across the Moon: up to four features in sunlight, the
/// craters and mountains near the edge of night where their shadows show,
/// with a sea and a landing site among them. Places visited before give way.
pub fn moon_stops(
    features: &[MoonFeature],
    age_days: f64,
    night: (i32, u32, u32),
    visited: &dyn Fn(&str) -> bool,
) -> Vec<usize> {
    let mut lit: Vec<usize> = (0..features.len())
        .filter(|&i| {
            let f = &features[i];
            // Clear of the limb, where everything is squashed flat.
            let facing = f.lat.to_radians().cos() * f.lon.to_radians().cos();
            sunlight(f, age_days) > 0.04 && facing > 0.25
        })
        .collect();
    lit.sort_by_key(|&i| {
        let name = &features[i].name;
        (visited(name), stable_hash(night, name))
    });
    let relief = |i: &usize| {
        matches!(
            features[*i].kind.as_str(),
            "crater" | "mountains" | "valley"
        )
    };
    let shadowed = |i: &usize| relief(i) && sunlight(&features[*i], age_days) < 0.45;
    let mut out: Vec<usize> = Vec::new();
    let take = |pick: Option<&usize>, out: &mut Vec<usize>| {
        if let Some(&i) = pick
            && !out.contains(&i)
        {
            out.push(i);
        }
    };
    take(lit.iter().find(|i| shadowed(i)), &mut out);
    take(lit.iter().find(|i| features[**i].kind == "sea"), &mut out);
    take(
        lit.iter().find(|i| features[**i].kind == "landing"),
        &mut out,
    );
    take(
        lit.iter().find(|i| shadowed(i) && !out.contains(i)),
        &mut out,
    );
    for i in &lit {
        if out.len() >= 4 {
            break;
        }
        take(Some(i), &mut out);
    }
    out.truncate(4);
    // West to east across the disc, so the walk doesn't zigzag.
    out.sort_by(|a, b| {
        on_disc(&features[*a])
            .0
            .total_cmp(&on_disc(&features[*b]).0)
    });
    out
}

/// Algol's eclipses: a primary minimum (Kreiner's ephemeris, in Julian days)
/// and the period in days.
const ALGOL_EPOCH_JD: f64 = 2_452_500.172;
const ALGOL_PERIOD: f64 = 2.867_338;
/// Half the length of an eclipse, in hours.
const ALGOL_HALF_HOURS: f64 = 4.8;

fn julian_day(at: UnixMs) -> f64 {
    at as f64 / 86_400_000.0 + 2_440_587.5
}

/// Hours from the nearest middle of an eclipse: negative before, positive after.
pub fn algol_hours_from_minimum(at: UnixMs) -> f64 {
    let cycles = (julian_day(at) - ALGOL_EPOCH_JD) / ALGOL_PERIOD;
    (cycles - cycles.round()) * ALGOL_PERIOD * 24.0
}

/// Algol's magnitude: 2.1 most of the time, fading to 3.4 for a few hours
/// every two days and twenty-one hours as its dimmer companion passes in front.
pub fn algol_magnitude(at: UnixMs) -> f64 {
    let h = algol_hours_from_minimum(at) / ALGOL_HALF_HOURS;
    if h.abs() >= 1.0 {
        2.12
    } else {
        2.12 + 1.27 * (1.0 - h * h).powf(1.5)
    }
}

/// When the middle of the eclipse under way at `at` falls, if one is.
pub fn algol_eclipse(at: UnixMs) -> Option<UnixMs> {
    let h = algol_hours_from_minimum(at);
    (h.abs() < ALGOL_HALF_HOURS - 0.8).then(|| at - (h * 3_600_000.0) as UnixMs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::{HOUR, midnight_utc};

    #[test]
    fn the_data_files_load_and_point_at_real_things() {
        let t = Trails::bundled();
        let sky = crate::sky::Sky::bundled();
        assert!(t.hops.len() >= 12 && t.stories.len() >= 20 && t.moon.len() >= 40);
        let real = |s: &Stop| match (&s.hr, &s.showpiece) {
            (Some(hr), None) => sky.stars.get(*hr).is_some(),
            (None, Some(id)) => sky.lists.showpieces.iter().any(|p| &p.id == id),
            (None, None) => true,
            _ => false,
        };
        for hop in &t.hops {
            assert!(hop.steps.len() >= 2, "{}", hop.id);
            assert!(
                hop.steps
                    .iter()
                    .all(|s| real(s) && (s.hr.is_some() || s.showpiece.is_some())),
                "{}",
                hop.id
            );
        }
        for story in &t.stories {
            assert!(sky.stars.get(story.anchor).is_some(), "{}", story.id);
            assert!(story.lines.iter().all(real), "{}", story.id);
            assert!(!story.mirror.is_empty());
            assert!(
                story
                    .figure
                    .iter()
                    .flatten()
                    .all(|hr| sky.stars.get(*hr).is_some()),
                "{}",
                story.id
            );
        }
    }

    #[test]
    fn a_story_ending_names_who_it_is_about() {
        // The last card is read on its own, so it can't lean on "he" or
        // "the two" from the cards before.
        for story in Trails::bundled().stories {
            let first = story.mirror.split_whitespace().next().unwrap_or("");
            assert!(
                !["He", "She", "They", "It", "It's", "Its", "This", "That"].contains(&first),
                "{}: {}",
                story.id,
                story.mirror
            );
        }
    }

    #[test]
    fn stories_never_name_the_feeling_either() {
        let t = Trails::bundled();
        for story in &t.stories {
            let all = std::iter::once(&story.mirror)
                .chain([&t.hop_closing, &t.moon_intro, &t.moon_closing])
                .chain(story.lines.iter().map(|l| &l.say))
                .map(|s| s.to_lowercase())
                .collect::<Vec<_>>()
                .join(" ");
            for word in crate::questions::UNSAID {
                assert!(!all.contains(word), "{}: {word}", story.id);
            }
        }
    }

    #[test]
    fn the_sun_rises_over_crisium_first() {
        let t = Trails::bundled();
        let crisium = t.moon.iter().find(|f| f.name == "Mare Crisium").unwrap();
        let procellarum = t
            .moon
            .iter()
            .find(|f| f.name == "Oceanus Procellarum")
            .unwrap();
        // A crescent five days old lights the Crisium side only.
        assert!(sunlight(crisium, 5.0) > 0.0);
        assert!(sunlight(procellarum, 5.0) < 0.0);
        // Full: both lit.
        assert!(sunlight(crisium, 14.8) > 0.0 && sunlight(procellarum, 14.8) > 0.0);
        assert!(on_disc(crisium).0 > 0.5);
    }

    #[test]
    fn a_moon_walk_is_lit_and_changes() {
        let t = Trails::bundled();
        let night = (2026, 10, 20);
        let stops = moon_stops(&t.moon, 8.0, night, &|_| false);
        assert!((3..=4).contains(&stops.len()), "{stops:?}");
        assert!(stops.iter().all(|&i| sunlight(&t.moon[i], 8.0) > 0.0));
        let visited: Vec<String> = stops.iter().map(|&i| t.moon[i].name.clone()).collect();
        let again = moon_stops(&t.moon, 8.0, night, &|n| visited.iter().any(|v| v == n));
        assert!(again.iter().filter(|i| stops.contains(i)).count() < stops.len());
    }

    #[test]
    fn algol_dims_on_schedule() {
        let epoch = ((ALGOL_EPOCH_JD - 2_440_587.5) * 86_400_000.0) as UnixMs;
        let later = epoch + (1000.0 * ALGOL_PERIOD * 86_400_000.0) as UnixMs;
        assert!((algol_magnitude(later) - 3.39).abs() < 0.01);
        assert!((algol_magnitude(later + 36 * HOUR) - 2.12).abs() < 1e-9);
        assert!(algol_magnitude(later + 3 * HOUR) > 2.3);
        assert!(algol_eclipse(later + HOUR).is_some_and(|m| (m - later).abs() < 1_000));
        assert_eq!(algol_eclipse(later + 20 * HOUR), None);
        // Every date has one within three days.
        let start = midnight_utc(2026, 11, 1);
        assert!((0..72).any(|h| algol_eclipse(start + h * HOUR).is_some()));
    }
}
