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

pub fn new_element_view_rec(view: model::ElementView) -> model::Record {
    model::Record {
        r#type: Some(model::record::Type::ElementView(view)),
    }
}

pub fn new_element_view_state_rec(
    view_state: model::ElementViewState,
) -> model::Record {
    use model::record::Type;
    model::Record {
        r#type: Some(Type::ElementViewState(view_state)),
    }
}

pub fn new_ev_face(
    vertex1: u32,
    vertex2: u32,
    vertex3: u32,
    texture1: u32,
    texture2: u32,
    texture3: u32,
    normal1: u32,
    normal2: u32,
    normal3: u32,
) -> model::element_view::Face {
    model::element_view::Face {
        vertex1,
        vertex2,
        vertex3,
        texture1,
        texture2,
        texture3,
        normal1,
        normal2,
        normal3,
    }
}

pub fn new_point2(x: f32, y: f32) -> model::Point2 {
    model::Point2 { x, y }
}

pub fn new_point3(x: f32, y: f32, z: f32) -> model::Point3 {
    model::Point3 { x, y, z }
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

#[macro_export]
macro_rules! assert_eq_f32 {
    ($a:expr, $b:expr) => {{
        float_cmp::approx_eq!(f32, $a, $b, epsilon = 0.000001);
    }};
}

#[macro_export]
macro_rules! assert_eq_point2 {
    ($a:expr, $b:expr) => {{
        base::assert_eq_f32!($a.x, $b.x);
        base::assert_eq_f32!($a.y, $b.y);
    }};
}

#[macro_export]
macro_rules! assert_eq_point3 {
    ($a:expr, $b:expr) => {{
        base::assert_eq_f32!($a.x, $b.x);
        base::assert_eq_f32!($a.y, $b.y);
        base::assert_eq_f32!($a.z, $b.z);
    }};
}
