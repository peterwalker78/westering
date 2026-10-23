//! From the sky's own coordinates to where things stand for someone on the
//! ground: sidereal time, altitude and azimuth, precession of the catalogue
//! to the date, refraction near the horizon and the Moon's parallax.

use crate::ephem::{acosd, asind, atan2d, cosd, obliquity, rev, sidereal_time, sind};
use crate::time::{UnixMs, day_number};

/// Somewhere on the Earth, in degrees: north and east positive.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Observer {
    pub lat: f64,
    pub lon: f64,
}

pub type Vec3 = [f64; 3];

/// A unit vector for right ascension and declination (degrees).
pub fn unit(ra: f64, dec: f64) -> Vec3 {
    [cosd(dec) * cosd(ra), cosd(dec) * sind(ra), sind(dec)]
}

/// Right ascension and declination of a unit vector.
pub fn angles(v: Vec3) -> (f64, f64) {
    (rev(atan2d(v[1], v[0])), asind(v[2]))
}

pub type Mat3 = [[f64; 3]; 3];

pub fn apply(m: &Mat3, v: Vec3) -> Vec3 {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

pub fn dot(a: Vec3, b: Vec3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `v` scaled to length 1. A zero vector stays zero rather than turning
/// into NaNs.
pub fn normalise(v: Vec3) -> Vec3 {
    let n = dot(v, v).sqrt().max(1e-12);
    [v[0] / n, v[1] / n, v[2] / n]
}

/// Rows for columns: for a rotation, the turn back again.
pub fn transpose(m: &Mat3) -> Mat3 {
    [
        [m[0][0], m[1][0], m[2][0]],
        [m[0][1], m[1][1], m[2][1]],
        [m[0][2], m[1][2], m[2][2]],
    ]
}

/// The shortest signed turn from azimuth `from` to `to`, in degrees: from
/// -180 up to but not including 180, positive clockwise.
pub fn turn(from: f64, to: f64) -> f64 {
    (to - from + 540.0).rem_euclid(360.0) - 180.0
}

pub fn multiply(a: &Mat3, b: &Mat3) -> Mat3 {
    let mut out = [[0.0; 3]; 3];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    out
}

/// Turns J2000 catalogue vectors into vectors for the equinox of the date,
/// by turning the equinox along the ecliptic at the general precession rate.
pub fn precession(at: UnixMs) -> Mat3 {
    let d = day_number(at);
    let e0 = obliquity(0.0);
    let e1 = obliquity(d);
    let p = 3.82394e-5 * d;
    // Equatorial J2000 to ecliptic J2000.
    let to_ecl = [
        [1.0, 0.0, 0.0],
        [0.0, cosd(e0), sind(e0)],
        [0.0, -sind(e0), cosd(e0)],
    ];
    let turn = [
        [cosd(p), -sind(p), 0.0],
        [sind(p), cosd(p), 0.0],
        [0.0, 0.0, 1.0],
    ];
    let to_eq = [
        [1.0, 0.0, 0.0],
        [0.0, cosd(e1), -sind(e1)],
        [0.0, sind(e1), cosd(e1)],
    ];
    multiply(&to_eq, &multiply(&turn, &to_ecl))
}

/// Turns equatorial vectors of the date into horizon vectors
/// `[north, east, up]` for an observer at a moment.
pub fn horizon(observer: Observer, at: UnixMs) -> Mat3 {
    let theta = sidereal_time(at, observer.lon);
    let (st, ct) = (sind(theta), cosd(theta));
    let (sp, cp) = (sind(observer.lat), cosd(observer.lat));
    [
        [-sp * ct, -sp * st, cp],
        [-st, ct, 0.0],
        [cp * ct, cp * st, sp],
    ]
}

/// Altitude and azimuth (degrees; azimuth from north through east) of a
/// horizon vector.
pub fn alt_az(h: Vec3) -> (f64, f64) {
    (asind(h[2]), rev(atan2d(h[1], h[0])))
}

/// A horizon vector for an altitude and azimuth.
pub fn from_alt_az(alt: f64, az: f64) -> Vec3 {
    [cosd(alt) * cosd(az), cosd(alt) * sind(az), sind(alt)]
}

/// How much the air lifts something at true altitude `alt`, in degrees
/// (Sæmundsson's formula; about half a degree at the horizon).
pub fn refraction(alt: f64) -> f64 {
    if alt < -1.5 {
        return 0.0;
    }
    1.02 / 60.0 / (alt + 10.3 / (alt + 5.11)).to_radians().tan()
}

/// Altitude and azimuth of a body, refraction included.
pub fn observe(observer: Observer, at: UnixMs, ra: f64, dec: f64) -> (f64, f64) {
    let (alt, az) = alt_az(apply(&horizon(observer, at), unit(ra, dec)));
    (alt + refraction(alt), az)
}

/// The Moon as seen from the ground rather than the Earth's centre: moved
/// down towards the horizon by up to a degree. `distance` is in Earth radii.
pub fn topocentric(observer: Observer, at: UnixMs, ra: f64, dec: f64, distance: f64) -> (f64, f64) {
    let par = asind(1.0 / distance);
    let gclat = observer.lat - 0.1924 * sind(2.0 * observer.lat);
    let rho = 0.99833 + 0.00167 * cosd(2.0 * observer.lat);
    let ha = rev(sidereal_time(at, observer.lon) - ra + 180.0) - 180.0;
    let top_ra = ra - par * rho * cosd(gclat) * sind(ha) / cosd(dec);
    let top_dec = if gclat.abs() < 1e-6 {
        dec - par * rho * sind(-dec) * cosd(ha)
    } else {
        let g = (sind(gclat) / cosd(gclat) / cosd(ha)).atan().to_degrees();
        dec - par * rho * sind(gclat) * sind(g - dec) / sind(g)
    };
    (rev(top_ra), top_dec)
}

/// Angle between two directions given as right ascension and declination.
pub fn separation(ra1: f64, dec1: f64, ra2: f64, dec2: f64) -> f64 {
    acosd(sind(dec1) * sind(dec2) + cosd(dec1) * cosd(dec2) * cosd(ra1 - ra2))
}

/// Eight compass words for an azimuth.
pub fn compass(az: f64) -> &'static str {
    const WORDS: [&str; 8] = [
        "north",
        "north-east",
        "east",
        "south-east",
        "south",
        "south-west",
        "west",
        "north-west",
    ];
    WORDS[((rev(az) + 22.5) / 45.0) as usize % 8]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalising_gives_length_one_and_leaves_zero_alone() {
        let v = normalise([3.0, 0.0, 4.0]);
        assert!((dot(v, v) - 1.0).abs() < 1e-12);
        assert!((v[0] - 0.6).abs() < 1e-12 && (v[2] - 0.8).abs() < 1e-12);
        assert_eq!(normalise([0.0; 3]), [0.0; 3]);
    }

    #[test]
    fn the_transpose_of_a_rotation_turns_it_back() {
        let m = horizon(
            Observer {
                lat: 51.5,
                lon: -0.1,
            },
            1_790_000_000_000,
        );
        let v = unit(123.0, -40.0);
        let back = apply(&transpose(&m), apply(&m, v));
        for k in 0..3 {
            assert!((back[k] - v[k]).abs() < 1e-12);
        }
    }

    #[test]
    fn a_turn_takes_the_short_way_round() {
        assert_eq!(turn(350.0, 10.0), 20.0);
        assert_eq!(turn(10.0, 350.0), -20.0);
        assert_eq!(turn(90.0, 90.0), 0.0);
        assert_eq!(turn(0.0, 180.0), -180.0);
        assert_eq!(turn(-30.0, 690.0), 0.0);
    }

    #[test]
    fn the_celestial_pole_stands_at_the_latitude() {
        let observer = Observer {
            lat: 51.5,
            lon: -0.1,
        };
        let (alt, az) = alt_az(apply(
            &horizon(observer, 1_790_000_000_000),
            [0.0, 0.0, 1.0],
        ));
        assert!((alt - 51.5).abs() < 1e-9);
        assert!(az < 1e-6 || az > 359.999_999);
    }

    #[test]
    fn precession_moves_the_equinox_about_fifty_arcseconds_a_year() {
        let m = precession(crate::time::midnight_utc(2026, 1, 1));
        let (ra, _) = angles(apply(&m, unit(0.0, 0.0)));
        let years = 26.0;
        let expected = 50.29 * years / 3600.0 * cosd(23.44);
        assert!((ra - expected).abs() < 0.003, "{ra} vs {expected}");
    }

    #[test]
    fn refraction_is_about_half_a_degree_at_the_horizon() {
        assert!((refraction(0.0) - 0.48).abs() < 0.05);
        assert!(refraction(45.0) < 0.02);
    }

    #[test]
    fn compass_words() {
        assert_eq!(compass(0.0), "north");
        assert_eq!(compass(92.0), "east");
        assert_eq!(compass(200.0), "south");
        assert_eq!(compass(300.0), "north-west");
    }
}
