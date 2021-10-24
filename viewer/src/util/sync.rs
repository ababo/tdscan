use std::cell::Cell;

use base::defs::{Error, ErrorKind::*, Result};

pub struct LevelLock<L: Copy + Ord> {
    level: Cell<L>,
}

impl<L: Copy + Ord> LevelLock<L> {
    pub fn new(level: L) -> Self {
        Self {
            level: Cell::new(level),
        }
    }

    pub fn try_lock(&self, level: L) -> Result<LevelGuard<'_, L>> {
        if level > self.level.get() {
            Ok(LevelGuard {
                lock: self,
                level: self.level.replace(level),
            })
        } else {
            Err(Error::new(BadOperation, "currently busy".to_string()))
        }
    }
}

#[must_use = "if unused the LevelLock will immediately unlock"]
pub struct LevelGuard<'a, L: Copy + Ord> {
    level: L,
    lock: &'a LevelLock<L>,
}

impl<'a, L: Copy + Ord> Drop for LevelGuard<'a, L> {
    fn drop(&mut self) {
        self.lock.level.set(self.level);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_level_lock() {
        let lock = LevelLock::new(0);
        assert!(lock.try_lock(0).is_err());
        {
            let guard = lock.try_lock(1);
            assert!(guard.is_ok());
            assert!(lock.try_lock(1).is_err());
            assert!(lock.try_lock(2).is_ok());
        }
        println!("{:?}", lock.level);
        assert!(lock.try_lock(1).is_ok());
    }
}
