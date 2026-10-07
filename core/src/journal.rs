//! Everything the app remembers, kept as plain files in one folder so the
//! user can read, copy or delete them: one Markdown page per night in
//! `logbook/`, and a small TOML file for each kind of record that repeats.

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Found {
    pub id: String,
    pub night: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Weight {
    pub id: u32,
    pub text: String,
    #[serde(default)]
    pub nights: Vec<String>,
    #[serde(default)]
    pub sorted: bool,
    /// How it sat when the sky looked back at it, later.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub looks: Vec<Check>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Mention {
    pub night: String,
    pub question: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Person {
    pub name: String,
    /// Stars given this person's name, by HR number.
    #[serde(default)]
    pub stars: Vec<u16>,
    #[serde(default)]
    pub mentions: Vec<Mention>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Plan {
    pub id: u32,
    pub what: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub who: Option<String>,
    /// YYYY-MM-DD.
    pub date: String,
    /// The sky event it hangs on, as it reads in a sentence.
    pub event: String,
    /// The night it was made.
    pub made: String,
    /// "went" or "didnt", once asked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Check {
    pub night: String,
    pub answer: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Course {
    pub id: u32,
    pub weight: u32,
    pub wish: String,
    pub outcome: String,
    pub obstacle: String,
    pub plan: String,
    pub made: String,
    /// Ask how it's going on or after this date, YYYY-MM-DD.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check_after: Option<String>,
    #[serde(default)]
    pub checks: Vec<Check>,
    /// True while it is still being written.
    #[serde(default)]
    pub draft: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Asked {
    pub question: String,
    pub night: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Drawing {
    pub name: String,
    pub edges: Vec<(u16, u16)>,
    pub night: String,
    /// The real constellation it turned out to be part of.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reveals: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Ask {
    #[default]
    Sometimes,
    Rarely,
    Never,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    #[serde(default)]
    pub ask: Ask,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lat: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lon: Option<f64>,
    /// No music.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub quiet: bool,
    /// How loud the music is, 0 to 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<f64>,
    /// Which style of music, by its folder's name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    /// Less movement: the wisp stays on its moss and the stars twinkle less.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub calm: bool,
    /// What the wisp has already shown, so it doesn't say it twice.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seen: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct NightWeight {
    pub weight: u32,
    pub ra: f64,
    pub dec: f64,
}

/// Where something found stood among the stars when it was found.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Spot {
    pub name: String,
    pub ra: f64,
    pub dec: f64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Answer {
    pub question: String,
    /// The words the sky asked, as asked.
    pub prompt: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub person: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub star: Option<u16>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Night {
    pub key: String,
    #[serde(default)]
    pub moon: String,
    /// When the first thing was found, for the page's chart of the sky.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<i64>,
    #[serde(default)]
    pub finds: Vec<String>,
    /// Where each find stood, for the same chart. Pages from before this
    /// was kept have none, and no chart.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spots: Vec<Spot>,
    #[serde(default)]
    pub weights: Vec<NightWeight>,
    #[serde(default)]
    pub answers: Vec<Answer>,
    #[serde(default)]
    pub drawings: Vec<String>,
    #[serde(default)]
    pub plans: Vec<u32>,
}

macro_rules! file_of {
    ($name:ident, $field:ident, $ty:ty) => {
        #[derive(Default, Serialize, Deserialize)]
        struct $name {
            #[serde(default)]
            $field: Vec<$ty>,
        }
    };
}
file_of!(FoundFile, found, Found);
file_of!(WeightFile, weight, Weight);
file_of!(PersonFile, person, Person);
file_of!(PlanFile, plan, Plan);
file_of!(CourseFile, course, Course);
file_of!(AskedFile, asked, Asked);
file_of!(DrawingFile, drawing, Drawing);

/// The record files, beside the logbook pages and the settings.
const KEPT: [&str; 7] = [
    "found.toml",
    "weights.toml",
    "people.toml",
    "plans.toml",
    "courses.toml",
    "asked.toml",
    "drawings.toml",
];

#[derive(Serialize, Deserialize)]
struct BackupFile {
    path: String,
    text: String,
}

#[derive(Serialize, Deserialize)]
struct Backup {
    /// Backups made before the name changed call it night_sky_backup.
    #[serde(alias = "night_sky_backup")]
    westering_backup: u32,
    made: String,
    #[serde(default)]
    file: Vec<BackupFile>,
}

pub struct Journal {
    dir: PathBuf,
    pub found: Vec<Found>,
    pub weights: Vec<Weight>,
    pub people: Vec<Person>,
    pub plans: Vec<Plan>,
    pub courses: Vec<Course>,
    pub asked: Vec<Asked>,
    pub drawings: Vec<Drawing>,
    pub settings: Settings,
}

fn read<T: DeserializeOwned + Default>(path: &Path) -> T {
    match fs::read_to_string(path) {
        Ok(text) => toml::from_str(&text).unwrap_or_else(|e| {
            eprintln!(
                "westering: {} could not be read ({e}); starting it afresh",
                path.display()
            );
            T::default()
        }),
        Err(_) => T::default(),
    }
}

/// Writes beside the file and renames over it, so a crash never leaves half a file.
fn write_atomic(path: &Path, text: &str) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, text)?;
    fs::rename(&tmp, path)
}

fn write<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    let text = toml::to_string(value).map_err(io::Error::other)?;
    write_atomic(path, &text)
}

impl Journal {
    pub fn open(dir: &Path) -> Journal {
        let at = |name: &str| dir.join(name);
        Journal {
            dir: dir.to_owned(),
            found: read::<FoundFile>(&at("found.toml")).found,
            weights: read::<WeightFile>(&at("weights.toml")).weight,
            people: read::<PersonFile>(&at("people.toml")).person,
            plans: read::<PlanFile>(&at("plans.toml")).plan,
            courses: read::<CourseFile>(&at("courses.toml")).course,
            asked: read::<AskedFile>(&at("asked.toml")).asked,
            drawings: read::<DrawingFile>(&at("drawings.toml")).drawing,
            settings: read(&at("settings.toml")),
        }
    }

    /// Takes in a logbook kept under an earlier name, the first time: when
    /// `to` holds nothing yet and `from` has a logbook, copies it across.
    /// Says whether it did.
    pub fn adopt(from: &Path, to: &Path) -> io::Result<bool> {
        let empty = fs::read_dir(to).map_or(true, |mut d| d.next().is_none());
        let there =
            from.join("logbook").is_dir() || KEPT.iter().any(|name| from.join(name).is_file());
        if !empty || !there {
            return Ok(false);
        }
        fn copy(from: &Path, to: &Path) -> io::Result<()> {
            fs::create_dir_all(to)?;
            for entry in fs::read_dir(from)? {
                let entry = entry?;
                let target = to.join(entry.file_name());
                if entry.file_type()?.is_dir() {
                    copy(&entry.path(), &target)?;
                } else {
                    fs::copy(entry.path(), target)?;
                }
            }
            Ok(())
        }
        copy(from, to)?;
        Ok(true)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn save_found(&self) -> io::Result<()> {
        write(
            &self.dir.join("found.toml"),
            &FoundFile {
                found: self.found.clone(),
            },
        )
    }
    pub fn save_weights(&self) -> io::Result<()> {
        write(
            &self.dir.join("weights.toml"),
            &WeightFile {
                weight: self.weights.clone(),
            },
        )
    }
    pub fn save_people(&self) -> io::Result<()> {
        write(
            &self.dir.join("people.toml"),
            &PersonFile {
                person: self.people.clone(),
            },
        )
    }
    pub fn save_plans(&self) -> io::Result<()> {
        write(
            &self.dir.join("plans.toml"),
            &PlanFile {
                plan: self.plans.clone(),
            },
        )
    }
    pub fn save_courses(&self) -> io::Result<()> {
        write(
            &self.dir.join("courses.toml"),
            &CourseFile {
                course: self.courses.clone(),
            },
        )
    }
    pub fn save_asked(&self) -> io::Result<()> {
        write(
            &self.dir.join("asked.toml"),
            &AskedFile {
                asked: self.asked.clone(),
            },
        )
    }
    pub fn save_drawings(&self) -> io::Result<()> {
        write(
            &self.dir.join("drawings.toml"),
            &DrawingFile {
                drawing: self.drawings.clone(),
            },
        )
    }
    pub fn save_settings(&self) -> io::Result<()> {
        write(&self.dir.join("settings.toml"), &self.settings)
    }

    pub fn found_before(&self, id: &str, night: &str) -> bool {
        self.found.iter().any(|f| f.id == id && f.night != night)
    }

    /// How many earlier nights something was found on.
    pub fn times_found_before(&self, id: &str, night: &str) -> usize {
        self.found
            .iter()
            .filter(|f| f.id == id && f.night != night)
            .count()
    }

    pub fn found_on(&self, id: &str, night: &str) -> bool {
        self.found.iter().any(|f| f.id == id && f.night == night)
    }

    pub fn mark_found(&mut self, id: &str, night: &str) -> io::Result<()> {
        if !self.found_on(id, night) {
            self.found.push(Found {
                id: id.to_owned(),
                night: night.to_owned(),
            });
            self.save_found()?;
        }
        Ok(())
    }

    fn night_path(&self, key: &str) -> PathBuf {
        self.dir.join("logbook").join(format!("{key}.md"))
    }

    /// Every night with a page, newest first.
    pub fn nights(&self) -> Vec<String> {
        let mut keys: Vec<String> = fs::read_dir(self.dir.join("logbook"))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().into_string().ok()?;
                let key = name.strip_suffix(".md")?;
                (key.len() == 10).then(|| key.to_owned())
            })
            .collect();
        keys.sort_unstable_by(|a, b| b.cmp(a));
        keys
    }

    pub fn night(&self, key: &str) -> Option<Night> {
        let text = fs::read_to_string(self.night_path(key)).ok()?;
        let front = text.strip_prefix("+++\n")?.split_once("\n+++")?.0;
        toml::from_str(front).ok()
    }

    pub fn save_night(&self, night: &Night) -> io::Result<()> {
        let front = toml::to_string(night).map_err(io::Error::other)?;
        let body = self.render(night);
        write_atomic(
            &self.night_path(&night.key),
            &format!("+++\n{front}+++\n\n{body}"),
        )
    }

    pub fn weight(&self, id: u32) -> Option<&Weight> {
        self.weights.iter().find(|w| w.id == id)
    }

    pub fn next_id(ids: impl Iterator<Item = u32>) -> u32 {
        ids.max().unwrap_or(0) + 1
    }

    /// Adds a weight, or brings back an earlier one with the same words.
    pub fn add_weight(&mut self, text: &str, night: &str) -> u32 {
        let text = text.trim();
        if let Some(w) = self
            .weights
            .iter_mut()
            .find(|w| w.text.to_lowercase() == text.to_lowercase())
        {
            if !w.nights.iter().any(|n| n == night) {
                w.nights.push(night.to_owned());
            }
            w.sorted = false;
            return w.id;
        }
        let id = Journal::next_id(self.weights.iter().map(|w| w.id));
        self.weights.push(Weight {
            id,
            text: text.to_owned(),
            nights: vec![night.to_owned()],
            sorted: false,
            looks: Vec::new(),
        });
        id
    }

    /// Brings an earlier weight back for tonight.
    pub fn bring_back(&mut self, id: u32, night: &str) {
        if let Some(w) = self.weights.iter_mut().find(|w| w.id == id)
            && !w.nights.iter().any(|n| n == night)
        {
            w.nights.push(night.to_owned());
        }
    }

    /// Weights from earlier nights still unsorted, most recent first.
    pub fn open_weights(&self, night: &str) -> Vec<&Weight> {
        let mut out: Vec<&Weight> = self
            .weights
            .iter()
            .filter(|w| !w.sorted && w.nights.iter().any(|n| n != night))
            .collect();
        out.sort_by(|a, b| b.nights.last().cmp(&a.nights.last()));
        out
    }

    pub fn person_mut(&mut self, name: &str) -> &mut Person {
        let key = name.trim().to_lowercase();
        if let Some(i) = self
            .people
            .iter()
            .position(|p| p.name.to_lowercase() == key)
        {
            return &mut self.people[i];
        }
        self.people.push(Person {
            name: name.trim().to_owned(),
            ..Person::default()
        });
        self.people.last_mut().expect("just pushed")
    }

    /// Names already used that start with `prefix` (two letters at least).
    pub fn names_like(&self, prefix: &str) -> Vec<&str> {
        let p = prefix.trim().to_lowercase();
        if p.chars().count() < 2 {
            return Vec::new();
        }
        self.people
            .iter()
            .filter(|x| x.name.to_lowercase().starts_with(&p))
            .map(|x| x.name.as_str())
            .collect()
    }

    /// People named on more than one night: the fixed stars.
    pub fn fixed_stars(&self) -> Vec<&Person> {
        let mut out: Vec<&Person> = self
            .people
            .iter()
            .filter(|p| {
                let mut nights: Vec<&str> = p.mentions.iter().map(|m| m.night.as_str()).collect();
                nights.sort_unstable();
                nights.dedup();
                nights.len() > 1
            })
            .collect();
        out.sort_by_key(|p| std::cmp::Reverse(p.mentions.len()));
        out
    }

    /// The person whose name is on a star, if any.
    pub fn person_on(&self, hr: u16) -> Option<&Person> {
        self.people.iter().find(|p| p.stars.contains(&hr))
    }

    /// Clears everything. The folder itself stays.
    pub fn forget_everything(&mut self) -> io::Result<()> {
        for name in KEPT {
            let _ = fs::remove_file(self.dir.join(name));
        }
        let _ = fs::remove_dir_all(self.dir.join("logbook"));
        let settings = self.settings.clone();
        *self = Journal::open(&self.dir);
        self.settings = settings;
        Ok(())
    }

    /// Rewrites every night's page through `change`, saving the ones it
    /// changed.
    fn each_night(&self, mut change: impl FnMut(&mut Night) -> bool) -> io::Result<()> {
        for key in self.nights() {
            if let Some(mut night) = self.night(&key)
                && change(&mut night)
            {
                self.save_night(&night)?;
            }
        }
        Ok(())
    }

    /// Removes a night's page, and everything that belongs only to that
    /// night: what was found, the questions asked, the shapes drawn. A
    /// weight or a name that also came up on another night stays.
    pub fn delete_night(&mut self, key: &str) -> io::Result<()> {
        let _ = fs::remove_file(self.night_path(key));
        self.found.retain(|f| f.night != key);
        self.asked.retain(|a| a.night != key);
        self.drawings.retain(|d| d.night != key);
        for w in &mut self.weights {
            w.nights.retain(|n| n != key);
            w.looks.retain(|k| k.night != key);
        }
        let gone: Vec<u32> = self
            .weights
            .iter()
            .filter(|w| w.nights.is_empty())
            .map(|w| w.id)
            .collect();
        self.weights.retain(|w| !w.nights.is_empty());
        self.courses.retain(|c| !gone.contains(&c.weight));
        for p in &mut self.people {
            p.mentions.retain(|m| m.night != key);
        }
        self.people
            .retain(|p| !p.mentions.is_empty() || !p.stars.is_empty());
        self.save_all()
    }

    /// Removes a weight from every night it was set down on, with any
    /// course charted for it.
    pub fn delete_weight(&mut self, id: u32) -> io::Result<()> {
        self.weights.retain(|w| w.id != id);
        self.courses.retain(|c| c.weight != id);
        self.each_night(|n| {
            let before = n.weights.len();
            n.weights.retain(|w| w.weight != id);
            n.weights.len() != before
        })?;
        self.save_all()
    }

    /// Forgets a name: the person, their star, and the answers that named them.
    pub fn delete_person(&mut self, name: &str) -> io::Result<()> {
        let same = |n: &str| n.eq_ignore_ascii_case(name);
        self.people.retain(|p| !same(&p.name));
        for plan in &mut self.plans {
            if plan.who.as_deref().is_some_and(same) {
                plan.who = None;
            }
        }
        self.each_night(|n| {
            let before = n.answers.len();
            n.answers.retain(|a| !a.person.as_deref().is_some_and(same));
            n.answers.len() != before
        })?;
        self.save_all()
    }

    /// Removes one answer from a night's page; a name it gave stays only if
    /// it came up somewhere else too.
    pub fn delete_answer(&mut self, night: &str, question: &str) -> io::Result<()> {
        if let Some(mut page) = self.night(night) {
            page.answers.retain(|a| a.question != question);
            self.save_night(&page)?;
        }
        for p in &mut self.people {
            p.mentions
                .retain(|m| !(m.night == night && m.question == question));
        }
        self.people
            .retain(|p| !p.mentions.is_empty() || !p.stars.is_empty());
        self.save_all()
    }

    pub fn delete_plan(&mut self, id: u32) -> io::Result<()> {
        self.plans.retain(|p| p.id != id);
        self.each_night(|n| {
            let before = n.plans.len();
            n.plans.retain(|p| *p != id);
            n.plans.len() != before
        })?;
        self.save_all()
    }

    pub fn delete_course(&mut self, id: u32) -> io::Result<()> {
        self.courses.retain(|c| c.id != id);
        self.save_all()
    }

    /// Removes a shape drawn on a night, from the sky and from its page.
    pub fn delete_drawing(&mut self, name: &str, night: &str) -> io::Result<()> {
        self.drawings
            .retain(|d| !(d.name == name && d.night == night));
        if let Some(mut page) = self.night(night) {
            page.drawings.retain(|d| !d.starts_with(name));
            self.save_night(&page)?;
        }
        self.save_all()
    }

    fn save_all(&self) -> io::Result<()> {
        self.save_found()?;
        self.save_weights()?;
        self.save_people()?;
        self.save_plans()?;
        self.save_courses()?;
        self.save_asked()?;
        self.save_drawings()
    }

    /// Everything kept, as one plain-text file to put somewhere safe.
    pub fn backup(&self, made: &str) -> String {
        let mut files = Vec::new();
        for name in KEPT.iter().chain(["settings.toml"].iter()) {
            if let Ok(text) = fs::read_to_string(self.dir.join(name)) {
                files.push(BackupFile {
                    path: (*name).to_owned(),
                    text,
                });
            }
        }
        for key in self.nights() {
            if let Ok(text) = fs::read_to_string(self.night_path(&key)) {
                files.push(BackupFile {
                    path: format!("logbook/{key}.md"),
                    text,
                });
            }
        }
        let backup = Backup {
            westering_backup: 1,
            made: made.to_owned(),
            file: files,
        };
        format!(
            "# A copy of everything Westering keeps. Restore it from the menu.\n\n{}",
            toml::to_string(&backup).unwrap_or_default()
        )
    }

    /// When a backup was made, if the text is one.
    pub fn backup_date(text: &str) -> Option<String> {
        toml::from_str::<Backup>(text).ok().map(|b| b.made)
    }

    /// Replaces everything kept with a backup's contents. Nothing is touched
    /// unless the whole backup reads cleanly.
    pub fn restore(&mut self, text: &str) -> io::Result<usize> {
        let backup: Backup = toml::from_str(text).map_err(io::Error::other)?;
        let safe = |path: &str| {
            KEPT.contains(&path)
                || path == "settings.toml"
                || path
                    .strip_prefix("logbook/")
                    .and_then(|p| p.strip_suffix(".md"))
                    .is_some_and(|k| {
                        k.len() == 10 && k.chars().all(|c| c.is_ascii_digit() || c == '-')
                    })
        };
        if let Some(bad) = backup.file.iter().find(|f| !safe(&f.path)) {
            return Err(io::Error::other(format!(
                "it holds a file it shouldn't: {}",
                bad.path
            )));
        }
        for name in KEPT.iter().chain(["settings.toml"].iter()) {
            let _ = fs::remove_file(self.dir.join(name));
        }
        let _ = fs::remove_dir_all(self.dir.join("logbook"));
        for f in &backup.file {
            write_atomic(&self.dir.join(&f.path), &f.text)?;
        }
        *self = Journal::open(&self.dir);
        Ok(backup.file.len())
    }

    /// The human half of a night's page.
    fn render(&self, night: &Night) -> String {
        let mut out = format!("# {}\n\n", long_date(&night.key));
        if !night.moon.is_empty() {
            out.push_str(&format!("{}.\n\n", night.moon));
        }
        if !night.finds.is_empty() {
            out.push_str("## Found\n\n");
            for f in &night.finds {
                out.push_str(&format!("- {f}\n"));
            }
            out.push('\n');
        }
        if !night.weights.is_empty() {
            out.push_str("## Weights\n\n");
            for w in &night.weights {
                if let Some(weight) = self.weight(w.weight) {
                    out.push_str(&format!("- {}\n", weight.text));
                }
            }
            out.push('\n');
        }
        if !night.answers.is_empty() {
            out.push_str("## The sky asked\n\n");
            for a in &night.answers {
                out.push_str(&format!("> {}\n\n{}\n\n", a.prompt, a.text));
            }
        }
        if !night.drawings.is_empty() {
            out.push_str("## Drawn\n\n");
            for d in &night.drawings {
                out.push_str(&format!("- {d}\n"));
            }
            out.push('\n');
        }
        let plans: Vec<&Plan> = self
            .plans
            .iter()
            .filter(|p| night.plans.contains(&p.id))
            .collect();
        if !plans.is_empty() {
            out.push_str("## Coming up\n\n");
            for p in plans {
                let who = p
                    .who
                    .as_deref()
                    .map(|w| format!(", with {w}"))
                    .unwrap_or_default();
                out.push_str(&format!(
                    "- {} ({}, {}{who})\n",
                    p.what,
                    p.event,
                    long_date(&p.date)
                ));
            }
            out.push('\n');
        }
        out
    }
}

/// "Monday 28 September 2026" for "2026-09-28".
pub fn long_date(key: &str) -> String {
    let mut parts = key.split('-').map(|p| p.parse::<i64>().unwrap_or(1));
    let (y, m, d) = (
        parts.next().unwrap_or(2000),
        parts.next().unwrap_or(1),
        parts.next().unwrap_or(1),
    );
    let at = crate::time::midnight_utc(y as i32, m as u32, d as u32) + 12 * crate::time::HOUR;
    format!(
        "{} {} {} {}",
        crate::time::weekday(at, 0),
        d,
        crate::time::MONTHS[(m as usize).clamp(1, 12) - 1],
        y
    )
}

/// "Monday 28 September" for "2026-09-28".
pub fn short_date(key: &str) -> String {
    let long = long_date(key);
    long.rsplit_once(' ')
        .map(|(a, _)| a.to_owned())
        .unwrap_or(long)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("westering-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_night_round_trips_through_its_page() {
        let dir = scratch("night");
        let mut j = Journal::open(&dir);
        let w = j.add_weight("The boiler", "2026-09-28");
        j.save_weights().unwrap();
        let night = Night {
            key: "2026-09-28".into(),
            moon: "Waxing gibbous".into(),
            at: Some(1_790_634_000_000),
            finds: vec!["Jupiter".into()],
            spots: vec![Spot {
                name: "Jupiter".into(),
                ra: 131.5,
                dec: 18.2,
            }],
            weights: vec![NightWeight {
                weight: w,
                ra: 250.0,
                dec: -10.0,
            }],
            answers: vec![Answer {
                question: "polaris".into(),
                prompt: "Who would pick up?".into(),
                text: "Sam".into(),
                person: Some("Sam".into()),
                star: Some(424),
            }],
            ..Night::default()
        };
        j.save_night(&night).unwrap();
        let again = Journal::open(&dir);
        assert_eq!(again.night("2026-09-28"), Some(night));
        assert_eq!(again.nights(), vec!["2026-09-28".to_owned()]);
        let page = fs::read_to_string(dir.join("logbook/2026-09-28.md")).unwrap();
        assert!(page.contains("# Monday 28 September 2026"));
        assert!(page.contains("- The boiler"));
        // A page written before finds' places were kept still opens.
        let old = "+++\nkey = \"2026-09-20\"\nmoon = \"Full Moon\"\nfinds = [\"Saturn\"]\n+++\n\n# A night\n";
        fs::write(dir.join("logbook/2026-09-20.md"), old).unwrap();
        let before = again.night("2026-09-20").expect("the older page");
        assert_eq!(before.finds, vec!["Saturn".to_owned()]);
        assert!(before.at.is_none() && before.spots.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_weight_brought_back_is_the_same_record() {
        let dir = scratch("weights");
        let mut j = Journal::open(&dir);
        let a = j.add_weight("Work", "2026-09-27");
        let b = j.add_weight("work ", "2026-09-28");
        assert_eq!(a, b);
        assert_eq!(j.weights[0].nights.len(), 2);
        assert_eq!(j.open_weights("2026-09-29").len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn people_named_on_two_nights_are_fixed_stars() {
        let dir = scratch("people");
        let mut j = Journal::open(&dir);
        for night in ["2026-09-01", "2026-09-05"] {
            j.person_mut("Sam").mentions.push(Mention {
                night: night.into(),
                question: "q".into(),
            });
        }
        j.person_mut("Alex").mentions.push(Mention {
            night: "2026-09-01".into(),
            question: "q".into(),
        });
        let fixed: Vec<&str> = j.fixed_stars().iter().map(|p| p.name.as_str()).collect();
        assert_eq!(fixed, ["Sam"]);
        assert_eq!(j.names_like("sa"), ["Sam"]);
        assert!(j.names_like("s").is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    fn two_nights(dir: &Path) -> Journal {
        let mut j = Journal::open(dir);
        let w = j.add_weight("The boiler", "2026-09-20");
        j.add_weight("The boiler", "2026-09-21");
        let only = j.add_weight("Tax", "2026-09-21");
        for (key, weights) in [("2026-09-20", vec![w]), ("2026-09-21", vec![w, only])] {
            j.mark_found("moon", key).unwrap();
            j.person_mut("Sam").mentions.push(Mention {
                night: key.into(),
                question: "q".into(),
            });
            j.save_night(&Night {
                key: key.into(),
                weights: weights
                    .into_iter()
                    .map(|weight| NightWeight {
                        weight,
                        ra: 0.0,
                        dec: 0.0,
                    })
                    .collect(),
                answers: vec![Answer {
                    question: "q".into(),
                    prompt: "Who?".into(),
                    text: "Sam".into(),
                    person: Some("Sam".into()),
                    star: None,
                }],
                ..Night::default()
            })
            .unwrap();
        }
        j.courses.push(Course {
            id: 1,
            weight: only,
            ..Course::default()
        });
        j.save_all().unwrap();
        j
    }

    #[test]
    fn deleting_a_night_keeps_what_other_nights_share() {
        let dir = scratch("delete-night");
        let mut j = two_nights(&dir);
        j.delete_night("2026-09-21").unwrap();
        let j = Journal::open(&dir);
        assert_eq!(j.nights(), ["2026-09-20"]);
        assert_eq!(j.weights.len(), 1, "the boiler stays, tax goes");
        assert!(j.courses.is_empty(), "tax's course goes with it");
        assert_eq!(j.people[0].mentions.len(), 1);
        assert_eq!(j.found.len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn deleting_a_weight_or_a_name_reaches_every_page() {
        let dir = scratch("delete-things");
        let mut j = two_nights(&dir);
        let boiler = j.weights[0].id;
        j.delete_weight(boiler).unwrap();
        j.delete_answer("2026-09-20", "q").unwrap();
        assert!(j.night("2026-09-20").unwrap().answers.is_empty());
        assert_eq!(
            j.people[0].mentions.len(),
            1,
            "Sam still came up on the 21st"
        );
        j.delete_person("sam").unwrap();
        let j = Journal::open(&dir);
        assert!(j.weights.iter().all(|w| w.id != boiler));
        assert!(j.people.is_empty());
        for key in j.nights() {
            let n = j.night(&key).unwrap();
            assert!(n.weights.iter().all(|w| w.weight != boiler));
            assert!(n.answers.is_empty());
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_backup_restores_exactly_and_refuses_strange_paths() {
        let dir = scratch("backup");
        let j = two_nights(&dir);
        let text = j.backup("2026-09-28");
        assert_eq!(Journal::backup_date(&text).as_deref(), Some("2026-09-28"));
        let other = scratch("restore");
        let mut fresh = Journal::open(&other);
        fresh.add_weight("Something else", "2026-09-27");
        fresh.save_weights().unwrap();
        fresh.restore(&text).unwrap();
        assert_eq!(fresh.nights(), j.nights());
        assert_eq!(fresh.weights, j.weights);
        assert_eq!(fresh.people, j.people);
        let evil = text.replace("path = \"weights.toml\"", "path = \"../evil.toml\"");
        assert!(fresh.restore(&evil).is_err());
        assert_eq!(fresh.weights, j.weights, "a bad backup changes nothing");
        assert!(fresh.restore("not a backup").is_err());
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&other);
    }

    #[test]
    fn a_logbook_kept_under_the_old_name_comes_across_once() {
        let old = scratch("adopt-old");
        let new = scratch("adopt-new");
        let j = two_nights(&old);
        assert!(Journal::adopt(&old, &new).unwrap());
        let moved = Journal::open(&new);
        assert_eq!(moved.nights(), j.nights());
        assert_eq!(moved.weights, j.weights);
        // Once there's something here, it's left alone.
        assert!(!Journal::adopt(&old, &new).unwrap());
        let nowhere = scratch("adopt-none");
        assert!(!Journal::adopt(&nowhere, &scratch("adopt-empty")).unwrap());
        let old_backup = j
            .backup("2026-09-28")
            .replace("westering_backup", "night_sky_backup");
        assert!(Journal::backup_date(&old_backup).is_some());
        for d in [old, new] {
            let _ = fs::remove_dir_all(&d);
        }
    }
}
