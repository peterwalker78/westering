//! Winding down, asked plainly: a few slow breaths with the wisp, if
//! wanted, and a line to think over, before the night's ending. The
//! evening waits while they happen, and Esc, or choosing something else,
//! lets them go at any point.

use crate::game::{Game, smoothstep};
use crate::talk::{Flow, Prompt};
use crate::view::{Point, Text};
use westering_core::time::UnixMs;
use westering_core::winddown::{About, WindDown};

/// A line to think over moves on by itself after this long.
const LINE_MS: UnixMs = 25_000;

pub enum Stage {
    Breathing(UnixMs),
    /// The line to think over, and since when.
    Reflecting(UnixMs),
}

pub struct Wind {
    pub(crate) words: WindDown,
    pub(crate) stage: Option<Stage>,
    /// Tonight's line, chosen once the breaths are done, and the few
    /// words above it saying what it is.
    line: String,
    kicker: String,
}

impl Wind {
    pub fn bundled() -> Wind {
        Wind {
            words: WindDown::bundled(),
            stage: None,
            line: String::new(),
            kicker: String::new(),
        }
    }
}

impl Game {
    pub(crate) fn winding(&self) -> bool {
        self.wind.stage.is_some()
    }

    /// Asks, as the winding down begins, whether to take it slowly.
    pub(crate) fn ask_wind(&mut self) {
        self.talk.flow = Some(Flow::WindChoice);
        let w = &self.wind.words;
        self.set_prompt(Some(Prompt {
            text: w.ask.clone(),
            chips: vec![w.yes.clone(), w.no.clone()],
            entry: false,
            hint: "K keeps you company instead".into(),
            ..Prompt::default()
        }));
    }

    /// Yes: the evening waits while the wisp breathes with the user.
    pub(crate) fn start_breathing(&mut self, real: UnixMs) {
        self.hush();
        self.session.hold(real);
        self.wind.stage = Some(Stage::Breathing(real));
    }

    /// The line to think over and the words above it, chosen now the
    /// evening's been had. A story heard to its end comes back as the
    /// thought it closed on; otherwise it's a line that doesn't go back to
    /// something tonight was already about.
    fn wind_line(&self) -> (String, String) {
        let words = &self.wind.words;
        if let Some(story) = self.story_heard {
            let mirror = self.sky.tours.stories[story].mirror.clone();
            return (mirror, words.story_kicker.clone());
        }
        let mut covered = Vec::new();
        if !self.page.weights.is_empty() {
            covered.push(About::Leave);
        }
        if !self.page.plans.is_empty() {
            covered.push(About::Ahead);
        }
        let answered = self.page.answers.iter().map(|a| &a.question);
        for id in self.talk.asked_now.iter().chain(answered) {
            let thread = self.talk.bank.iter().find(|q| &q.id == id);
            if let Some(about) = thread.and_then(|q| About::of_thread(&q.thread)) {
                covered.push(about);
            }
        }
        let line = words.reflection(&self.night, &covered).to_owned();
        (line, words.kicker.clone())
    }

    /// On to the next part: from the breaths to the line, and then how
    /// the night ends.
    pub(crate) fn wind_next(&mut self, real: UnixMs) {
        self.wind.stage = match self.wind.stage {
            Some(Stage::Breathing(_)) => {
                (self.wind.line, self.wind.kicker) = self.wind_line();
                Some(Stage::Reflecting(real))
            }
            _ => None,
        };
        if self.wind.stage.is_none() {
            self.wind_done(real);
        }
    }

    /// Finished, or let go: the evening carries on to its ending.
    pub(crate) fn wind_done(&mut self, real: UnixMs) {
        self.wind.stage = None;
        self.session.resume(real);
        self.ask_ending();
    }

    /// Stops it all, without asking anything more: for K, which keeps the
    /// user company instead.
    pub(crate) fn wind_cancel(&mut self) {
        self.wind.stage = None;
    }

    /// Moves things on by themselves.
    pub(crate) fn wind_tick(&mut self, real: UnixMs) {
        match self.wind.stage {
            Some(Stage::Breathing(since)) if self.wind.words.breath(real - since).is_none() => {
                self.wind_next(real)
            }
            Some(Stage::Reflecting(since)) if real - since > LINE_MS => self.wind_next(real),
            _ => {}
        }
        // The wisp breathes in time.
        let phase = match self.wind.stage {
            Some(Stage::Breathing(since)) => Some(self.wind.words.phase(real - since)),
            _ => None,
        };
        self.guide.wisp.breathe_with(phase);
    }

    /// The words in the middle of the screen.
    pub(crate) fn wind_texts(&self, real: UnixMs) -> Vec<Text> {
        let (w, h) = (self.camera.width, self.camera.height);
        let words = &self.wind.words;
        let mut out = Vec::new();
        let (line, since) = match self.wind.stage {
            Some(Stage::Breathing(since)) => {
                let Some(b) = words.breath(real - since) else {
                    return out;
                };
                let say = if b.inhaling {
                    &words.in_words
                } else {
                    &words.out_words
                };
                // Each half fades in as it begins.
                let a = smoothstep(b.through * 4.0);
                out.push(
                    Text::new(w / 2.0, h * 0.62, say.clone(), 24.0, 0.9 * a)
                        .centred()
                        .color([1.0, 0.94, 0.84]),
                );
                out.push(
                    Text::new(
                        w / 2.0,
                        h * 0.62 + 40.0,
                        format!("{} of {}", b.count + 1, words.breaths),
                        13.0,
                        0.45,
                    )
                    .centred(),
                );
                (None, since)
            }
            Some(Stage::Reflecting(since)) => (Some(self.wind.line.clone()), since),
            None => return out,
        };
        if let Some(line) = line {
            let a = smoothstep((real - since) as f64 / 1_500.0);
            out.push(
                Text::new(
                    w / 2.0,
                    h * 0.4 - 38.0,
                    self.wind.kicker.clone(),
                    13.0,
                    0.5 * a,
                )
                .centred(),
            );
            out.push(
                Text::new(w / 2.0, h * 0.4, line, 23.0, a)
                    .centred()
                    .wrap((w * 0.75).min(720.0))
                    .color([1.0, 0.94, 0.84]),
            );
        }
        out.push(Text::new(w / 2.0, h - 70.0, words.skip.clone(), 12.0, 0.45).centred());
        let _ = since;
        out
    }

    /// A ring that swells as the breath comes in and settles as it goes out.
    pub(crate) fn wind_ring(&self, real: UnixMs) -> Vec<Point> {
        let Some(Stage::Breathing(since)) = self.wind.stage else {
            return Vec::new();
        };
        let Some(b) = self.wind.words.breath(real - since) else {
            return Vec::new();
        };
        let ease = |x: f64| 0.5 - 0.5 * (x * std::f64::consts::PI).cos();
        let open = if b.inhaling {
            ease(b.through)
        } else {
            1.0 - ease(b.through)
        };
        let (cx, cy) = (self.camera.width / 2.0, self.camera.height * 0.4);
        let r = 40.0 + 60.0 * open;
        (0..48)
            .map(|k| {
                let a = k as f64 / 48.0 * std::f64::consts::TAU;
                Point {
                    x: cx + r * a.cos(),
                    y: cy + r * a.sin(),
                    radius: 1.6,
                    color: [1.0, 0.86, 0.62],
                    alpha: (0.35 + 0.45 * open) as f32,
                    halo: 0.6,
                }
            })
            .collect()
    }
}
