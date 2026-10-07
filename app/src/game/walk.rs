//! Walks whole evenings the way someone at the keyboard would, and fails if
//! the evening ever stops answering: a tap of Space that does nothing,
//! several times running, with nothing on screen saying what to do instead.

use super::{Clock, Game, Options};
use gtk::gdk;
use westering_core::coords::Observer;
use westering_core::finds::Target;
use westering_core::journal::Journal;
use westering_core::session::{Phase, Timings};
use westering_core::time::UnixMs;

const FRAME: UnixMs = 50;
/// Moves that get no answer before the evening counts as stuck. One is
/// already one too many.
const PATIENCE: usize = 1;

struct Walker {
    game: Game,
    real: UnixMs,
    /// How long a tap keeps the key down.
    press: UnixMs,
    log: Vec<String>,
}

impl Walker {
    fn new(dir: &std::path::Path, sky0: UnixMs, press: UnixMs) -> Walker {
        let real = 1_000_000;
        let options = Options {
            clock: Clock {
                real0: real,
                sky0,
                speed: 1.0,
            },
            observer: Observer {
                lat: 51.48,
                lon: 0.0,
            },
            offset_s: 3600,
            timings: Timings::STANDARD,
            journal: Journal::open(dir),
        };
        let mut game = Game::new(options, real);
        game.resize(1280.0, 800.0, 1.0);
        Walker {
            game,
            real,
            press,
            log: Vec::new(),
        }
    }

    fn wait(&mut self, ms: UnixMs) {
        let until = self.real + ms;
        while self.real < until {
            self.real += FRAME;
            self.game.tick(self.real);
        }
    }

    fn tap(&mut self, key: gdk::Key) {
        self.game.key_pressed(key, self.real);
        self.wait(self.press);
        self.game.key_released(key, self.real);
        self.wait(200);
    }

    /// Everything someone could see change after a key.
    fn seen(&self) -> String {
        let g = &self.game;
        let (line, queued) = g.guide.said();
        format!(
            "{:?}|line={:?}|q={queued}|card={:?}|page={:?}|caught={}|viewed={}|steps={:?}|prompt={:?}|ring={:?}|rest={}|wind={}|day={}|look={:?}|word={:?}",
            g.session.phase(),
            line,
            g.card.as_ref().map(|c| (&c.title, c.body.len())),
            g.tour.as_ref().map(|t| t.at),
            g.caught.iter().filter(|c| **c).count(),
            g.viewed.iter().filter(|c| **c).count(),
            g.steps,
            g.talk.prompt.as_ref().map(|p| &p.text),
            g.catch.target.map(|i| &g.finds[i].name),
            g.ring_resting,
            g.winding(),
            g.by_day(),
            g.look
                .as_ref()
                .map(|l| ((l.az * 10.0) as i64, (l.alt * 10.0) as i64)),
            // A word at the foot of the screen, put up or put up afresh.
            g.hint.as_ref().map(|h| (&h.text, h.shown + h.hold)),
        )
    }

    /// One move, as someone following the hints would make it.
    fn act(&mut self) -> &'static str {
        if let Some(prompt) = self.game.talk.prompt.clone() {
            if prompt.entry {
                // Esc in the box lets a question pass.
                self.game.skip_prompt(self.real);
                self.wait(300);
                return "skip the question";
            }
            if !prompt.chips.is_empty() {
                self.game.answer_chip(0, self.real);
                self.wait(300);
                return "first choice";
            }
        }
        if self.game.placing() {
            self.tap(gdk::Key::Return);
            return "place the weight";
        }
        let g = &self.game;
        if g.hunting()
            && g.card.is_none()
            && g.tour.is_none()
            && !g.more_now()
            && !g.ring_resting
            && g.catch.target.is_some()
        {
            // Something's in the ring: hold Space, as the ring says.
            self.game.key_pressed(gdk::Key::space, self.real);
            for _ in 0..80 {
                self.wait(FRAME);
                if self.game.card.is_some() || self.game.catch.target.is_none() {
                    break;
                }
            }
            self.game.key_released(gdk::Key::space, self.real);
            self.wait(200);
            return "hold Space";
        }
        self.tap(gdk::Key::space);
        "tap Space"
    }

    /// Plays until the evening is winding down or over. Says where it got
    /// stuck, if it did.
    fn play(&mut self, most: UnixMs) -> Result<(), String> {
        let end = self.real + most;
        let mut idle = 0;
        self.wait(1_000);
        while self.real < end {
            let phase = self.game.session.phase();
            if self.game.winding()
                || !matches!(phase, Phase::Arrival | Phase::Weights | Phase::Hunt)
            {
                return Ok(());
            }
            // Everything's found, or only a meteor is left to wait for.
            let left = (0..self.game.finds.len())
                .filter(|&i| !self.game.caught[i])
                .all(|i| matches!(self.game.finds[i].target, Target::Meteor(_)));
            if left
                && !self.game.more_now()
                && self.game.card.is_none()
                && self.game.talk.prompt.is_none()
            {
                // A tap now has nowhere to go, and must still be answered.
                self.wait(4_000);
                let before = self.seen();
                self.tap(gdk::Key::space);
                self.wait(600);
                return if before == self.seen() {
                    Err("a tap with nowhere to go met with silence".into())
                } else {
                    Ok(())
                };
            }
            let before = self.seen();
            let did = self.act();
            // Time enough for an answer to show.
            self.wait(600);
            let after = self.seen();
            // And to read it.
            self.wait(900);
            self.log.push(format!("{did}: {after}"));
            if before == after && self.game.session.phase() == Phase::Arrival {
                // The sky is still coming up: a tap may wait for it, but
                // not for long.
                for _ in 0..12 {
                    if self.game.session.phase() != Phase::Arrival {
                        break;
                    }
                    self.wait(500);
                }
                if self.game.session.phase() == Phase::Arrival {
                    return Err("the evening never began".into());
                }
                idle = 0;
            } else if before == after {
                idle += 1;
            } else {
                idle = 0;
            }
            if idle >= PATIENCE {
                let hint = self.game.hint.as_ref().map(|h| h.text.clone());
                let tail = self.log[self.log.len().saturating_sub(PATIENCE + 4)..].join("\n  ");
                return Err(format!(
                    "stuck after {idle} moves that changed nothing; hint on screen: {hint:?}\n  {tail}"
                ));
            }
        }
        Err(format!(
            "the evening never came to its end\n  {}",
            self.log[self.log.len().saturating_sub(6)..].join("\n  ")
        ))
    }
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("westering-walk-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Midnight UTC on a day of October 2026, plus hours.
fn october(day: i64, hours: f64) -> UnixMs {
    // 1 October 2026 00:00 UTC.
    1_790_812_800_000 + (day - 1) * 86_400_000 + (hours * 3_600_000.0) as UnixMs
}

fn walk(name: &str, moments: &[UnixMs], press: UnixMs) {
    let dir = scratch(name);
    let mut failures = Vec::new();
    for (n, &sky0) in moments.iter().enumerate() {
        let mut walker = Walker::new(&dir, sky0, press);
        if let Err(why) = walker.play(40 * 60_000) {
            failures.push(format!("evening {n} (sky {sky0}): {why}"));
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(failures.is_empty(), "\n{}", failures.join("\n\n"));
}

#[test]
fn space_carries_a_first_evening_through() {
    walk("first", &[october(11, 22.5)], 100);
}

/// A press that's neither quick nor long still carries on.
#[test]
fn an_unhurried_press_carries_on_too() {
    walk("unhurried", &[october(11, 22.5)], 500);
}

/// Looking back at an old weight: no answer is met better than another,
/// and the way to chart a course is pointed out once, ever.
#[test]
fn looking_back_meets_every_answer_alike() {
    let dir = scratch("look-back");
    let mut w = Walker::new(&dir, october(11, 22.5), 100);
    w.wait(2_000);
    for chip in [0, 1, 3] {
        let before = w.game.guide.said().1;
        w.game.guide_looked_back(chip, w.real);
        assert_eq!(w.game.guide.said().1, before, "answer {chip} got a word");
        let word = w.game.hint.as_ref().map(|h| h.text.clone());
        assert!(word.is_some_and(|t| t.contains("logbook")), "answer {chip}");
    }
    let before = w.game.guide.said().1;
    w.game.guide_looked_back(2, w.real);
    assert_eq!(w.game.guide.said().1, before + 1);
    w.game.guide_looked_back(2, w.real);
    assert_eq!(w.game.guide.said().1, before + 1, "said once, ever");
    let _ = std::fs::remove_dir_all(&dir);
}
