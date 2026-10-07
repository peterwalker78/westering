//! A small chart of one night for its page in the logbook: the whole sky as
//! it stood when the first thing was found, seen looking straight up, with
//! what was found, what was drawn and what was set down.
//!
//! Places are on a disc of radius one: the middle is overhead and the edge
//! is the horizon. North is at the top and, because the chart is held
//! overhead rather than laid on the ground, east is on the left.

use crate::coords::{Observer, alt_az, apply, horizon, precession, unit};
use crate::journal::{Drawing, Night};
use crate::sky::Sky;

/// The faintest star drawn: enough for the brighter constellations to show.
const FAINTEST: f32 = 3.6;

#[derive(Clone, Debug, PartialEq)]
pub struct Chart {
    /// Bright stars, for the lie of the sky: place and magnitude.
    pub stars: Vec<(f64, f64, f32)>,
    /// What was found, in the order it was found.
    pub finds: Vec<(f64, f64, String)>,
    /// What was set down that night.
    pub weights: Vec<(f64, f64)>,
    /// Shapes drawn that night, star to star.
    pub lines: Vec<[(f64, f64); 2]>,
}

/// Where an altitude and azimuth fall on the disc, with y running down the
/// page.
pub fn place(alt: f64, az: f64) -> (f64, f64) {
    let r = (90.0 - alt.clamp(0.0, 90.0)) / 90.0;
    let a = az.to_radians();
    (-r * a.sin(), -r * a.cos())
}

/// The chart for a night's page, if the page says when and where its finds
/// were. Pages from before that was kept have none.
pub fn chart(sky: &Sky, observer: Observer, night: &Night, drawings: &[Drawing]) -> Option<Chart> {
    let at = night.at?;
    if night.spots.is_empty() {
        return None;
    }
    let hz = horizon(observer, at);
    let prec = precession(at);
    let up = |v| {
        let (alt, az) = alt_az(apply(&hz, v));
        (alt > 0.0).then(|| place(alt, az))
    };
    // Found a little later than the chart's moment, or hung just over the
    // hills: kept on the edge rather than dropped.
    let held = |ra: f64, dec: f64| {
        let (alt, az) = alt_az(apply(&hz, unit(ra, dec)));
        place(alt, az)
    };
    let stars = sky
        .stars
        .stars
        .iter()
        .take_while(|s| s.mag <= FAINTEST)
        .filter_map(|s| up(apply(&prec, s.dir)).map(|(x, y)| (x, y, s.mag)))
        .collect();
    let finds = night
        .spots
        .iter()
        .map(|s| {
            let (x, y) = held(s.ra, s.dec);
            (x, y, s.name.clone())
        })
        .collect();
    let weights = night.weights.iter().map(|w| held(w.ra, w.dec)).collect();
    let star = |hr: u16| sky.stars.get(hr).and_then(|s| up(apply(&prec, s.dir)));
    let lines = drawings
        .iter()
        .filter(|d| d.night == night.key)
        .flat_map(|d| d.edges.iter())
        .filter_map(|&(a, b)| Some([star(a)?, star(b)?]))
        .collect();
    Some(Chart {
        stars,
        finds,
        weights,
        lines,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::{NightWeight, Spot};
    use crate::time::{HOUR, midnight_utc};

    const LONDON: Observer = Observer {
        lat: 51.5072,
        lon: -0.1276,
    };

    fn near(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < 0.03 && (a.1 - b.1).abs() < 0.03
    }

    #[test]
    fn overhead_is_the_middle_north_the_top_and_east_the_left() {
        assert!(near(place(90.0, 123.0), (0.0, 0.0)));
        assert!(near(place(0.0, 0.0), (0.0, -1.0)));
        assert!(near(place(0.0, 90.0), (-1.0, 0.0)));
        assert!(near(place(0.0, 270.0), (1.0, 0.0)));
        // Below the horizon is held on the edge.
        assert!(near(place(-20.0, 180.0), (0.0, 1.0)));
    }

    fn page() -> Night {
        Night {
            key: "2026-10-11".into(),
            at: Some(midnight_utc(2026, 10, 11) + 21 * HOUR),
            spots: vec![Spot {
                name: "Polaris".into(),
                ra: 37.95,
                dec: 89.26,
            }],
            ..Night::default()
        }
    }

    #[test]
    fn the_pole_star_sits_due_north_as_high_as_the_latitude() {
        let sky = Sky::bundled();
        let chart = chart(&sky, LONDON, &page(), &[]).expect("a chart");
        let (x, y, name) = chart.finds[0].clone();
        assert_eq!(name, "Polaris");
        assert!(near((x, y), (0.0, -(90.0 - LONDON.lat) / 90.0)), "{x} {y}");
        // The sky's there to place it by, and all of it is above the horizon.
        assert!(chart.stars.len() > 50, "{}", chart.stars.len());
        for (x, y, _) in &chart.stars {
            assert!(x * x + y * y <= 1.0 + 1e-9);
        }
    }

    #[test]
    fn weights_and_drawn_shapes_are_on_it() {
        let sky = Sky::bundled();
        let mut night = page();
        night.weights.push(NightWeight {
            weight: 1,
            ra: 37.95,
            dec: 89.26,
        });
        // The Plough's pointers, Dubhe to Merak: never set from London.
        let drawn = [
            Drawing {
                name: "Pointers".into(),
                edges: vec![(4301, 4295)],
                night: night.key.clone(),
                reveals: None,
            },
            Drawing {
                name: "Another night's".into(),
                edges: vec![(4301, 4295)],
                night: "2026-10-01".into(),
                reveals: None,
            },
        ];
        let chart = chart(&sky, LONDON, &night, &drawn).expect("a chart");
        assert_eq!(chart.weights.len(), 1);
        assert_eq!(chart.lines.len(), 1);
    }

    #[test]
    fn an_older_page_has_no_chart() {
        let sky = Sky::bundled();
        let mut night = page();
        night.at = None;
        assert!(chart(&sky, LONDON, &night, &[]).is_none());
        let mut night = page();
        night.spots.clear();
        assert!(chart(&sky, LONDON, &night, &[]).is_none());
    }
}
