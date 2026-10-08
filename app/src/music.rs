//! Quiet music under the sky, in a few styles. Each style is a folder of
//! slow tracks with a `pack.toml` naming it; the tracks play through in a
//! different order each night, fading in with the sky and out with it.
//! Changing style fades the track playing out before the next begins.
//! Winding down, only the calmest few tracks play: anything livelier fades
//! out and one of those takes over.

use gtk::prelude::*;
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Where the styles are: beside the installed app, or in the source tree.
fn folder() -> Option<PathBuf> {
    let mut places = vec![PathBuf::from("/app/share/westering/music")];
    if let Ok(exe) = std::env::current_exe()
        && let Some(prefix) = exe.parent().and_then(|p| p.parent())
    {
        places.push(prefix.join("share/westering/music"));
    }
    places.push(PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/data/music"
    )));
    places.into_iter().find(|p| p.is_dir())
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct PackFile {
    name: String,
    #[serde(default)]
    order: i64,
    /// The tracks calm enough to wind down to, by file name without its
    /// ending.
    #[serde(default)]
    wind_down: Vec<String>,
}

/// One style of music.
pub struct Pack {
    /// The folder's name, kept in the settings.
    pub id: String,
    pub name: String,
    order: i64,
    tracks: Vec<PathBuf>,
    /// Which of the tracks are calm enough to wind down to.
    calm: Vec<usize>,
}

fn load(dir: &Path, seed: u64) -> Option<Pack> {
    let about: PackFile =
        toml::from_str(&std::fs::read_to_string(dir.join("pack.toml")).ok()?).ok()?;
    let mut tracks: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "opus" || e == "ogg"))
        .collect();
    if tracks.is_empty() {
        return None;
    }
    tracks.sort();
    let turn = (seed % tracks.len() as u64) as usize;
    tracks.rotate_left(turn);
    let calm = (0..tracks.len())
        .filter(|&i| {
            let stem = tracks[i].file_stem().and_then(|s| s.to_str());
            stem.is_some_and(|s| about.wind_down.iter().any(|w| w == s))
        })
        .collect();
    Some(Pack {
        id: dir.file_name()?.to_string_lossy().into_owned(),
        name: about.name,
        order: about.order,
        tracks,
        calm,
    })
}

pub struct Music {
    packs: Vec<Pack>,
    /// The style playing, and the one asked for when that's different.
    pack: usize,
    wanted: usize,
    next: usize,
    stream: Option<gtk::MediaFile>,
    volume: f64,
    pub quiet: bool,
    /// The track playing: its style, and which of that style's tracks.
    current: Option<(usize, usize)>,
    /// Winding down: only the calmest tracks play.
    settling: bool,
    /// The track playing is too lively to wind down to, and is on its way
    /// out.
    giving_way: bool,
    calm_next: usize,
    seed: u64,
}

/// How loud the music sits at its fullest. The tracks are levelled gently
/// (-20 LUFS), so this can be the whole of it.
pub const FULL: f64 = 1.0;

impl Music {
    /// Every style, each in an order that depends on `seed`, so each night
    /// differs; `style` is the one to start with.
    pub fn new(seed: u64, quiet: bool, style: Option<&str>) -> Music {
        let mut packs: Vec<Pack> = folder()
            .and_then(|dir| std::fs::read_dir(dir).ok())
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_dir())
            .filter_map(|e| load(&e.path(), seed))
            .collect();
        packs.sort_by(|a, b| a.order.cmp(&b.order).then(a.name.cmp(&b.name)));
        let pack = style
            .and_then(|s| packs.iter().position(|p| p.id == s))
            .unwrap_or(0);
        Music {
            packs,
            pack,
            wanted: pack,
            next: 0,
            stream: None,
            volume: 0.0,
            quiet,
            current: None,
            settling: false,
            giving_way: false,
            calm_next: 0,
            seed,
        }
    }

    /// The styles, by id and name, in their order.
    pub fn styles(&self) -> Vec<(String, String)> {
        self.packs
            .iter()
            .map(|p| (p.id.clone(), p.name.clone()))
            .collect()
    }

    /// Asks for a style by its id; it takes over once the track playing
    /// has faded.
    pub fn want(&mut self, style: Option<&str>) {
        if let Some(i) = style.and_then(|s| self.packs.iter().position(|p| p.id == s)) {
            self.wanted = i;
        }
    }

    /// The style and title of the track playing: "02-moon-unit" is "Moon
    /// unit".
    pub fn playing(&self) -> Option<(String, String)> {
        if self.quiet || self.volume < 0.01 {
            return None;
        }
        let (pack, track) = self.current?;
        let pack = self.packs.get(pack)?;
        let path = pack.tracks.get(track)?;
        let stem = path.file_stem()?.to_str()?;
        let words = stem.trim_start_matches(|c: char| c.is_ascii_digit() || c == '-');
        let mut title = words.replace('-', " ");
        if let Some(first) = title.get(..1) {
            title = first.to_uppercase() + &title[1..];
        }
        Some((pack.name.clone(), title))
    }

    /// The tracks calm enough to wind down to: the style playing's own
    /// first, so the change is as small as it can be, then the others'.
    fn calm_tracks(&self) -> Vec<(usize, usize)> {
        if self.packs.is_empty() {
            return Vec::new();
        }
        let of = |p: usize| self.packs[p].calm.iter().map(move |&t| (p, t));
        let mut own: Vec<(usize, usize)> = of(self.pack).collect();
        let mut others: Vec<(usize, usize)> = (0..self.packs.len())
            .filter(|&p| p != self.pack)
            .flat_map(of)
            .collect();
        // A different one to begin with each night.
        for list in [&mut own, &mut others] {
            if !list.is_empty() {
                let turn = (self.seed % list.len() as u64) as usize;
                list.rotate_left(turn);
            }
        }
        own.extend(others);
        own
    }

    /// Winding down or not. As it begins, a track that isn't one of the
    /// calmest fades out and one of those follows; a calm one plays on.
    pub fn settle(&mut self, on: bool) {
        if on == self.settling {
            return;
        }
        self.settling = on;
        let calm = self.calm_tracks();
        self.giving_way = on
            && !calm.is_empty()
            && self.stream.is_some()
            && self.current.is_some_and(|c| !calm.contains(&c));
    }

    fn start_next(&mut self) {
        let calm = if self.settling {
            self.calm_tracks()
        } else {
            Vec::new()
        };
        let (pack, track) = if calm.is_empty() {
            let Some(pack) = self.packs.get(self.pack) else {
                return;
            };
            let track = self.next % pack.tracks.len();
            self.next += 1;
            (self.pack, track)
        } else {
            let pick = calm[self.calm_next % calm.len()];
            self.calm_next += 1;
            pick
        };
        let stream = gtk::MediaFile::for_filename(&self.packs[pack].tracks[track]);
        stream.set_volume(self.volume);
        stream.play();
        self.stream = Some(stream);
        self.current = Some((pack, track));
    }

    /// Eases the volume towards `target` (0 to 1 of full) and moves on to the
    /// next track when one ends. Call it every frame.
    pub fn tick(&mut self, target: f64, dt: f64) {
        let switching = self.wanted != self.pack;
        let leaving = switching || self.giving_way;
        let target = if self.quiet {
            0.0
        } else if leaving && self.stream.is_some() {
            // Fade the old one out first: a little quicker for a change
            // of style, gently for winding down.
            0.0
        } else {
            target.clamp(0.0, 1.0) * FULL
        };
        let ease = if switching {
            0.6
        } else if self.giving_way {
            1.2
        } else {
            1.6
        };
        let k = 1.0 - (-dt / ease).exp();
        self.volume += (target - self.volume) * k;
        if leaving && (self.volume < 0.01 || self.stream.is_none()) {
            if let Some(s) = self.stream.take() {
                s.pause();
            }
            if switching {
                self.pack = self.wanted;
                self.next = 0;
            }
            self.giving_way = false;
            self.volume = 0.0;
        }
        if self.volume < 0.002 && target == 0.0 {
            if let Some(s) = &self.stream
                && s.is_playing()
            {
                s.pause();
            }
            return;
        }
        match &self.stream {
            Some(s) if s.is_ended() || s.error().is_some() => {
                if let Some(e) = s.error() {
                    eprintln!("westering: a track wouldn't play: {e}");
                }
                self.start_next()
            }
            Some(s) => {
                if !s.is_playing() {
                    s.play();
                }
                s.set_volume(self.volume);
            }
            None => self.start_next(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_tracks_are_calm_enough_to_wind_down_to() {
        let music = Music::new(3, true, None);
        let calm = music.calm_tracks();
        assert_eq!(calm.len(), 5, "{calm:?}");
        // Every name in a pack's list is a track that's really there.
        let named: usize = ["ambient", "acoustic", "electro", "lofi"]
            .iter()
            .filter_map(|id| {
                let dir = folder()?.join(id);
                let text = std::fs::read_to_string(dir.join("pack.toml")).ok()?;
                let about: PackFile = toml::from_str(&text).ok()?;
                Some(about.wind_down.len())
            })
            .sum();
        assert_eq!(named, 5);
    }

    #[test]
    fn the_style_playing_gives_its_own_calm_tracks_first() {
        for seed in 0..7 {
            let music = Music::new(seed, true, Some("acoustic"));
            let calm = music.calm_tracks();
            let own = music.packs[music.pack].calm.len();
            assert_eq!(own, 2);
            assert!(calm[..own].iter().all(|c| c.0 == music.pack), "{calm:?}");
            assert!(calm[own..].iter().all(|c| c.0 != music.pack), "{calm:?}");
        }
        // A style with none of its own still winds down to one of the five.
        let music = Music::new(1, true, Some("lofi"));
        assert!(music.packs[music.pack].calm.is_empty());
        assert_eq!(music.calm_tracks().len(), 5);
    }

    #[test]
    fn winding_down_only_sends_a_livelier_track_away() {
        let mut music = Music::new(1, true, Some("lofi"));
        // Nothing playing yet: nothing to fade, the next track is a calm one.
        music.settle(true);
        assert!(!music.giving_way);
        music.settle(false);
        // A calm track playing is left to play on.
        let calm = music.calm_tracks();
        music.current = Some(calm[0]);
        music.settle(true);
        assert!(!music.giving_way);
    }
}
