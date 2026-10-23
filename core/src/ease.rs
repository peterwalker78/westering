//! Easing curves.

/// 0 to 1, eased at both ends: flat at 0 and at 1, and 0.5 halfway.
/// Anything below 0 or past 1 holds at the end.
pub fn smoothstep(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoothstep_eases_from_nothing_to_all_and_holds() {
        assert_eq!(smoothstep(-1.0), 0.0);
        assert_eq!(smoothstep(0.0), 0.0);
        assert_eq!(smoothstep(0.5), 0.5);
        assert_eq!(smoothstep(1.0), 1.0);
        assert_eq!(smoothstep(2.0), 1.0);
        // Gentle at the ends, symmetric about the middle.
        assert!(smoothstep(0.1) < 0.1);
        assert!((smoothstep(0.3) + smoothstep(0.7) - 1.0).abs() < 1e-12);
    }
}
