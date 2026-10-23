//! Going from page to page: what a page asks the window to open.

use crate::game::Game;
use std::cell::RefCell;

/// How a page asks the window to go somewhere: None is back to the sky.
pub type Go = RefCell<Option<Box<dyn Fn(Option<Request>)>>>;

/// Something for the window to open.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    /// The logbook, at a night's page or a contents page.
    Book(Option<String>),
    /// Chart a course for a weight.
    Course(u32),
    Settings,
}

impl Game {
    pub fn take_request(&mut self) -> Option<Request> {
        self.request.take()
    }
}
