use base::defs::Result;
use base::model;

pub trait Adapter {}

pub struct Controller<A: Adapter> {
    adapter: A,
}

impl<A: Adapter> Controller<A> {
    pub fn new(adapter: A) -> Self {
        Self { adapter }
    }

    pub fn clear(&mut self) {}

    pub fn add_record(&mut self, _record: model::Record) -> Result<()> {
        Ok(())
    }
}
