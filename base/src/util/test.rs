use crate::model;

pub struct MethodMock<Args, Ret> {
    pub args: Vec<Args>,
    pub rets: Vec<Ret>,
}

impl<Args, Ret> MethodMock<Args, Ret> {
    pub fn new() -> Self {
        MethodMock {
            args: vec![],
            rets: vec![],
        }
    }

    pub fn call(&mut self, args: Args) -> Ret {
        assert!(!self.rets.is_empty());
        self.args.push(args);
        self.rets.pop().unwrap()
    }

    pub fn finish(&self) {
        assert!(self.args.is_empty());
        assert!(self.rets.is_empty());
    }
}

#[macro_export]
macro_rules! record_variant {
    ($variant:path, $record:expr) => {
        match $record.r#type.unwrap() {
            $variant(val) => Some(val),
            _ => None,
        }
        .unwrap()
    };
}

pub fn new_point2(x: f32, y: f32) -> model::Point2 {
    model::Point2 { x, y }
}

pub fn new_point3(x: f32, y: f32, z: f32) -> model::Point3 {
    model::Point3 { x, y, z }
}

pub fn new_ev_face(
    vertex1: u32,
    vertex2: u32,
    vertex3: u32,
    texture1: u32,
    texture2: u32,
    texture3: u32,
) -> model::element_view::Face {
    model::element_view::Face {
        vertex1,
        vertex2,
        vertex3,
        texture1,
        texture2,
        texture3,
    }
}
