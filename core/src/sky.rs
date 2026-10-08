//! The whole sky for one observer: what is up, how dark it is, and when
//! things rise and set.

use crate::catalogues::Catalogues;
use crate::coords::{Observer, observe, topocentric};
use crate::ephem::{Body, Position, position};
use crate::figures::{self, Figure};
use crate::stars::Catalogue;
use crate::time::{MINUTE, UnixMs};

pub struct Sky {
    pub stars: Catalogue,
    pub figures: Vec<Figure>,
    pub notes: Vec<figures::Note>,
    pub lists: Catalogues,
    pub tours: crate::tours::Tours,
}

impl Sky {
    pub fn bundled() -> Sky {
        Sky {
            stars: Catalogue::bundled(),
            figures: figures::bundled(),
            notes: figures::notes(),
            lists: Catalogues::bundled(),
            tours: crate::tours::Tours::bundled(),
        }
    }
}

/// A body as someone on the ground sees it.
#[derive(Clone, Copy, Debug)]
pub struct Seen {
    pub alt: f64,
    pub az: f64,
    /// Right ascension and declination as seen from the ground.
    pub ra: f64,
    pub dec: f64,
    pub position: Position,
}

pub fn see(body: Body, observer: Observer, at: UnixMs) -> Seen {
    let p = position(body, at);
    let (ra, dec) = if body == Body::Moon {
        topocentric(observer, at, p.ra, p.dec, p.distance)
    } else {
        (p.ra, p.dec)
    };
    let (alt, az) = observe(observer, at, ra, dec);
    Seen {
        alt,
        az,
        ra,
        dec,
        position: p,
    }
}

pub fn sun_altitude(observer: Observer, at: UnixMs) -> f64 {
    see(Body::Sun, observer, at).alt
}

/// The faintest magnitude an eye can pick out for the Sun's altitude,
/// assuming a suburban sky once it is fully dark.
pub fn limiting_magnitude(sun_alt: f64) -> f64 {
    const STOPS: [(f64, f64); 5] = [
        (-18.0, 6.0),
        (-12.0, 4.5),
        (-6.0, 1.5),
        (-1.0, -1.5),
        (5.0, -4.0),
    ];
    if sun_alt <= STOPS[0].0 {
        return STOPS[0].1;
    }
    for pair in STOPS.windows(2) {
        let ((a, ma), (b, mb)) = (pair[0], pair[1]);
        if sun_alt <= b {
            return ma + (mb - ma) * (sun_alt - a) / (b - a);
        }
    }
    STOPS[4].1
}

/// The first moment in `from..from + span` at which `alt` climbs through
/// `h0` (rising, `up = true`) or falls through it (setting).
pub fn crossing(
    from: UnixMs,
    span: UnixMs,
    h0: f64,
    up: bool,
    alt: impl Fn(UnixMs) -> f64,
) -> Option<UnixMs> {
    let step = 10 * MINUTE;
    let mut t = from;
    let mut a = alt(t) - h0;
    while t < from + span {
        let t2 = t + step;
        let b = alt(t2) - h0;
        if (up && a < 0.0 && b >= 0.0) || (!up && a >= 0.0 && b < 0.0) {
            let (mut lo, mut hi) = (t, t2);
            while hi - lo > 20_000 {
                let mid = (lo + hi) / 2;
                let m = alt(mid) - h0;
                if (m >= 0.0) == up {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            return Some(hi);
        }
        t = t2;
        a = b;
    }
    None
}

/// Standard altitudes for rising and setting: the Sun's and Moon's upper
/// limbs touching the horizon, and a point of light for everything else.
/// Refraction is already in the altitudes `see` returns.
pub fn horizon_altitude(body: Body) -> f64 {
    match body {
        Body::Sun => -0.27,
        Body::Moon => -0.25,
        _ => 0.0,
    }
}

pub fn next_rise(body: Body, observer: Observer, from: UnixMs, span: UnixMs) -> Option<UnixMs> {
    crossing(from, span, horizon_altitude(body), true, |t| {
        see(body, observer, t).alt
    })
}

pub fn next_set(body: Body, observer: Observer, from: UnixMs, span: UnixMs) -> Option<UnixMs> {
    crossing(from, span, horizon_altitude(body), false, |t| {
        see(body, observer, t).alt
    })
}

/// When and where something that's down next comes up: the moment it
/// climbs through the horizon within `span`, and the bearing it does so
/// at. `place` gives its altitude and azimuth at a moment.
pub fn rising(
    from: UnixMs,
    span: UnixMs,
    place: impl Fn(UnixMs) -> (f64, f64),
) -> Option<(UnixMs, f64)> {
    let at = crossing(from, span, 0.0, true, |t| place(t).0)?;
    Some((at, place(at).1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::{DAY, HOUR, midnight_utc};

    const LONDON: Observer = Observer {
        lat: 51.5072,
        lon: -0.1276,
    };

    #[test]
    fn something_down_comes_up_in_the_east_not_where_it_is_now() {
        // The Eta Aquariids' radiant on a May evening in London: under the
        // northern horizon at nine, and up due east in the small hours.
        let place = |t: UnixMs| {
            let hz = crate::coords::horizon(LONDON, t);
            let v = crate::coords::apply(
                &crate::coords::precession(t),
                crate::coords::unit(338.0, -1.0),
            );
            crate::coords::alt_az(crate::coords::apply(&hz, v))
        };
        let evening = midnight_utc(2026, 5, 6) + 20 * HOUR;
        let (alt, az) = place(evening);
        assert!(alt < -30.0, "{alt}");
        assert!(!(45.0..315.0).contains(&az), "now in the north: {az}");
        let (at, az) = rising(evening, 12 * HOUR, place).expect("it rises");
        assert!((80.0..100.0).contains(&az), "rises in the east: {az}");
        let hours = (at - evening) as f64 / HOUR as f64;
        assert!((4.0..7.0).contains(&hours), "{hours} hours on");
        assert!(place(at).0.abs() < 0.2);
        // Something that's up already has no rising to wait for.
        assert!(rising(at + HOUR, HOUR, place).is_none());
    }

    #[test]
    fn the_sun_sets_in_london_at_the_published_time() {
        // Sunset in London on 21 June 2026 is 21:21 BST (20:21 UTC).
        let from = midnight_utc(2026, 6, 21) + 12 * HOUR;
        let set = next_set(Body::Sun, LONDON, from, DAY).expect("sets");
        let expected = midnight_utc(2026, 6, 21) + 20 * HOUR + 21 * MINUTE;
        assert!(
            (set - expected).abs() < 3 * MINUTE,
            "{} minutes off",
            (set - expected) / MINUTE
        );
    }

    #[test]
    fn darkness_shows_more_stars() {
        assert!(limiting_magnitude(10.0) < -3.0);
        assert!(limiting_magnitude(-9.0) > 2.5);
        assert_eq!(limiting_magnitude(-30.0), 6.0);
    }
}
