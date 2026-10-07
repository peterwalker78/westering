//! The logbook: a page for every night visited, and three contents pages
//! that gather what keeps coming back. It never counts anything.

use crate::game::Game;
use crate::talk::{Go, Request};
use crate::ui::{clear, confirm, label, trash};
use gtk::prelude::*;
use gtk::{gdk, glib};
use std::cell::RefCell;
use std::rc::Rc;
use westering_core::chart::{Chart, chart};
use westering_core::journal::{Journal, long_date, short_date};
use westering_core::questions::days_between;

pub struct Book {
    pub root: gtk::Box,
    list: gtk::ListBox,
    content: gtk::Box,
    /// The key behind each list row: a night, or a contents page.
    keys: RefCell<Vec<String>>,
    game: Rc<RefCell<Game>>,
    me: RefCell<std::rc::Weak<Book>>,
    /// Asks the window to go somewhere else: back to the sky, or a course.
    pub on_request: Go,
}

impl Book {
    pub fn new(game: &Rc<RefCell<Game>>) -> Rc<Book> {
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        root.add_css_class("page");
        let side = gtk::ScrolledWindow::new();
        side.set_width_request(240);
        side.set_hscrollbar_policy(gtk::PolicyType::Never);
        let list = gtk::ListBox::new();
        list.add_css_class("book-side");
        list.set_vexpand(true);
        side.set_child(Some(&list));
        let scroller = gtk::ScrolledWindow::new();
        scroller.set_hexpand(true);
        scroller.set_hscrollbar_policy(gtk::PolicyType::Never);
        let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
        content.set_margin_top(40);
        content.set_margin_bottom(60);
        content.set_margin_start(56);
        content.set_margin_end(56);
        let clamp = gtk::Box::new(gtk::Orientation::Vertical, 0);
        clamp.set_halign(gtk::Align::Start);
        clamp.set_width_request(560);
        clamp.append(&content);
        scroller.set_child(Some(&clamp));
        root.append(&side);
        root.append(&scroller);
        let book = Rc::new(Book {
            root,
            list,
            content,
            keys: RefCell::new(Vec::new()),
            game: game.clone(),
            me: RefCell::new(std::rc::Weak::new()),
            on_request: RefCell::new(None),
        });
        *book.me.borrow_mut() = Rc::downgrade(&book);
        {
            let weak = Rc::downgrade(&book);
            book.list.connect_row_selected(move |_, row| {
                if let (Some(book), Some(row)) = (weak.upgrade(), row) {
                    let key = book.keys.borrow().get(row.index() as usize).cloned();
                    if let Some(key) = key {
                        book.render(&key);
                    }
                }
            });
        }
        {
            let weak = Rc::downgrade(&book);
            let keys = gtk::EventControllerKey::new();
            keys.set_propagation_phase(gtk::PropagationPhase::Capture);
            keys.connect_key_pressed(move |_, key, _, _| {
                if key == gdk::Key::Escape
                    && let Some(book) = weak.upgrade()
                {
                    book.request(None);
                    return glib::Propagation::Stop;
                }
                glib::Propagation::Proceed
            });
            book.root.add_controller(keys);
        }
        book
    }

    fn request(&self, r: Option<Request>) {
        if let Some(f) = &*self.on_request.borrow() {
            f(r);
        }
    }

    /// Fills the list and opens a page: a night key, or "fixed", "coming",
    /// "weights", "courses". None opens the newest night.
    pub fn open(&self, at: Option<&str>) {
        while let Some(row) = self.list.first_child() {
            self.list.remove(&row);
        }
        let game = self.game.borrow();
        let journal = game.journal();
        let mut keys = Vec::new();
        let mut add = |key: &str, text: &str, section: bool| {
            let l = gtk::Label::new(Some(text));
            l.set_xalign(0.0);
            if section {
                l.add_css_class("section");
                let row = gtk::ListBoxRow::new();
                row.set_child(Some(&l));
                row.set_selectable(false);
                row.set_activatable(false);
                self.list.append(&row);
            } else {
                self.list.append(&l);
            }
            keys.push(key.to_owned());
        };
        add("", "CONTENTS", true);
        add("fixed", "Fixed stars", false);
        add("coming", "Coming up", false);
        add("weights", "Recurring weights", false);
        add("courses", "Courses", false);
        add("", "NIGHTS", true);
        let nights = journal.nights();
        for n in &nights {
            add(n, &short_date(n), false);
        }
        if nights.is_empty() {
            add("", "None yet", true);
        }
        drop(game);
        let fallback = nights.first().cloned().unwrap_or_else(|| "fixed".into());
        let want = at.map(str::to_owned).unwrap_or_else(|| fallback.clone());
        let index = keys
            .iter()
            .position(|k| *k == want)
            .or_else(|| keys.iter().position(|k| *k == fallback))
            .unwrap_or(1);
        let key = keys[index].clone();
        *self.keys.borrow_mut() = keys;
        if let Some(row) = self.list.row_at_index(index as i32) {
            self.list.select_row(Some(&row));
            row.grab_focus();
        }
        self.render(&key);
    }

    fn render(&self, key: &str) {
        clear(&self.content);
        let back = gtk::Button::with_label("Back to the sky  (Esc)");
        back.add_css_class("quiet");
        back.set_halign(gtk::Align::Start);
        self.content.append(&back);
        let me = self.me.borrow().clone();
        back.connect_clicked(move |_| {
            if let Some(book) = me.upgrade() {
                book.request(None);
            }
        });
        let game = self.game.clone();
        let g = game.borrow();
        let journal = g.journal();
        match key {
            "fixed" => self.fixed(journal),
            "coming" => self.coming(journal, g.night()),
            "weights" => self.weights(journal),
            "courses" => self.courses(journal),
            night if night.len() == 10 => self.night(&g, night),
            _ => {}
        }
    }

    /// A bin button that asks first, then deletes and redraws the logbook.
    fn delete_button(
        &self,
        tooltip: &str,
        message: String,
        detail: String,
        yes: &'static str,
        delete: impl Fn(&mut Game) -> std::io::Result<()> + 'static,
    ) -> gtk::Button {
        let button = trash(tooltip);
        let me = self.me.borrow().clone();
        let game = self.game.clone();
        let delete = Rc::new(delete);
        button.connect_clicked(move |b| {
            let (me, game, delete) = (me.clone(), game.clone(), delete.clone());
            confirm(b, &message, &detail, yes, move || {
                let r = {
                    let mut g = game.borrow_mut();
                    let r = delete(&mut g);
                    g.refresh();
                    r
                };
                if let Err(e) = r {
                    eprintln!("westering: couldn't delete it: {e}");
                }
                if let Some(book) = me.upgrade() {
                    let key = book
                        .list
                        .selected_row()
                        .and_then(|row| book.keys.borrow().get(row.index() as usize).cloned());
                    book.open(key.as_deref());
                }
            });
        });
        button
    }

    /// A line of words with a bin beside it.
    fn with_trash(&self, words: &impl IsA<gtk::Widget>, bin: &gtk::Button) -> gtk::Box {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        words.set_hexpand(true);
        row.append(words);
        row.append(bin);
        row
    }

    fn heading(&self, text: &str) {
        self.content.append(&label(text, "book-heading"));
    }

    fn title(&self, text: &str) {
        let l = label(text, "book-title");
        l.set_margin_top(18);
        self.content.append(&l);
    }

    fn night(&self, game: &Game, key: &str) {
        let journal = game.journal();
        let date = long_date(key);
        let title = label(&date, "book-title");
        let owned = key.to_owned();
        let bin = self.delete_button(
            "Delete this night",
            format!("Delete the page for {date}?"),
            "This removes that night from the logbook: what you found, what the sky asked and what you answered, and any shapes you drew. Weights and names that also came up on other nights stay.\n\nIt can't be undone.".into(),
            "Delete the night",
            move |g| g.journal_mut().delete_night(&owned),
        );
        bin.set_valign(gtk::Align::Center);
        let row = self.with_trash(&title, &bin);
        row.set_margin_top(18);
        self.content.append(&row);
        let Some(night) = journal.night(key) else {
            return;
        };
        if !night.moon.is_empty() {
            // That night's Moon, drawn at its phase, beside its name.
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            let moon = gtk::DrawingArea::new();
            moon.set_content_width(26);
            moon.set_content_height(26);
            let age = moon_age_on(key);
            moon.set_draw_func(move |_, cr, w, h| draw_moon(cr, w as f64, h as f64, age));
            row.append(&moon);
            let name = label(&night.moon, "book-quiet");
            name.set_valign(gtk::Align::Center);
            row.append(&name);
            self.content.append(&row);
        }
        if !night.finds.is_empty() {
            self.heading("FOUND");
            let chart = chart(&game.sky, game.observer, &night, &journal.drawings);
            // Numbered where the chart shows them, so each can be found on it.
            let number = |name: &String| {
                let chart = chart.as_ref()?;
                chart.finds.iter().position(|(_, _, n)| n == name)
            };
            let names: Vec<String> = night
                .finds
                .iter()
                .map(|name| {
                    // Each name stays whole, with its number, when the list wraps.
                    let name_safe = glib::markup_escape_text(name).replace(' ', "\u{a0}");
                    match number(name) {
                        Some(n) => {
                            format!("<span alpha=\"55%\">{}</span>\u{a0}{name_safe}", n + 1)
                        }
                        None => name_safe,
                    }
                })
                .collect();
            if let Some(chart) = chart {
                let mut says = vec![
                    "The sky that evening, as if you were lying on your back looking up: the edge is the horizon all round, and the numbers are what you found.",
                ];
                if !chart.lines.is_empty() {
                    says.push("The lines are the shape you drew.");
                }
                if !chart.weights.is_empty() {
                    says.push("The warm dots low in the west are what you set down.");
                }
                let area = gtk::DrawingArea::new();
                area.set_content_width(300);
                area.set_content_height(300);
                area.set_halign(gtk::Align::Start);
                area.set_margin_top(6);
                area.set_draw_func(move |_, cr, w, h| draw_chart(cr, w as f64, h as f64, &chart));
                self.content.append(&area);
                self.content.append(&label(&says.join(" "), "book-quiet"));
            }
            let list = label("", "book-body");
            list.set_markup(&names.join("  ·  "));
            self.content.append(&list);
        }
        if !night.weights.is_empty() {
            self.heading("WEIGHTS");
            for w in &night.weights {
                if let Some(weight) = journal.weight(w.weight) {
                    self.weight_row(weight.id, &weight.text, weight.sorted);
                    self.looks(weight);
                }
            }
        }
        if !night.answers.is_empty() {
            self.heading("THE SKY ASKED");
            for a in &night.answers {
                let words = gtk::Box::new(gtk::Orientation::Vertical, 2);
                words.append(&label(&a.prompt, "book-asked"));
                words.append(&label(&a.text, "book-body"));
                let (key, question) = (key.to_owned(), a.question.clone());
                let bin = self.delete_button(
                    "Delete this answer",
                    "Delete this answer?".into(),
                    format!(
                        "“{}” goes from this night's page{}.\n\nIt can't be undone.",
                        a.text,
                        if a.person.is_some() {
                            ". The name stays only if it came up on another night too"
                        } else {
                            ""
                        }
                    ),
                    "Delete it",
                    move |g| g.journal_mut().delete_answer(&key, &question),
                );
                let row = self.with_trash(&words, &bin);
                row.set_margin_top(8);
                self.content.append(&row);
            }
        }
        let drawn: Vec<_> = journal.drawings.iter().filter(|d| d.night == key).collect();
        if !drawn.is_empty() {
            self.heading("DRAWN");
            for d in drawn {
                let words = match &d.reveals {
                    Some(r) => format!("{}, part of {r}", d.name),
                    None => d.name.clone(),
                };
                let (name, night) = (d.name.clone(), d.night.clone());
                let bin = self.delete_button(
                    "Delete this drawing",
                    format!("Delete “{}”?", d.name),
                    "The shape goes from the sky and from this night's page.\n\nIt can't be undone.".into(),
                    "Delete it",
                    move |g| g.journal_mut().delete_drawing(&name, &night),
                );
                self.content
                    .append(&self.with_trash(&label(&words, "book-body"), &bin));
            }
        }
        let plans: Vec<_> = journal
            .plans
            .iter()
            .filter(|p| night.plans.contains(&p.id))
            .collect();
        if !plans.is_empty() {
            self.heading("PLANNED");
            for p in plans {
                let who = p
                    .who
                    .as_deref()
                    .map(|w| format!(", with {w}"))
                    .unwrap_or_default();
                let words = label(
                    &format!("{}{who} · {}, {}", p.what, p.event, short_date(&p.date)),
                    "book-body",
                );
                let bin = self.plan_bin(p);
                self.content.append(&self.with_trash(&words, &bin));
            }
        }
    }

    /// How a weight sat when it was looked back at, later.
    fn looks(&self, weight: &westering_core::journal::Weight) {
        if weight.looks.is_empty() {
            return;
        }
        let later: Vec<String> = weight
            .looks
            .iter()
            .map(|k| format!("{} on {}", k.answer, short_date(&k.night)))
            .collect();
        self.content.append(&label(
            &format!("Looking back: {}", later.join(" · ")),
            "book-quiet",
        ));
    }

    fn plan_bin(&self, p: &westering_core::journal::Plan) -> gtk::Button {
        let id = p.id;
        self.delete_button(
            "Delete this plan",
            "Delete this plan?".into(),
            format!(
                "“{}” goes from Coming up and from the night it was made, and the sky won't mention it again.\n\nIt can't be undone.",
                p.what
            ),
            "Delete the plan",
            move |g| g.journal_mut().delete_plan(id),
        )
    }

    fn weight_row(&self, id: u32, text: &str, sorted: bool) {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        row.set_margin_top(4);
        let l = label(text, "book-body");
        l.set_hexpand(true);
        if sorted {
            l.set_opacity(0.55);
        }
        row.append(&l);
        let sort = gtk::Button::with_label(if sorted {
            "Not sorted after all"
        } else {
            "Sorted"
        });
        sort.add_css_class("quiet");
        let course = gtk::Button::with_label("Chart a course");
        course.add_css_class("quiet");
        row.append(&sort);
        row.append(&course);
        let charted = self
            .game
            .borrow()
            .journal()
            .courses
            .iter()
            .any(|c| c.weight == id);
        let bin = self.delete_button(
            "Delete this weight",
            format!("Delete “{text}”?"),
            format!(
                "This removes it from every night it was set down on{}. The nights themselves stay.\n\nIt can't be undone.",
                if charted {
                    ", along with the course charted for it"
                } else {
                    ""
                }
            ),
            "Delete the weight",
            move |g| g.journal_mut().delete_weight(id),
        );
        bin.set_valign(gtk::Align::Center);
        row.append(&bin);
        self.content.append(&row);
        let me = self.me.borrow().clone();
        let game = self.game.clone();
        sort.connect_clicked(move |_| {
            {
                let mut g = game.borrow_mut();
                let j = g.journal_mut();
                if let Some(w) = j.weights.iter_mut().find(|w| w.id == id) {
                    w.sorted = !w.sorted;
                }
                let _ = j.save_weights();
            }
            // Re-draw whatever page this row sits on, once this click is done.
            let me = me.clone();
            glib::idle_add_local_once(move || {
                if let Some(book) = me.upgrade()
                    && let Some(row) = book.list.selected_row()
                {
                    let key = book.keys.borrow().get(row.index() as usize).cloned();
                    if let Some(key) = key {
                        book.render(&key);
                    }
                }
            });
        });
        let me = self.me.borrow().clone();
        course.connect_clicked(move |_| {
            if let Some(book) = me.upgrade() {
                book.request(Some(Request::Course(id)));
            }
        });
    }

    fn fixed(&self, journal: &Journal) {
        self.title("Fixed stars");
        let people = journal.fixed_stars();
        if people.is_empty() {
            self.content.append(&label(
                "The people whose names come up on more than one night will gather here.",
                "book-quiet",
            ));
            return;
        }
        for p in people {
            let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
            card.add_css_class("book-card");
            card.set_margin_top(12);
            let name = p.name.clone();
            let bin = self.delete_button(
                "Forget this name",
                format!("Forget {}?", p.name),
                format!(
                    "This removes {} from the logbook: their star, and the answers that named them, on every page. Plans with them stay, without their name.\n\nIt can't be undone.",
                    p.name
                ),
                "Forget them",
                move |g| g.journal_mut().delete_person(&name),
            );
            card.append(&self.with_trash(&label(&p.name, "book-big"), &bin));
            if !p.stars.is_empty() {
                let names: Vec<String> = p
                    .stars
                    .iter()
                    .map(|hr| {
                        self.game
                            .borrow()
                            .star_name(*hr)
                            .unwrap_or_else(|| format!("HR {hr}"))
                    })
                    .collect();
                card.append(&label(
                    &format!("Their star: {}", names.join(", ")),
                    "book-quiet",
                ));
            }
            for m in &p.mentions {
                let Some(night) = journal.night(&m.night) else {
                    continue;
                };
                if let Some(a) = night.answers.iter().find(|a| {
                    a.question == m.question
                        && a.person
                            .as_deref()
                            .is_some_and(|n| n.eq_ignore_ascii_case(&p.name))
                }) {
                    let l = label(
                        &format!("{} · {}", short_date(&m.night), a.prompt),
                        "book-asked",
                    );
                    l.set_margin_top(4);
                    card.append(&l);
                } else if m.question == "plan" {
                    card.append(&label(
                        &format!("{} · a plan together", short_date(&m.night)),
                        "book-asked",
                    ));
                }
            }
            self.content.append(&card);
        }
    }

    fn coming(&self, journal: &Journal, today: &str) {
        self.title("Coming up");
        let mut ahead: Vec<_> = journal
            .plans
            .iter()
            .filter(|p| days_between(today, &p.date) >= 0)
            .collect();
        ahead.sort_by(|a, b| a.date.cmp(&b.date));
        let mut past: Vec<_> = journal
            .plans
            .iter()
            .filter(|p| days_between(today, &p.date) < 0)
            .collect();
        past.sort_by(|a, b| b.date.cmp(&a.date));
        if ahead.is_empty() && past.is_empty() {
            self.content.append(&label(
                "Plans made under the sky will wait here until their night comes.",
                "book-quiet",
            ));
            return;
        }
        if !ahead.is_empty() {
            self.heading("AHEAD");
            for p in ahead {
                let who = p
                    .who
                    .as_deref()
                    .map(|w| format!(", with {w}"))
                    .unwrap_or_default();
                let card = gtk::Box::new(gtk::Orientation::Vertical, 2);
                card.add_css_class("book-card");
                card.set_margin_top(8);
                let bin = self.plan_bin(p);
                card.append(
                    &self.with_trash(&label(&format!("{}{who}", p.what), "book-body"), &bin),
                );
                card.append(&label(
                    &format!("{} · {}", long_date(&p.date), p.event),
                    "book-quiet",
                ));
                self.content.append(&card);
            }
        }
        if !past.is_empty() {
            self.heading("DONE AND GONE");
            for p in past {
                let who = p
                    .who
                    .as_deref()
                    .map(|w| format!(", with {w}"))
                    .unwrap_or_default();
                let how = match (p.outcome.as_deref(), p.note.as_deref()) {
                    (_, Some(note)) => note.to_owned(),
                    (Some("went"), None) => "It happened.".into(),
                    _ => String::new(),
                };
                let card = gtk::Box::new(gtk::Orientation::Vertical, 2);
                card.add_css_class("book-card");
                card.set_margin_top(8);
                let bin = self.plan_bin(p);
                card.append(
                    &self.with_trash(&label(&format!("{}{who}", p.what), "book-body"), &bin),
                );
                card.append(&label(
                    &format!("{} · {}", short_date(&p.date), p.event),
                    "book-quiet",
                ));
                if !how.is_empty() {
                    card.append(&label(&how, "book-asked"));
                }
                self.content.append(&card);
            }
        }
    }

    fn weights(&self, journal: &Journal) {
        self.title("Recurring weights");
        let recurring: Vec<_> = journal
            .weights
            .iter()
            .filter(|w| w.nights.len() >= 3)
            .collect();
        if recurring.is_empty() {
            self.content.append(&label(
                "Weights you bring back on three or more nights gather here, with a way to chart a course if you'd like one.",
                "book-quiet",
            ));
            return;
        }
        for w in recurring {
            self.weight_row(w.id, &w.text, w.sorted);
            self.looks(w);
            let dates: Vec<String> = w.nights.iter().map(|n| short_date(n)).collect();
            self.content
                .append(&label(&dates.join(" · "), "book-quiet"));
        }
    }

    fn courses(&self, journal: &Journal) {
        self.title("Courses");
        let started = |c: &&westering_core::journal::Course| {
            !c.wish.is_empty() || !c.outcome.is_empty() || !c.obstacle.is_empty()
        };
        let courses: Vec<_> = journal.courses.iter().filter(started).collect();
        if courses.is_empty() {
            self.content.append(&label(
                "Any weight in the logbook has a Chart a course button. Courses you chart will be kept here.",
                "book-quiet",
            ));
            return;
        }
        // Newest first; ones still being charted at the top.
        let mut courses = courses;
        courses.sort_by_key(|c| (!c.draft, std::cmp::Reverse(c.id)));
        for c in courses {
            let weight = journal.weight(c.weight).map(|w| w.text.as_str());
            let card = crate::course::course_card(c, weight);
            card.set_margin_top(12);
            let id = c.id;
            let bin = self.delete_button(
                "Delete this course",
                "Delete this course?".into(),
                format!(
                    "The course{} goes. The weight itself stays, and you can chart another any time.\n\nIt can't be undone.",
                    weight.map(|w| format!(" for “{w}”")).unwrap_or_default()
                ),
                "Delete the course",
                move |g| g.journal_mut().delete_course(id),
            );
            bin.set_halign(gtk::Align::End);
            card.prepend(&bin);
            if c.draft {
                card.prepend(&label("STILL BEING CHARTED", "course-step"));
            }
            if !c.checks.is_empty() {
                let row = gtk::Box::new(gtk::Orientation::Vertical, 2);
                row.set_margin_top(10);
                row.append(&label("HOW IT'S GONE", "course-step"));
                for k in &c.checks {
                    row.append(&label(
                        &format!("{} · {}", short_date(&k.night), k.answer),
                        "book-asked",
                    ));
                }
                card.append(&row);
            }
            let button = gtk::Button::with_label(if c.draft {
                "Carry on charting"
            } else {
                "Change the plan"
            });
            button.add_css_class("quiet");
            button.set_halign(gtk::Align::Start);
            button.set_margin_top(10);
            let me = self.me.borrow().clone();
            let weight_id = c.weight;
            button.connect_clicked(move |_| {
                if let Some(book) = me.upgrade() {
                    book.request(Some(Request::Course(weight_id)));
                }
            });
            card.append(&button);
            self.content.append(&card);
        }
    }
}

/// The Moon's age, as a share of its cycle, in the evening of a night key.
fn moon_age_on(key: &str) -> f64 {
    let mut p = key.split('-').map(|x| x.parse::<i64>().unwrap_or(1));
    let (y, m, d) = (
        p.next().unwrap_or(2000),
        p.next().unwrap_or(1),
        p.next().unwrap_or(1),
    );
    let evening = westering_core::time::midnight_utc(y as i32, m as u32, d as u32)
        + 21 * westering_core::time::HOUR;
    westering_core::ephem::moon_age(evening)
}

/// A small Moon at its phase: lit on the right while it waxes, the left as
/// it wanes, as it looks from the north.
fn draw_moon(cr: &gtk::cairo::Context, w: f64, h: f64, age: f64) {
    let (cx, cy, r) = (w / 2.0, h / 2.0, w.min(h) / 2.0 - 1.0);
    cr.set_source_rgba(0.85, 0.87, 0.95, 0.12);
    cr.arc(cx, cy, r, 0.0, std::f64::consts::TAU);
    let _ = cr.fill();
    let waxing = age < 0.5;
    let side = if waxing { 1.0 } else { -1.0 };
    let into = if waxing { age } else { 1.0 - age };
    let e = r * (into * std::f64::consts::TAU).cos();
    let steps = 32;
    for k in 0..=steps {
        let t = -std::f64::consts::FRAC_PI_2 + std::f64::consts::PI * k as f64 / steps as f64;
        let (x, y) = (cx + side * r * t.cos(), cy + r * t.sin());
        if k == 0 {
            cr.move_to(x, y);
        } else {
            cr.line_to(x, y);
        }
    }
    for k in 0..=steps {
        let t = std::f64::consts::FRAC_PI_2 - std::f64::consts::PI * k as f64 / steps as f64;
        cr.line_to(cx + side * e * t.cos(), cy + r * t.sin());
    }
    cr.close_path();
    cr.set_source_rgba(0.98, 0.94, 0.84, 0.92);
    let _ = cr.fill();
}

/// A night's sky on its page: bright stars for the lie of it, each find
/// numbered where it stood, any shape drawn, and the weights low in the west.
fn draw_chart(cr: &gtk::cairo::Context, w: f64, h: f64, chart: &Chart) {
    use std::f64::consts::TAU;
    let (cx, cy) = (w / 2.0, h / 2.0);
    let r = w.min(h) / 2.0 - 18.0;
    let at = |x: f64, y: f64| (cx + x * r, cy + y * r);
    let dot = |x: f64, y: f64, radius: f64, rgba: (f64, f64, f64, f64)| {
        cr.set_source_rgba(rgba.0, rgba.1, rgba.2, rgba.3);
        cr.arc(x, y, radius, 0.0, TAU);
        let _ = cr.fill();
    };
    dot(cx, cy, r, (0.5, 0.58, 0.85, 0.07));
    cr.set_source_rgba(0.82, 0.85, 0.94, 0.25);
    cr.set_line_width(1.0);
    cr.arc(cx, cy, r, 0.0, TAU);
    let _ = cr.stroke();
    cr.set_font_size(10.0);
    let words = |text: &str, x: f64, y: f64, alpha: f64| {
        let (tw, th) = cr
            .text_extents(text)
            .map(|e| (e.width(), e.height()))
            .unwrap_or((6.0, 8.0));
        cr.set_source_rgba(0.86, 0.88, 0.95, alpha);
        cr.move_to(x - tw / 2.0, y + th / 2.0);
        let _ = cr.show_text(text);
    };
    // East is on the left, as it is looking up.
    for (name, x, y) in [
        ("N", 0.0, -1.0),
        ("E", -1.0, 0.0),
        ("S", 0.0, 1.0),
        ("W", 1.0, 0.0),
    ] {
        words(name, cx + x * (r + 10.0), cy + y * (r + 10.0), 0.55);
    }
    for &(x, y, mag) in &chart.stars {
        let mag = mag as f64;
        let (px, py) = at(x, y);
        dot(
            px,
            py,
            (1.9 - 0.36 * mag).clamp(0.5, 2.2),
            (0.86, 0.89, 1.0, (0.8 - 0.13 * mag).clamp(0.25, 0.85)),
        );
    }
    cr.set_source_rgba(0.78, 0.84, 1.0, 0.55);
    for [a, b] in &chart.lines {
        let (a, b) = (at(a.0, a.1), at(b.0, b.1));
        cr.move_to(a.0, a.1);
        cr.line_to(b.0, b.1);
        let _ = cr.stroke();
    }
    for &(x, y) in &chart.weights {
        let (px, py) = at(x, y);
        dot(px, py, 6.0, (1.0, 0.72, 0.4, 0.16));
        dot(px, py, 2.4, (1.0, 0.78, 0.45, 0.95));
    }
    for (n, (x, y, _)) in chart.finds.iter().enumerate() {
        let (px, py) = at(*x, *y);
        dot(px, py, 7.0, (1.0, 0.94, 0.84, 0.14));
        dot(px, py, 2.8, (1.0, 0.95, 0.86, 1.0));
        words(&(n + 1).to_string(), px + 9.0, py - 7.0, 0.9);
    }
}
