//! Charting a course for a weight, only ever when the user asks: four short
//! steps (a wish, how it looks at its best, what gets in the way, and what
//! to do when it does), with the user's own earlier words to hand. Each step
//! lights a star; the course ends as a little constellation of its own.

use crate::game::Game;
use crate::pages::{Go, Request};
use crate::ui::{clear, detach, label};
use gtk::prelude::*;
use gtk::{gdk, glib};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use westering_core::journal::{Course as Record, Journal};

const STEPS: [(&str, &str, &str); 4] = [
    (
        "WISH",
        "If this weight got lighter, what would that look like?",
        "One line is plenty.",
    ),
    (
        "AT ITS BEST",
        "Picture it going well. What would be different on an ordinary Tuesday?",
        "Take a moment with it before you write.",
    ),
    (
        "WHAT GETS IN THE WAY",
        "What in you tends to get in the way?",
        "Not other people, not luck: the thing you do, feel or put off.",
    ),
    (
        "THE PLAN",
        "When that happens, what will you do instead?",
        "Something small and specific enough to do on the spot.",
    ),
];

/// Where the course's four stars sit in the little chart at the top.
const CHART: [(f64, f64); 4] = [(10.0, 30.0), (62.0, 14.0), (116.0, 26.0), (170.0, 8.0)];

#[derive(Default)]
struct State {
    weight: u32,
    id: u32,
    step: usize,
    fields: [String; 4],
}

pub struct Course {
    pub root: gtk::Box,
    game: Rc<RefCell<Game>>,
    me: RefCell<Weak<Course>>,
    state: RefCell<State>,
    subtitle: gtk::Label,
    chart: gtk::DrawingArea,
    reached: Rc<Cell<usize>>,
    main: gtk::Box,
    side: gtk::Box,
    words: gtk::Box,
    entry: gtk::Entry,
    check: gtk::CheckButton,
    pub on_request: Go,
}

/// One labelled part of a course: a small heading and the user's words.
fn part(card: &gtk::Box, heading: &str, text: &str, class: &str) {
    if text.trim().is_empty() {
        return;
    }
    let row = gtk::Box::new(gtk::Orientation::Vertical, 2);
    row.set_margin_top(8);
    row.append(&label(heading, "course-step"));
    row.append(&label(text.trim(), class));
    card.append(&row);
}

/// A course as the logbook and the last step show it: the four parts in
/// the user's own words, the plan largest.
pub fn course_card(c: &Record, weight: Option<&str>) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 2);
    card.add_css_class("book-card");
    if let Some(w) = weight {
        card.append(&label(&format!("For “{w}”"), "book-quiet"));
    }
    part(&card, "WISH", &c.wish, "book-body");
    part(&card, "AT ITS BEST", &c.outcome, "book-body");
    part(&card, "WHEN", &c.obstacle, "course-when");
    part(&card, "I'LL", &c.plan, "book-big");
    card
}

impl Course {
    pub fn new(game: &Rc<RefCell<Game>>) -> Rc<Course> {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("page");
        root.add_css_class("course");
        root.set_margin_top(40);
        root.set_margin_start(56);
        root.set_margin_end(48);
        root.set_margin_bottom(32);

        // The heading, with the course's stars joining up as it's charted.
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 24);
        let titles = gtk::Box::new(gtk::Orientation::Vertical, 4);
        titles.set_hexpand(true);
        titles.append(&label("Chart a course", "book-title"));
        let subtitle = label("", "book-quiet");
        titles.append(&subtitle);
        let reached = Rc::new(Cell::new(0usize));
        let chart = gtk::DrawingArea::new();
        chart.set_content_width(184);
        chart.set_content_height(40);
        chart.set_valign(gtk::Align::Center);
        {
            let reached = reached.clone();
            chart.set_draw_func(move |_, cr, _, _| draw_chart(cr, reached.get()));
        }
        head.append(&titles);
        head.append(&chart);
        root.append(&head);

        let body = gtk::Box::new(gtk::Orientation::Horizontal, 48);
        body.set_vexpand(true);
        let main = gtk::Box::new(gtk::Orientation::Vertical, 12);
        main.set_width_request(540);
        main.set_hexpand(true);
        let side = gtk::Box::new(gtk::Orientation::Vertical, 6);
        side.set_width_request(280);
        side.set_margin_top(28);
        let words = gtk::Box::new(gtk::Orientation::Vertical, 6);
        side.append(&label("FROM YOUR LOGBOOK", "course-step"));
        let hint = label("Click one to use it.", "book-quiet");
        hint.set_tooltip_text(Some("Or press Ctrl and its number"));
        side.append(&hint);
        side.append(&words);
        body.append(&main);
        body.append(&side);
        root.append(&body);

        let course = Rc::new(Course {
            root,
            game: game.clone(),
            me: RefCell::new(Weak::new()),
            state: RefCell::new(State::default()),
            subtitle,
            chart,
            reached,
            main,
            side,
            words,
            entry: gtk::Entry::new(),
            check: gtk::CheckButton::with_label("Ask me how it's going in a week"),
            on_request: RefCell::new(None),
        });
        *course.me.borrow_mut() = Rc::downgrade(&course);
        {
            let me = Rc::downgrade(&course);
            course.entry.connect_activate(move |_| {
                if let Some(c) = me.upgrade() {
                    c.next();
                }
            });
        }
        let me = Rc::downgrade(&course);
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        keys.connect_key_pressed(move |_, key, _, state| {
            let Some(c) = me.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if key == gdk::Key::Escape {
                c.leave();
                return glib::Propagation::Stop;
            }
            if state.contains(gdk::ModifierType::CONTROL_MASK)
                && let Some(n) = key.to_unicode().and_then(|ch| ch.to_digit(10))
            {
                c.pull(n as usize);
                return glib::Propagation::Stop;
            }
            if c.state.borrow().step == 4 && matches!(key, gdk::Key::Return | gdk::Key::KP_Enter) {
                c.finish();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        course.root.add_controller(keys);
        course
    }

    fn request(&self, r: Option<Request>) {
        if let Some(f) = &*self.on_request.borrow() {
            f(r);
        }
    }

    /// Starts a course for a weight, picks up one left half-charted, or
    /// reopens a finished one at its plan to change it.
    pub fn open(&self, weight: u32) {
        let mut game = self.game.borrow_mut();
        let night = game.night().to_owned();
        let journal = game.journal_mut();
        let existing = journal
            .courses
            .iter()
            .filter(|c| c.weight == weight)
            .max_by_key(|c| (c.draft, c.id))
            .cloned();
        let record = existing.unwrap_or_else(|| {
            let id = Journal::next_id(journal.courses.iter().map(|c| c.id));
            let fresh = Record {
                id,
                weight,
                made: night,
                draft: true,
                ..Record::default()
            };
            journal.courses.push(fresh.clone());
            fresh
        });
        // Courses kept before the plan was asked on its own had the whole
        // if-then in one field.
        let plan = match record.plan.split_once(" → ") {
            Some((_, then)) => then.to_owned(),
            None => record.plan.clone(),
        };
        let fields = [
            record.wish.clone(),
            record.outcome.clone(),
            record.obstacle.clone(),
            plan,
        ];
        let step = fields.iter().position(|f| f.is_empty()).unwrap_or(3);
        *self.state.borrow_mut() = State {
            weight,
            id: record.id,
            step,
            fields,
        };
        drop(game);
        self.show();
    }

    fn words_for(&self) -> Vec<String> {
        let game = self.game.borrow();
        let journal = game.journal();
        let state = self.state.borrow();
        let mut out = Vec::new();
        if let Some(w) = journal.weight(state.weight) {
            out.push(w.text.clone());
            for night in w.nights.iter().rev() {
                if let Some(page) = journal.night(night) {
                    out.extend(page.answers.iter().map(|a| a.text.clone()));
                }
            }
        }
        for c in journal
            .courses
            .iter()
            .filter(|c| c.weight == state.weight && c.id != state.id)
        {
            out.extend([c.wish.clone(), c.obstacle.clone(), c.plan.clone()]);
        }
        for f in &state.fields {
            out.retain(|x| x != f);
        }
        out.retain(|x| !x.trim().is_empty());
        let mut seen = std::collections::HashSet::new();
        out.retain(|x| seen.insert(x.to_lowercase()));
        out.truncate(9);
        out
    }

    fn show(&self) {
        clear(&self.main);
        clear(&self.words);
        detach(&self.entry);
        detach(&self.check);
        let state = self.state.borrow();
        let weight = self
            .game
            .borrow()
            .journal()
            .weight(state.weight)
            .map(|w| w.text.clone())
            .unwrap_or_default();
        self.subtitle
            .set_text(&format!("For “{weight}”, only as far as you'd like to go."));
        self.reached.set(state.step);
        self.chart.queue_draw();
        self.side.set_visible(state.step < 4);
        if state.step == 4 {
            drop(state);
            self.summary(&weight);
            return;
        }
        let (step, ask, sub) = STEPS[state.step];
        let s = label(
            &format!("{} · {} OF 4", step, state.step + 1),
            "course-step",
        );
        s.set_margin_top(28);
        self.main.append(&s);
        self.main.append(&label(ask, "course-ask"));
        self.main.append(&label(sub, "book-quiet"));
        if state.step == 3 {
            // The plan answers what gets in the way, in the user's own words.
            let when = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            when.set_margin_top(14);
            let w = label("WHEN", "course-step");
            w.set_width_chars(6);
            w.set_valign(gtk::Align::Center);
            when.append(&w);
            when.append(&label(state.fields[2].trim(), "course-when"));
            self.main.append(&when);
            let then = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            let i = label("I'LL", "course-step");
            i.set_width_chars(6);
            i.set_valign(gtk::Align::Center);
            then.append(&i);
            self.entry.set_hexpand(true);
            self.entry.set_placeholder_text(Some(
                "put the phone in another room, ring someone, go for a walk…",
            ));
            then.append(&self.entry);
            self.main.append(&then);
        } else {
            self.entry.set_placeholder_text(None);
            self.entry.set_margin_top(12);
            self.main.append(&self.entry);
        }
        self.entry.set_text(&state.fields[state.step]);
        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        buttons.set_margin_top(18);
        let next = gtk::Button::with_label(if state.step == 3 {
            "See it whole"
        } else {
            "Next"
        });
        next.add_css_class("quiet");
        next.set_tooltip_text(Some("Enter"));
        let back = gtk::Button::with_label("A step back");
        back.add_css_class("quiet");
        back.set_sensitive(state.step > 0);
        let leave = gtk::Button::with_label("Leave it for now");
        leave.add_css_class("quiet");
        leave.set_tooltip_text(Some("Esc. It will keep, half-charted."));
        buttons.append(&next);
        buttons.append(&back);
        buttons.append(&leave);
        self.main.append(&buttons);
        let me = self.me.borrow().clone();
        next.connect_clicked(move |_| {
            if let Some(c) = me.upgrade() {
                c.next();
            }
        });
        let me = self.me.borrow().clone();
        back.connect_clicked(move |_| {
            if let Some(c) = me.upgrade() {
                c.back();
            }
        });
        let me = self.me.borrow().clone();
        leave.connect_clicked(move |_| {
            if let Some(c) = me.upgrade() {
                c.leave();
            }
        });
        drop(state);
        for (i, w) in self.words_for().into_iter().enumerate() {
            let b = gtk::Button::with_label(&w);
            b.add_css_class("quiet");
            b.set_tooltip_text(Some(&format!("Ctrl+{}", i + 1)));
            if let Some(l) = b.child().and_downcast::<gtk::Label>() {
                l.set_wrap(true);
                l.set_xalign(0.0);
                l.set_max_width_chars(34);
            }
            let me = self.me.borrow().clone();
            b.connect_clicked(move |_| {
                if let Some(c) = me.upgrade() {
                    c.pull(i + 1);
                }
            });
            self.words.append(&b);
        }
        let entry = self.entry.clone();
        glib::idle_add_local_once(move || {
            entry.grab_focus();
            entry.set_position(-1);
        });
    }

    /// Brings one of the user's own lines into the box.
    fn pull(&self, n: usize) {
        if let Some(w) = self.words_for().get(n.wrapping_sub(1)) {
            self.entry.set_text(w);
            self.entry.set_position(-1);
        }
    }

    fn take_field(&self) {
        let mut state = self.state.borrow_mut();
        let step = state.step;
        if step < 4 {
            state.fields[step] = self.entry.text().trim().to_owned();
        }
    }

    fn save(&self, draft: bool, check: bool) {
        let state = self.state.borrow();
        let mut game = self.game.borrow_mut();
        let today = game.night().to_owned();
        let journal = game.journal_mut();
        if let Some(c) = journal.courses.iter_mut().find(|c| c.id == state.id) {
            c.wish = state.fields[0].clone();
            c.outcome = state.fields[1].clone();
            c.obstacle = state.fields[2].clone();
            c.plan = state.fields[3].clone();
            c.draft = draft;
            if !draft && check {
                c.check_after = Some(add_days(&today, 7));
            }
        }
        if let Err(e) = journal.save_courses() {
            eprintln!("westering: couldn't save the course: {e}");
        }
    }

    fn next(&self) {
        self.take_field();
        if self.state.borrow().fields[self.state.borrow().step].is_empty() {
            return;
        }
        self.state.borrow_mut().step += 1;
        // Half-charted until it's kept; changing a kept one keeps it kept.
        let kept = {
            let s = self.state.borrow();
            self.game
                .borrow()
                .journal()
                .courses
                .iter()
                .any(|c| c.id == s.id && !c.draft)
        };
        self.save(!kept, false);
        self.show();
    }

    fn back(&self) {
        self.take_field();
        {
            let mut s = self.state.borrow_mut();
            s.step = s.step.saturating_sub(1);
        }
        self.show();
    }

    fn leave(&self) {
        if self.state.borrow().step < 4 {
            self.take_field();
            let kept = {
                let s = self.state.borrow();
                self.game
                    .borrow()
                    .journal()
                    .courses
                    .iter()
                    .any(|c| c.id == s.id && !c.draft)
            };
            self.save(!kept, false);
        }
        self.request(Some(Request::Book(Some("courses".into()))));
    }

    /// The whole course, to look at once before keeping it.
    fn summary(&self, weight: &str) {
        let state = self.state.borrow();
        let record = Record {
            wish: state.fields[0].clone(),
            outcome: state.fields[1].clone(),
            obstacle: state.fields[2].clone(),
            plan: state.fields[3].clone(),
            ..Record::default()
        };
        drop(state);
        let s = label("YOUR COURSE", "course-step");
        s.set_margin_top(28);
        self.main.append(&s);
        let card = course_card(&record, None);
        card.set_margin_top(6);
        self.main.append(&card);
        self.main.append(&label(
            &format!(
                "Next time it comes up, you already know what you'll do. It's kept in the logbook beside “{weight}”."
            ),
            "book-quiet",
        ));
        self.check.set_margin_top(10);
        self.main.append(&self.check);
        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        buttons.set_margin_top(12);
        let done = gtk::Button::with_label("Keep it");
        done.add_css_class("quiet");
        done.set_tooltip_text(Some("Enter"));
        let change = gtk::Button::with_label("Change something");
        change.add_css_class("quiet");
        buttons.append(&done);
        buttons.append(&change);
        self.main.append(&buttons);
        let me = self.me.borrow().clone();
        done.connect_clicked(move |_| {
            if let Some(c) = me.upgrade() {
                c.finish();
            }
        });
        let me = self.me.borrow().clone();
        change.connect_clicked(move |_| {
            if let Some(c) = me.upgrade() {
                c.back();
            }
        });
        let d = done.clone();
        glib::idle_add_local_once(move || {
            d.grab_focus();
        });
    }

    fn finish(&self) {
        self.save(false, self.check.is_active());
        self.request(Some(Request::Book(Some("courses".into()))));
    }
}

/// The course's stars: joined as far as it has got, the next one waiting.
fn draw_chart(cr: &gtk::cairo::Context, reached: usize) {
    let warm = (1.0, 0.86, 0.62);
    cr.set_line_width(1.2);
    cr.set_line_cap(gtk::cairo::LineCap::Round);
    for (k, pair) in CHART.windows(2).enumerate() {
        let lit = k + 1 < reached;
        cr.set_source_rgba(warm.0, warm.1, warm.2, if lit { 0.55 } else { 0.1 });
        cr.move_to(pair[0].0 + 4.0, pair[0].1 + 6.0);
        cr.line_to(pair[1].0 + 4.0, pair[1].1 + 6.0);
        let _ = cr.stroke();
    }
    for (k, &(x, y)) in CHART.iter().enumerate() {
        let (x, y) = (x + 4.0, y + 6.0);
        let (radius, alpha) = if k < reached {
            (3.2, 0.95)
        } else if k == reached {
            (2.6, 0.6)
        } else {
            (1.8, 0.22)
        };
        if k <= reached {
            let glow = gtk::cairo::RadialGradient::new(x, y, 0.0, x, y, radius * 4.0);
            glow.add_color_stop_rgba(0.0, warm.0, warm.1, warm.2, 0.35 * alpha);
            glow.add_color_stop_rgba(1.0, warm.0, warm.1, warm.2, 0.0);
            let _ = cr.set_source(&glow);
            cr.arc(x, y, radius * 4.0, 0.0, std::f64::consts::TAU);
            let _ = cr.fill();
        }
        cr.set_source_rgba(warm.0, warm.1, warm.2, alpha);
        cr.arc(x, y, radius, 0.0, std::f64::consts::TAU);
        let _ = cr.fill();
    }
}

/// A night key some days on.
fn add_days(key: &str, days: i64) -> String {
    let mut p = key.split('-').map(|x| x.parse::<i64>().unwrap_or(1));
    let (y, m, d) = (
        p.next().unwrap_or(2000),
        p.next().unwrap_or(1),
        p.next().unwrap_or(1),
    );
    let at = westering_core::time::midnight_utc(y as i32, m as u32, d as u32)
        + days * westering_core::time::DAY
        + westering_core::time::HOUR;
    westering_core::questions::key_of(at, 0)
}
