//! The wisp as a guide. It lives on a tuft of moss on the horizon, but it's
//! free to fly: off to the list, the compass, the ring or whatever it's
//! talking about, looping wide round things in the sky, trailing embers.
//! On the first night it shows the way and says what each part is for.
//! After that it mostly keeps quiet, speaking up if someone seems stuck or
//! asks (? or a click on it). It brightens when something is caught and
//! goes home to sleep when the lights go out.

use crate::flight::Flight;
use crate::game::{Game, envelope};
use crate::idle::{Idle, Kind, Scene};
use crate::sprite::{FLYING, NOOK, SIZE, render_flying, render_moss};
use crate::talk::{Flow, lower_first};
use crate::view::{Bubble, Point, Sprite};
use westering_core::coords::{angles, apply, observe, unit};
use westering_core::finale::whereabouts;
use westering_core::finds::{Target, stable_hash};
use westering_core::journal::long_date;
use westering_core::journey::{Lines, Tonight, risen, season, turned, year_round};
use westering_core::questions::{days_between, key_of};
use westering_core::session::Phase;
use westering_core::time::UnixMs;
use westering_core::time::{DAY, civil_date, clock, weekday};
use westering_core::wisp::{Feel, Gesture, Mode, Trend, Wisp};

/// A place on the screen, in pixels.
type Spot = (f64, f64);

/// Where the wisp goes while it says something.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Aim {
    /// Stay where it is.
    Stay,
    Home,
    /// A spot on the screen, as fractions of its width and height.
    Near(f64, f64),
    /// The Tonight list.
    List,
    /// The strip at the top.
    Compass,
    /// The prompt at the foot of the sky.
    Prompt,
    /// The card for what was just caught.
    Card,
    /// The ring in the middle.
    Ring,
    /// One of tonight's finds, wherever it is on the screen.
    Find(usize),
    /// Tonight's weights, low in the west.
    Weights,
    /// A place on the sky, by right ascension and declination (J2000).
    Sky(f64, f64),
    /// A place on the Moon, by index into the features.
    Moon(usize),
    /// The evening's guide, top left.
    Evening,
    /// Something at a place on the screen, in pixels.
    Spot(f64, f64),
    /// Hanging about at a place on the screen, looking at another.
    Hang(f64, f64, f64, f64),
}

#[derive(Clone)]
struct Line {
    text: String,
    shown: UnixMs,
    hold: UnixMs,
    aim: Aim,
}

pub struct Guide {
    pub(crate) wisp: Wisp,
    line: Option<Line>,
    /// Lines waiting their turn.
    queue: Vec<Line>,
    cheer_until: UnixMs,
    last_nudge: UnixMs,
    pub(crate) last_progress: UnixMs,
    pub(crate) scale: f64,
    flight: Flight,
    /// Where it's going, and until when it has something to do there.
    aim: Aim,
    busy_until: UnixMs,
    /// Which side of a thing it hovers on (+1 right, -1 left), kept until
    /// the thing is well over to the other side, so it doesn't dart about.
    side: std::cell::Cell<f64>,
    /// Where it's making for, eased, so a target that moves or jumps is
    /// followed smoothly.
    goal: Option<(f64, f64)>,
    /// Where the last bubble was, for a click on it.
    pub(crate) bubble_at: Option<(f64, f64, f64, f64)>,
    /// When it arrived at what it's pointing at, for the glow round it.
    pointing_since: Option<UnixMs>,
    last_frame: UnixMs,
    /// When the user last typed, and how much was there.
    typed_at: UnixMs,
    typed_len: usize,
    /// Warmed by something just said, until then.
    moved_until: UnixMs,
    idle: Idle,
    /// Where an idle look is aimed.
    idle_spot: Option<Spot>,
    lines: Lines,
    /// The moss as last drawn: when, at what scale.
    moss: Option<(UnixMs, f64, gtk::gdk::Texture)>,
    /// The wisp's picture as last drawn: when it's next due, at what scale.
    wisp_drawn: Option<(UnixMs, f64, gtk::gdk::Texture)>,
    /// Whether it's suggested the guided way, after a long free look.
    suggested_guided: bool,
}

/// What the wisp is holding its tongue for.
#[derive(Clone, Copy)]
struct Held {
    card_up: bool,
    asking: bool,
    /// Looking round freely: only a word from the moss, and everything
    /// else waits for the guided way.
    quiet: bool,
}

/// Whether a line waits: while a card is up only its own line speaks,
/// while a question is asked only lines about the question do, and in
/// free look only what it says from its moss.
fn waiting(held: Held, l: &Line) -> bool {
    (held.card_up && l.aim != Aim::Card && l.aim != Aim::Stay)
        || (held.asking && l.aim != Aim::Prompt && l.aim != Aim::Stay)
        || (held.quiet && l.aim != Aim::Home)
}

/// How often, at the least, the wisp's picture is drawn afresh when it
/// hasn't said: its breath runs smoothly between, as a scale.
const WISP_REDRAW_MS: UnixMs = 66;
/// How long in free look before the wisp suggests a moment of the guided
/// evening, once.
const FREE_SUGGEST_MS: UnixMs = 8 * 60_000;
/// How long someone can look around without finding anything before the
/// wisp offers to help.
const STUCK_MS: UnixMs = 35_000;
/// How long a line stays up when nothing follows it: long enough to read
/// it slowly, twice if need be.
fn read_time(text: &str) -> UnixMs {
    (2_000 + 65 * text.chars().count() as UnixMs).clamp(5_000, 20_000)
}
/// The pause between one line and the next.
const BREATH_MS: UnixMs = 700;
/// How long it lingers after speaking before it drifts home.
const LINGER_MS: UnixMs = 5_000;

fn number(n: usize) -> String {
    const WORDS: [&str; 17] = [
        "No", "One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight", "Nine", "Ten",
        "Eleven", "Twelve", "Thirteen", "Fourteen", "Fifteen", "Sixteen",
    ];
    WORDS
        .get(n)
        .map(|w| (*w).to_owned())
        .unwrap_or_else(|| n.to_string())
}

impl Guide {
    pub(crate) fn lines(&self) -> &Lines {
        &self.lines
    }

    pub fn new(real: UnixMs) -> Guide {
        let mut wisp = Wisp::new(FLYING.0, FLYING.1);
        wisp.set_breath_outside(true);
        // It wakes as the sky arrives.
        wisp.update(0.05, Mode::Away, Trend::Steady, false, true, false);
        Guide {
            wisp,
            line: None,
            queue: Vec::new(),
            cheer_until: 0,
            last_nudge: 0,
            last_progress: real,
            scale: 1.0,
            flight: Flight::new(-100.0, -100.0),
            aim: Aim::Home,
            busy_until: 0,
            side: std::cell::Cell::new(1.0),
            goal: None,
            bubble_at: None,
            pointing_since: None,
            last_frame: real,
            typed_at: 0,
            typed_len: 0,
            moved_until: 0,
            idle: Idle::new(real),
            idle_spot: None,
            lines: Lines::bundled(),
            moss: None,
            wisp_drawn: None,
            suggested_guided: false,
        }
    }
}

impl Guide {
    /// What the wisp is saying, until when, and how much more is queued,
    /// for tests.
    #[cfg(test)]
    pub(crate) fn said(&self) -> (Option<(&str, UnixMs)>, usize) {
        (
            self.line
                .as_ref()
                .map(|l| (l.text.as_str(), l.shown + l.hold)),
            self.queue.len(),
        )
    }
}

impl Game {
    pub(crate) fn seen(&self, key: &str) -> bool {
        self.journal.settings.seen.iter().any(|k| k == key)
    }

    pub(crate) fn mark_seen(&mut self, key: &str) {
        if !self.seen(key) {
            self.journal.settings.seen.push(key.to_owned());
            if let Err(e) = self.journal.save_settings() {
                eprintln!("westering: couldn't save settings: {e}");
            }
        }
    }

    /// Says something where it is.
    pub(crate) fn say(&mut self, text: impl Into<String>, real: UnixMs, hold: UnixMs) {
        self.say_at(Aim::Stay, text, real, hold);
    }

    /// Flies somewhere and says something, from `real` on. With a line
    /// already up, it waits its turn: the user moves the wisp on.
    pub(crate) fn say_at(&mut self, aim: Aim, text: impl Into<String>, real: UnixMs, hold: UnixMs) {
        let text = text.into();
        let line = Line {
            hold: hold.max(read_time(&text)),
            text,
            shown: real,
            aim,
        };
        self.guide.queue.retain(|l| l.text != line.text);
        self.guide.queue.push(line);
    }

    /// Says something the first time only, ever.
    fn say_once(
        &mut self,
        key: &str,
        aim: Aim,
        text: impl Into<String>,
        real: UnixMs,
        hold: UnixMs,
    ) -> bool {
        if self.seen(key) {
            return false;
        }
        self.mark_seen(key);
        self.say_at(aim, text, real, hold);
        true
    }

    /// Home to its moss now, with nothing to linger for.
    pub(crate) fn wisp_home(&mut self, real: UnixMs) {
        self.guide.aim = Aim::Home;
        self.guide.pointing_since = None;
        self.guide.busy_until = real;
    }

    /// Flies somewhere without saying anything, and lingers a while.
    pub(crate) fn fly(&mut self, aim: Aim, real: UnixMs, linger: UnixMs) {
        if self.guide.aim != aim {
            self.guide.aim = aim;
            self.guide.pointing_since = None;
        }
        self.guide.busy_until = self.guide.busy_until.max(real + linger);
    }

    /// Moves on to the next line, if there is one; with none, puts the
    /// line showing away. Says whether there was anything to do.
    pub(crate) fn guide_next(&mut self, real: UnixMs) -> bool {
        let held = self.held_back();
        if let Some(i) = self.guide.queue.iter().position(|l| !waiting(held, l)) {
            let mut next = self.guide.queue.remove(i);
            next.shown = real;
            self.guide.line = Some(next);
            return true;
        }
        match &mut self.guide.line {
            Some(l) if real >= l.shown && real - l.shown < l.hold => {
                // Let it fade from where it is.
                l.hold = real - l.shown;
                true
            }
            _ => false,
        }
    }

    /// What the wisp is holding its tongue for: a card that's up, a
    /// question being asked, or someone looking round on their own.
    fn held_back(&self) -> Held {
        let asking =
            self.talk.prompt.is_some() && matches!(self.talk.flow, Some(Flow::Question { .. }));
        // A question offered in free look is still asked in full.
        let quiet = self.free_look()
            && self.talk.prompt.is_none()
            && self.hunting()
            && self.tour.is_none()
            && !self.keeping()
            && !self.winding();
        Held {
            card_up: self.card.is_some(),
            asking,
            quiet,
        }
    }

    /// Whether the wisp has more to say now, rather than later.
    pub(crate) fn more_now(&self) -> bool {
        let held = self.held_back();
        self.guide.queue.iter().any(|l| !waiting(held, l))
    }

    /// Whether the wisp is saying something, or has more on the way.
    pub(crate) fn wisp_busy(&self, real: UnixMs) -> bool {
        self.more_now()
            || self
                .guide
                .line
                .as_ref()
                .is_some_and(|l| real < l.shown + l.hold + BREATH_MS)
    }

    /// How much the wisp is moving, for the frame rate: 2 flying, 1 out
    /// and hovering, 0 settled on its moss.
    pub(crate) fn wisp_motion(&self) -> u8 {
        let real = self.last_real;
        if self.guide.flight.speed() > 12.0 {
            2
        } else if !self.guide.flight.home
            || self.guide.flight.has_embers()
            || self.guide.wisp.gesturing(real as f64)
            || self.guide.flight.wobbling()
            || self.guide.idle.doing(real).is_some()
        {
            1
        } else {
            0
        }
    }

    /// Something warm was just said: a glow, and a softer face for a while.
    pub(crate) fn wisp_moved(&mut self, real: UnixMs) {
        self.guide.moved_until = real + 3_500;
        self.wisp_gesture(Gesture::Glow, real);
    }

    /// A gesture, if the wisp can: with calm motion on, only a glow.
    pub(crate) fn wisp_gesture(&mut self, gesture: Gesture, real: UnixMs) {
        if !self.calm() || gesture == Gesture::Glow {
            self.guide.wisp.gesture(gesture, real as f64);
        }
    }

    /// The user is writing: the wisp listens, and nods at the end of a
    /// sentence.
    pub(crate) fn typed(&mut self, text: &str, real: UnixMs) {
        let len = text.chars().count();
        if len > self.guide.typed_len && text.ends_with(['.', '!', '?']) {
            self.wisp_gesture(Gesture::Nod, real);
        }
        self.guide.typed_len = len;
        if len > 0 {
            self.guide.typed_at = real;
        }
    }

    /// What the wisp is saying, for the prompt to show in a small window
    /// while something is asked, and whether there's more to come.
    pub fn aside(&self, real: UnixMs) -> Option<(String, bool)> {
        let small = crate::game::compact(self.camera.width, self.camera.height);
        if !small || self.talk.prompt.is_none() {
            return None;
        }
        self.guide
            .line
            .as_ref()
            .filter(|l| real >= l.shown && real - l.shown < l.hold)
            .map(|l| (l.text.clone(), self.more_now()))
    }

    /// Whether a point is on the wisp's bubble.
    pub(crate) fn on_bubble(&self, x: f64, y: f64) -> bool {
        self.guide
            .bubble_at
            .is_some_and(|(bx, by, bw, bh)| x >= bx && x <= bx + bw && y >= by && y <= by + bh)
    }

    /// A card has been put away: what the wisp was saying about it is done.
    pub(crate) fn guide_card_gone(&mut self, real: UnixMs) {
        self.guide.queue.retain(|l| l.aim != Aim::Card);
        if let Some(l) = &mut self.guide.line
            && l.aim == Aim::Card
            && real - l.shown < l.hold
        {
            l.hold = real - l.shown;
        }
        if self.guide.aim == Aim::Card {
            self.guide.aim = Aim::Home;
            self.guide.pointing_since = None;
        }
    }

    pub(crate) fn guide_prompt_gone(&mut self, real: UnixMs) {
        self.guide.queue.retain(|l| l.aim != Aim::Prompt);
        if let Some(l) = &mut self.guide.line
            && l.aim == Aim::Prompt
        {
            l.hold = l.hold.min(real - l.shown);
        }
    }

    pub(crate) fn hush(&mut self) {
        self.guide.line = None;
        self.guide.queue.clear();
    }

    fn to_find(&self) -> usize {
        self.caught.iter().filter(|c| !**c).count()
    }

    pub(crate) fn guide_arrival(&mut self, real: UnixMs) {
        let (_, month, _) = civil_date(self.sky_now(real), self.offset_s);
        self.guide.wisp.set_season(season(month, self.observer.lat));
        let n = self.to_find();
        let things = match n {
            0 => String::new(),
            1 => " One thing to find tonight.".into(),
            n => format!(" {} things to find tonight.", number(n)),
        };
        if !self.seen("hello") {
            self.mark_seen("hello");
            self.say_at(
                Aim::Near(0.36, 0.42),
                "Hello. I'm the wisp. This is the real sky over you tonight, just as it is outside.",
                real + 1_200,
                7_000,
            );
            self.say_at(
                Aim::Near(0.4, 0.4),
                "Come here for a few quiet minutes at the end of the day: to put the day down, come back to what matters to you, and get ready for sleep. The sky is never the same two nights running, so there's always something new to find.",
                real + 1_200,
                9_000,
            );
            self.say_at(
                Aim::Evening,
                "Every evening has the same three parts, shown up here: set down what's on your mind, look up for a while, then wind down.",
                real + 1_200,
                9_000,
            );
            return;
        }
        let hello = self.hello(real);
        let text = if n == 0 && !self.finds.is_empty() {
            format!("{hello} You've found tonight's sky already; look around as long as you like.")
        } else if self.finds.is_empty() {
            format!("{hello} It's still light out, so there's not much to find yet.")
        } else if crate::game::late(self.sky_now(real), self.offset_s) {
            format!(
                "{hello}{things} It's late, so tonight's sky is a short one. W winds down whenever you're ready."
            )
        } else {
            format!("{hello}{things}")
        };
        self.say_at(Aim::Near(0.3, 0.5), text, real + 1_200, 6_000);
        self.one_more_thing(real);
    }

    /// The evening before this one, and the first, from the logbook.
    fn earlier_nights(&self) -> (Option<String>, Option<String>) {
        let earlier: Vec<String> = self
            .journal
            .nights()
            .into_iter()
            .filter(|k| *k < self.night)
            .collect();
        (earlier.first().cloned(), earlier.last().cloned())
    }

    /// Tonight's hello: one that fits the night, or, after a while away,
    /// what the sky has done since.
    fn hello(&self, real: UnixMs) -> String {
        let now = self.sky_now(real);
        let (hour, _) = clock(now, self.offset_s);
        let (_, month, _) = civil_date(now, self.offset_s);
        let lines = &self.guide.lines;
        if let (Some(last), _) = self.earlier_nights() {
            let days = days_between(&last, &self.night);
            if days >= 10 {
                if let Some(name) = risen(&self.sky, self.observer, now - days * DAY, now, &[]) {
                    return format!("Hello. {}", lines.news.risen.replace("{name}", &name));
                }
                if (30..=45).contains(&days) {
                    return format!("Hello. {}", lines.news.moon);
                }
            }
        }
        lines
            .greeting(&Tonight {
                night: &self.night,
                hour,
                weekday: weekday(now, self.offset_s),
                moon: &self.page.moon,
                month,
                lat: self.observer.lat,
            })
            .to_owned()
    }

    /// At most one thing more on arriving, when there's something worth
    /// it: the year come round, the Sun at an equinox or solstice, or the
    /// star of someone named, now and then.
    fn one_more_thing(&mut self, real: UnixMs) {
        let now = self.sky_now(real);
        let (last, first) = self.earlier_nights();
        if let Some(first) = &first
            && let Some(years) = year_round(first, &self.night)
            && !self.seen(&format!("round-{years}"))
        {
            self.mark_seen(&format!("round-{years}"));
            let lines = &self.guide.lines.round;
            let mut text = lines.text.replace("{date}", &long_date(first));
            let person = self
                .journal
                .people
                .iter()
                .filter(|p| p.mentions.iter().any(|m| m.night == *first))
                .map(|p| p.name.clone())
                .next();
            if let Some(name) = person {
                text = format!("{text} {}", lines.person.replace("{name}", &name));
            } else if let Some(find) = self
                .journal
                .night(first)
                .and_then(|p| p.finds.first().cloned())
            {
                text = format!(
                    "{text} {}",
                    lines.find.replace("{find}", &lower_first(&find))
                );
            }
            self.say_at(Aim::Near(0.4, 0.4), text, real + 1_200, 10_000);
            return;
        }
        if let Some((kind, at)) = turned(now, self.observer.lat)
            && last.is_none_or(|l| l < key_of(at, self.offset_s))
        {
            let text = self.guide.lines.turn(kind, at, now, self.offset_s);
            self.say_at(Aim::Near(0.4, 0.4), text, real + 1_200, 9_000);
            return;
        }
        // Someone's star, about one evening in three that it's up.
        if !stable_hash((0, 0, 1), &self.night).is_multiple_of(3) {
            return;
        }
        let star = self.journal.people.iter().find_map(|p| {
            p.stars.iter().find_map(|&hr| {
                let star = self.sky.stars.get(hr)?;
                let (ra, dec) = angles(star.dir);
                let (alt, az) = observe(self.observer, now, ra, dec);
                let name = self.sky.lists.star_name(hr)?.name.clone();
                (alt > 20.0).then(|| (p.name.clone(), name, ra, dec, alt, az))
            })
        });
        if let Some((person, name, ra, dec, alt, az)) = star {
            let text = self.guide.lines.star(&person, &name, &whereabouts(alt, az));
            self.say_at(Aim::Sky(ra, dec), text, real + 1_200, 8_000);
        }
    }

    pub(crate) fn guide_weights(&mut self, real: UnixMs) {
        self.say_once(
            "weights",
            Aim::Prompt,
            "Each evening starts here. A worry written down is easier to put down. Yours becomes a star in the west of this sky, and you'll watch it set at the end. Only you will ever see it.",
            real,
            30_000,
        );
    }

    pub(crate) fn guide_placing(&mut self, real: UnixMs) {
        // The prompt says what to do; the wisp only says why, the first time.
        self.say_once(
            "placing",
            Aim::Ring,
            "Stars low in the west are the next to set. At the end of tonight's visit, you'll watch this one go down.",
            real,
            12_000,
        );
    }

    pub(crate) fn guide_placed(&mut self) {
        self.hush();
    }

    pub(crate) fn guide_hunt(&mut self, real: UnixMs) {
        self.guide.last_progress = real;
        let n = self.to_find();
        if n == 0 || self.seen("hunt") {
            return;
        }
        self.mark_seen("hunt");
        let what = if n == 1 {
            "One thing is worth finding tonight. It's listed here, with where to look. Click it and I'll turn you to it.".to_owned()
        } else {
            format!(
                "{} things are worth finding tonight. They're listed here, with where to look for each. Click one and I'll turn you to it.",
                number(n)
            )
        };
        self.say_at(Aim::List, what, real + 500, 9_000);
        self.say_at(
            Aim::Compass,
            "This strip shows which way you're facing. The arrows, or a drag, turn you round.",
            real + 500,
            7_000,
        );
        self.say_at(
            Aim::Ring,
            "Tap Space to turn to the next thing to find, and hold it, or press and hold on the ring, to catch whatever's inside.",
            real + 500,
            8_000,
        );
        self.say_at(
            Aim::Near(0.5, 0.35),
            "Point at anything in the sky to see what it is, and click it to hear more.",
            real + 500,
            8_000,
        );
    }

    pub(crate) fn guide_caught(&mut self, real: UnixMs) {
        // Tips about the ring are done with once something's caught.
        self.guide.queue.retain(|l| l.aim != Aim::Ring);
        if let Some(l) = &mut self.guide.line
            && l.aim == Aim::Ring
        {
            l.hold = l.hold.min(real - l.shown);
        }
        self.guide.cheer_until = real + 4_000;
        self.guide.last_progress = real;
        // Over to the card, pleased.
        self.fly(Aim::Card, real, 4_000);
        if self.say_once(
            "caught",
            Aim::Card,
            "Lovely. Every find comes with a card saying something true about it. Tap Space to carry on: it puts the card away, then turns you to the next find.",
            real + 600,
            9_000,
        ) {
            return;
        }
        let caught = self.caught.iter().filter(|c| **c).count();
        if caught >= 2 {
            self.say_once(
                "draw",
                Aim::Ring,
                "You can join bright stars into a shape of your own, too: press C. People have drawn the sky that way for thousands of years.",
                real + 600,
                9_000,
            );
        }
    }

    /// A story or a walk has begun: the wisp goes along, and keeps its tips
    /// for later.
    pub(crate) fn guide_tour_began(&mut self, real: UnixMs) {
        self.guide.cheer_until = real + 3_000;
        self.guide.last_progress = real;
        self.hush();
    }

    /// Whether the guided part of tonight is done enough to hand the sky
    /// over: the first time, the whole list (but for meteors, which need
    /// luck, and anything that's set); after that, the first find and
    /// whatever it brought up.
    fn ready_to_hand_over(&self, real: UnixMs) -> bool {
        if self.card.is_some()
            || self.talk.prompt.is_some()
            || self.talk.pending.is_some()
            || self.tour.is_some()
            || self.drawing.is_some()
            || self.wisp_busy(real)
        {
            return false;
        }
        if self.seen("handover") {
            return self.caught.iter().any(|c| *c);
        }
        let now = self.sky_now(real);
        let hz = westering_core::coords::horizon(self.observer, now);
        let prec = westering_core::coords::precession(now);
        (0..self.finds.len()).all(|i| {
            self.caught[i]
                || matches!(self.finds[i].target, Target::Meteor(_))
                || self
                    .find_dir(i, now, &hz, &prec)
                    .is_none_or(|v| westering_core::coords::alt_az(v).0 < 0.0)
        })
    }

    /// Free look opens: said in full the first time, and after that only
    /// at the foot of the screen.
    fn hand_over(&mut self, real: UnixMs) {
        self.handed_over = true;
        if self.seen("handover") {
            let line = self.guide.lines.free.open.clone();
            self.status_for(line, real, 7_000);
        } else {
            self.mark_seen("handover");
            let line = self.guide.lines.free.handover.clone();
            self.say_at(Aim::Near(0.5, 0.35), line, real, 12_000);
        }
    }

    /// F before the sky's been handed over: a word once, then just a line
    /// at the foot of the screen.
    pub(crate) fn guide_not_yet(&mut self, real: UnixMs) {
        let line = if self.seen("handover") {
            self.guide.lines.free.wait.clone()
        } else {
            self.guide.lines.free.wait_first.clone()
        };
        if self.told_wait {
            self.status_for(line, real, 5_000);
        } else {
            self.told_wait = true;
            self.say(line, real, 7_000);
        }
    }

    pub(crate) fn guide_all_found(&mut self, real: UnixMs) {
        if self.by_day() {
            let done = self.guide.lines.day.done.clone();
            self.say_at(Aim::Near(0.4, 0.4), done, real + 400, 9_000);
            return;
        }
        self.guide.cheer_until = real + 5_000;
        if self.say_once(
            "tomorrow",
            Aim::Near(0.5, 0.35),
            "That's tonight's sky. By tomorrow it will have turned: new things will be up and the Moon will have moved on. Look around as long as you like, then W, or Wind down at the top left, when you're ready.",
            real,
            11_000,
        ) {
            return;
        }
        self.say_at(
            Aim::Near(0.5, 0.35),
            "That's tonight's sky, all of it. Look around as long as you like, then wind down when you're ready.",
            real,
            9_000,
        );
        self.say_once(
            "company",
            Aim::Near(0.5, 0.35),
            "Or, if you'd like the music on while you get on with something else, press K. I'll keep you company in the background, in a window as small as you like, and look in on you now and then.",
            real,
            12_000,
        );
    }

    /// More has come out as the sky darkened.
    pub(crate) fn guide_darker(&mut self, n: usize, real: UnixMs) {
        let news = &self.guide.lines.news;
        let text = if n == 1 {
            news.darker_one.clone()
        } else {
            news.darker_many.replace("{n}", &number(n).to_lowercase())
        };
        // News, not a matter for the wisp to fly out about.
        self.status_for(text, real, 6_000);
    }

    pub(crate) fn guide_meteor_left(&mut self, real: UnixMs) {
        self.say_at(
            Aim::Near(0.5, 0.3),
            "The last one is a meteor. Keep watching, and press Space the moment one flies.",
            real,
            8_000,
        );
    }

    pub(crate) fn guide_question(&mut self, real: UnixMs) {
        let (plan, name) = match &self.talk.flow {
            Some(Flow::Question { chosen, .. }) => (
                chosen.question.answer == westering_core::questions::AnswerKind::Plan,
                chosen.question.answer == westering_core::questions::AnswerKind::Name,
            ),
            _ => (false, false),
        };
        if plan
            && self.say_once(
                "plans",
                Aim::Prompt,
                "Something to look forward to is worth having. If you make a plan, I'll mention it when its night comes near.",
                real + 400,
                11_000,
            )
        {
            return;
        }
        if name
            && self.seen("question")
            && self.say_once(
                "names",
                Aim::Prompt,
                "Just a first name will do. Only you will ever see it.",
                real + 400,
                8_000,
            )
        {
            return;
        }
        self.say_once(
            "question",
            Aim::Prompt,
            "Now and then the sky asks something small. There's no right answer, and only you will ever see what you write. Esc lets it pass.",
            real + 400,
            12_000,
        );
    }

    /// After something's been written down, where it all goes.
    pub(crate) fn guide_answered(&mut self, real: UnixMs) {
        self.say_once(
            "logbook",
            Aim::Near(0.3, 0.55),
            "It's kept in your logbook, which L opens. Anything that keeps coming up gathers on a page of its own, so over time you can see what matters.",
            real + 800,
            11_000,
        );
    }

    pub(crate) fn guide_look_back(&mut self, real: UnixMs) {
        self.say_once(
            "look-back",
            Aim::Prompt,
            "Now and then I'll bring back a worry you set down a while ago. Worries often change shape once they're written down, and it helps to notice when one has.",
            real + 400,
            11_000,
        );
    }

    /// Tonight's end chosen: what happens now.
    pub(crate) fn guide_ending(&mut self, ending: crate::game::Ending, real: UnixMs) {
        let line = match ending {
            crate::game::Ending::Outside => {
                "Then I'll show you what's up out there. Give your eyes twenty minutes in the dark: they keep opening all that time."
            }
            crate::game::Ending::Bed => {
                "Then this is the last of the screen tonight. Watch the day go down with the sky."
            }
        };
        self.say_at(Aim::Near(0.4, 0.4), line, real + 300, 9_000);
    }

    /// A word after an old weight has been looked at again.
    pub(crate) fn guide_looked_back(&mut self, chip: usize, real: UnixMs) {
        let line = match chip {
            0 | 3 => "Good. Whatever helped lighten it is worth remembering.",
            1 => {
                "Some things take longer to lighten. You can set it down again any night you like."
            }
            _ => {
                "If you'd like, the logbook can help you chart a small plan for it: find it there and choose Chart a course. Only if you want to."
            }
        };
        self.say_at(Aim::Near(0.3, 0.55), line, real + 500, 8_000);
    }

    pub(crate) fn guide_drawing(&mut self, real: UnixMs) {
        self.say_at(
            Aim::Ring,
            "Arrows step between bright stars, Enter joins them, Backspace takes one back. Press C when it's done.",
            real,
            600_000,
        );
    }

    pub(crate) fn guide_help(&mut self, real: UnixMs) {
        let free = if self.handed_over {
            "F switches to free look: drag to look around, and click anything that glows to close in on it and read all about it; Esc, or a click on the sky, zooms back out. In free look the wisp keeps quiet; F brings it back."
        } else {
            "Once we've looked together a while, F opens free look, for wandering with the mouse."
        };
        self.say_at(
            Aim::Near(0.32, 0.5),
            format!("Arrows or a drag look around. Tapping Space carries on: the wisp's next word, then past a card, then to the next find (Enter does the same, and so does a click while a card is up). Tab goes back to the first thing on the list you haven't seen yet. Hold Space, or press and hold on the ring, to catch whatever's in it; click the list to turn to something. {free} Point at anything to see what it is. C draws, L opens the logbook, M changes the music's style (and after the last, turns it off), K keeps you company in the background, and W winds down. The button top left opens the menu."),
            real,
            15_000,
        );
    }

    pub(crate) fn guide_phase(&mut self, phase: Phase, real: UnixMs) {
        match phase {
            Phase::Dimming => self.say_at(
                Aim::Prompt,
                "Let's wind down. The screen dims a little at a time from here: bright light keeps a mind awake.",
                real + 600,
                10_000,
            ),
            Phase::Finale => {
                let first = self
                    .page
                    .weights
                    .first()
                    .and_then(|w| self.journal.weight(w.weight))
                    .map(|w| w.text.clone());
                if let Some(first) = first {
                    self.say_at(
                        Aim::Weights,
                        format!("Watch the west. The turning sky takes \u{201c}{first}\u{201d} down with it."),
                        real + 800,
                        9_000,
                    );
                } else if self.page.weights.is_empty() {
                    self.say_at(
                        Aim::Near(0.4, 0.45),
                        "Watch the sky turn.",
                        real + 800,
                        6_000,
                    );
                } else {
                    self.say_at(
                        Aim::Weights,
                        "Watch the west: the turning sky is taking down what you set down tonight.",
                        real + 800,
                        9_000,
                    );
                }
            }
            Phase::LightsOut => self.say_at(Aim::Home, "Goodnight.", real, 5_000),
            _ => {}
        }
    }

    /// Nudges someone who seems stuck, now and then, and keeps the face right.
    pub(crate) fn guide_tick(&mut self, real: UnixMs) {
        let hunting = self.hunting()
            && self.card.is_none()
            && self.talk.prompt.is_none()
            && self.drawing.is_none();
        if hunting && self.catch.target.is_some() {
            self.say_once(
                "ring",
                Aim::Ring,
                "There's one, inside the ring. Hold Space.",
                real,
                8_000,
            );
        }
        // Only on the very first night, once, and only if nothing at all
        // has happened for a while: otherwise the wisp waits to be asked.
        let left = self.to_find();
        if hunting
            && left > 0
            && !self.free_look()
            && self.talk.first_night
            && self.guide.last_nudge == 0
            && real - self.guide.last_progress.max(self.last_input) > STUCK_MS
        {
            self.guide.last_nudge = real;
            let only_meteor = (0..self.finds.len())
                .filter(|&i| !self.caught[i])
                .all(|i| matches!(self.finds[i].target, Target::Meteor(_)));
            if self.catch.target.is_some() {
                self.say_at(
                    Aim::Ring,
                    "There's one in the ring. Hold Space to catch it.",
                    real,
                    8_000,
                );
            } else if only_meteor {
                self.guide_meteor_left(real);
            } else {
                self.say_at(
                    Aim::List,
                    "Lost? Pick one from the list, or tap Space, and I'll turn you to the next.",
                    real,
                    8_000,
                );
            }
        }
        // Once tonight's sky has been walked together, it's handed over.
        if hunting && !self.handed_over && real >= self.next_handover_check {
            self.next_handover_check = real + 1_000;
            if self.ready_to_hand_over(real) {
                self.hand_over(real);
            }
        }
        // A long while in free look: one quiet suggestion, from the moss.
        if hunting
            && self.free_look()
            && !self.guide.suggested_guided
            && real - self.free_since > FREE_SUGGEST_MS
            && !self.wisp_busy(real)
        {
            self.guide.suggested_guided = true;
            let line = self.guide.lines.free.suggest.clone();
            self.say_at(Aim::Home, line, real, 12_000);
        }
        // A long look: offer to wind down, once.
        if hunting
            && !self.by_day()
            && self.session.in_phase(real) > self.session.timings.hunt_most * 4 / 5
            && !self.offered_wind_down()
        {
            self.set_offered_wind_down();
            self.status_for(
                "It's getting late: W winds down whenever you're ready".into(),
                real,
                8_000,
            );
        }
        let mode = match self.session.phase() {
            Phase::LightsOut | Phase::Over => Mode::Away,
            Phase::Arrival if self.session.in_phase(real) < 1_200 => Mode::Away,
            _ if real < self.guide.cheer_until => Mode::Nourishing,
            _ if self.talk.prompt.is_some() => Mode::Holding,
            _ => Mode::Resting,
        };
        self.guide
            .wisp
            .update(0.05, mode, Trend::Steady, false, true, false);
        self.guide.wisp.set_feel(self.wisp_feel(real));
        self.guide
            .wisp
            .set_humming(self.keeping() && self.playing.is_some());
        // Its breath slows with the evening, to about five and a half breaths
        // a minute at the calmest: slow enough to fall in with.
        let calmest = westering_core::session::CALMEST;
        let slowed = ((1.0 - self.session.tempo(real)) / (1.0 - calmest)).clamp(0.0, 1.0);
        self.guide.wisp.set_breath(6_000.0 + 4_900.0 * slowed);
        self.idle_tick(real);
    }

    fn wisp_feel(&self, real: UnixMs) -> Feel {
        let phase = self.session.phase();
        let writing = self.talk.prompt.as_ref().is_some_and(|p| p.entry);
        let (hour, _) = westering_core::time::clock(self.sky_now(real), self.offset_s);
        Feel {
            listening: writing && real - self.guide.typed_at < 6_000,
            moved: real < self.guide.moved_until,
            tender: matches!(self.talk.flow, Some(Flow::Weight { .. }))
                || (phase == Phase::Finale && !self.page.weights.is_empty()),
            sleepy: !(5..23).contains(&hour) || matches!(phase, Phase::Dimming | Phase::Finale),
        }
    }

    /// Small things the wisp does on its moss while nothing else is going on.
    fn idle_tick(&mut self, real: UnixMs) {
        let free = matches!(
            self.session.phase(),
            Phase::Weights | Phase::Hunt | Phase::Dimming
        ) && self.guide.flight.home
            && !self.more_now()
            && self.card.is_none()
            && self.talk.prompt.is_none()
            && self.drawing.is_none();
        if !free {
            if self.guide.idle.doing(real).is_some() {
                self.guide.idle.interrupt(real);
            }
            return;
        }
        let sky = self.sky_spot(real);
        let scene = Scene {
            sky: sky.is_some(),
            pointer: self.pointer.is_some_and(|(_, _, at)| real - at < 1_500),
            sleepy: self.wisp_feel(real).sleepy,
            calm: self.calm(),
            tempo: self.session.tempo(real),
        };
        if let Some(kind) = self.guide.idle.tick(real, &scene) {
            self.guide.idle_spot = if kind == Kind::LookUp { sky } else { None };
            if let Some(g) = kind.gesture() {
                self.wisp_gesture(g, real);
            }
        }
    }

    /// Something in the sky worth a look: one of tonight's finds on the
    /// screen, not yet caught if there is one.
    fn sky_spot(&self, real: UnixMs) -> Option<Spot> {
        if self.by_day() {
            return self.day_bird(real);
        }
        let now = self.sky_now(real);
        let hz = westering_core::coords::horizon(self.observer, now);
        let prec = westering_core::coords::precession(now);
        let mut spots: Vec<(bool, Spot)> = (0..self.finds.len())
            .filter_map(|i| {
                let (x, y) = self.camera.project(self.find_dir(i, now, &hz, &prec)?)?;
                self.camera
                    .on_screen(x, y, 40.0)
                    .then_some((self.caught[i], (x, y)))
            })
            .collect();
        spots.sort_by_key(|(caught, _)| *caught);
        spots.first().map(|(_, s)| *s)
    }

    /// Where the wisp's nook sits: on the horizon, bottom left.
    pub(crate) fn nook(&self) -> (f64, f64, f64, f64) {
        let (w, h) = (NOOK.0 * SIZE, NOOK.1 * SIZE);
        // Keeping company it drifts to the middle, where it reads in a
        // window of any size.
        let mix = self.keep.mix * self.keep.mix * (3.0 - 2.0 * self.keep.mix);
        let x = 10.0 + (((self.camera.width - w) / 2.0).max(10.0) - 10.0) * mix;
        (x, self.camera.height - h - 34.0 + 20.0 * mix, w, h)
    }

    /// Where the wisp's body is when it sits on its moss.
    fn home_spot(&self) -> (f64, f64) {
        let (x, y, _, _) = self.nook();
        (
            x + NOOK.0 / 2.0 * SIZE,
            y + (NOOK.1 - 7.0 - 8.0 - 13.5 * 1.15) * SIZE,
        )
    }

    pub(crate) fn on_wisp(&self, x: f64, y: f64) -> bool {
        let (wx, wy) = if self.guide.flight.home {
            self.home_spot()
        } else {
            (self.guide.flight.x, self.guide.flight.y)
        };
        ((x - wx).powi(2) + (y - wy).powi(2)).sqrt() < 34.0
    }

    /// What an aim means on the screen now: where to hover, and the thing
    /// being pointed at, if any.
    fn resolve(&self, aim: Aim) -> Option<(Spot, Option<Spot>)> {
        let (w, h) = (self.camera.width, self.camera.height);
        let (cx, cy) = (w / 2.0, h / 2.0);
        let beside = |px: f64, py: f64| {
            // Hover off to the side nearer the middle of the screen, keeping
            // to the side it's on until the thing is well past the middle.
            if px > cx + 90.0 {
                self.guide.side.set(-1.0);
            } else if px < cx - 90.0 {
                self.guide.side.set(1.0);
            }
            let dx = 110.0 * self.guide.side.get();
            (
                (px + dx).clamp(60.0, w - 60.0),
                (py - 45.0).clamp(60.0, h - 80.0),
            )
        };
        // Something off the screen is pointed at from the nearest edge.
        let edge = |x: f64, y: f64| (x.clamp(70.0, w - 70.0), y.clamp(90.0, h - 110.0));
        Some(match aim {
            Aim::Stay => return None,
            Aim::Home => (self.home_spot(), None),
            Aim::Near(fx, fy) => ((w * fx, h * fy), None),
            Aim::List => ((w - 330.0, 110.0), Some((w - 270.0, 70.0))),
            // Below the strip, clear of the evening's guide at the top left.
            Aim::Compass => ((cx - 290.0, 100.0), Some((cx - 200.0, 30.0))),
            Aim::Evening => ((150.0, 130.0), Some((120.0, 40.0))),
            // Above the prompt's left end, so the words sit clear of it.
            Aim::Prompt => ((cx - 330.0, h - 360.0), Some((cx - 250.0, h - 250.0))),
            Aim::Card => {
                let x = self.card.as_ref().map_or(cx + 100.0, |c| c.x);
                let right = x + 440.0 < w - 40.0;
                let hover = if right {
                    (x + 440.0, cy - 110.0)
                } else {
                    (x - 50.0, cy - 110.0)
                };
                (hover, Some((x + 30.0, cy - 60.0)))
            }
            Aim::Ring => {
                let r = self.reticle_radius();
                // Above and to the left, clear of a card either side.
                ((cx - r - 30.0, cy - r - 80.0), Some((cx, cy)))
            }
            Aim::Find(i) => {
                let now = self.sky_now(self.last_real);
                let hz = westering_core::coords::horizon(self.observer, now);
                let prec = westering_core::coords::precession(now);
                match self
                    .find_dir(i, now, &hz, &prec)
                    .and_then(|v| self.camera.project(v))
                {
                    Some((x, y)) if self.camera.on_screen(x, y, -40.0) => {
                        (beside(x, y), Some((x, y)))
                    }
                    Some((x, y)) => {
                        let (ex, ey) = edge(x, y);
                        (beside(ex, ey), Some((ex, ey)))
                    }
                    None => ((cx - 120.0, cy - 60.0), None),
                }
            }
            Aim::Sky(ra, dec) => {
                let now = self.sky_now(self.last_real);
                let hz = westering_core::coords::horizon(self.observer, now);
                let prec = westering_core::coords::precession(now);
                match self.camera.project(apply(&hz, apply(&prec, unit(ra, dec)))) {
                    // Above it, clear of a card beside the middle.
                    Some((x, y)) if self.camera.on_screen(x, y, -40.0) => (
                        ((x + 20.0).clamp(60.0, w - 60.0), (y - 95.0).max(60.0)),
                        Some((x, y)),
                    ),
                    _ => ((cx - 120.0, cy - 60.0), None),
                }
            }
            Aim::Spot(x, y) => (beside(x, y), Some((x, y))),
            Aim::Hang(x, y, px, py) => ((x, y), Some((px, py))),
            Aim::Moon(f) => match self.moon_spot(f) {
                Some((x, y, _)) => {
                    // Hover just off the place, towards the middle of the disc.
                    let (dx, dy) = (cx - x, cy - y);
                    let l = (dx * dx + dy * dy).sqrt().max(1.0);
                    let hover = if l > 90.0 {
                        (x + dx / l * 70.0, y + dy / l * 70.0 - 20.0)
                    } else {
                        (x - 70.0, y - 40.0)
                    };
                    (hover, Some((x, y)))
                }
                None => ((cx - 120.0, cy - 60.0), None),
            },
            Aim::Weights => {
                let now = self.sky_now(self.last_real);
                let hz = westering_core::coords::horizon(self.observer, now);
                let spot = self
                    .page
                    .weights
                    .iter()
                    .filter_map(|wt| self.camera.project(apply(&hz, unit(wt.ra, wt.dec))))
                    .find(|&(x, y)| self.camera.on_screen(x, y, -40.0));
                match spot {
                    Some((x, y)) => (beside(x, y), Some((x, y))),
                    None => ((cx, cy - 80.0), None),
                }
            }
        })
    }

    pub(crate) fn guide_frame(
        &mut self,
        real: UnixMs,
        brightness: f64,
    ) -> (Vec<Sprite>, Option<Bubble>, Vec<Point>) {
        let dt = ((real - self.guide.last_frame) as f64 / 1000.0).clamp(0.0, 0.1);
        self.guide.last_frame = real;
        let (nx, ny, nw, nh) = self.nook();
        // Fading out with the lights, from wherever the finale left it.
        let alpha = if matches!(self.session.phase(), Phase::LightsOut | Phase::Over) {
            0.6 * brightness / westering_core::session::LAST
        } else {
            brightness.max(0.6)
        };

        // The line to show. One up stays until it's read and, with more to
        // come, until the user moves the wisp on; nothing moves on by itself.
        // While a card is up or a question is asked, lines about anything
        // else wait for it to go.
        // A line about something else steps aside for a card or a question,
        // and is said again once it's gone.
        let held = self.held_back();
        if let Some(l) = &mut self.guide.line
            && real - l.shown < l.hold
            && waiting(held, l)
        {
            self.guide.queue.insert(
                0,
                Line {
                    shown: real,
                    ..l.clone()
                },
            );
            l.hold = real - l.shown;
        }
        let more = self.more_now();
        if let Some(l) = &mut self.guide.line {
            let age = real - l.shown;
            if more && age < l.hold {
                l.hold = l.hold.max(age + 450);
            }
        }
        let finished = self
            .guide
            .line
            .as_ref()
            .is_none_or(|l| real - l.shown > l.hold + BREATH_MS);
        let free = self.guide.queue.iter().position(|l| !waiting(held, l));
        if finished
            && let Some(i) = free
            && self.guide.queue[i].shown <= real
        {
            let mut next = self.guide.queue.remove(i);
            next.shown = real;
            self.guide.line = Some(next);
        }
        let speaking = self
            .guide
            .line
            .as_ref()
            .filter(|l| real >= l.shown && real - l.shown < l.hold + BREATH_MS)
            .map(|l| l.aim);
        if let Some(aim) = speaking {
            if aim != Aim::Stay && aim != self.guide.aim {
                self.guide.aim = aim;
                self.guide.pointing_since = None;
            }
            // Between lines of the same run it stays where it is.
            self.guide.busy_until = self.guide.busy_until.max(real + LINGER_MS.min(2_000));
        }

        // Nothing more to say: home to its moss, and it's the user's turn.
        // It never flies off on its own, so when it's out, it's showing
        // you something.
        let busy = self.wisp_busy(real);
        if !busy && real > self.guide.busy_until && self.guide.aim != Aim::Home {
            self.guide.aim = Aim::Home;
            self.guide.pointing_since = None;
        }

        // With calm motion on, or in a small window, the wisp keeps to its
        // moss and speaks from there.
        let small = crate::game::compact(self.camera.width, self.camera.height);
        let aim = if self.calm() || small {
            Aim::Home
        } else {
            self.guide.aim
        };
        let (target, pointing) = match self.resolve(aim) {
            Some(g) => g,
            None => ((self.guide.flight.x, self.guide.flight.y), None),
        };
        // Ease the goal itself, so a thing that drifts or jumps across the
        // screen is followed without a lurch.
        let eased = match self.guide.goal {
            Some((gx, gy)) if aim != Aim::Home => {
                let k = 1.0 - (-dt / 0.18).exp();
                (gx + (target.0 - gx) * k, gy + (target.1 - gy) * k)
            }
            _ => target,
        };
        self.guide.goal = Some(eased);
        let goal = eased;
        let home = self.home_spot();
        // Until the window has its size there's nowhere to be; after that,
        // at home it sits on the moss wherever the moss has moved to.
        if self.camera.height < 100.0 {
            return (Vec::new(), None, Vec::new());
        }
        if self.guide.flight.x < -50.0 || (self.guide.flight.home && aim == Aim::Home) {
            self.guide.flight.place(home.0, home.1);
        }
        let at_home_goal = (goal.0 - home.0).abs() < 1.0 && (goal.1 - home.1).abs() < 1.0;
        // Pointed at from beside, with a soft ring, never by flying round it.
        let circle = false;
        // In free look it drifts rather than darts.
        self.guide.flight.gentle = self.free_look();
        if !self.panel_open {
            self.guide
                .flight
                .step(real, dt, goal, pointing, circle, !at_home_goal);
        }
        let settled = at_home_goal
            && ((self.guide.flight.x - home.0).powi(2) + (self.guide.flight.y - home.1).powi(2))
                .sqrt()
                < 3.0
            && self.guide.flight.speed() < 25.0;
        if settled {
            self.guide.flight.place(home.0, home.1);
        }
        self.guide.flight.home = settled;
        let near_goal = ((self.guide.flight.x - goal.0).powi(2)
            + (self.guide.flight.y - goal.1).powi(2))
        .sqrt()
            < 60.0;
        if pointing.is_some() && near_goal && self.guide.pointing_since.is_none() {
            self.guide.pointing_since = Some(real);
        }

        let (fx, fy) = (self.guide.flight.x, self.guide.flight.y);
        let toward = |(px, py): Spot| {
            (
                ((px - fx) / 120.0).clamp(-1.0, 1.0),
                ((py - fy) / 160.0).clamp(-1.0, 1.0),
            )
        };
        // What it looks at: whatever it's showing; where it's about to go;
        // a meteor as it flies; or whatever it's idly looking at.
        let idle = self.guide.idle.doing(real);
        let gaze = if let Some(p) = pointing {
            Some(toward(p))
        } else if let Some(h) = self.guide.flight.heading(real) {
            Some((h, 0.0))
        } else if let Some(m) = self.meteor_head(real).filter(|_| settled) {
            Some(toward(m))
        } else {
            match idle {
                // By day it watches a bird as it goes over.
                Some(Kind::LookUp) if self.by_day() => self.day_bird(real).map(toward),
                Some(Kind::LookUp) => self.guide.idle_spot.map(toward),
                Some(Kind::Watch) => self.pointer.map(|(x, y, _)| toward((x, y))),
                Some(Kind::LookOut) => Some((0.0, 0.0)),
                // By day, any bird going over catches its eye.
                _ if settled && self.by_day() => self.day_bird(real).map(toward),
                _ => None,
            }
        };
        let look = gaze.map(|g| g.0);
        self.guide.wisp.set_look_up(gaze.map(|g| g.1));
        let mut sprites = Vec::new();
        let mut points = self.guide.flight.embers(real, alpha as f32);
        // The moss stays put; the wisp is drawn the same way whether it sits
        // on it or flies, so going home has no seam.
        // It changes slowly, so it's drawn a few times a second.
        let stale = self
            .guide
            .moss
            .as_ref()
            .is_none_or(|(at, scale, _)| real - at > 250 || *scale != self.guide.scale);
        if stale {
            self.guide.moss = render_moss(&self.guide.wisp, real as f64, self.guide.scale)
                .map(|t| (real, self.guide.scale, t));
        }
        if let Some((_, _, texture)) = self.guide.moss.clone() {
            sprites.push(Sprite {
                texture,
                x: nx,
                y: ny,
                width: nw,
                height: nh,
                alpha,
            });
        }
        let lean = if settled {
            0.0
        } else {
            self.guide.flight.lean()
        };
        let look = if settled {
            look
        } else {
            look.or(Some(lean * 0.8))
        };
        self.guide.wisp.set_flight(true, lean, look);
        let (bw, bh) = if self.calm() {
            (1.0, 1.0)
        } else {
            self.guide.flight.body()
        };
        self.guide.wisp.set_body(bw, bh);
        // Behind a page opened over the sky the frames come slowly, so the
        // wisp rests still rather than breathing and bobbing in jerks.
        // The picture is drawn a few times a second, or every frame while it
        // flies or does something; in between, its breath is a scale of the
        // same picture on the GPU, so it breathes smoothly for next to nothing.
        let busy = self.guide.flight.speed() > 12.0
            || self.guide.flight.wobbling()
            || self.guide.wisp.gesturing(real as f64);
        // The wisp says itself how soon it needs drawing again: a few times
        // a second at rest, quickly for a blink or a change of mood.
        let stale = self
            .guide
            .wisp_drawn
            .as_ref()
            .is_none_or(|(due, scale, _)| real >= *due || *scale != self.guide.scale);
        if busy || stale || self.panel_open {
            self.guide.wisp_drawn = render_flying(
                &mut self.guide.wisp,
                real as f64,
                self.guide.scale,
                !self.panel_open,
            )
            .map(|(t, next)| {
                let wait = next.map_or(WISP_REDRAW_MS, |ms| (ms as UnixMs).min(WISP_REDRAW_MS * 4));
                (real + wait, self.guide.scale, t)
            });
        }
        if let Some((_, _, texture)) = self.guide.wisp_drawn.clone() {
            let breath = if self.panel_open {
                1.0
            } else {
                self.guide.wisp.breath_scale(real as f64)
            };
            let (sw, sh) = (FLYING.0 * SIZE * breath, FLYING.1 * SIZE * breath);
            sprites.push(Sprite {
                texture,
                x: fx - sw / 2.0,
                y: fy - sh / 2.0,
                width: sw,
                height: sh,
                alpha,
            });
        }

        // A soft glow round what it's pointing at, for a moment.
        // Not in free look: the thing already glows under the pointer.
        if let (Some(since), Some((px, py)), false) =
            (self.guide.pointing_since, pointing, self.free_look())
        {
            let age = (real - since) as f64;
            if age < 2_600.0 {
                let a = (1.0 - age / 2_600.0) as f32;
                let r = 22.0 + 6.0 * (age / 400.0).sin();
                for k in 0..16 {
                    let t = k as f64 / 16.0 * std::f64::consts::TAU + age / 900.0;
                    points.push(Point {
                        x: px + r * t.cos(),
                        y: py + r * t.sin(),
                        radius: 1.4,
                        color: [1.0, 0.82, 0.55],
                        alpha: 0.7 * a * alpha as f32,
                        halo: 0.6,
                    });
                }
            }
        }

        // In a small window, while something is asked, what the wisp says
        // goes into the prompt itself (see `aside`), not a bubble behind it.
        let aside = small && self.talk.prompt.is_some();
        let bubble = self.guide.line.as_ref().filter(|_| !aside).and_then(|l| {
            let a = envelope(real - l.shown, 350, l.hold, 450);
            if a <= 0.0 {
                return None;
            }
            // It rises a little as it appears, and sinks as it goes.
            let lift = (1.0 - a) * 8.0;
            let w = self.camera.width;
            // Narrower in a narrow window, and clear of the list there.
            let list = if small && self.show_tonight() {
                196.0
            } else {
                0.0
            };
            let width = (w - 70.0 - list).clamp(150.0, 300.0);
            let (x, bottom, tail) = if small {
                // In a small window it sits just under the panels at the top,
                // clear of the prompt below.
                let lines = (l.text.chars().count() as f64 * 7.6 / width)
                    .ceil()
                    .max(1.0);
                (10.0, 76.0 + lines * 20.0 + 28.0, false)
            } else if settled {
                ((nx + 26.0).min(w - width - 38.0).max(10.0), ny + 14.0, true)
            } else {
                let right = fx + 36.0 + width + 28.0 < w - 10.0;
                let x = if right {
                    fx + 36.0
                } else {
                    fx - 36.0 - width - 28.0
                };
                (x.max(10.0), (fy - 26.0).max(150.0), false)
            };
            let bottom = bottom + lift;
            // As the screen dims for the night, so do the words.
            let late = if matches!(
                self.session.phase(),
                Phase::Dimming | Phase::Finale | Phase::LightsOut
            ) {
                0.35 + 0.65 * brightness
            } else {
                1.0
            };
            Some(Bubble {
                x,
                bottom,
                text: l.text.clone(),
                width,
                alpha: a * alpha.min(1.0) * late,
                tail,
                // Space moves the wisp on first, except while typing.
                more: more.then_some((
                    real as f64 / 1000.0,
                    if self.talk.prompt.as_ref().is_some_and(|p| p.entry) {
                        "click me for more"
                    } else {
                        "Space for more"
                    },
                )),
            })
        });
        // Where the bubble sits, roughly, for a click on it.
        self.guide.bubble_at = bubble.as_ref().map(|b| {
            let lines = (b.text.chars().count() as f64 * 7.6 / b.width)
                .ceil()
                .max(1.0);
            let h = lines * 20.0 + 28.0 + if b.more.is_some() { 14.0 } else { 0.0 };
            (b.x, b.bottom - h, b.width + 28.0, h)
        });
        (sprites, bubble, points)
    }
}
