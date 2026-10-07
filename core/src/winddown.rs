//! Winding down, if wanted: a few slow breaths paced by the wisp, then a
//! line to think over. Nothing is written or kept.

use serde::Deserialize;

use crate::finds::stable_hash;
use crate::time::UnixMs;

/// What a line to think over is about.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum About {
    /// How the day went.
    Day,
    People,
    /// Something to look forward to.
    Ahead,
    /// Something to leave here for the night.
    Leave,
    /// Nothing asked: letting go.
    Rest,
}

impl About {
    /// What a question on this thread has already been about.
    pub fn of_thread(thread: &str) -> Option<About> {
        match thread {
            "people" => Some(About::People),
            "plans" => Some(About::Ahead),
            "perspective" => Some(About::Day),
            _ => None,
        }
    }
}

#[derive(Deserialize)]
struct Reflection {
    about: About,
    text: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct WindDown {
    pub ask: String,
    pub yes: String,
    pub no: String,
    pub breaths: u32,
    pub in_seconds: f64,
    pub out_seconds: f64,
    pub in_words: String,
    pub out_words: String,
    pub skip: String,
    /// Said small above the line to think over.
    pub kicker: String,
    /// The same, when the line is the thought tonight's story closed on.
    pub story_kicker: String,
    #[serde(rename = "reflection")]
    reflections: Vec<Reflection>,
}

/// Where a breath is: which one (from zero), whether breathing in, and how
/// far through that half, 0 to 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Breath {
    pub count: u32,
    pub inhaling: bool,
    pub through: f64,
}

impl WindDown {
    pub fn bundled() -> WindDown {
        toml::from_str(include_str!("../data/wind-down.toml")).expect("wind-down.toml")
    }

    fn breath_ms(&self) -> f64 {
        (self.in_seconds + self.out_seconds) * 1000.0
    }

    /// The breath `elapsed` milliseconds in, or none once they're done.
    pub fn breath(&self, elapsed: UnixMs) -> Option<Breath> {
        let t = elapsed.max(0) as f64;
        let count = (t / self.breath_ms()) as u32;
        if count >= self.breaths {
            return None;
        }
        let into = t % self.breath_ms() / 1000.0;
        Some(if into < self.in_seconds {
            Breath {
                count,
                inhaling: true,
                through: into / self.in_seconds,
            }
        } else {
            Breath {
                count,
                inhaling: false,
                through: (into - self.in_seconds) / self.out_seconds,
            }
        })
    }

    /// How far through the whole breath, 0 to 1, in then out: the wisp
    /// breathes in time with this.
    pub fn phase(&self, elapsed: UnixMs) -> f64 {
        (elapsed.max(0) as f64 % self.breath_ms()) / self.breath_ms()
    }

    /// Tonight's line to think over: one, so the evening winds down
    /// rather than opening up again, and never about something the evening
    /// has already been about. The lines that ask nothing are always there.
    pub fn reflection(&self, night: &str, covered: &[About]) -> &str {
        self.reflections
            .iter()
            .filter(|r| r.about == About::Rest || !covered.contains(&r.about))
            .min_by_key(|r| stable_hash((0, 0, 11), &format!("{night}:{}", r.text)))
            .map(|r| r.text.as_str())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::questions::UNSAID;

    #[test]
    fn five_slow_breaths_then_done() {
        let w = WindDown::bundled();
        let b = w.breath(1_000).expect("breathing");
        assert!(b.inhaling && b.count == 0 && (b.through - 0.25).abs() < 1e-9);
        let b = w.breath(7_000).expect("breathing");
        assert!(!b.inhaling && (b.through - 0.5).abs() < 1e-9);
        assert_eq!(w.breath(12_000).map(|b| b.count), Some(1));
        assert!(w.breath(49_999).is_some());
        assert!(w.breath(50_000).is_none());
        // In over the first two fifths, as the wisp's own breath goes.
        assert!((w.in_seconds / (w.in_seconds + w.out_seconds) - 0.4).abs() < 1e-9);
    }

    #[test]
    fn one_line_a_night_that_never_names_the_feeling() {
        let w = WindDown::bundled();
        let tonight = w.reflection("2026-09-29", &[]);
        assert!(!tonight.is_empty());
        assert_eq!(tonight, w.reflection("2026-09-29", &[]));
        let all = include_str!("../data/wind-down.toml").to_lowercase();
        for word in UNSAID {
            assert!(!all.contains(word), "{word}");
        }
    }

    fn nights() -> impl Iterator<Item = String> {
        (1..=28).flat_map(|d| (1..=12).map(move |m| format!("2026-{m:02}-{d:02}")))
    }

    fn about(w: &WindDown, text: &str) -> About {
        w.reflections
            .iter()
            .find(|r| r.text == text)
            .expect("a line from the file")
            .about
    }

    #[test]
    fn nothing_the_evening_was_already_about_is_asked_again() {
        let w = WindDown::bundled();
        let mut seen = Vec::new();
        for night in nights() {
            // Weights were set down, and the sky asked about someone.
            let line = w.reflection(&night, &[About::Leave, About::People]);
            let a = about(&w, line);
            assert!(a != About::Leave && a != About::People, "{night}: {line}");
            if !seen.contains(&a) {
                seen.push(a);
            }
        }
        // The rest still come round.
        for a in [About::Day, About::Ahead, About::Rest] {
            assert!(seen.contains(&a), "{a:?} never chosen");
        }
    }

    #[test]
    fn a_full_evening_still_ends_on_a_line() {
        let w = WindDown::bundled();
        let all = [
            About::Day,
            About::People,
            About::Ahead,
            About::Leave,
            About::Rest,
        ];
        for night in nights() {
            let line = w.reflection(&night, &all);
            assert_eq!(about(&w, line), About::Rest, "{night}");
        }
    }

    #[test]
    fn question_threads_are_all_accounted_for() {
        for q in crate::questions::bundled() {
            assert!(About::of_thread(&q.thread).is_some(), "{}", q.thread);
        }
    }
}
