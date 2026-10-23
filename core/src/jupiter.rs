//! Jupiter's four big moons, where they are tonight: the low-accuracy
//! method from Jean Meeus's Astronomical Algorithms (chapter 44), good to a
//! fraction of Jupiter's width, which is all a picture needs.

use crate::coords::{Vec3, unit};
use crate::ephem::{asind, cosd, sind};
use crate::time::{UnixMs, day_number};

pub const NAMES: [&str; 4] = ["Io", "Europa", "Ganymede", "Callisto"];

/// Each moon's place beside Jupiter, in Jupiter's equatorial radii: `x`
/// along its equator, positive towards the sky's west; `y` along its axis,
/// positive towards its north pole. `behind` if it's further than Jupiter.
#[derive(Clone, Copy, Debug)]
pub struct Moon {
    pub x: f64,
    pub y: f64,
    pub behind: bool,
}

pub fn moons(at: UnixMs) -> [Moon; 4] {
    // Days since J2000.0 (Meeus's d); the ephemeris day number is 1.5 behind.
    let d = day_number(at) - 1.5;
    let v = 172.74 + 0.00111588 * d;
    let m = 357.529 + 0.9856003 * d;
    let n = 20.020 + 0.0830853 * d + 0.329 * sind(v);
    let j = 66.115 + 0.9025179 * d - 0.329 * sind(v);
    let a = 1.915 * sind(m) + 0.020 * sind(2.0 * m);
    let b = 5.555 * sind(n) + 0.168 * sind(2.0 * n);
    let k = j + a - b;
    let r_earth = 1.00014 - 0.01671 * cosd(m) - 0.00014 * cosd(2.0 * m);
    let r = 5.20872 - 0.25208 * cosd(n) - 0.00611 * cosd(2.0 * n);
    let delta = (r * r + r_earth * r_earth - 2.0 * r * r_earth * cosd(k)).sqrt();
    let psi = asind(r_earth / delta * sind(k));
    let dl = d - delta / 173.0;
    let mut u = [
        163.8069 + 203.4058646 * dl + psi - b,
        358.4140 + 101.2916335 * dl + psi - b,
        5.7176 + 50.2345180 * dl + psi - b,
        224.8092 + 21.4879800 * dl + psi - b,
    ];
    let g = 331.18 + 50.310482 * dl;
    let h = 87.45 + 21.569231 * dl;
    let (u1, u2, u3) = (u[0], u[1], u[2]);
    u[0] += 0.473 * sind(2.0 * (u1 - u2));
    u[1] += 1.065 * sind(2.0 * (u2 - u3));
    u[2] += 0.165 * sind(g);
    u[3] += 0.843 * sind(h);
    let r_moon = [
        5.9057 - 0.0244 * cosd(2.0 * (u1 - u2)),
        9.3966 - 0.0882 * cosd(2.0 * (u2 - u3)),
        14.9883 - 0.0216 * cosd(g),
        26.3627 - 0.1939 * cosd(h),
    ];
    let lambda = 34.35 + 0.083091 * d + 0.329 * sind(v) + b;
    let ds = 3.12 * sind(lambda + 42.8);
    let de = ds
        - 2.22 * sind(psi) * cosd(lambda + 22.0)
        - 1.30 * (r - delta) / delta * sind(lambda - 100.5);
    std::array::from_fn(|i| Moon {
        x: r_moon[i] * sind(u[i]),
        y: -r_moon[i] * cosd(u[i]) * sind(de),
        behind: cosd(u[i]) < 0.0,
    })
}

/// Where the moons sit tonight, in a sentence.
pub fn arrangement(at: UnixMs) -> String {
    let ms = moons(at);
    let (mut west, mut east, mut hidden, mut crossing) = (vec![], vec![], vec![], vec![]);
    for (i, m) in ms.iter().enumerate() {
        if m.x.abs() < 1.0 {
            if m.behind {
                hidden.push(NAMES[i])
            } else {
                crossing.push(NAMES[i])
            }
        } else if m.x > 0.0 {
            west.push(NAMES[i]);
        } else {
            east.push(NAMES[i]);
        }
    }
    let list = |v: &[&str]| match v.len() {
        1 => v[0].to_owned(),
        n => format!("{} and {}", v[..n - 1].join(", "), v[n - 1]),
    };
    let mut parts = Vec::new();
    if !west.is_empty() {
        parts.push(format!("{} to its west", list(&west)));
    }
    if !east.is_empty() {
        parts.push(format!("{} to its east", list(&east)));
    }
    let mut out = format!(
        "Tonight its four big moons line up beside it: {}.",
        parts.join(", ")
    );
    if !crossing.is_empty() {
        out.push_str(&format!(" {} is crossing in front of it.", list(&crossing)));
    }
    if !hidden.is_empty() {
        out.push_str(&format!(" {} is hidden behind it.", list(&hidden)));
    }
    out
}

/// Jupiter's north pole (IAU), as a J2000 direction.
pub fn pole() -> Vec3 {
    unit(268.056_595, 64.495_303)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coords::{dot, normalise};

    /// (unix ms, body, ra, dec) from JPL Horizons (tools/horizons-jupiter.py).
    const FIXTURES: &[(i64, &str, f64, f64)] = &[
        (1790625600000, "jupiter", 141.8139057, 15.6263072),
        (1790625600000, "io", 141.7980923, 15.6318826),
        (1790625600000, "europa", 141.7807351, 15.6382746),
        (1790625600000, "ganymede", 141.8492933, 15.6133742),
        (1790625600000, "callisto", 141.8764713, 15.6042918),
        (1805079600000, "jupiter", 141.0710119, 16.3618178),
        (1805079600000, "io", 141.1015559, 16.3509806),
        (1805079600000, "europa", 141.0158075, 16.3811086),
        (1805079600000, "ganymede", 140.9864426, 16.3915689),
        (1805079600000, "callisto", 141.1746280, 16.3248715),
        (1909173600000, "jupiter", 225.8645153, -16.1974804),
        (1909173600000, "io", 225.8489614, -16.1947204),
        (1909173600000, "europa", 225.8099177, -16.1822245),
        (1909173600000, "ganymede", 225.8331441, -16.1931679),
        (1909173600000, "callisto", 225.9137270, -16.2192512),
        (2082088800000, "jupiter", 38.9047512, 13.9953333),
        (2082088800000, "io", 38.8706854, 13.9831610),
        (2082088800000, "europa", 38.8600405, 13.9821754),
        (2082088800000, "ganymede", 38.9908990, 14.0261813),
        (2082088800000, "callisto", 38.9132994, 13.9891584),
        (1795000000000, "jupiter", 148.5717679, 13.5384812),
        (1795000000000, "io", 148.6000173, 13.5273269),
        (1795000000000, "europa", 148.5394045, 13.5506472),
        (1795000000000, "ganymede", 148.5859765, 13.5331678),
        (1795000000000, "callisto", 148.6511174, 13.5068861),
        (1800000000000, "jupiter", 147.8928992, 14.0203971),
        (1800000000000, "io", 147.8761500, 14.0266413),
        (1800000000000, "europa", 147.9431255, 14.0005498),
        (1800000000000, "ganymede", 147.8599753, 14.0339337),
        (1800000000000, "callisto", 147.8264008, 14.0475117),
    ];

    #[test]
    fn the_moons_are_where_horizons_says() {
        // Jupiter's equatorial radius in km, and an AU.
        const RADIUS: f64 = 71_492.0;
        const AU: f64 = 149_597_870.7;
        for at in FIXTURES
            .iter()
            .map(|f| f.0)
            .collect::<std::collections::BTreeSet<_>>()
        {
            let get = |name: &str| FIXTURES.iter().find(|f| f.0 == at && f.1 == name).unwrap();
            let jup = get("jupiter");
            let dist = crate::ephem::position(crate::ephem::Body::Jupiter, at).distance;
            let r_j = (RADIUS / (dist * AU)).atan();
            let jd = unit(jup.2, jup.3);
            let (ra, dec) = (jup.2.to_radians(), jup.3.to_radians());
            let east = [-ra.sin(), ra.cos(), 0.0];
            let north = [-dec.sin() * ra.cos(), -dec.sin() * ra.sin(), dec.cos()];
            let p = pole();
            let p = normalise([
                p[0] - dot(p, jd) * jd[0],
                p[1] - dot(p, jd) * jd[1],
                p[2] - dot(p, jd) * jd[2],
            ]);
            // The equator's direction on the sky, pointing west.
            let mut e = [
                jd[1] * p[2] - jd[2] * p[1],
                jd[2] * p[0] - jd[0] * p[2],
                jd[0] * p[1] - jd[1] * p[0],
            ];
            if dot(e, east) > 0.0 {
                e = [-e[0], -e[1], -e[2]];
            }
            for (i, name) in ["io", "europa", "ganymede", "callisto"].iter().enumerate() {
                let m = moons(at)[i];
                let off = [
                    (m.x * e[0] + m.y * p[0]) * r_j,
                    (m.x * e[1] + m.y * p[1]) * r_j,
                    (m.x * e[2] + m.y * p[2]) * r_j,
                ];
                let want = get(name);
                let wr = (want.2 - jup.2).to_radians() * dec.cos();
                let wd = (want.3 - jup.3).to_radians();
                let got_e = dot(off, east);
                let got_n = dot(off, north);
                let miss = ((got_e - wr).powi(2) + (got_n - wd).powi(2)).sqrt() / r_j;
                assert!(miss < 0.2, "{name} at {at}: off by {miss:.2} Jupiter radii");
            }
        }
    }
}
