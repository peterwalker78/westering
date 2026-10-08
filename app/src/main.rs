//! Westering: the real sky over you tonight, a few quiet questions, and then
//! the real sky outside.

mod book;
mod camera;
mod course;
mod day;
mod drawing;
mod eyepiece;
mod field;
mod flight;
mod game;
mod guide;
mod hover;
mod hud;
mod idle;
mod keep;
mod life;
mod music;
mod settings;
mod sprite;
mod talk;
mod tour;
mod ui;
mod view;
mod wind;

use book::Book;
use course::Course;
use game::{Clock, Game, Options};
use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use settings::Settings;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use talk::Request;
use ui::PromptBar;
use view::SkyView;
use westering_core::coords::Observer;
use westering_core::journal::Journal;
use westering_core::place;
use westering_core::session::Timings;
use westering_core::time::UnixMs;

const APP_ID: &str = "io.github.peterwalker78.Westering";

/// Options for trying the app out: `--at=2026-12-14T21:00` starts the sky at
/// another moment, `--speed=N` runs it N times faster, `--quick=N` shortens
/// the visit's own timings N times, `--data=DIR` keeps everything in DIR and
/// `--place=LAT,LON` stands somewhere else, and `--profile` prints what
/// frames cost.
#[derive(Default)]
struct Args {
    at: Option<String>,
    speed: Option<f64>,
    quick: Option<i64>,
    data: Option<PathBuf>,
    place: Option<(f64, f64)>,
    /// Print how long frames take to make, every few seconds.
    profile: bool,
}

fn parse_args() -> Args {
    let mut args = Args::default();
    for arg in std::env::args().skip(1) {
        let (key, value) = arg.split_once('=').unwrap_or((arg.as_str(), ""));
        match key {
            "--at" => args.at = Some(value.to_owned()),
            "--speed" => args.speed = value.parse().ok(),
            "--quick" => args.quick = value.parse().ok(),
            "--data" => args.data = Some(PathBuf::from(value)),
            "--profile" => args.profile = true,
            "--place" => {
                args.place = value
                    .split_once(',')
                    .and_then(|(a, b)| Some((a.trim().parse().ok()?, b.trim().parse().ok()?)))
            }
            _ => eprintln!("westering: ignoring {arg}"),
        }
    }
    args
}

pub fn wall_clock() -> UnixMs {
    glib::real_time() / 1000
}

fn utc_offset(at: UnixMs) -> i32 {
    glib::DateTime::from_unix_local(at / 1000)
        .map(|t| t.utc_offset().as_seconds() as i32)
        .unwrap_or(0)
}

fn parse_moment(text: &str) -> Option<UnixMs> {
    let local = glib::TimeZone::local();
    let with_seconds = if text.len() == 16 {
        format!("{text}:00")
    } else {
        text.to_owned()
    };
    glib::DateTime::from_iso8601(&with_seconds, Some(&local))
        .ok()
        .map(|t| t.to_unix() * 1000)
}

fn data_dir(args: &Args) -> PathBuf {
    if let Some(dir) = &args.data {
        return dir.clone();
    }
    let dir = glib::user_data_dir().join("westering");
    // Westering was called Night Sky: bring its logbook across the first time.
    let earlier = [
        glib::home_dir().join(".var/app/io.github.peterwalker78.NightSky/data/night-sky"),
        glib::home_dir().join(".local/share/night-sky"),
    ];
    for old in earlier {
        match Journal::adopt(&old, &dir) {
            Ok(true) => {
                eprintln!(
                    "westering: brought the logbook across from {}",
                    old.display()
                );
                break;
            }
            Ok(false) => {}
            Err(e) => eprintln!("westering: couldn't bring the old logbook across: {e}"),
        }
    }
    dir
}

fn build(app: &gtk::Application, args: &Rc<Args>) {
    if let Some(settings) = gtk::Settings::default() {
        settings.set_gtk_interface_color_scheme(gtk::InterfaceColorScheme::Dark);
    }
    let real = wall_clock();
    let sky0 = args.at.as_deref().and_then(parse_moment).unwrap_or(real);
    let journal = Journal::open(&data_dir(args));
    let observer = args
        .place
        .map(|(lat, lon)| Observer { lat, lon })
        .or_else(|| {
            let s = &journal.settings;
            Some(Observer {
                lat: s.lat?,
                lon: s.lon?,
            })
        })
        .or_else(|| place::here(glib::TimeZone::local().identifier().as_str().into()))
        .unwrap_or(Observer {
            lat: 51.48,
            lon: 0.0,
        });
    let timings = args.quick.map(Timings::quick).unwrap_or(Timings::STANDARD);
    let options = Options {
        clock: Clock {
            real0: real,
            sky0,
            speed: args.speed.unwrap_or(1.0),
        },
        observer,
        offset_s: utc_offset(sky0),
        timings,
        journal,
    };
    let game = Rc::new(RefCell::new(Game::new(options, real)));

    ui::install_css();
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("Westering")
        .default_width(1280)
        .default_height(800)
        .decorated(false)
        .build();
    window.maximize();

    let view = SkyView::default();
    view.set_hexpand(true);
    view.set_vexpand(true);
    view.set_focusable(true);
    view.set_cursor_from_name(Some("crosshair"));
    let prompt = PromptBar::new(&game);
    let sky_page = gtk::Overlay::new();
    sky_page.set_child(Some(&view));
    sky_page.add_overlay(&prompt.root);
    let tonight = hud::Tonight::new();
    sky_page.add_overlay(&tonight.root);
    let evening = hud::EveningGuide::new(&game);
    sky_page.add_overlay(&evening.root);
    let menu = gtk::Button::from_icon_name("open-menu-symbolic");
    menu.set_tooltip_text(Some("Menu (Ctrl+,)"));
    menu.add_css_class("menu-button");
    menu.set_halign(gtk::Align::Start);
    menu.set_valign(gtk::Align::Start);
    menu.set_margin_start(14);
    menu.set_margin_top(12);
    menu.set_focus_on_click(false);
    sky_page.add_overlay(&menu);

    let book = Book::new(&game);
    let course = Course::new(&game);
    let settings = Settings::new(&game);
    // The logbook, a course and the menu open as a panel over the sky,
    // which carries on turning, dimmed, behind it.
    let stack = gtk::Stack::new();
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    stack.set_transition_duration(250);
    stack.add_named(&book.root, Some("book"));
    stack.add_named(&course.root, Some("course"));
    stack.add_named(&settings.root, Some("settings"));
    let panel = gtk::Box::new(gtk::Orientation::Vertical, 0);
    panel.add_css_class("sheet");
    panel.set_overflow(gtk::Overflow::Hidden);
    panel.set_halign(gtk::Align::Center);
    panel.set_width_request(1060);
    panel.set_margin_top(36);
    panel.set_margin_bottom(36);
    stack.set_vexpand(true);
    panel.append(&stack);
    let veil = gtk::Box::new(gtk::Orientation::Vertical, 0);
    veil.add_css_class("veil");
    let over = gtk::Overlay::new();
    over.set_child(Some(&veil));
    over.add_overlay(&panel);
    let sheet = gtk::Revealer::new();
    sheet.set_transition_type(gtk::RevealerTransitionType::Crossfade);
    sheet.set_transition_duration(450);
    sheet.set_child(Some(&over));
    sheet.set_can_target(false);
    sky_page.add_overlay(&sheet);
    ui::set_confirm_host(&sky_page);
    window.set_child(Some(&sky_page));

    // Going from page to page. The pages ask for this with a Request.
    let go: Rc<dyn Fn(Option<Request>)> = {
        let (stack, sheet, view) = (stack.clone(), sheet.clone(), view.clone());
        let (book, course, settings) = (book.clone(), course.clone(), settings.clone());
        Rc::new(move |r: Option<Request>| {
            let open = r.is_some();
            match r {
                None => {
                    view.grab_focus();
                }
                Some(Request::Book(at)) => {
                    stack.set_visible_child_name("book");
                    book.open(at.as_deref());
                }
                Some(Request::Course(weight)) => {
                    stack.set_visible_child_name("course");
                    course.open(weight);
                }
                Some(Request::Settings) => {
                    stack.set_visible_child_name("settings");
                    settings.open();
                }
            }
            sheet.set_reveal_child(open);
            sheet.set_can_target(open);
        })
    };
    {
        // A click on the sky around the panel puts it away.
        let go = go.clone();
        let click = gtk::GestureClick::new();
        click.connect_released(move |_, _, _, _| go(None));
        veil.add_controller(click);
    }
    {
        let go = go.clone();
        menu.connect_clicked(move |_| go(Some(Request::Settings)));
    }
    for slot in [&book.on_request, &course.on_request, &settings.on_request] {
        let go = go.clone();
        *slot.borrow_mut() = Some(Box::new(move |r| go(r)));
    }

    // Keys reach the sky before GTK's own arrow-key focus navigation can
    // claim them; anything the sky doesn't want goes on to the focused widget.
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    {
        let game = game.clone();
        let window = window.clone();
        let sheet = sheet.clone();
        let go = go.clone();
        keys.connect_key_pressed(move |_, key, _, state| {
            let ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
            if ctrl && matches!(key, gdk::Key::q | gdk::Key::w) {
                window.close();
                return glib::Propagation::Stop;
            }
            if ctrl && key == gdk::Key::comma {
                go(Some(Request::Settings));
                return glib::Propagation::Stop;
            }
            if key == gdk::Key::F11 {
                if window.is_fullscreen() {
                    window.unfullscreen();
                } else {
                    window.fullscreen();
                }
                return glib::Propagation::Stop;
            }
            if sheet.reveals_child() {
                return glib::Propagation::Proceed;
            }
            let handled = game.borrow_mut().key_pressed(key, wall_clock());
            let request = game.borrow_mut().take_request();
            if request.is_some() {
                go(request);
            }
            if handled {
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
    }
    {
        let game = game.clone();
        keys.connect_key_released(move |_, key, _, _| {
            game.borrow_mut().key_released(key, wall_clock());
        });
    }
    window.add_controller(keys);

    let drag = gtk::GestureDrag::new();
    {
        let game = game.clone();
        drag.connect_drag_begin(move |_, x, y| game.borrow_mut().drag_begin(x, y, wall_clock()));
    }
    {
        let game = game.clone();
        drag.connect_drag_update(move |_, dx, dy| {
            game.borrow_mut().drag_update(dx, dy, wall_clock())
        });
    }
    {
        let game = game.clone();
        drag.connect_drag_end(move |gesture, dx, dy| {
            // A press held on the ring was a catch, not a click.
            let held = game.borrow_mut().drag_end();
            if !held
                && dx.abs() < 3.0
                && dy.abs() < 3.0
                && let Some((x, y)) = gesture.start_point()
            {
                game.borrow_mut().click(x, y, wall_clock());
            }
        });
    }
    view.add_controller(drag);

    // The pointer over the sky names what it's near.
    let motion = gtk::EventControllerMotion::new();
    {
        let game = game.clone();
        let view = view.clone();
        motion.connect_motion(move |_, x, y| {
            let real = wall_clock();
            game.borrow_mut().pointer_moved(x, y, real);
            // A hand over something a click would tell more about.
            let hand = game.borrow().clickable_at(x, y, real);
            view.set_cursor_from_name(Some(if hand { "pointer" } else { "crosshair" }));
        });
    }
    {
        let game = game.clone();
        motion.connect_leave(move |_| game.borrow_mut().pointer_left());
    }
    view.add_controller(motion);

    let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
    {
        let game = game.clone();
        scroll.connect_scroll(move |_, _, dy| {
            game.borrow_mut().scroll(dy, wall_clock());
            glib::Propagation::Stop
        });
    }
    view.add_controller(scroll);

    // A different track to start with every time.
    let music = Rc::new(RefCell::new(music::Music::new(
        glib::random_int() as u64,
        game.borrow().quiet(),
        game.borrow().style(),
    )));
    game.borrow_mut().styles = music.borrow().styles();
    let last_frame = Rc::new(std::cell::Cell::new(0i64));
    let last_music = Rc::new(std::cell::Cell::new(real));
    let last_list = std::cell::Cell::new(0i64);
    let last_size = std::cell::Cell::new((0, 0));
    let spent = std::cell::Cell::new((0.0f64, 0u32, real));
    let profile = args.profile;
    // Keeping company while the window is out of sight, frames stop: a slow
    // timer keeps the music going and lets the wisp look in. Otherwise it
    // does nothing.
    {
        let (game, music) = (game.clone(), music.clone());
        let (last_frame, last_music) = (last_frame.clone(), last_music.clone());
        glib::timeout_add_local(std::time::Duration::from_secs(1), move || {
            let real = wall_clock();
            if real - last_frame.get() > 1_500
                && let Ok(mut g) = game.try_borrow_mut()
                && g.keeping()
            {
                g.company_tick(real, false);
                let level = g.music_level(real) * g.volume();
                drop(g);
                let dt = (real - last_music.get()).clamp(0, 2_000) as f64 / 1000.0;
                last_music.set(real);
                music.borrow_mut().tick(level, dt);
            }
            glib::ControlFlow::Continue
        });
    }
    {
        let game = game.clone();
        let window = window.clone();
        let sheet = sheet.clone();
        let go = go.clone();
        view.add_tick_callback(move |view, _clock| {
            let real = wall_clock();
            let pace = game.borrow().frame_ms(real);
            let on_sky = !sheet.reveals_child();
            // Out of focus it goes on smoothly but a little slower; the
            // sky itself is only redrawn now and then (see Game::tick).
            let interval = if !window.is_active() {
                pace.max(40)
            } else if !on_sky {
                // Still turning behind the panel, gently.
                100
            } else {
                pace
            };
            let urgent = game.borrow().urgent;
            if urgent || real - last_frame.get() >= interval {
                last_frame.set(real);
                let mut g = game.borrow_mut();
                g.urgent = false;
                g.focused = window.is_active();
                g.panel_open = !on_sky;
                // The panels over the sky fit the window, down to a small tile.
                let size = (view.width(), view.height());
                if size != last_size.get() && size.0 > 0 {
                    last_size.set(size);
                    let compact = crate::game::compact(size.0 as f64, size.1 as f64);
                    prompt.fit(size.0 as f64, size.1 as f64, compact);
                    tonight.fit(size.1 as f64, compact);
                    evening.fit(compact);
                }
                g.playing = music.borrow().playing();
                g.resize(
                    view.width() as f64,
                    view.height() as f64,
                    view.scale_factor() as f64,
                );
                let began = std::time::Instant::now();
                let frame = g.tick(real);
                if profile {
                    let (sum, n, since) = spent.get();
                    let sum = sum + began.elapsed().as_secs_f64() * 1000.0;
                    if real - since > 5_000 {
                        eprintln!(
                            "westering: {:.2} ms a frame, {:.1} frames a second",
                            sum / (n + 1) as f64,
                            (n + 1) as f64 / ((real - since) as f64 / 1000.0)
                        );
                        spent.set((0.0, 0, real));
                    } else {
                        spent.set((sum, n + 1, since));
                    }
                }
                let quit = g.quit;
                let request = g.take_request();
                let (level, quiet) = (g.music_level(real) * g.volume(), g.quiet());
                drop(g);
                {
                    let mut m = music.borrow_mut();
                    m.quiet = quiet;
                    m.want(game.borrow().style());
                    m.settle(game.borrow().settling());
                    let dt = (real - last_music.get()).clamp(0, 500) as f64 / 1000.0;
                    last_music.set(real);
                    m.tick(level, dt);
                }
                view.show(frame);
                if on_sky && real - last_list.get() > 700 {
                    last_list.set(real);
                    tonight.sync(&game);
                    evening.sync(&game);
                }
                prompt.aside(&game);
                if prompt.sync(&game) && on_sky {
                    view.grab_focus();
                }
                if request.is_some() {
                    go(request);
                }
                if quit {
                    window.close();
                }
            }
            glib::ControlFlow::Continue
        });
    }

    window.present();
    view.grab_focus();
}

fn main() -> glib::ExitCode {
    let args = Rc::new(parse_args());
    let mut flags = gio::ApplicationFlags::empty();
    if args.data.is_some() || args.at.is_some() {
        flags |= gio::ApplicationFlags::NON_UNIQUE;
    }
    let app = gtk::Application::builder()
        .application_id(APP_ID)
        .flags(flags)
        .build();
    app.connect_activate(move |app| {
        if let Some(window) = app.active_window() {
            window.present();
        } else {
            build(app, &args);
        }
    });
    let program: Vec<String> = std::env::args().take(1).collect();
    app.run_with_args(&program)
}
