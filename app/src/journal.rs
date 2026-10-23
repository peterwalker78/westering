//! The journal as the window's pages see it: reading it, wiping it,
//! restoring a backup, and the names kept in it.

use crate::game::Game;
use westering_core::journal::Journal;

impl Game {
    pub fn journal(&self) -> &Journal {
        &self.journal
    }

    pub fn journal_mut(&mut self) -> &mut Journal {
        &mut self.journal
    }

    pub fn night(&self) -> &str {
        &self.night
    }

    /// Clears the journal and tonight's page with it.
    pub fn forget_everything(&mut self) -> std::io::Result<()> {
        self.journal.forget_everything()?;
        self.refresh();
        Ok(())
    }

    /// Picks up the journal again after something in it was deleted or
    /// restored: tonight's page, and what's been found tonight.
    pub fn refresh(&mut self) {
        self.page =
            self.journal
                .night(&self.night)
                .unwrap_or_else(|| westering_core::journal::Night {
                    key: self.night.clone(),
                    ..Default::default()
                });
        let night = self.night.clone();
        for f in &mut self.finds {
            f.caught = self.journal.found_on(&f.id, &night);
        }
    }

    /// Replaces everything kept with a backup.
    pub fn restore(&mut self, text: &str) -> std::io::Result<usize> {
        let n = self.journal.restore(text)?;
        self.refresh();
        Ok(n)
    }

    /// A star's everyday name, if it has one.
    pub fn star_name(&self, hr: u16) -> Option<String> {
        self.sky.lists.star_name(hr).map(|s| s.name.clone())
    }

    /// Names already used that begin with what's typed.
    pub fn names_like(&self, prefix: &str) -> Vec<String> {
        self.journal
            .names_like(prefix)
            .into_iter()
            .filter(|n| !n.eq_ignore_ascii_case(prefix.trim()))
            .take(4)
            .map(str::to_owned)
            .collect()
    }
}
