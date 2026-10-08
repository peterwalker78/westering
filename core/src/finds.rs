//! Tonight's handful of finds, chosen from what is really up. The set depends
//! only on the night, the place and what has been found before, so opening
//! the app again the same night never rerolls it.

use crate::catalogues::Kind;
use crate::coords::{Observer, alt_az, apply, horizon, precession, unit};
use crate::ephem::Body;
use crate::sky::{Sky, limiting_magnitude, see, sun_altitude};
use crate::time::{HOUR, UnixMs, civil_date};

/// A moment as a local clock time, like "10:40 pm".
pub fn civil_time(at: UnixMs, offset_s: i32) -> String {
    let minutes = ((at / 60_000) + offset_s as i64 / 60).rem_euclid(24 * 60);
    let (h, m) = (minutes / 60, minutes % 60);
    let half = if h < 12 { "am" } else { "pm" };
    let h12 = if h % 12 == 0 { 12 } else { h % 12 };
    format!("{h12}:{m:02} {half}")
}

#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Body(Body),
    /// Index into the showpiece list.
    Showpiece(usize),
    /// A catalogue star, by HR number.
    Star(u16),
    /// Index into the shower list.
    Meteor(usize),
    /// A constellation found by its shape: index into the figures.
    Figure(usize),
    /// A star-hop, step by step: index into the hops.
    Hop(usize),
    /// Tonight's story, begun at its anchor star: index into the stories.
    Story(usize),
    /// A walk across the Moon's lit face.
    MoonWalk,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Find {
    pub id: String,
    pub target: Target,
    pub name: String,
    pub fact: String,
}

pub const MOST: usize = 16;

/// The night a moment belongs to: until noon, it is still last night.
pub fn night_of(at: UnixMs, offset_s: i32) -> (i32, u32, u32) {
    civil_date(at - 12 * HOUR, offset_s)
}

pub fn night_key(night: (i32, u32, u32)) -> String {
    format!("{:04}-{:02}-{:02}", night.0, night.1, night.2)
}

/// A stable pseudo-random number for a night and a name.
pub fn stable_hash(night: (i32, u32, u32), name: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let key = format!("{}-{}-{}:{}", night.0, night.1, night.2, name);
    for b in key.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

/// Groups digits in threes: 384400 becomes "384,400".
pub fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// A bright named star within a few degrees of the Moon tonight, if any.
fn moon_beside(sky: &Sky, at: UnixMs, ra: f64, dec: f64) -> Option<(String, f64)> {
    let prec = precession(at);
    sky.lists
        .stars
        .iter()
        .filter_map(|n| {
            let star = sky.stars.get(n.hr)?;
            if star.mag > 2.0 {
                return None;
            }
            let (sra, sdec) = crate::coords::angles(apply(&prec, star.dir));
            let gap = crate::coords::separation(ra, dec, sra, sdec);
            (gap < 6.0).then(|| (n.name.clone(), gap))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

/// A body's line for tonight, and on later visits one more thing about it.
fn body_line(
    sky: &Sky,
    body: Body,
    at: UnixMs,
    seen: &crate::sky::Seen,
    phase_name: &str,
    visits: usize,
) -> String {
    let tonight = body_fact(sky, body, at, seen, phase_name);
    let more = sky
        .lists
        .bodies
        .iter()
        .find(|b| b.id == body.id())
        .filter(|b| visits > 0 && !b.facts.is_empty())
        .map(|b| b.facts[(visits - 1) % b.facts.len()].clone());
    match more {
        Some(m) => format!("{tonight} {m}"),
        None => tonight,
    }
}

fn body_fact(
    sky: &Sky,
    body: Body,
    at: UnixMs,
    seen: &crate::sky::Seen,
    phase_name: &str,
) -> String {
    let p = &seen.position;
    if body == Body::Moon {
        let km = (p.distance * 6378.14 / 100.0).round() as u64 * 100;
        let beside = match moon_beside(sky, at, seen.ra, seen.dec) {
            Some((name, gap)) if gap < 1.5 => format!(" Tonight it's passing close by {name}."),
            Some((name, gap)) => format!(" Tonight it's {:.0} degrees from {name}.", gap),
            None => " It always turns the same face towards us.".to_owned(),
        };
        return format!("{phase_name}, {} km away tonight.{beside}", thousands(km));
    }
    let minutes = (p.distance * 8.3168).round() as u64;
    let light = if minutes < 120 {
        format!(
            "It's so far away that sunlight bouncing off it takes {minutes} minutes to get here: you're seeing it as it was {minutes} minutes ago."
        )
    } else {
        let hours = (minutes as f64 / 60.0).round();
        format!(
            "It's so far away that sunlight bouncing off it takes about {hours} hours to get here: you're seeing it as it was {hours} hours ago."
        )
    };
    let tonight = match body {
        Body::Mercury | Body::Venus => {
            let lit = (p.phase * 100.0).round();
            let shape = if lit < 35.0 {
                "a crescent"
            } else if lit < 65.0 {
                "half lit"
            } else if lit < 95.0 {
                "gibbous, like a Moon a few days from full"
            } else {
                "almost full"
            };
            format!(" Tonight it's {shape}: {lit}% of the side facing us is in sunlight.")
        }
        Body::Jupiter => format!(" {}", crate::jupiter::arrangement(at)),
        Body::Saturn => {
            let tilt = p.ring_tilt.abs();
            if tilt < 3.0 {
                format!(
                    " Its rings are almost edge-on to us tonight, tipped just {tilt:.1} degrees."
                )
            } else {
                format!(" Its rings are tipped {tilt:.0} degrees towards us tonight.")
            }
        }
        _ => String::new(),
    };
    format!("{light}{tonight}")
}

/// Tonight's set. `times_found` says how many earlier nights an id was
/// found on: unfound things are preferred, and things found before come
/// with something new to say.
pub fn tonight(
    sky: &Sky,
    observer: Observer,
    at: UnixMs,
    offset_s: i32,
    times_found: &dyn Fn(&str) -> usize,
) -> Vec<Find> {
    let night = night_of(at, offset_s);
    let sun_alt = sun_altitude(observer, at);
    let limit = limiting_magnitude(sun_alt);
    let mut out = Vec::new();

    let moon = see(Body::Moon, observer, at);
    if moon.alt > 3.0 {
        let age = crate::ephem::moon_age(at);
        out.push(Find {
            id: "moon".into(),
            target: Target::Body(Body::Moon),
            name: "The Moon".into(),
            fact: body_line(
                sky,
                Body::Moon,
                at,
                &moon,
                crate::ephem::moon_phase_name(age),
                times_found("moon"),
            ),
        });
    }
    for body in [
        Body::Mercury,
        Body::Venus,
        Body::Mars,
        Body::Jupiter,
        Body::Saturn,
    ] {
        let seen = see(body, observer, at);
        if seen.alt > 5.0 && seen.position.magnitude <= limit.min(3.0) {
            out.push(Find {
                id: body.id().into(),
                target: Target::Body(body),
                name: body.name().into(),
                fact: body_line(sky, body, at, &seen, "", times_found(body.id())),
            });
        }
    }

    let horizon = horizon(observer, at);
    let prec = precession(at);
    let alt_of = |ra: f64, dec: f64| alt_az(apply(&horizon, apply(&prec, unit(ra, dec)))).0;
    let order = |id: &str| (times_found(id) > 0, stable_hash(night, id));

    // Showpieces the eye can see, and on a dark night one fainter thing
    // from the longer list, to zoom in on.
    let candidates = |deep: bool| {
        let mut v: Vec<(usize, String)> = sky
            .lists
            .showpieces
            .iter()
            .enumerate()
            .filter(|(_, p)| p.deep == deep && p.kind != Kind::Dark)
            .filter(|(_, p)| {
                if deep {
                    limit >= 4.5 && p.mag <= limit + 3.0 && alt_of(p.ra, p.dec) > 20.0
                } else {
                    p.mag <= limit && alt_of(p.ra, p.dec) > 15.0
                }
            })
            .map(|(i, p)| (i, format!("showpiece:{}", p.id)))
            .collect();
        v.sort_by_key(|(_, id)| order(id));
        v
    };
    let room = if out.len() >= 4 { 2 } else { 3 };
    let deep = candidates(true).into_iter().next();
    let mut pieces: Vec<(usize, String)> = candidates(false)
        .into_iter()
        .take(room - usize::from(deep.is_some()))
        .collect();
    pieces.extend(deep);
    for (i, id) in pieces {
        let p = &sky.lists.showpieces[i];
        let fact = p.fact_for(times_found(&id));
        out.push(Find {
            id,
            target: Target::Showpiece(i),
            name: p.name.clone(),
            fact,
        });
    }

    let mut named: Vec<(u16, String)> = sky
        .lists
        .stars
        .iter()
        .filter(|s| s.fact.is_some() || s.light_years.is_some())
        .filter_map(|s| {
            let star = sky.stars.get(s.hr)?;
            let (alt, _) = alt_az(apply(&horizon, apply(&prec, star.dir)));
            (alt > 15.0 && (star.mag as f64) <= limit).then(|| (s.hr, format!("star:{}", s.hr)))
        })
        .collect();
    named.sort_by_key(|(_, id)| order(id));
    // Every third night or so the star the sky turns around takes the named
    // star's place, found before or not: Polaris, or the Southern Cross.
    let pole = if observer.lat >= 0.0 { 424 } else { 4730 };
    if stable_hash(night, "pole").is_multiple_of(3)
        && let Some(i) = named.iter().position(|(hr, _)| *hr == pole)
    {
        let p = named.remove(i);
        named.insert(0, p);
    }
    for (hr, id) in named.into_iter().take(2) {
        let s = sky.lists.star_name(hr).expect("listed star");
        let visits = times_found(&id);
        out.push(Find {
            id,
            target: Target::Star(hr),
            name: s.name.clone(),
            fact: s
                .describe_for(civil_date(at, offset_s).0, visits)
                .unwrap_or_default(),
        });
    }

    // Constellations to find by their shape: well up, whole, and small
    // enough to take in at a glance.
    if limit >= 2.5 {
        let mut shapes: Vec<(usize, String)> = sky
            .notes
            .iter()
            .filter_map(|note| {
                let i = sky.figures.iter().position(|f| f.abbrev == note.abbrev)?;
                let figure = &sky.figures[i];
                let (centre, reach) = figure.centre(&sky.stars)?;
                let centre_alt = alt_az(apply(&horizon, apply(&prec, centre))).0;
                let lowest = figure
                    .stars()
                    .iter()
                    .filter_map(|hr| sky.stars.get(*hr))
                    .map(|s| alt_az(apply(&horizon, apply(&prec, s.dir))).0)
                    .fold(90.0, f64::min);
                (centre_alt > 22.0 && lowest > 3.0 && reach < 30.0)
                    .then(|| (i, format!("figure:{}", figure.abbrev)))
            })
            .collect();
        shapes.sort_by_key(|(_, id)| order(id));
        for (i, id) in shapes.into_iter().take(3) {
            let figure = &sky.figures[i];
            let visits = times_found(&id);
            let fact = sky
                .notes
                .iter()
                .find(|n| n.abbrev == figure.abbrev)
                .map(|n| crate::catalogues::say_for(&n.fact, &n.more, visits))
                .unwrap_or_default();
            out.push(Find {
                id,
                target: Target::Figure(i),
                name: figure.name.clone(),
                fact,
            });
        }
    }

    let (_, month, day) = civil_date(at, offset_s);
    if sun_alt < -12.0
        && let Some((i, shower)) = sky
            .lists
            .showers
            .iter()
            .enumerate()
            .filter(|(_, s)| s.zhr >= 10 && s.active_on(month, day) && alt_of(s.ra, s.dec) > 0.0)
            .max_by_key(|(_, s)| s.zhr)
    {
        out.push(Find {
            id: format!("meteor:{}", shower.id),
            target: Target::Meteor(i),
            name: format!("A {} meteor", shower.name.trim_end_matches('s')),
            fact: format!(
                "A grain of dust burning up about a hundred kilometres overhead, arriving at {} km a second.",
                shower.speed
            ),
        });
    }

    let alt_of_star = |hr: u16| {
        sky.stars
            .get(hr)
            .map(|s| (alt_az(apply(&horizon, apply(&prec, s.dir))).0, s.mag as f64))
    };
    let alt_of_stop = |stop: &crate::tours::Stop| match (stop.hr, &stop.showpiece) {
        (Some(hr), _) => alt_of_star(hr),
        (None, Some(id)) => sky
            .lists
            .showpieces
            .iter()
            .find(|p| &p.id == id)
            .map(|p| (alt_of(p.ra, p.dec), p.mag)),
        _ => None,
    };

    // Tonight's story, told from a star that's well up.
    if limit >= 2.5 {
        let mut stories: Vec<(usize, String)> = sky
            .tours
            .stories
            .iter()
            .enumerate()
            .filter(|(_, s)| alt_of_star(s.anchor).is_some_and(|(alt, _)| alt > 20.0))
            .map(|(i, s)| (i, format!("story:{}", s.id)))
            .collect();
        stories.sort_by_key(|(_, id)| order(id));
        if let Some((i, id)) = stories.into_iter().next() {
            out.push(Find {
                id,
                target: Target::Story(i),
                name: sky.tours.stories[i].title.clone(),
                fact: sky.tours.stories[i].mirror.clone(),
            });
        }
    }

    // A star-hop whose every step is up and bright enough to see.
    if limit >= 3.0 {
        let north = observer.lat >= -10.0;
        let south = observer.lat <= 10.0;
        let mut hops: Vec<(usize, String)> = sky
            .tours
            .hops
            .iter()
            .enumerate()
            .filter(|(_, h)| match h.hemisphere.as_str() {
                "north" => north,
                "south" => south,
                _ => true,
            })
            .filter(|(_, h)| {
                h.steps.iter().all(|stop| {
                    alt_of_stop(stop).is_some_and(|(alt, mag)| alt > 12.0 && mag <= limit)
                })
            })
            .map(|(i, h)| (i, format!("hop:{}", h.id)))
            .collect();
        hops.sort_by_key(|(_, id)| order(id));
        if let Some((i, id)) = hops.into_iter().next() {
            let hop = &sky.tours.hops[i];
            out.push(Find {
                id,
                target: Target::Hop(i),
                name: hop.title.clone(),
                fact: hop.steps.last().map(|s| s.say.clone()).unwrap_or_default(),
            });
        }
    }

    // A walk on the Moon, when there's enough of it lit to walk on.
    let age = crate::ephem::moon_age(at) * crate::tours::SYNODIC_DAYS;
    if moon.alt > 10.0 && (2.5..27.0).contains(&age) {
        out.push(Find {
            id: "moon-walk".into(),
            target: Target::MoonWalk,
            name: "A walk on the Moon".into(),
            fact: sky.tours.moon_intro.clone(),
        });
    }

    // Algol, the Demon Star, in the middle of an eclipse.
    if let Some(middle) = crate::tours::algol_eclipse(at)
        && alt_of_star(936).is_some_and(|(alt, _)| alt > 15.0)
        && limit >= 3.5
    {
        let local = civil_time(middle, offset_s);
        out.retain(|f| f.id != "star:936");
        out.push(Find {
            id: "algol-dimming".into(),
            target: Target::Star(936),
            name: "Algol, dimming".into(),
            fact: format!(
                "Every two days and twenty-one hours, a dimmer star passes in front of Algol and it fades to a third of its brightness for a few hours. It's in eclipse tonight, dimmest at about {local}. Compare it with Mirfak nearby, and come back later to watch it recover."
            ),
        });
    }

    while out.len() > MOST {
        match out
            .iter()
            .rposition(|f| matches!(f.target, Target::Showpiece(_)))
            .or_else(|| {
                out.iter()
                    .rposition(|f| matches!(f.target, Target::Figure(_)))
            }) {
            Some(i) => {
                out.remove(i);
            }
            None => {
                out.truncate(MOST);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::midnight_utc;

    const LONDON: Observer = Observer {
        lat: 51.5072,
        lon: -0.1276,
    };

    #[test]
    fn a_dark_night_has_a_handful_of_finds_and_keeps_them() {
        let sky = Sky::bundled();
        let at = midnight_utc(2026, 12, 14) + 21 * HOUR;
        let never = |_: &str| 0;
        let finds = tonight(&sky, LONDON, at, 0, &never);
        assert!((8..=MOST).contains(&finds.len()), "{finds:#?}");
        assert!(
            finds.iter().any(|f| matches!(f.target, Target::Figure(_))),
            "{finds:#?}"
        );
        assert!(
            finds.iter().any(|f| f.id == "meteor:geminids"),
            "Geminids night: {finds:#?}"
        );
        let later = tonight(&sky, LONDON, at + 5 * 60_000, 0, &never);
        assert_eq!(
            finds.iter().map(|f| &f.id).collect::<Vec<_>>(),
            later.iter().map(|f| &f.id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn found_things_give_way_to_new_ones() {
        let sky = Sky::bundled();
        let at = midnight_utc(2026, 12, 14) + 21 * HOUR;
        let first = tonight(&sky, LONDON, at, 0, &|_| 0);
        let piece = first
            .iter()
            .find(|f| matches!(f.target, Target::Showpiece(_)))
            .expect("a showpiece")
            .id
            .clone();
        let again = tonight(&sky, LONDON, at, 0, &|id| usize::from(id == piece));
        assert!(again.iter().all(|f| f.id != piece), "{piece} came back");
    }

    #[test]
    fn midday_has_few_finds() {
        let sky = Sky::bundled();
        let at = midnight_utc(2026, 6, 21) + 12 * HOUR;
        let finds = tonight(&sky, LONDON, at, 0, &|_| 0);
        assert!(finds.len() <= 2, "{finds:#?}");
    }

    #[test]
    fn a_long_night_has_things_that_take_longer() {
        let sky = Sky::bundled();
        let at = midnight_utc(2026, 12, 14) + 21 * HOUR;
        let finds = tonight(&sky, LONDON, at, 0, &|_| 0);
        assert!(
            finds.iter().any(|f| matches!(f.target, Target::Story(_))),
            "{finds:#?}"
        );
        assert!(
            finds.iter().any(|f| matches!(f.target, Target::Hop(_))),
            "{finds:#?}"
        );
        // Heard stories give way to new ones.
        let story = finds
            .iter()
            .find(|f| matches!(f.target, Target::Story(_)))
            .unwrap()
            .id
            .clone();
        let again = tonight(&sky, LONDON, at, 0, &|id| usize::from(id == story));
        assert!(again.iter().all(|f| f.id != story));
    }

    #[test]
    fn clock_times_read_naturally() {
        assert_eq!(
            civil_time(midnight_utc(2026, 1, 1) + 22 * HOUR + 40 * 60_000, 0),
            "10:40 pm"
        );
        assert_eq!(civil_time(midnight_utc(2026, 1, 1), 3600), "1:00 am");
    }

    #[test]
    fn things_found_again_say_something_new() {
        let sky = Sky::bundled();
        let at = midnight_utc(2026, 12, 14) + 21 * HOUR;
        let first = tonight(&sky, LONDON, at, 0, &|_| 0);
        let again = tonight(&sky, LONDON, at, 0, &|_| 1);
        let body = |f: &[Find]| {
            f.iter()
                .find(|f| matches!(f.target, Target::Body(_)))
                .map(|f| f.fact.clone())
        };
        assert!(body(&first).is_some(), "{first:#?}");
        assert_ne!(body(&first), body(&again));
        assert!(
            first
                .iter()
                .any(|f| matches!(f.target, Target::Showpiece(p) if sky.lists.showpieces[p].deep)),
            "a dark night offers one fainter thing: {first:#?}"
        );
    }

    #[test]
    fn digits_group() {
        assert_eq!(thousands(384_400), "384,400");
        assert_eq!(thousands(999), "999");
    }
}
