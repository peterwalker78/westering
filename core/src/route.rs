//! The order to look at tonight's things in: one sweep across the sky from
//! where the view opens, rather than a zig-zag from one side to the other.

use crate::coords::Vec3;

fn apart(a: Vec3, b: Vec3) -> f64 {
    (a[0] * b[0] + a[1] * b[1] + a[2] * b[2])
        .clamp(-1.0, 1.0)
        .acos()
}

/// Indices into `dirs` in the order to visit them, starting from `start`:
/// the nearest first, then the nearest to that, and so on, with any
/// stretch turned round where that makes the way shorter. Anything with no
/// direction (nowhere to point, or not up) comes last, in the order given.
pub fn route(start: Vec3, dirs: &[Option<Vec3>]) -> Vec<usize> {
    let mut left: Vec<(usize, Vec3)> = dirs
        .iter()
        .enumerate()
        .filter_map(|(i, d)| d.map(|d| (i, d)))
        .collect();
    // The way, as points: where it starts, then each thing in turn.
    let mut way = vec![start];
    let mut order = Vec::new();
    while !left.is_empty() {
        let here = *way.last().unwrap();
        let k = (0..left.len())
            .min_by(|&a, &b| apart(here, left[a].1).total_cmp(&apart(here, left[b].1)))
            .unwrap();
        let (i, d) = left.remove(k);
        way.push(d);
        order.push(i);
    }
    // Where two legs cross, turning the stretch between them round
    // uncrosses them. The start stays put; the end is free.
    let n = way.len();
    let mut better = true;
    while better {
        better = false;
        for a in 1..n {
            for b in a + 1..n {
                // Past the end there's no leg to count.
                let leg = |x: usize, y: usize| if y < n { apart(way[x], way[y]) } else { 0.0 };
                let before = leg(a - 1, a) + leg(b, b + 1);
                let after = leg(a - 1, b) + leg(a, b + 1);
                if after < before - 1e-9 {
                    way[a..=b].reverse();
                    order[a - 1..b].reverse();
                    better = true;
                }
            }
        }
    }
    order.extend((0..dirs.len()).filter(|&i| dirs[i].is_none()));
    order
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coords::{from_alt_az, turn};

    fn length(start: Vec3, dirs: &[Option<Vec3>], order: &[usize]) -> f64 {
        let mut here = start;
        let mut total = 0.0;
        for &i in order {
            if let Some(d) = dirs[i] {
                total += apart(here, d);
                here = d;
            }
        }
        total
    }

    #[test]
    fn the_evening_sweeps_across_the_sky_rather_than_zig_zagging() {
        // Round the sky at 30 degrees up, listed in a scattered order.
        let azimuths = [200.0, 40.0, 160.0, 80.0, 240.0, 120.0, 0.0, 280.0, 320.0];
        let dirs: Vec<Option<Vec3>> = azimuths
            .iter()
            .map(|&az| Some(from_alt_az(30.0, az)))
            .collect();
        let start = from_alt_az(30.0, 175.0);
        let order = route(start, &dirs);
        let seen: Vec<f64> = order.iter().map(|&i| azimuths[i]).collect();
        // One way round from the south, never back across what's been seen.
        assert_eq!(seen[0], 160.0, "{seen:?}");
        let steps: Vec<f64> = seen.windows(2).map(|w| turn(w[0], w[1])).collect();
        assert!(
            steps.iter().all(|s| s.signum() == steps[0].signum()),
            "{seen:?}"
        );
        assert!(length(start, &dirs, &order) < 360.0, "{seen:?}");
    }

    #[test]
    fn nothing_is_left_out_and_what_isnt_up_comes_last() {
        let dirs = vec![
            None,
            Some(from_alt_az(40.0, 90.0)),
            Some(from_alt_az(40.0, 270.0)),
            None,
            Some(from_alt_az(40.0, 180.0)),
        ];
        let order = route(from_alt_az(30.0, 180.0), &dirs);
        assert_eq!(order.len(), dirs.len());
        assert_eq!(order[0], 4);
        assert_eq!(&order[3..], &[0, 3]);
    }
}
