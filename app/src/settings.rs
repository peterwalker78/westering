//! The menu, in a few short pages: how the sky behaves, what Westering is
//! for, what's kept and why, the logbook, and credits.

use crate::game::Game;
use crate::pages::{Go, Request};
use crate::ui::{clear, confirm, label};
use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use westering_core::journal::{Ask, Journal, long_date};

pub struct Settings {
    pub root: gtk::Box,
    list: gtk::ListBox,
    body: gtk::Box,
    game: Rc<RefCell<Game>>,
    me: RefCell<Weak<Settings>>,
    pub on_request: Go,
}

/// The menu's pages, as the sidebar lists them.
const PAGES: [&str; 5] = [
    "Settings",
    "What it's for",
    "What's kept",
    "Your logbook",
    "Credits",
];

/// What Westering is for: a line, then the four things underneath it.
const LEAD: &str =
    "A few quiet minutes at the end of the day, to wind down and come back to what matters to you.";

const PILLARS: [(&str, &str); 4] = [
    (
        "Wind down",
        "The sky slows and dims as you go, and ends by sending you outside to look, or off to bed.",
    ),
    (
        "The people who are there",
        "Now and then a small question brings someone to mind. A name can have a star of its own.",
    ),
    (
        "Something to look forward to",
        "Plans hung on real nights in the sky: a meteor shower, a full Moon, a planet at its best.",
    ),
    (
        "Setting it down",
        "Write down what's weighing on you. It becomes a star low in the west, and at the end of the visit you watch it set.",
    ),
];

const WHY_THE_SKY: &str = "The stars are the way in. Every story ends on a thought turned back to everyday life, and the sky never repeats: the Moon moves on, planets wander, and over the year the seasons turn the whole sky round.";

const OVER_TIME: &str = "Over weeks, the logbook becomes a quiet record of your evenings: what you saw, what was on your mind and how it turned out, and the people and plans that keep coming up.";

const HOW_IT_WENT: &str = "Say how your evenings with Westering went, good or bad. It opens a short form on GitHub in your browser, where you'll need an account.";

/// Opens in the browser through the portal, so Westering itself still has no
/// network.
const HOW_IT_WENT_FORM: &str =
    "https://github.com/peterwalker78/westering/discussions/new?category=how-did-it-feel";

const CREDITS: [&str; 9] = [
    "Westering is free software under the GNU GPL, version 3 or later.",
    "Stars from the Yale Bright Star Catalogue (Hoffleit and Warren), through the CDS in Strasbourg.",
    "Constellation figures from Stellarium's modern sky culture (CC BY-SA 4.0); names from d3-celestial by Olaf Frohn (BSD licence).",
    "Positions of the Sun, Moon and planets after Paul Schlyter's method.",
    "Meteor showers from the International Meteor Organization's working list.",
    "Deep-sky positions from SIMBAD (CDS, Strasbourg), and facts checked against Wikipedia.",
    "Names on the Moon from the IAU Gazetteer of Planetary Nomenclature.",
    "Jupiter's moons after Jean Meeus; Algol's eclipses after Kreiner.",
    "Music, all dedicated to the public domain (CC0): \"Ease into Night\", \"Moon Unit\", \"Into The Mist\" and \"Calm Currents\" by HoliznaCC0; \"Chill lofi inspired\" and \"Lofi Hip Hop Loop\" by omfgdude.",
];

/// The way to say how the evenings went, beside the Settings title.
fn how_it_went() -> gtk::Button {
    let open = gtk::Button::with_label("How did it feel?");
    open.add_css_class("quiet");
    open.set_valign(gtk::Align::Center);
    open.set_tooltip_text(Some(HOW_IT_WENT));
    open.connect_clicked(|b| {
        let window = b.root().and_downcast::<gtk::Window>();
        gtk::UriLauncher::new(HOW_IT_WENT_FORM).launch(
            window.as_ref(),
            gio::Cancellable::NONE,
            |_| {},
        );
    });
    open
}

/// A settings row: what it is and a line about it on the left, the control
/// on the right.
fn row(title: &str, note: &str, control: &impl IsA<gtk::Widget>) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    row.add_css_class("menu-row");
    let words = gtk::Box::new(gtk::Orientation::Vertical, 2);
    words.set_hexpand(true);
    words.append(&label(title, "book-body"));
    if !note.is_empty() {
        let n = label(note, "book-quiet");
        n.set_max_width_chars(46);
        words.append(&n);
    }
    control.set_valign(gtk::Align::Center);
    row.append(&words);
    row.append(control);
    row
}

/// A panel of rows, with a small heading above it.
fn group(body: &gtk::Box, heading: &str, rows: &[gtk::Box]) {
    body.append(&label(heading, "book-heading"));
    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    card.add_css_class("menu-card");
    for r in rows {
        card.append(r);
    }
    body.append(&card);
}

impl Settings {
    pub fn new(game: &Rc<RefCell<Game>>) -> Rc<Settings> {
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        root.add_css_class("page");
        root.add_css_class("settings");

        let side = gtk::Box::new(gtk::Orientation::Vertical, 0);
        side.add_css_class("book-side");
        side.set_width_request(220);
        let back = gtk::Button::with_label("‹  Back to the sky");
        back.add_css_class("quiet");
        back.set_halign(gtk::Align::Start);
        back.set_margin_top(18);
        back.set_margin_start(14);
        back.set_margin_bottom(10);
        back.set_tooltip_text(Some("Esc"));
        let list = gtk::ListBox::new();
        list.add_css_class("book-side");
        list.set_vexpand(true);
        for page in PAGES {
            let l = gtk::Label::new(Some(page));
            l.set_xalign(0.0);
            list.append(&l);
        }
        side.append(&back);
        side.append(&list);

        // A fixed-width column: measured for its width, so the text wraps
        // there however long the page runs.
        let scroller = gtk::ScrolledWindow::new();
        scroller.set_vexpand(true);
        scroller.set_width_request(720);
        scroller.set_hscrollbar_policy(gtk::PolicyType::Never);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 10);
        body.set_margin_top(40);
        body.set_margin_start(56);
        body.set_margin_end(56);
        body.set_margin_bottom(48);
        scroller.set_child(Some(&body));
        let rest = gtk::Box::new(gtk::Orientation::Vertical, 0);
        rest.set_hexpand(true);
        root.append(&side);
        root.append(&scroller);
        root.append(&rest);

        let settings = Rc::new(Settings {
            root,
            list,
            body,
            game: game.clone(),
            me: RefCell::new(Weak::new()),
            on_request: RefCell::new(None),
        });
        *settings.me.borrow_mut() = Rc::downgrade(&settings);
        {
            let me = Rc::downgrade(&settings);
            back.connect_clicked(move |_| {
                if let Some(s) = me.upgrade() {
                    s.request(None);
                }
            });
        }
        {
            let me = Rc::downgrade(&settings);
            settings.list.connect_row_selected(move |_, row| {
                if let (Some(s), Some(row)) = (me.upgrade(), row) {
                    s.render(row.index() as usize);
                }
            });
        }
        let me = Rc::downgrade(&settings);
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        keys.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape
                && let Some(s) = me.upgrade()
            {
                s.request(None);
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        settings.root.add_controller(keys);
        settings
    }

    fn request(&self, r: Option<Request>) {
        if let Some(f) = &*self.on_request.borrow() {
            f(r);
        }
    }

    /// Opens the menu at its first page.
    pub fn open(&self) {
        match self.list.row_at_index(0) {
            Some(row) if self.list.selected_row().as_ref() != Some(&row) => {
                self.list.select_row(Some(&row));
            }
            _ => self.render(0),
        }
        if let Some(row) = self.list.row_at_index(0) {
            row.grab_focus();
        }
    }

    fn render(&self, page: usize) {
        clear(&self.body);
        let title = label(PAGES.get(page).copied().unwrap_or("Menu"), "book-title");
        title.set_hexpand(true);
        let top = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        top.set_margin_bottom(6);
        top.append(&title);
        if page == 0 {
            top.append(&how_it_went());
        }
        self.body.append(&top);
        match page {
            0 => self.page_settings(),
            1 => self.page_purpose(),
            2 => self.page_kept(),
            3 => self.page_logbook(),
            _ => self.page_credits(),
        }
    }

    fn page_settings(&self) {
        let settings = self.game.borrow().journal().settings.clone();

        let music = gtk::Switch::new();
        music.set_active(!settings.quiet);
        let game = self.game.clone();
        music.connect_active_notify(move |b| {
            let mut g = game.borrow_mut();
            let j = g.journal_mut();
            j.settings.quiet = !b.is_active();
            g.save_settings();
        });
        let volume = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 0.05);
        volume.set_value(settings.volume.unwrap_or(1.0));
        volume.set_width_request(180);
        volume.set_draw_value(false);
        let game = self.game.clone();
        volume.connect_value_changed(move |v| {
            let mut g = game.borrow_mut();
            let j = g.journal_mut();
            j.settings.volume = Some(v.value());
            g.save_settings();
        });
        // The styles of music, as there are.
        let styles = self.game.borrow().styles.clone();
        let names: Vec<&str> = styles.iter().map(|s| s.1.as_str()).collect();
        let style = gtk::DropDown::from_strings(&names);
        style.set_selected(
            styles
                .iter()
                .position(|s| Some(&s.0) == settings.style.as_ref())
                .unwrap_or(0) as u32,
        );
        let game = self.game.clone();
        style.connect_selected_notify(move |d| {
            let mut g = game.borrow_mut();
            let id = g.styles.get(d.selected() as usize).map(|s| s.0.clone());
            let j = g.journal_mut();
            j.settings.style = id;
            g.save_settings();
        });
        let company = gtk::Button::with_label("Start");
        company.add_css_class("chip");
        let me = self.me.borrow().clone();
        company.connect_clicked(move |_| {
            if let Some(me) = me.upgrade() {
                me.request(None);
                me.game.borrow_mut().toggle_company(crate::ui::wall_clock());
            }
        });
        group(
            &self.body,
            "SOUND",
            &[
                row(
                    "Music",
                    "Quiet music under the sky. M goes through the styles in turn, then off.",
                    &music,
                ),
                row("Style", "", &style),
                row("Volume", "", &volume),
                row(
                    "Keep me company",
                    "Leave the music playing while you get on with something else, in a window of any size. The sky rests, and the wisp looks in on you now and then — a word about the night, or a gentle nudge to drink some water, stretch, eat, or send word to someone you've been meaning to. K starts and stops it.",
                    &company,
                ),
            ],
        );

        let calm = gtk::Switch::new();
        calm.set_active(settings.calm);
        let game = self.game.clone();
        calm.connect_active_notify(move |b| {
            let mut g = game.borrow_mut();
            let j = g.journal_mut();
            j.settings.calm = b.is_active();
            g.save_settings();
        });
        let motion = [row(
            "Calmer",
            "The wisp stays on its moss and the stars twinkle less.",
            &calm,
        )];
        group(&self.body, "MOTION", &motion);

        let photos = gtk::Switch::new();
        photos.set_active(!settings.everything);
        let game = self.game.clone();
        photos.connect_active_notify(move |b| {
            let mut g = game.borrow_mut();
            let j = g.journal_mut();
            j.settings.everything = !b.is_active();
            g.save_settings();
        });
        group(
            &self.body,
            "LOOK UP",
            &[row(
                "Photographs only",
                "In Look up, only what has a photograph to show glows. Turn it off to see every constellation and named star as well.",
                &photos,
            )],
        );

        let ask = gtk::DropDown::from_strings(&["Sometimes", "Rarely", "Never"]);
        ask.set_selected(match settings.ask {
            Ask::Sometimes => 0,
            Ask::Rarely => 1,
            Ask::Never => 2,
        });
        let game = self.game.clone();
        ask.connect_selected_notify(move |d| {
            let mut g = game.borrow_mut();
            let j = g.journal_mut();
            j.settings.ask = match d.selected() {
                0 => Ask::Sometimes,
                1 => Ask::Rarely,
                _ => Ask::Never,
            };
            g.save_settings();
        });
        group(
            &self.body,
            "QUESTIONS",
            &[row(
                "How often the sky asks",
                "Sometimes is at most two a visit. Nothing is ever required.",
                &ask,
            )],
        );

        // Where you are: the current answer, and a way to change it.
        let (place, note) = match (settings.lat, settings.lon) {
            (Some(lat), Some(lon)) => (format!("{lat:.2}, {lon:.2}"), "Set by hand."),
            _ => (
                "From your time zone".to_owned(),
                "Close enough for the sky; a nearer place moves it by a few degrees.",
            ),
        };
        let change = gtk::Button::with_label("Change…");
        change.add_css_class("quiet");
        let place_row = row(&place, note, &change);
        let editor = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        editor.add_css_class("menu-row");
        let lat = gtk::Entry::new();
        lat.set_placeholder_text(Some("Latitude, e.g. 53.48"));
        lat.set_width_chars(14);
        let lon = gtk::Entry::new();
        lon.set_placeholder_text(Some("Longitude, e.g. -2.24"));
        lon.set_width_chars(14);
        if let (Some(a), Some(b)) = (settings.lat, settings.lon) {
            lat.set_text(&format!("{a}"));
            lon.set_text(&format!("{b}"));
        }
        let set = gtk::Button::with_label("Use these");
        set.add_css_class("quiet");
        let clear_place = gtk::Button::with_label("Use the time zone");
        clear_place.add_css_class("quiet");
        editor.append(&lat);
        editor.append(&lon);
        editor.append(&set);
        editor.append(&clear_place);
        editor.set_visible(false);
        let said = label("", "book-quiet");
        said.add_css_class("menu-row");
        said.set_visible(false);
        {
            let editor = editor.clone();
            change.connect_clicked(move |_| editor.set_visible(!editor.is_visible()));
        }
        {
            let game = self.game.clone();
            let said = said.clone();
            set.connect_clicked(move |_| {
                let (a, b) = (lat.text().trim().parse::<f64>(), lon.text().trim().parse::<f64>());
                said.set_visible(true);
                match (a, b) {
                    (Ok(a), Ok(b)) if (-90.0..=90.0).contains(&a) && (-180.0..=180.0).contains(&b) => {
                        let mut g = game.borrow_mut();
                        let j = g.journal_mut();
                        j.settings.lat = Some(a);
                        j.settings.lon = Some(b);
                        said.set_text(if g.save_settings() {
                            "Saved. The sky will use it next time it opens."
                        } else {
                            "That couldn't be saved."
                        });
                    }
                    _ => said.set_text("Latitude runs from -90 to 90 and longitude from -180 to 180, north and east positive."),
                }
            });
        }
        {
            let game = self.game.clone();
            let said = said.clone();
            clear_place.connect_clicked(move |_| {
                let mut g = game.borrow_mut();
                let j = g.journal_mut();
                j.settings.lat = None;
                j.settings.lon = None;
                g.save_settings();
                said.set_visible(true);
                said.set_text("Back to the time zone, from next time.");
            });
        }
        group(&self.body, "WHERE YOU ARE", &[place_row, editor]);
        self.body.append(&said);
    }

    fn page_purpose(&self) {
        let lead = label(LEAD, "book-big");
        lead.set_max_width_chars(44);
        self.body.append(&lead);
        let grid = gtk::Grid::new();
        grid.set_row_spacing(10);
        grid.set_column_spacing(10);
        grid.set_column_homogeneous(true);
        grid.set_margin_top(14);
        for (k, (title, text)) in PILLARS.iter().enumerate() {
            let card = gtk::Box::new(gtk::Orientation::Vertical, 4);
            card.add_css_class("book-card");
            card.append(&label(title, "menu-pillar"));
            let t = label(text, "book-quiet");
            t.set_max_width_chars(34);
            t.set_width_chars(34);
            card.append(&t);
            grid.attach(&card, (k % 2) as i32, (k / 2) as i32, 1, 1);
        }
        self.body.append(&grid);
        for (heading, text) in [("WHY THE SKY", WHY_THE_SKY), ("OVER TIME", OVER_TIME)] {
            self.body.append(&label(heading, "book-heading"));
            let l = label(text, "book-body");
            l.set_max_width_chars(64);
            self.body.append(&l);
        }
    }

    fn page_kept(&self) {
        self.body.append(&label(
            "Everything stays on this computer, as plain text files you can read. The app has no network access at all.",
            "book-body",
        ));
        let g = self.game.borrow();
        let j = g.journal();
        let kept = [
            (
                "Nights",
                j.nights().len(),
                "What you found, set down and wrote each night, for the logbook.",
            ),
            (
                "Finds",
                j.found.len(),
                "So each night offers something new, and old friends say something new.",
            ),
            (
                "Weights",
                j.weights.len(),
                "To bring one back, mark it sorted, chart a course, or look back at it later.",
            ),
            (
                "Names",
                j.people.len(),
                "So the people who keep coming up gather on one page, and can have a star.",
            ),
            (
                "Plans",
                j.plans.len(),
                "To mention one as its night comes near, and ask once how it went.",
            ),
            (
                "Courses",
                j.courses.len(),
                "The plans you've charted for a weight, and how they're going.",
            ),
            (
                "Questions asked",
                j.asked.len(),
                "So the same one isn't asked again within a month.",
            ),
            (
                "Drawings",
                j.drawings.len(),
                "So the shapes you've drawn stay in the sky.",
            ),
        ];
        let rows: Vec<gtk::Box> = kept
            .iter()
            .map(|(what, n, why)| {
                let count = label(&n.to_string(), "menu-count");
                row(what, why, &count)
            })
            .chain(std::iter::once(row(
                "Settings",
                "These choices, and which tips the wisp has already given.",
                &label("", "menu-count"),
            )))
            .collect();
        group(&self.body, "WHAT'S KEPT, AND WHY", &rows);
    }

    fn page_logbook(&self) {
        self.body.append(&label(
            "A page for each night, and pages that gather what keeps coming back. Anything in it can be deleted on its own with the bin beside it.",
            "book-body",
        ));
        let open_book = gtk::Button::with_label("Open");
        open_book.add_css_class("quiet");
        {
            let me = self.me.borrow().clone();
            open_book.connect_clicked(move |_| {
                if let Some(s) = me.upgrade() {
                    s.request(Some(Request::Book(None)));
                }
            });
        }
        let said = label("", "book-quiet");
        said.set_margin_top(6);

        let backup = gtk::Button::with_label("Back up");
        backup.add_css_class("quiet");
        {
            let game = self.game.clone();
            let said = said.clone();
            backup.connect_clicked(move |b| {
                let today = game.borrow().night().to_owned();
                let mut dialog = gtk::FileDialog::builder()
                    .title("Save a Westering backup")
                    .initial_name(format!("Westering backup {today}.toml"));
                if let Some(downloads) = glib::user_special_dir(glib::UserDirectory::Downloads) {
                    dialog = dialog.initial_folder(&gio::File::for_path(downloads));
                }
                let window = b.root().and_downcast::<gtk::Window>();
                let (game, said) = (game.clone(), said.clone());
                dialog
                    .build()
                    .save(window.as_ref(), gio::Cancellable::NONE, move |r| {
                        let Some(path) = r.ok().and_then(|f| f.path()) else {
                            return;
                        };
                        let text = {
                            let g = game.borrow();
                            g.journal().backup(g.night())
                        };
                        match std::fs::write(&path, text) {
                            Ok(()) => said.set_text(&format!(
                                "Saved as “{}”.",
                                path.file_name()
                                    .and_then(|f| f.to_str())
                                    .unwrap_or_default()
                            )),
                            Err(e) => said.set_text(&format!("That didn't work: {e}.")),
                        }
                    });
            });
        }

        let restore = gtk::Button::with_label("Restore…");
        restore.add_css_class("quiet");
        {
            let game = self.game.clone();
            let said = said.clone();
            restore.connect_clicked(move |b| {
                let filter = gtk::FileFilter::new();
                filter.set_name(Some("Westering backups"));
                filter.add_pattern("*.toml");
                let filters = gio::ListStore::new::<gtk::FileFilter>();
                filters.append(&filter);
                let mut dialog = gtk::FileDialog::builder()
                    .title("Choose a Westering backup")
                    .filters(&filters);
                if let Some(downloads) = glib::user_special_dir(glib::UserDirectory::Downloads) {
                    dialog = dialog.initial_folder(&gio::File::for_path(downloads));
                }
                let dialog = dialog.build();
                let window = b.root().and_downcast::<gtk::Window>();
                let (game, said, button) = (game.clone(), said.clone(), b.clone());
                dialog.open(window.as_ref(), gio::Cancellable::NONE, move |r| {
                    let Ok(file) = r else { return };
                    let text = file
                        .path()
                        .and_then(|p| std::fs::read_to_string(p).ok())
                        .unwrap_or_default();
                    let Some(made) = Journal::backup_date(&text) else {
                        said.set_text("That file isn't a Westering backup.");
                        return;
                    };
                    let when = long_date(&made);
                    let (game, said) = (game.clone(), said.clone());
                    confirm(
                        &button,
                        &format!("Restore the backup from {when}?"),
                        "Everything Westering keeps now will be replaced by what's in the backup: every logbook page, weight, name, plan, course and drawing, and your settings. Anything added since the backup was made will be lost.\n\nIf you might want what's here now, cancel and back it up first.",
                        "Replace with the backup",
                        move || {
                            let r = game.borrow_mut().restore(&text);
                            said.set_text(&match r {
                                Ok(_) => format!("Restored. The logbook is as it was on {when}."),
                                Err(e) => format!("That backup couldn't be restored ({e}). Nothing was changed."),
                            });
                        },
                    );
                });
            });
        }

        let forget = gtk::Button::with_label("Start again…");
        forget.add_css_class("quiet");
        {
            let game = self.game.clone();
            let said = said.clone();
            forget.connect_clicked(move |b| {
                let (game, said) = (game.clone(), said.clone());
                confirm(
                    b,
                    "Forget everything and start again?",
                    "This deletes every logbook page, weight, name, plan, course and drawing, and the record of what you've found, so Westering starts again as if new. Your settings stay.\n\nIt can't be undone. If you might want any of it, cancel and back up first.",
                    "Forget everything",
                    move || {
                        said.set_text(match game.borrow_mut().forget_everything() {
                            Ok(()) => "Forgotten. The logbook is empty.",
                            Err(_) => "Some files couldn't be removed.",
                        });
                    },
                );
            });
        }

        group(
            &self.body,
            "LOGBOOK",
            &[row("Read it", "L opens it from the sky, too.", &open_book)],
        );
        group(
            &self.body,
            "KEEPING IT SAFE",
            &[
                row(
                    "Back up",
                    "Puts a copy of everything in your Downloads folder, as one file.",
                    &backup,
                ),
                row(
                    "Restore",
                    "Brings back a backup, in place of what's kept now.",
                    &restore,
                ),
                row(
                    "Start again",
                    "Forgets everything the logbook holds.",
                    &forget,
                ),
            ],
        );
        self.body.append(&said);
    }

    fn page_credits(&self) {
        let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
        card.add_css_class("menu-card");
        for line in CREDITS {
            let l = label(line, "book-quiet");
            l.set_max_width_chars(54);
            l.add_css_class("menu-row");
            card.append(&l);
        }
        self.body.append(&card);
        let photos = crate::eyepiece::Photos::load();
        if !photos.credits().is_empty() {
            self.body.append(&label("PHOTOGRAPHS", "book-heading"));
            let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
            card.add_css_class("menu-card");
            for c in photos.credits() {
                let line = gtk::Box::new(gtk::Orientation::Vertical, 1);
                line.add_css_class("menu-row");
                let title = label(&c.title, "book-body");
                title.set_max_width_chars(48);
                line.append(&title);
                let by = label(&format!("{} · {}", c.credit, c.licence), "book-quiet");
                by.set_max_width_chars(54);
                by.set_tooltip_text(Some(&c.source));
                line.append(&by);
                card.append(&line);
            }
            self.body.append(&card);
        }
    }
}
