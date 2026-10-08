//! One visit: the real sky drawn as dots, the hunt, and the ending. Owns the
//! session, the camera and the dot field, takes input, and turns each tick
//! into a frame for the view.

mod catching;
mod detail;
mod finds;
mod input;
mod photos;
mod sky;
#[cfg(test)]
mod walk;

pub(crate) use detail::Subject;

use crate::camera::Camera;
use crate::field::{Field, Rgb, noise3};
use crate::view::{Frame, Text};
use gtk::gdk;
use gtk::gdk::prelude::TextureExt;
use westering_core::catalogues::Kind;
use westering_core::coords::{
    Mat3, Observer, Vec3, alt_az, apply, from_alt_az, horizon, precession, refraction, unit,
};
use westering_core::ephem::{Body, moon_age, moon_phase_name};
use westering_core::finale::{Handoff, handoff};
use westering_core::finds::{Find, Target, night_key, night_of, tonight};
use westering_core::journal::{Journal, Night};
use westering_core::session::{Phase, Session, Timings};
use westering_core::sky::{Sky, limiting_magnitude, see};
use westering_core::time::{MONTHS, UnixMs, civil_date, weekday};

/// How tonight ends: outside to look at the real sky, or off to bed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ending {
    Outside,
    Bed,
}

/// The evening's shape, as the guide at the top left shows it: which step
/// it's at, and what to do now.
#[derive(Clone, Debug, PartialEq)]
pub struct Evening {
    /// 0 setting things down, 1 looking up, 2 winding down.
    pub step: usize,
    pub now: String,
    /// The wisp has finished and it's over to the user.
    pub yours: bool,
}

/// One line of the Tonight list.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub name: String,
    pub kind: &'static str,
    pub whereabouts: String,
    pub found: bool,
}

/// Converts the real clock to the sky's: normally the same, but the feel lab
/// can start the sky at another moment and run it faster.
pub struct Clock {
    pub real0: UnixMs,
    pub sky0: UnixMs,
    pub speed: f64,
}

impl Clock {
    pub fn sky(&self, real: UnixMs) -> UnixMs {
        self.sky0 + ((real - self.real0) as f64 * self.speed) as UnixMs
    }
}

pub struct Options {
    pub clock: Clock,
    pub observer: Observer,
    pub offset_s: i32,
    pub timings: Timings,
    pub journal: Journal,
}

#[derive(Default)]
pub(crate) struct Held {
    pub(crate) space: bool,
    pub(crate) left: bool,
    pub(crate) right: bool,
    pub(crate) up: bool,
    pub(crate) down: bool,
    pub(crate) zoom_in: bool,
    pub(crate) zoom_out: bool,
    pub(crate) fast: bool,
}

/// Easing the view towards somewhere.
#[derive(Clone, Copy)]
pub(crate) struct Look {
    pub(crate) az: f64,
    pub(crate) alt: f64,
    pub(crate) fov: f64,
    /// Per second, 0 to 1: how much of the gap closes.
    pub(crate) rate: f64,
}

#[derive(Default)]
pub(crate) struct Catch {
    pub(crate) target: Option<usize>,
    pub(crate) holding: bool,
    pub(crate) progress: f64,
    /// The field of view before the zoom, to go back to.
    pub(crate) fov_before: Option<f64>,
}

pub(crate) struct Card {
    /// A photograph for the card itself, by its id.
    pub(crate) picture: Option<String>,
    /// A small line at the foot: whose the photograph in the sky is.
    pub(crate) footnote: Option<String>,
    /// The find it's about, if it's one of tonight's.
    pub(crate) find: Option<usize>,
    pub(crate) x: f64,
    pub(crate) kicker: String,
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) shown: UnixMs,
}

/// What the photograph over the sky is of: one of tonight's finds, or any
/// pictured object the view has closed in on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Eye {
    Find(usize),
    Piece(usize),
    /// A planet or the Moon looked at close up, not one of tonight's finds.
    Body(Body),
}

/// A full frame kept for reuse, less the wisp and anything alive.
struct Cached {
    at: UnixMs,
    size: (f64, f64),
    brightness: f64,
    frame: Frame,
}

pub(crate) struct Timed {
    pub(crate) text: String,
    pub(crate) shown: UnixMs,
    pub(crate) hold: UnixMs,
}

pub(crate) struct Meteor {
    start: UnixMs,
    duration: UnixMs,
    from: Vec3,
    toward: Vec3,
    length: f64,
    brightness: f32,
}

/// What the slow-changing base layer was last drawn for.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct BaseStamp {
    az: f64,
    alt: f64,
    fov: f64,
    width: f64,
    height: f64,
    sky_minute: i64,
    lapse: bool,
}

/// A star with everything that doesn't change during a visit worked out.
pub(crate) struct Prepared {
    dir: Vec3,
    mag: f64,
    light: f32,
    tint: Rgb,
    rate: f64,
    phase: f64,
}

pub struct Game {
    pub(crate) sky: Sky,
    pub(crate) prepared: Vec<Prepared>,
    pub(crate) observer: Observer,
    pub(crate) clock: Clock,
    pub(crate) offset_s: i32,
    pub(crate) night: String,
    pub(crate) session: Session,
    pub(crate) finds: Vec<Find>,
    pub(crate) caught: Vec<bool>,
    pub(crate) journal: Journal,
    pub(crate) page: Night,
    pub(crate) camera: Camera,
    pub(crate) look: Option<Look>,
    pub(crate) field: Field,
    pub(crate) base: Option<BaseStamp>,
    pub(crate) star_dirs: Vec<Vec3>,
    pub(crate) held: Held,
    pub(crate) pan: (f64, f64),
    pub(crate) drag_from: Option<(f64, f64, f64, f64)>,
    pub(crate) catch: Catch,
    pub(crate) card: Option<Card>,
    pub(crate) caption: Option<Timed>,
    pub(crate) hint: Option<Timed>,
    pub(crate) meteors: Vec<Meteor>,
    pub(crate) next_meteor: UnixMs,
    pub(crate) handoff: Option<Handoff>,
    pub(crate) finale_turned: bool,
    pub(crate) last_input: UnixMs,
    /// When the guided evening next turns to something to find by itself:
    /// a beat after a card's put away, or a question's answered or let
    /// pass. Anything the user does in the meantime takes over instead.
    pub(crate) move_on: Option<UnixMs>,
    /// Whether it was about to when the user last did something.
    pub(crate) move_on_was: bool,
    /// When Space went down, to tell a tap from a hold.
    pub(crate) space_since: Option<UnixMs>,
    /// Whether the press of Space now down has caught something.
    pub(crate) space_caught: bool,
    /// Something the user did wants drawing now.
    pub urgent: bool,
    /// After a story or a walk, the ring rests until the view moves on.
    pub(crate) ring_resting: bool,
    /// Looking round freely tonight, with the mouse, rather than the
    /// guided way. Every evening starts guided.
    pub(crate) free: bool,
    /// Whether the wisp has handed tonight's sky over, opening free look.
    pub(crate) handed_over: bool,
    /// Whether it's said tonight to look together first.
    pub(crate) told_wait: bool,
    /// When the next look at whether to hand the sky over is due.
    pub(crate) next_handover_check: UnixMs,
    /// In free look, when a waiting question may be offered: a moment
    /// after backing out from a card that was read.
    pub(crate) free_moment: Option<UnixMs>,
    /// Whether free look has had its one question tonight.
    pub(crate) free_asked: bool,
    /// When a press on the ring began, held with the mouse to catch as
    /// Space does.
    pub(crate) mouse_hold: Option<UnixMs>,
    /// When free look began, for the wisp's one suggestion.
    pub(crate) free_since: UnixMs,
    /// Which of tonight's finds have been seen: caught with the card up
    /// long enough to read, or a story begun.
    pub(crate) viewed: Vec<bool>,
    /// Whether it's been said that Tab brings back what was skipped.
    told_tab: bool,
    /// 0-1, eased: how far the keys in the corner are showing.
    legend_mix: f64,
    /// The table of keys, up or put away, and since when.
    pub(crate) help: bool,
    pub(crate) help_changed: UnixMs,
    /// Key releases wait a moment: X11's auto-repeat sends a release before
    /// every repeated press, and a real release has no press behind it.
    pub(crate) releases: Vec<(gdk::Key, UnixMs)>,
    pub(crate) esc_armed: UnixMs,
    pub(crate) last_real: UnixMs,
    pub(crate) rng: u64,
    pub quit: bool,
    pub(crate) talk: crate::talk::Talk,
    pub(crate) drawing: Option<crate::drawing::Drawing>,
    pub(crate) reveal: Option<(usize, UnixMs)>,
    /// Something the window should open: the logbook, a course, settings.
    pub(crate) request: Option<crate::talk::Request>,
    pub(crate) guide: crate::guide::Guide,
    pub(crate) points: Vec<crate::view::Point>,
    /// Where Jupiter's moons were drawn this frame, to name them.
    moon_labels: Vec<(f64, f64, &'static str, f32)>,
    pub(crate) photos: crate::eyepiece::Photos,
    /// A find the view keeps centred as the sky turns, like a telescope's drive.
    pub(crate) track: Option<usize>,
    /// The find the eyepiece shows, and how far it has faded in.
    eye: Option<Eye>,
    eye_alpha: f64,
    /// How many steps of each hop have been found.
    pub(crate) steps: Vec<usize>,
    /// A find being told a card at a time.
    pub(crate) tour: Option<crate::tour::Tour>,
    /// Tonight's story, once it's been heard to its last page.
    pub(crate) story_heard: Option<usize>,
    /// Tonight's places on the Moon, by index into the features.
    pub(crate) moon_stops: Vec<usize>,
    /// Algol's place among the prepared stars: it dims now and then.
    algol: Option<usize>,
    /// Points drawn over the photographs.
    pub(crate) marks: Vec<crate::view::Point>,
    /// The star pattern being shown, and how far it has faded in.
    pub(crate) pattern: Option<Vec<(u16, u16)>>,
    pub(crate) pattern_alpha: f64,
    /// The card as last drawn, and one on its way out with when it went.
    shown_card: Option<crate::view::CardView>,
    leaving_card: Option<(crate::view::CardView, UnixMs)>,
    /// Where the pointer is over the sky, and when it last moved.
    pub(crate) pointer: Option<(f64, f64, UnixMs)>,
    pub(crate) hovered: Option<crate::hover::Hovered>,
    /// What's glowing near the pointer, in free look.
    pub(crate) glowing: Vec<crate::hover::GlowState>,
    /// How far the ring round what the pointer is on has come in.
    pub(crate) ring_level: f64,
    /// Something clicked in free look, being looked at close up.
    pub(crate) inspect: Option<detail::Inspect>,
    /// Until when the view is still backing out from a close look.
    pub(crate) backing_out: UnixMs,
    /// How tonight ends, once chosen.
    pub(crate) ending: Option<Ending>,
    /// The guide has offered to wind down.
    offered_wind_down: bool,
    /// Plans whose night is tonight: what, and where in the sky it happens.
    pub(crate) plan_marks: Vec<PlanMark>,
    /// Keeping the user company in the background.
    pub(crate) keep: crate::keep::Keep,
    /// A visit while the Sun is up, looking down at the ground.
    pub(crate) day: Option<crate::day::Day>,
    /// Winding down slowly, with breaths and a few thoughts, if wanted.
    pub(crate) wind: crate::wind::Wind,
    /// Whether the window has the keyboard.
    pub focused: bool,
    /// When to see again whether the darkening sky has more to find.
    next_look_again: UnixMs,
    /// Whether a page (the logbook, the menu) is open over the sky.
    pub panel_open: bool,
    /// The last full frame, for drawing the wisp over while little else
    /// is changing.
    cached: Option<Cached>,
    /// How bright the wisp was drawn in the last full frame.
    pub(crate) sprite_brightness: f64,
    /// The style and title of the track playing.
    pub playing: Option<(String, String)>,
    /// The styles of music there are, by id and name.
    pub styles: Vec<(String, String)>,
    pub(crate) ground: westering_core::ground::Ground,
}

/// Where a planned sky event happens.
#[derive(Clone, Debug)]
pub(crate) enum PlanWhere {
    Body(Body),
    /// A fixed place among the stars, J2000: a meteor shower's radiant, or
    /// a constellation.
    Stars(f64, f64),
}

/// Tonight's plan, marked on the horizon under where it happens.
#[derive(Clone, Debug)]
pub(crate) struct PlanMark {
    /// "Tonight: watching the Geminids, with Sam".
    pub(crate) words: String,
    /// What to look for, as it starts a sentence: "The Moon", "Saturn",
    /// "Orion"; for a shower, its name alone.
    pub(crate) name: String,
    pub(crate) shower: bool,
    pub(crate) place: PlanWhere,
    /// When it comes up and at what bearing, if it was down as the
    /// evening began and rises before morning.
    pub(crate) rise: Option<(UnixMs, f64)>,
}

impl PlanWhere {
    /// Its altitude and azimuth for someone at a moment.
    pub(crate) fn seen(&self, observer: Observer, at: UnixMs) -> (f64, f64) {
        match *self {
            PlanWhere::Body(b) => {
                let s = see(b, observer, at);
                (s.alt, s.az)
            }
            PlanWhere::Stars(ra, dec) => alt_az(apply(
                &horizon(observer, at),
                apply(&precession(at), unit(ra, dec)),
            )),
        }
    }
}

pub(crate) const WARM: Rgb = [1.0, 0.86, 0.66];

/// Whether a window is small enough for the compact layout: narrower
/// panels, and the compass strip along the bottom.
pub(crate) fn compact(width: f64, height: f64) -> bool {
    width < 820.0 || height < 560.0
}

/// A card up at least this long has been seen.
const VIEWED_MS: UnixMs = 3_000;

/// A press of Space on something in the ring, let go sooner than this, is
/// a tap rather than the start of a catch.
const TAP_MS: UnixMs = 350;

/// How many finds a visit in the small hours offers.
const LATE_FINDS: usize = 5;

/// Whether it's the small hours, by the local clock: past midnight and
/// before five.
pub(crate) fn late(now: UnixMs, offset_s: i32) -> bool {
    westering_core::time::clock(now, offset_s).0 < 5
}
const RETICLE: Rgb = [0.78, 0.84, 1.0];

pub(crate) fn smoothstep(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Fades in over `rise`, holds, and fades out over `fall`.
pub(crate) fn envelope(age: UnixMs, rise: UnixMs, hold: UnixMs, fall: UnixMs) -> f64 {
    if age < 0 {
        0.0
    } else if age < rise {
        smoothstep(age as f64 / rise as f64)
    } else if age < rise + hold {
        1.0
    } else {
        1.0 - smoothstep((age - rise - hold) as f64 / fall.max(1) as f64)
    }
}

/// The gentle hills along the horizon, in degrees of altitude.
pub(crate) fn hills(az: f64) -> f64 {
    let a = az.to_radians();
    (0.75
        + 0.45 * (3.0 * a + 0.7).sin()
        + 0.3 * (7.0 * a + 2.1).sin()
        + 0.18 * (13.0 * a + 0.3).sin())
    .max(0.15)
}

/// How many magnitudes fainter the view reaches when zoomed in to `fov`,
/// the way a telescope gathers more light than the eye.
pub(crate) fn gather(fov: f64) -> f64 {
    2.5 * (60.0 / fov).clamp(1.0, 100.0).log10()
}

/// Stars brighter than this are drawn as true points; fainter ones on the lattice.
const BRIGHT: f64 = 3.0;

/// A bright star or planet as a crisp point with a halo, sized by its light.
fn point(x: f64, y: f64, color: Rgb, amount: f32, halo: f32) -> crate::view::Point {
    crate::view::Point {
        x,
        y,
        radius: (1.05 + 0.62 * amount).min(4.4),
        color,
        alpha: (0.5 + 0.35 * amount).min(1.0),
        halo: (0.25 + amount / 2.2).min(1.0) * halo,
    }
}

/// How much light a star of magnitude `mag` puts on its dot.
fn light(mag: f64) -> f32 {
    (1.6 * 10f64.powf(-0.4 * (mag - 1.0)).powf(0.55)) as f32
}

pub(crate) fn transpose(m: &Mat3) -> Mat3 {
    [
        [m[0][0], m[1][0], m[2][0]],
        [m[0][1], m[1][1], m[2][1]],
        [m[0][2], m[1][2], m[2][2]],
    ]
}

pub(crate) fn dot3(a: Vec3, b: Vec3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub(crate) fn angle_between(a: Vec3, b: Vec3) -> f64 {
    dot3(a, b).clamp(-1.0, 1.0).acos().to_degrees()
}

/// Shortest signed turn from one azimuth to another.
pub(crate) fn turn(from: f64, to: f64) -> f64 {
    (to - from + 540.0).rem_euclid(360.0) - 180.0
}

impl Game {
    pub fn new(options: Options, real_now: UnixMs) -> Game {
        let sky = Sky::bundled();
        let Options {
            clock,
            observer,
            offset_s,
            timings,
            journal,
        } = options;
        let now = clock.sky(real_now);
        let ground = westering_core::ground::Ground::bundled();
        let by_day = crate::day::is_day(observer, now);
        // A day visit belongs to today, and shares its page with tonight.
        let night = if by_day {
            westering_core::questions::key_of(now, offset_s)
        } else {
            night_key(night_of(now, offset_s))
        };
        let day = by_day.then(|| crate::day::Day::new(&ground, observer, now, offset_s, &night));
        let mut finds = if by_day {
            Vec::new()
        } else {
            tonight(&sky, observer, now, offset_s, &|id| {
                journal.times_found_before(id, &night)
            })
        };
        // In the small hours, a shorter evening.
        if late(now, offset_s) {
            finds.truncate(LATE_FINDS);
        }
        let caught: Vec<bool> = finds
            .iter()
            .map(|f| journal.found_on(&f.id, &night))
            .collect();
        let still = match &day {
            Some(d) => d.finds.len(),
            None => caught.iter().filter(|c| !**c).count(),
        };
        let mut session = Session::new(real_now, timings, still, !by_day);
        if still == 0 {
            session.found_one(real_now);
        }
        let page = journal.night(&night).unwrap_or_else(|| Night {
            key: night.clone(),
            ..Night::default()
        });
        let talk = crate::talk::Talk::new(
            &journal,
            &night,
            crate::talk::calendar(&sky.lists.showers, now),
        );
        let already = !finds.is_empty() && caught.iter().all(|c| *c);
        // Tonight's plans, placed where their event happens.
        let around = westering_core::events::upcoming(
            &sky.lists.showers,
            now - westering_core::time::DAY * 2,
            4,
        );
        let plan_marks = journal
            .plans
            .iter()
            .filter(|p| p.date == night)
            .filter_map(|p| {
                use westering_core::events::Kind as Event;
                let (name, shower, place) = if p.event == westering_core::questions::ORION_RETURN {
                    let orion = sky.figures.iter().find(|f| f.abbrev == "Ori")?;
                    let (ra, dec) = westering_core::coords::angles(orion.centre(&sky.stars)?.0);
                    ("Orion".to_owned(), false, PlanWhere::Stars(ra, dec))
                } else {
                    match &around.iter().find(|e| e.title == p.event)?.kind {
                        Event::FullMoon => {
                            ("The Moon".to_owned(), false, PlanWhere::Body(Body::Moon))
                        }
                        Event::Pairing(b) => (
                            format!("The Moon, with {} beside it,", b.name()),
                            false,
                            PlanWhere::Body(Body::Moon),
                        ),
                        Event::Opposition(b) => (b.name().to_owned(), false, PlanWhere::Body(*b)),
                        Event::Shower { id, .. } => {
                            let s = sky.lists.showers.iter().find(|s| &s.id == id)?;
                            (s.name.clone(), true, PlanWhere::Stars(s.ra, s.dec))
                        }
                        Event::NewMoon => return None,
                    }
                };
                let who = p
                    .who
                    .as_deref()
                    .map(|w| format!(", with {w}"))
                    .unwrap_or_default();
                // Still down: the mark goes where it comes up, if that's
                // before the morning, and says when.
                let rise = (place.seen(observer, now).0 <= 0.0)
                    .then(|| {
                        westering_core::sky::rising(now, 12 * westering_core::time::HOUR, |t| {
                            place.seen(observer, t)
                        })
                    })
                    .flatten()
                    .filter(|(at, _)| westering_core::sky::sun_altitude(observer, *at) < -6.0);
                Some(PlanMark {
                    words: format!("Tonight: {}{who}", p.what),
                    name,
                    shower,
                    place,
                    rise,
                })
            })
            .collect();
        let star_dirs = sky.stars.precessed(&precession(now));
        let prepared: Vec<Prepared> = sky
            .stars
            .stars
            .iter()
            .zip(&star_dirs)
            .take_while(|(s, _)| s.mag <= 6.2)
            .map(|(s, &dir)| Prepared {
                dir,
                mag: s.mag as f64,
                light: light(s.mag as f64),
                tint: westering_core::stars::tint(s.bv),
                rate: 0.9 + (s.hr % 7) as f64 * 0.23,
                phase: s.hr as f64,
            })
            .collect();
        let moon_stops = westering_core::tours::moon_stops(
            &sky.tours.moon,
            moon_age(now) * westering_core::tours::SYNODIC_DAYS,
            night_of(now, offset_s),
            &|name| journal.found_before(&format!("moon:{name}"), &night),
        );
        let algol = sky
            .stars
            .stars
            .iter()
            .take(prepared.len())
            .position(|s| s.hr == 936);
        let steps = vec![0; finds.len()];
        // Anything caught earlier tonight was seen then.
        let viewed = caught.clone();
        let wind = crate::wind::Wind::bundled();
        let mut game = Game {
            sky,
            prepared,
            observer,
            clock,
            offset_s,
            night,
            session,
            finds,
            caught,
            journal,
            page,
            camera: Camera::new(180.0, 30.0, 95.0),
            look: None,
            field: Field::new(),
            base: None,
            star_dirs,
            held: Held::default(),
            pan: (0.0, 0.0),
            drag_from: None,
            catch: Catch::default(),
            card: None,
            caption: None,
            hint: None,
            meteors: Vec::new(),
            next_meteor: real_now + 8_000,
            handoff: None,
            finale_turned: false,
            last_input: real_now,
            move_on: None,
            move_on_was: false,
            space_since: None,
            space_caught: false,
            urgent: false,
            ring_resting: false,
            free_since: real_now,
            free: false,
            handed_over: false,
            told_wait: false,
            next_handover_check: 0,
            free_moment: None,
            free_asked: false,
            mouse_hold: None,
            viewed,
            told_tab: false,
            legend_mix: 0.0,
            help: false,
            help_changed: 0,
            releases: Vec::new(),
            esc_armed: 0,
            last_real: real_now,
            rng: (real_now as u64) ^ 0x9e37_79b9_7f4a_7c15,
            quit: false,
            talk,
            drawing: None,
            reveal: None,
            request: already.then_some(crate::talk::Request::Book(None)),
            guide: crate::guide::Guide::new(real_now),
            points: Vec::new(),
            photos: crate::eyepiece::Photos::load(),
            moon_labels: Vec::new(),
            track: None,
            eye: None,
            eye_alpha: 0.0,
            steps,
            tour: None,
            story_heard: None,
            moon_stops,
            algol,
            marks: Vec::new(),
            pattern: None,
            pattern_alpha: 0.0,
            shown_card: None,
            leaving_card: None,
            pointer: None,
            hovered: None,
            glowing: Vec::new(),
            ring_level: 0.0,
            inspect: None,
            backing_out: 0,
            ending: None,
            offered_wind_down: false,
            plan_marks,
            keep: crate::keep::Keep::new(),
            wind,
            day,
            ground,
            focused: true,
            next_look_again: 0,
            panel_open: false,
            cached: None,
            sprite_brightness: 1.0,
            playing: None,
            styles: Vec::new(),
        };
        game.arrive(real_now);
        game
    }

    fn random(&mut self) -> f64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 11) as f64 / (1u64 << 53) as f64
    }

    pub(crate) fn sky_now(&self, real: UnixMs) -> UnixMs {
        self.clock.sky(real) + self.session.lapse(real)
    }

    /// The first view and the line that names the night.
    fn arrive(&mut self, real: UnixMs) {
        if self.by_day() {
            self.day_arrival(real);
            return;
        }
        let now = self.clock.sky(real);
        let first = self
            .finds
            .iter()
            .enumerate()
            .find(|(i, f)| !self.caught[*i] && matches!(f.target, Target::Body(_)))
            .map(|(i, _)| i);
        let (az, alt) = first
            .and_then(|i| self.find_dir(i, now, &horizon(self.observer, now), &precession(now)))
            .map(|v| {
                let (alt, az) = alt_az(v);
                (az, alt.clamp(15.0, 62.0))
            })
            .unwrap_or((180.0, 30.0));
        self.camera.az = az;
        self.camera.alt = alt;
        self.camera.update();

        let (_, month, day) = civil_date(now, self.offset_s);
        let mut line = format!(
            "{} {} {}",
            weekday(now, self.offset_s),
            day,
            MONTHS[month as usize - 1]
        );
        let moon = moon_phase_name(moon_age(now));
        self.page.moon = moon.to_owned();
        if see(Body::Moon, self.observer, now).alt > 0.0 {
            line.push_str(&format!(" · {moon}"));
        }
        let planets: Vec<&str> = self
            .finds
            .iter()
            .filter_map(|f| match f.target {
                Target::Body(b) if b != Body::Moon => Some(b.name()),
                _ => None,
            })
            .collect();
        match planets.len() {
            0 => {}
            1 => line.push_str(&format!(" · {} is up", planets[0])),
            n => line.push_str(&format!(
                " · {} and {} are up",
                planets[..n - 1].join(", "),
                planets[n - 1]
            )),
        }
        if self.caught.iter().all(|c| *c) && !self.finds.is_empty() {
            line.push_str(" · you've found tonight's sky");
        }
        if let Some(extra) = self.arrival_extra(now) {
            line.push('\n');
            line.push_str(&extra);
        }
        self.caption = Some(Timed {
            text: line,
            shown: real + 900,
            hold: 8_000,
        });
        self.guide_arrival(real);
    }

    pub fn resize(&mut self, width: f64, height: f64, scale: f64) {
        self.camera.set_size(width, height);
        self.guide.scale = scale;
    }

    /// The style of music chosen, by its folder's name.
    pub fn style(&self) -> Option<&str> {
        self.journal.settings.style.as_deref()
    }

    pub(crate) fn hunting(&self) -> bool {
        self.session.phase() == Phase::Hunt
    }

    fn save_page(&self) {
        if let Err(e) = self.journal.save_night(&self.page) {
            eprintln!("westering: couldn't save tonight's page: {e}");
        }
    }

    /// Moves time on by one tick and draws.
    pub fn tick(&mut self, real: UnixMs) -> Frame {
        let dt = ((real - self.last_real) as f64 / 1000.0).clamp(0.0, 0.25);
        self.last_real = real;
        self.settle_releases(real);
        self.logic(real);
        // Unfocused, or keeping company, the sky and ground change slowly:
        // they're drawn a few times a second, while the wisp and anything
        // alive keep moving smoothly on top.
        // The sky and ground change slowly: at rest they're drawn a few
        // times a second (fewer unfocused, fewer still keeping company),
        // while the wisp and anything alive move smoothly on top.
        let every = if self.keeping() {
            250
        } else if !self.focused {
            125
        } else {
            66
        };
        let pointing = self.pointer.is_some_and(|(_, _, at)| real - at < 400);
        // Keeping company the sky is drawn so seldom that a word fading at
        // the top would step: it's drawn afresh while one is up.
        let telling = self.keeping() && self.status_up(real);
        let quiet = !self.winding()
            && !pointing
            && !telling
            && self.wisp_motion() < 2
            && !self.wants_fast_frames(real);
        if quiet
            && let Some(c) = &self.cached
            && real - c.at < every
            && c.size == (self.camera.width, self.camera.height)
        {
            let (mut frame, brightness) = (c.frame.clone(), c.brightness);
            let (sprites, bubble, embers) = self.guide_frame(real, brightness);
            frame.sprites = sprites;
            frame.bubble = bubble;
            frame.marks.extend(embers);
            if self.by_day() {
                frame.silhouettes = self.day_life(real);
            }
            self.overlay(&mut frame, real, dt);
            return frame;
        }
        let mut frame = if self.by_day() {
            self.day_frame(real)
        } else {
            self.night_frame(real, dt)
        };
        if self.winding() {
            frame.texts.extend(self.wind_texts(real));
            frame.points.extend(self.wind_ring(real));
        }
        self.cached = Some(Cached {
            at: real,
            size: (self.camera.width, self.camera.height),
            brightness: self.sprite_brightness,
            frame: Frame {
                glows: Vec::new(),
                sprites: Vec::new(),
                bubble: None,
                marks: Vec::new(),
                silhouettes: Vec::new(),
                ..frame.clone()
            },
        });
        self.overlay(&mut frame, real, dt);
        frame
    }

    /// What follows the pointer, drawn fresh every frame over a full or a
    /// reused one.
    fn overlay(&mut self, frame: &mut Frame, real: UnixMs, dt: f64) {
        if self.by_day() || self.keeping() {
            return;
        }
        let (glows, texts) = self.hover_overlay(real, dt);
        frame.glows = glows;
        frame.texts.extend(texts);
    }

    /// Everything that isn't drawing: the arc of the visit, company, what's
    /// being asked and what the wisp is up to.
    fn logic(&mut self, real: UnixMs) {
        if self.by_day() {
            self.day_logic(real);
        } else {
            // One thing at a time: the evening begins once the wisp has had
            // its say.
            let greeting = self.session.phase() == Phase::Arrival && self.wisp_busy(real);
            if !greeting && let Some(phase) = self.session.tick(real) {
                self.entered(phase, real);
            }
            if self.session.phase() == Phase::Over {
                self.quit = true;
            }
            if !self.keeping() {
                self.more_as_it_darkens(real);
            }
        }
        self.company_tick(real, true);
        if !self.keeping() {
            self.tick_talk(real);
        }
        self.tick_move_on(real);
        self.wind_tick(real);
        self.guide_tick(real);
    }

    fn night_frame(&mut self, real: UnixMs, dt: f64) -> Frame {
        let tempo = self.session.tempo(real);
        self.steer(dt, tempo);
        let now = self.sky_now(real);
        let hz = horizon(self.observer, now);
        let prec = precession(now);
        self.update_catch(dt, tempo, real, now, &hz, &prec);
        self.spawn_meteors(real, now, &hz);
        self.draw(real, now, &hz, &prec, tempo, dt)
    }

    pub(crate) fn entered(&mut self, phase: Phase, real: UnixMs) {
        if matches!(phase, Phase::Dimming | Phase::Finale) {
            self.talk.flow = None;
            self.talk.pending = None;
            self.set_prompt(None);
            self.drawing = None;
            // Nothing said about the hunt still applies.
            self.hush();
        }
        match phase {
            Phase::Weights => self.begin_weights(real),
            Phase::Hunt => self.guide_hunt(real),
            Phase::Dimming => {
                self.card = None;
                self.catch = Catch::default();
                self.hint = None;
                // Back out to the open sky, held on nothing.
                self.track = None;
                self.look = Some(Look {
                    az: self.camera.az,
                    alt: self.camera.alt.clamp(15.0, 45.0),
                    fov: 90.0,
                    rate: 0.8,
                });
                self.ask_wind();
            }
            Phase::Finale => {
                self.set_prompt(None);
                self.talk.flow = None;
                let now = self.clock.sky(real);
                let mut last = match self.ending {
                    Some(Ending::Bed) => self.goodnight(),
                    _ => handoff(&self.sky, self.observer, now, self.offset_s),
                };
                // When someone has written something heavy, the last line is a person.
                let pole = if self.observer.lat >= 0.0 { 424 } else { 4730 };
                if self.talk.caring
                    && let Some(p) = self.journal.person_on(pole)
                {
                    last.line = format!(
                        "{} would pick up if you rang, any time. {}",
                        p.name, last.line
                    );
                }
                self.handoff = Some(last);
                self.caption = None;
                self.track = None;
                // Face the west, where tonight's stars go down.
                self.look = Some(Look {
                    az: 268.0,
                    alt: 14.0,
                    fov: 100.0,
                    rate: 0.9,
                });
                self.finale_turned = false;
            }
            _ => {}
        }
        // After the prompt for it is up, so what's said is about that one.
        self.guide_phase(phase, real);
    }

    pub(crate) fn offered_wind_down(&self) -> bool {
        self.offered_wind_down
    }

    pub(crate) fn set_offered_wind_down(&mut self) {
        self.offered_wind_down = true;
    }

    /// Starts the evening's last part, when the user chooses to.
    pub fn wind_down(&mut self, real: UnixMs) {
        if !matches!(
            self.session.phase(),
            Phase::Arrival | Phase::Weights | Phase::Hunt
        ) {
            return;
        }
        self.end_tour(real);
        self.card = None;
        self.catch = Catch::default();
        if let Some(p) = self.session.finish(real) {
            self.entered(p, real);
        }
    }

    /// Where the evening is, and what to do now.
    pub fn evening(&self, real: UnixMs) -> Option<Evening> {
        if self.keeping() || self.by_day() {
            return None;
        }
        let (step, now) = match self.session.phase() {
            Phase::Arrival => (0, "Settling in"),
            Phase::Weights if self.placing() => (0, "Arrows move it, Enter leaves it there to set"),
            Phase::Weights => (
                0,
                "Write down what's on your mind, or choose Nothing tonight",
            ),
            Phase::Hunt if self.tour.is_some() => (1, "Space goes on, Esc stops here"),
            Phase::Hunt if self.card.is_some() && self.free_look() => {
                (1, "Esc, or a click on the sky, zooms back out")
            }
            Phase::Hunt if self.card.is_some() => (1, "Space to carry on"),
            Phase::Hunt if self.talk.prompt.is_some() => {
                (1, "Answer if you like, or Esc to let it pass")
            }
            Phase::Hunt if self.drawing.is_some() => {
                (1, "Arrows pick stars, Enter joins them, C when done")
            }
            Phase::Hunt if self.session.all_found() => (
                1,
                "That's tonight's sky. Look around, or wind down when you're ready",
            ),
            Phase::Hunt if self.free_look() => (
                1,
                "Drag to look around, and click anything that glows for a closer look. F for the guided way",
            ),
            Phase::Hunt if self.caught.iter().any(|c| *c) => (
                1,
                "Tap Space for the next find, hold it to catch. Point at anything to see what it is",
            ),
            Phase::Hunt => (
                1,
                "Pick something from Tonight's list, then hold Space when it's in the ring",
            ),
            Phase::Dimming if self.reflecting() => {
                (2, "One thought to end on. Enter moves on when you're ready")
            }
            Phase::Dimming if self.winding() => (2, "Breathe with the wisp, or Esc to stop"),
            Phase::Dimming if matches!(self.talk.flow, Some(crate::talk::Flow::WindChoice)) => {
                (2, "Choose, or let the screen dim")
            }
            Phase::Dimming if self.talk.prompt.is_some() => (2, "Choose how tonight ends"),
            Phase::Dimming => (2, "The screen is dimming: let your eyes and mind settle"),
            Phase::Finale if !self.session.last_line(real) && self.page.weights.is_empty() => {
                (2, "Watch the sky turn through the rest of the night")
            }
            Phase::Finale if !self.session.last_line(real) => {
                (2, "Watch the west: what you set down tonight is setting")
            }
            Phase::Finale => (2, "That's all for tonight"),
            Phase::LightsOut | Phase::Over => return None,
        };
        // When the wisp has nothing more to say and something is asked of
        // the user, say so: it's their turn.
        // In free look nothing is asked: it's theirs all along.
        let yours = !self.wisp_busy(real)
            && matches!(self.session.phase(), Phase::Weights | Phase::Hunt)
            && !(self.hunting() && self.free_look());
        Some(Evening {
            step,
            now: if yours {
                format!("Your turn: {now}")
            } else {
                now.to_owned()
            },
            yours,
        })
    }

    /// Whether the evening is winding down, when only the calmest music
    /// plays.
    pub fn settling(&self) -> bool {
        !self.by_day()
            && !self.keeping()
            && matches!(
                self.session.phase(),
                Phase::Dimming | Phase::Finale | Phase::LightsOut
            )
    }

    /// How loud the music should be, 0 to 1: it arrives with the sky, eases
    /// down as the sky dims and goes with the lights.
    pub fn music_level(&self, real: UnixMs) -> f64 {
        let b = self.session.brightness(real);
        let dim = westering_core::session::DIM;
        let level = match self.session.phase() {
            Phase::LightsOut | Phase::Over => 0.6 * b / dim,
            _ => 0.6 + 0.4 * ((b - dim) / (1.0 - dim)).max(0.0),
        };
        level * self.company_music(real) * self.day_music(real)
    }

    pub fn quiet(&self) -> bool {
        self.journal.settings.quiet
    }

    pub fn volume(&self) -> f64 {
        self.journal.settings.volume.unwrap_or(1.0).clamp(0.0, 1.0)
    }

    pub(crate) fn calm(&self) -> bool {
        self.journal.settings.calm
    }

    /// How often to draw, in milliseconds: smooth while anything moves,
    /// half pace while the wisp hovers or something fades, slow at rest.
    pub fn frame_ms(&self, real: UnixMs) -> i64 {
        let fast = self.wants_fast_frames(real);
        if self.keeping() && !fast && self.wisp_motion() == 0 && self.keep.mix > 0.99 {
            // In the background: only the wisp and anything alive move
            // smoothly, and cheaply; the sky is redrawn a few times a second.
            return 50;
        }
        if fast || self.wisp_motion() == 2 {
            16
        } else {
            // At rest the wisp still breathes at 30 frames a second; only
            // every other frame draws the sky (see `tick`).
            33
        }
    }

    /// Where a meteor's head is on the screen, while one is flying.
    pub(crate) fn meteor_head(&self, real: UnixMs) -> Option<(f64, f64)> {
        self.meteors.iter().find_map(|m| {
            let age = (real - m.start) as f64 / m.duration as f64;
            if !(0.0..1.0).contains(&age) {
                return None;
            }
            let th = age * m.length;
            let v = [
                m.from[0] * th.cos() + m.toward[0] * th.sin(),
                m.from[1] * th.cos() + m.toward[1] * th.sin(),
                m.from[2] * th.cos() + m.toward[2] * th.sin(),
            ];
            self.camera
                .project(v)
                .filter(|&(x, y)| self.camera.on_screen(x, y, 0.0))
        })
    }

    /// Keeps the frame clock's pace honest: fast while things move, slow at rest.
    pub fn wants_fast_frames(&self, real: UnixMs) -> bool {
        self.winding()
            || self.look.is_some()
            || self.pan.0.abs() > 1e-3
            || self.pan.1.abs() > 1e-3
            || self.drag_from.is_some()
            || self.catch.progress > 0.0
            || self.meteors.iter().any(|m| real >= m.start - 50)
            || matches!(
                self.session.phase(),
                Phase::Arrival | Phase::Finale | Phase::LightsOut
            )
            || self.session.lapse_progress(real) > 0.0
    }
}
