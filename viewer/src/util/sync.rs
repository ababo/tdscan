use std::cell::Cell;

use base::defs::{Error, ErrorKind::*, Result};

pub struct Mutex {
    locked: Cell<bool>,
}

impl Mutex {
    pub fn new() -> Mutex {
        Mutex {
            locked: Cell::new(false),
        }
    }
}

impl Mutex {
    pub fn try_lock<'a>(&'a self) -> Result<MutexGuard<'a>> {
        if self.locked.get() {
            return Err(Error::new(BadOperation, format!("currently busy")));
        }

        self.locked.set(true);

        Ok(MutexGuard { mutex: self })
    }
}

#[must_use = "if unused the Mutex will immediately unlock"]
pub struct MutexGuard<'a> {
    mutex: &'a Mutex,
}

impl<'a> Drop for MutexGuard<'a> {
    fn drop(&mut self) {
        self.mutex.locked.set(false);
    }
}
