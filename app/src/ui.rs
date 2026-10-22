//! The window's own widgets: the prompt at the foot of the sky, and the
//! style shared by every page.

use crate::game::Game;
use crate::talk::Prompt;
use gtk::prelude::*;
use gtk::{gdk, glib};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub const CSS: &str = r#"
window, .page { background: #070913; color: #dfe3ee; }
.veil { background: rgba(3, 4, 10, 0.7); }
.confirm-shade { background: rgba(2, 3, 8, 0.5); }
.confirm {
  background: rgba(16, 18, 32, 0.99);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 16px;
  padding: 22px 26px 18px 26px;
  box-shadow: 0 16px 50px rgba(0, 0, 0, 0.6);
}
.confirm-title { font-size: 19px; color: #f3ecd9; }
button.danger {
  background: rgba(255, 128, 104, 0.16);
  color: #ffd8cc;
  border: none;
  box-shadow: none;
  border-radius: 8px;
  padding: 3px 12px;
  font-size: 13px;
}
button.danger:hover { background: rgba(255, 128, 104, 0.3); }
button.trash {
  background: none;
  border: none;
  box-shadow: none;
  min-width: 26px;
  min-height: 26px;
  padding: 2px;
  color: rgba(220, 225, 240, 0.32);
  border-radius: 7px;
}
button.trash:hover { color: rgba(255, 190, 170, 0.95); background: rgba(255, 150, 130, 0.1); }
.sheet {
  background: rgba(10, 12, 24, 0.97);
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: 18px;
  box-shadow: 0 18px 60px rgba(0, 0, 0, 0.55);
}
.sheet .page { background: transparent; }
.sheet .book-side { border-top-left-radius: 18px; border-bottom-left-radius: 18px; }
.prompt {
  background: rgba(7, 9, 19, 0.78);
  border-radius: 16px;
  padding: 18px 22px 14px 22px;
  border: 1px solid rgba(255, 255, 255, 0.06);
}
.prompt-text { font-size: 17px; color: #eef0f6; }
.prompt.compact { padding: 10px 12px 8px 12px; border-radius: 12px; }
.prompt-aside { font-size: 13px; color: #f3dcae; }
.prompt-aside-more { font-size: 11px; color: rgba(243, 220, 174, 0.6); }
.prompt.compact .prompt-text { font-size: 14px; }
.prompt.compact entry { min-height: 30px; font-size: 14px; }
.prompt.compact .hint { font-size: 11px; }
.tonight.compact { padding: 8px 6px 6px 6px; border-radius: 11px; }
.tonight.compact button.find-row { padding: 2px 6px; }
.tonight.compact .find-name { font-size: 13px; }
.evening.compact { padding: 5px 9px 6px 9px; }
.prompt entry {
  background: rgba(255, 255, 255, 0.05);
  color: #f2f4f9;
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 9px;
  min-height: 38px;
  font-size: 15px;
  box-shadow: none;
  outline: none;
}
.prompt entry:focus-within { border-color: rgba(240, 214, 168, 0.55); }
button.chip {
  background: rgba(255, 255, 255, 0.06);
  border-radius: 16px;
  color: #dfe3ee;
  padding: 3px 13px;
  border: none;
  box-shadow: none;
  font-size: 14px;
}
button.chip:hover, button.chip:focus { background: rgba(240, 214, 168, 0.16); }
.chip-key {
  font-size: 11px;
  color: rgba(225, 230, 242, 0.55);
  border: 1px solid rgba(255, 255, 255, 0.2);
  border-bottom-width: 2px;
  border-radius: 5px;
  padding: 0 5px;
  min-width: 10px;
}
button.name { font-size: 13px; padding: 1px 10px; }
.hint { font-size: 12px; color: rgba(220, 225, 240, 0.48); }
.book-side { background: rgba(255, 255, 255, 0.025); }
.book-side row { padding: 7px 18px; color: #c9d0e2; font-size: 14px; }
.book-side row:selected { background: rgba(240, 214, 168, 0.12); color: #f6efdd; }
.book-side .section { font-size: 11px; color: rgba(220, 225, 240, 0.45); padding: 16px 18px 4px 18px; }
.book-title { font-size: 26px; color: #f3ecd9; }
.book-heading { font-size: 12px; color: rgba(210, 218, 240, 0.6); margin-top: 18px; letter-spacing: 1px; }
.book-body { font-size: 15px; color: #dfe3ee; }
.book-asked { font-size: 14px; color: rgba(210, 218, 240, 0.7); font-style: italic; }
.book-quiet { font-size: 13px; color: rgba(220, 225, 240, 0.5); }
.book-big { font-size: 21px; color: #f3ecd9; }
.menu-card {
  background: rgba(255, 255, 255, 0.035);
  border-radius: 12px;
  margin-top: 4px;
}
.menu-row { padding: 12px 18px; border-bottom: 1px solid rgba(255, 255, 255, 0.04); }
.menu-card > .menu-row:last-child { border-bottom: none; }
.menu-count { font-size: 15px; color: rgba(240, 214, 168, 0.85); min-width: 32px; }
.menu-pillar { font-size: 15px; color: #f3ecd9; font-weight: 600; }
.book-card {
  background: rgba(255, 255, 255, 0.035);
  border-radius: 12px;
  padding: 14px 18px;
}
button.quiet {
  background: rgba(255, 255, 255, 0.05);
  color: #d6dcec;
  border: none;
  box-shadow: none;
  border-radius: 8px;
  padding: 3px 12px;
  font-size: 13px;
}
button.quiet:hover { background: rgba(240, 214, 168, 0.14); }
button.menu-button {
  background: rgba(7, 9, 19, 0.6);
  border: 1px solid rgba(255, 255, 255, 0.07);
  border-radius: 10px;
  padding: 7px;
  color: rgba(236, 238, 246, 0.85);
  box-shadow: none;
}
button.menu-button:hover { background: rgba(240, 214, 168, 0.16); }
.tonight {
  background: rgba(7, 9, 19, 0.74);
  border-radius: 14px;
  padding: 12px 10px 10px 10px;
  border: 1px solid rgba(255, 255, 255, 0.06);
}
.evening {
  background: rgba(7, 9, 19, 0.6);
  border-radius: 12px;
  padding: 7px 12px 8px 12px;
  border: 1px solid rgba(255, 255, 255, 0.05);
}
.evening-step { font-size: 12px; color: rgba(220, 225, 240, 0.4); letter-spacing: 0.5px; }
.evening-step.done { color: rgba(240, 214, 168, 0.5); }
.evening-step.current { color: #f3dcae; font-weight: 600; }
.evening-dot { font-size: 12px; color: rgba(220, 225, 240, 0.25); }
button.evening-button {
  background: none;
  border: none;
  box-shadow: none;
  padding: 0 4px;
  min-height: 0;
  border-radius: 6px;
}
button.evening-button:hover { background: rgba(240, 214, 168, 0.14); color: #f3dcae; }
button.evening-button:disabled { color: rgba(220, 225, 240, 0.4); }
button.evening-button.current:disabled { color: #f3dcae; }
.evening-now { font-size: 12px; color: rgba(220, 225, 240, 0.62); transition: color 500ms ease-out; }
.evening-now.yours { color: #f3dcae; }
.evening-step { transition: color 400ms ease-out; }
.tonight-title { font-size: 11px; font-weight: 600; letter-spacing: 2px; color: rgba(240, 214, 168, 0.8); margin: 0 8px 4px 8px; }
.tonight-hint { font-size: 11px; color: rgba(220, 225, 240, 0.42); margin: 6px 8px 0 8px; }
button.find-row {
  background: none;
  border: none;
  box-shadow: none;
  border-radius: 9px;
  padding: 5px 8px;
}
button.find-row:hover { background: rgba(240, 214, 168, 0.1); }
button.find-row.found { opacity: 0.5; }
.find-mark { color: rgba(240, 214, 168, 0.9); font-size: 12px; margin-top: 2px; }
.find-name { font-size: 14px; color: #eef0f6; }
.find-where { font-size: 12px; color: rgba(210, 218, 240, 0.6); }
.course-step { font-size: 12px; color: rgba(210, 218, 240, 0.55); letter-spacing: 1px; }
.course-ask { font-size: 22px; color: #f3ecd9; }
.course-when { font-size: 16px; color: rgba(240, 222, 190, 0.9); font-style: italic; }
.course entry, .settings entry {
  background: rgba(255, 255, 255, 0.05);
  color: #f2f4f9;
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 9px;
  min-height: 40px;
  font-size: 16px;
}
"#;

thread_local! {
    /// Where "are you sure?" cards are shown: the window's top overlay.
    static CONFIRM_HOST: RefCell<Option<gtk::Overlay>> = const { RefCell::new(None) };
}

pub fn set_confirm_host(host: &gtk::Overlay) {
    CONFIRM_HOST.with(|h| *h.borrow_mut() = Some(host.clone()));
}

/// Asks before doing something that can't be undone, on a small card over
/// everything else. The detail says plainly what will happen; `then` runs
/// only if the second button is pressed. Esc, Cancel or a click outside
/// the card lets it go.
pub fn confirm(
    from: &impl IsA<gtk::Widget>,
    message: &str,
    detail: &str,
    yes: &str,
    then: impl Fn() + 'static,
) {
    let Some(host) = CONFIRM_HOST.with(|h| h.borrow().clone()) else {
        return;
    };
    let shade = gtk::Box::new(gtk::Orientation::Vertical, 0);
    shade.add_css_class("confirm-shade");
    let card = gtk::Box::new(gtk::Orientation::Vertical, 10);
    card.add_css_class("confirm");
    card.set_halign(gtk::Align::Center);
    card.set_valign(gtk::Align::Center);
    card.set_vexpand(true);
    card.set_width_request(460);
    let title = label(message, "confirm-title");
    title.set_max_width_chars(40);
    card.append(&title);
    for para in detail.split("\n\n") {
        let l = label(para, "book-body");
        l.set_max_width_chars(48);
        card.append(&l);
    }
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.set_halign(gtk::Align::End);
    buttons.set_margin_top(8);
    let cancel = gtk::Button::with_label("Cancel");
    cancel.add_css_class("quiet");
    let go = gtk::Button::with_label(yes);
    go.add_css_class("danger");
    buttons.append(&cancel);
    buttons.append(&go);
    card.append(&buttons);
    shade.append(&card);
    // Fades in, and out again before it's taken away.
    let wrap = fading(&shade);
    host.add_overlay(&wrap);
    {
        let wrap = wrap.clone();
        glib::idle_add_local_once(move || show(&wrap, true));
    }

    let from = from.clone().upcast::<gtk::Widget>();
    let close: Rc<dyn Fn()> = {
        let (host, wrap, from) = (host.clone(), wrap.clone(), from.clone());
        let closed = Cell::new(false);
        Rc::new(move || {
            if closed.replace(true) {
                return;
            }
            show(&wrap, false);
            let (host, wrap) = (host.clone(), wrap.clone());
            glib::timeout_add_local_once(std::time::Duration::from_millis(260), move || {
                host.remove_overlay(&wrap);
            });
            from.grab_focus();
        })
    };
    {
        let close = close.clone();
        cancel.connect_clicked(move |_| close());
    }
    {
        let close = close.clone();
        go.connect_clicked(move |_| {
            close();
            then();
        });
    }
    {
        let close = close.clone();
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        keys.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape {
                close();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        shade.add_controller(keys);
    }
    {
        let close = close.clone();
        let shade_ = shade.clone();
        let click = gtk::GestureClick::new();
        click.connect_released(move |_, _, x, y| {
            if shade_.pick(x, y, gtk::PickFlags::DEFAULT).as_ref() == Some(shade_.upcast_ref()) {
                close();
            }
        });
        shade.add_controller(click);
    }
    glib::idle_add_local_once(move || {
        cancel.grab_focus();
    });
}

/// A small bin button, for deleting one thing.
pub fn trash(tooltip: &str) -> gtk::Button {
    let b = gtk::Button::from_icon_name("user-trash-symbolic");
    b.add_css_class("trash");
    b.set_tooltip_text(Some(tooltip));
    b.set_valign(gtk::Align::Start);
    b.set_focus_on_click(false);
    b
}

/// A revealer that fades its child in and out, taking no clicks while
/// hidden.
pub fn fading(child: &impl IsA<gtk::Widget>) -> gtk::Revealer {
    let r = gtk::Revealer::new();
    r.set_transition_type(gtk::RevealerTransitionType::Crossfade);
    r.set_transition_duration(220);
    r.set_child(Some(child));
    r.set_reveal_child(false);
    r.set_can_target(false);
    r
}

/// Shows or hides a fading revealer.
pub fn show(r: &gtk::Revealer, on: bool) {
    if r.reveals_child() != on {
        r.set_reveal_child(on);
    }
    r.set_can_target(on);
}

pub fn install_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(CSS);
    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

pub fn wall_clock() -> i64 {
    glib::real_time() / 1000
}

/// Removes every child of a box.
pub fn clear(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}

/// Takes a widget out of whatever box it sits in, so it can go somewhere else.
pub fn detach(widget: &impl IsA<gtk::Widget>) {
    if let Some(parent) = widget.parent().and_downcast::<gtk::Box>() {
        parent.remove(widget);
    }
}

pub fn label(text: &str, class: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    l.add_css_class(class);
    l.set_wrap(true);
    l.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    l.set_xalign(0.0);
    l.set_selectable(false);
    l.set_max_width_chars(72);
    l
}

pub struct PromptBar {
    /// Fades the prompt in and out.
    pub root: gtk::Revealer,
    panel: gtk::Box,
    /// What the wisp is saying, in a small window, where its bubble would
    /// sit behind the prompt.
    aside: gtk::Label,
    aside_more: gtk::Label,
    aside_box: gtk::Box,
    text: gtk::Label,
    /// The choices, in a row, or stacked in a small window. A plain box:
    /// a flow box's height-for-width could leave the hint drawn over it.
    chips: gtk::Box,
    entry: gtk::Entry,
    names: gtk::Box,
    hint: gtk::Label,
    serial: Cell<u64>,
    showing: RefCell<Option<Prompt>>,
}

impl PromptBar {
    pub fn new(game: &Rc<RefCell<Game>>) -> Rc<PromptBar> {
        let panel = gtk::Box::new(gtk::Orientation::Vertical, 10);
        panel.add_css_class("prompt");
        panel.set_width_request(560);
        let root = fading(&panel);
        root.set_halign(gtk::Align::Center);
        root.set_valign(gtk::Align::End);
        root.set_margin_bottom(84);
        let text = label("", "prompt-text");
        text.set_max_width_chars(60);
        let aside = label("", "prompt-aside");
        let aside_more = label("Click here for more", "prompt-aside-more");
        let aside_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
        aside_box.append(&aside);
        aside_box.append(&aside_more);
        aside_box.set_visible(false);
        {
            let game = game.clone();
            let click = gtk::GestureClick::new();
            click.connect_released(move |_, _, _, _| {
                game.borrow_mut().guide_next(wall_clock());
            });
            aside_box.add_controller(click);
        }
        panel.append(&aside_box);
        let chips = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let entry = gtk::Entry::new();
        let names = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let hint = label("", "hint");
        panel.append(&text);
        panel.append(&entry);
        panel.append(&names);
        panel.append(&chips);
        panel.append(&hint);
        let bar = Rc::new(PromptBar {
            root,
            panel: panel.clone(),
            aside,
            aside_more,
            aside_box,
            text,
            chips,
            entry,
            names,
            hint,
            serial: Cell::new(0),
            showing: RefCell::new(None),
        });
        {
            let game = game.clone();
            bar.entry.connect_activate(move |entry| {
                let text = entry.text().to_string();
                entry.set_text("");
                game.borrow_mut().answer_text(&text, wall_clock());
            });
        }
        {
            let game = game.clone();
            let weak = Rc::downgrade(&bar);
            let keys = gtk::EventControllerKey::new();
            keys.set_propagation_phase(gtk::PropagationPhase::Capture);
            keys.connect_key_pressed(move |_, key, _, _| {
                if key == gdk::Key::Escape {
                    game.borrow_mut().skip_prompt(wall_clock());
                    return glib::Propagation::Stop;
                }
                // With nothing typed yet, a number picks a chip.
                if let Some(bar) = weak.upgrade()
                    && bar.entry.text().is_empty()
                    && let Some(n) = key.to_unicode().and_then(|c| c.to_digit(10))
                {
                    let chips = bar.showing.borrow().as_ref().map_or(0, |p| p.chips.len());
                    if (1..=chips as u32).contains(&n) {
                        game.borrow_mut().answer_chip(n as usize - 1, wall_clock());
                        return glib::Propagation::Stop;
                    }
                }
                glib::Propagation::Proceed
            });
            bar.entry.add_controller(keys);
        }
        {
            let game = game.clone();
            let weak = Rc::downgrade(&bar);
            bar.entry.connect_changed(move |entry| {
                if let Ok(mut g) = game.try_borrow_mut() {
                    g.typed(&entry.text(), wall_clock());
                }
                if let Some(bar) = weak.upgrade() {
                    bar.suggest(&game, &entry.text());
                }
            });
        }
        bar
    }

    /// Shows what the wisp is saying, when it goes in the prompt.
    pub fn aside(&self, game: &Rc<RefCell<Game>>) {
        let said = game.borrow().aside(wall_clock());
        match said {
            Some((text, more)) => {
                if self.aside.text() != text {
                    self.aside.set_text(&text);
                }
                self.aside_more.set_visible(more);
                self.aside_box.set_visible(true);
            }
            None => self.aside_box.set_visible(false),
        }
    }

    /// Fits the prompt to the window: narrower and tighter in a small one,
    /// sitting just above the compass strip there.
    pub fn fit(&self, width: f64, height: f64, compact: bool) {
        let w = (width - 24.0).clamp(200.0, 560.0);
        self.panel.set_width_request(w as i32);
        self.text.set_max_width_chars(if compact { 40 } else { 60 });
        self.chips.set_orientation(if compact {
            gtk::Orientation::Vertical
        } else {
            gtk::Orientation::Horizontal
        });
        if compact {
            self.panel.add_css_class("compact");
        } else {
            self.panel.remove_css_class("compact");
        }
        let bottom = if compact {
            46
        } else if height < 700.0 {
            56
        } else {
            84
        };
        self.root.set_margin_bottom(bottom);
    }

    fn suggest(&self, game: &Rc<RefCell<Game>>, typed: &str) {
        clear(&self.names);
        let wants = self.showing.borrow().as_ref().is_some_and(|p| p.names);
        if !wants {
            return;
        }
        let Ok(g) = game.try_borrow() else { return };
        for name in g.names_like(typed) {
            let b = gtk::Button::with_label(&name);
            b.add_css_class("chip");
            b.add_css_class("name");
            let entry = self.entry.clone();
            b.connect_clicked(move |_| {
                entry.set_text(&name);
                entry.set_position(-1);
                entry.grab_focus();
            });
            self.names.append(&b);
        }
    }

    /// Shows the game's current prompt, if it has changed. Returns true when
    /// the prompt went away, so the sky can take the keyboard back.
    pub fn sync(&self, game: &Rc<RefCell<Game>>) -> bool {
        let (serial, prompt) = game.borrow().prompt();
        if serial == self.serial.get() {
            return false;
        }
        self.serial.set(serial);
        clear(&self.names);
        while let Some(child) = self.chips.first_child() {
            self.chips.remove(&child);
        }
        let Some(p) = prompt else {
            show(&self.root, false);
            *self.showing.borrow_mut() = None;
            return true;
        };
        self.text.set_text(&p.text);
        self.entry.set_visible(p.entry);
        self.entry.set_text("");
        self.entry.set_placeholder_text(Some(&p.placeholder));
        for (i, chip) in p.chips.iter().enumerate() {
            // The words, then its key drawn as a small key, so the number
            // doesn't read as part of the words. Beside a box to write in
            // a number would be typed into it, so there's none.
            let inside = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            inside.append(&gtk::Label::new(Some(chip)));
            let key_name = match p.chip_keys.get(i) {
                Some(k) => Some(k.clone()),
                None => (!p.entry).then(|| (i + 1).to_string()),
            };
            if let Some(key_name) = key_name {
                let key = gtk::Label::new(Some(&key_name));
                key.add_css_class("chip-key");
                key.set_valign(gtk::Align::Center);
                key.set_tooltip_text(Some(&format!("Press {key_name} for this")));
                inside.append(&key);
            }
            let b = gtk::Button::new();
            b.set_child(Some(&inside));
            b.add_css_class("chip");
            let game = game.clone();
            b.connect_clicked(move |_| game.borrow_mut().answer_chip(i, wall_clock()));
            self.chips.append(&b);
        }
        self.chips.set_visible(!p.chips.is_empty());
        let hint = match (p.entry, p.chips.len()) {
            (false, n) if n > 0 && p.hint.is_empty() => {
                "Click one, or press the key beside it".into()
            }
            _ => p.hint.clone(),
        };
        self.hint.set_text(&hint);
        self.hint.set_visible(!hint.is_empty());
        show(&self.root, true);
        if p.entry {
            self.entry.grab_focus();
        }
        *self.showing.borrow_mut() = Some(p);
        false
    }
}
