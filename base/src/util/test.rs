use crate::fm;

use crate::fm::Write as _;
use std::io;

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
        assert!(($a - $b).abs() < 0.000001);
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

pub fn create_reader_with_records(
    records: &Vec<fm::Record>,
) -> fm::Reader<io::Cursor<Vec<u8>>> {
    let mut writer = create_writer();

    for rec in records {
        writer.write_record(rec).unwrap();
    }

    writer_to_reader(writer)
}

pub fn create_writer() -> fm::Writer<Vec<u8>> {
    fm::Writer::new(Vec::new(), &fm::WriterParams::default()).unwrap()
}

pub fn new_element_view_rec(view: fm::ElementView) -> fm::Record {
    fm::Record {
        r#type: Some(fm::record::Type::ElementView(view)),
    }
}

pub fn new_element_view_state_rec(
    view_state: fm::ElementViewState,
) -> fm::Record {
    use fm::record::Type;
    fm::Record {
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
) -> fm::element_view::Face {
    fm::element_view::Face {
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

pub fn new_point2(x: f32, y: f32) -> fm::Point2 {
    fm::Point2 { x, y }
}

pub fn new_point3(x: f32, y: f32, z: f32) -> fm::Point3 {
    fm::Point3 { x, y, z }
}

pub fn writer_to_reader(
    writer: fm::Writer<Vec<u8>>,
) -> fm::Reader<io::Cursor<Vec<u8>>> {
    let data = writer.into_inner().unwrap();
    fm::Reader::new(io::Cursor::new(data)).unwrap()
}
