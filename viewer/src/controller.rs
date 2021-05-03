use std::collections::{BTreeMap, HashMap};

use arrayvec::ArrayVec;

use base::defs::{Error, ErrorKind::*, Result};
use base::model;

pub trait Adapter {
    fn set_texture(&mut self, index: usize, image: model::Image) -> Result<()>;
}

#[derive(Default)]
pub struct Vertex {
    position: model::Point3,
    normal: model::Point3,
    texture: model::Point2,
}

pub struct Face {
    vertex1: u16,
    vertex2: u16,
    vertex3: u16,
}

pub struct State {
    element: String,
    vertices: Vec<model::Point3>,
    normals: Vec<model::Point3>,
}

pub type Time = i64;

pub struct Controller<A: Adapter> {
    adapter: A,
    indices: HashMap<String, u16>,
    vertices: Vec<Vertex>,
    faces: Vec<Face>,
    states: BTreeMap<Time, Vec<State>>,
}

impl<A: Adapter> Controller<A> {
    pub fn new(adapter: A) -> Self {
        Self {
            adapter,
            indices: HashMap::new(),
            vertices: vec![],
            faces: vec![],
            states: BTreeMap::new(),
        }
    }

    pub fn clear(&mut self) {}

    pub fn add_record(&mut self, record: model::Record) -> Result<()> {
        use model::record::Type::*;
        match record.r#type {
            Some(ElementView(v)) => self.add_element_view(v)?,
            Some(ElementViewState(s)) => self.add_element_view_state(s)?,
            _ => (),
        }
        Ok(())
    }

    fn add_element_view(&mut self, view: model::ElementView) -> Result<()> {
        if !self.states.is_empty() {
            let desc = format!(
                "view for element '{}' after element view states",
                view.element
            );
            return Err(Error::new(InconsistentState, desc));
        }

        if self.indices.contains_key(&view.element) {
            let desc = format!("duplicate view for element '{}'", view.element);
            return Err(Error::new(InconsistentState, desc));
        }

        if view.faces.is_empty() {
            let desc =
                format!("no faces in view for element '{}'", view.element);
            return Err(Error::new(InconsistentState, desc));
        }

        let num_vertices = self.vertices.len() as u16;
        self.indices.insert(view.element.clone(), num_vertices);

        self.extend_vertices(&view)?;

        for face in view.faces {
            self.faces.push(Face {
                vertex1: num_vertices + face.vertex1 as u16 - 1,
                vertex2: num_vertices + face.vertex2 as u16 - 1,
                vertex3: num_vertices + face.vertex3 as u16 - 1,
            })
        }

        if let Some(img) = view.texture {
            self.adapter.set_texture(self.indices.len() - 1, img)?;
        }

        Ok(())
    }

    fn extend_vertices(&mut self, view: &model::ElementView) -> Result<()> {
        let mut vertices: Vec<_> = view
            .faces
            .iter()
            .flat_map(|f| {
                ArrayVec::from([
                    (f.vertex1, f.texture1),
                    (f.vertex2, f.texture2),
                    (f.vertex3, f.texture3),
                ])
            })
            .collect();
        vertices.sort_by_key(|v| v.0);
        vertices.dedup();

        if self.vertices.len() + vertices.len() > u16::MAX as usize {
            let desc = format!("too many vertices");
            return Err(Error::new(InconsistentState, desc));
        }

        let mut pvi = 0;
        for (vi, ti) in vertices {
            if vi == 0 {
                let desc = format!(
                    "zero vertex number in view face for element '{}'",
                    view.element
                );
                return Err(Error::new(InconsistentState, desc));
            }

            if vi == pvi {
                let desc = format!(
                    concat!(
                        "multiple texture points for view vertex {} for ",
                        "element '{}'"
                    ),
                    pvi, view.element
                );
                return Err(Error::new(InconsistentState, desc));
            }

            if vi != pvi + 1 {
                let desc = format!(
                    "missing vertex {} in view for element '{}'",
                    pvi + 1,
                    view.element
                );
                return Err(Error::new(InconsistentState, desc));
            }

            self.vertices.push(Vertex {
                texture: view
                    .texture_points
                    .get(ti as usize)
                    .cloned()
                    .unwrap_or_default(),
                ..Default::default()
            });

            pvi = vi;
        }

        Ok(())
    }

    fn add_element_view_state(
        &mut self,
        _state: model::ElementViewState,
    ) -> Result<()> {
        Ok(())
    }

    pub fn adapter(&mut self) -> &mut A {
        &mut self.adapter
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base::util::test::MethodMock;

    struct TestAdapter {
        set_texture_mock: MethodMock<(usize, model::Image), Result<()>>,
    }

    impl TestAdapter {
        pub fn new() -> Self {
            TestAdapter {
                set_texture_mock: MethodMock::new(),
            }
        }
    }

    impl Adapter for TestAdapter {
        fn set_texture(
            &mut self,
            index: usize,
            image: model::Image,
        ) -> Result<()> {
            self.set_texture_mock.call((index, image))
        }
    }

    fn element_view_record(view: model::ElementView) -> model::Record {
        model::Record {
            r#type: Some(base::model::record::Type::ElementView(view)),
        }
    }

    fn inconsistent_state_result(description: &str) -> Result<()> {
        Err(Error {
            kind: InconsistentState,
            description: format!("{}", description),
            source: None,
        })
    }

    #[test]
    fn test_duplicate_view() {
        let mut controller = Controller::new(TestAdapter::new());

        let rec = element_view_record(model::ElementView {
            element: format!("a"),
            texture_points: vec![model::Point2 { x: 0.0, y: 0.0 }],
            faces: vec![model::element_view::Face {
                vertex1: 1,
                vertex2: 1,
                vertex3: 1,
                texture1: 1,
                texture2: 1,
                texture3: 1,
            }],
            ..Default::default()
        });

        let res = controller.add_record(rec.clone());
        assert_eq!(res, Ok(()));

        let res = controller.add_record(rec);
        assert_eq!(
            res,
            inconsistent_state_result("duplicate view for element 'a'"),
        );
    }

    #[test]
    fn test_missing_vertex() {
        let mut controller = Controller::new(TestAdapter::new());

        let rec = element_view_record(model::ElementView {
            element: format!("a"),
            texture_points: vec![model::Point2 { x: 0.0, y: 0.0 }],
            faces: vec![model::element_view::Face {
                vertex1: 1,
                vertex2: 3,
                vertex3: 3,
                texture1: 1,
                texture2: 1,
                texture3: 1,
            }],
            ..Default::default()
        });

        let res = controller.add_record(rec);
        assert_eq!(
            res,
            inconsistent_state_result(
                "missing vertex 2 in view for element 'a'"
            ),
        );
    }

    #[test]
    fn test_multiple_texture_points_for_vertex() {
        let mut controller = Controller::new(TestAdapter::new());

        let rec = element_view_record(model::ElementView {
            element: format!("a"),
            texture_points: vec![
                model::Point2 { x: 0.0, y: 0.0 },
                model::Point2 { x: 1.0, y: 1.0 },
            ],
            faces: vec![model::element_view::Face {
                vertex1: 1,
                vertex2: 1,
                vertex3: 1,
                texture1: 1,
                texture2: 1,
                texture3: 2,
            }],
            ..Default::default()
        });

        let res = controller.add_record(rec);
        assert_eq!(
            res,
            inconsistent_state_result(
                "multiple texture points for view vertex 1 for element 'a'"
            ),
        );
    }

    #[test]
    fn test_no_faces() {
        let mut controller = Controller::new(TestAdapter::new());

        let rec = element_view_record(model::ElementView {
            element: format!("a"),
            texture_points: vec![model::Point2 { x: 0.0, y: 0.0 }],
            faces: vec![],
            ..Default::default()
        });

        let res = controller.add_record(rec);
        assert_eq!(
            res,
            inconsistent_state_result("no faces in view for element 'a'")
        );
    }

    #[test]
    fn test_set_testure() {
        let mut controller = Controller::new(TestAdapter::new());

        let rec = element_view_record(model::ElementView {
            element: format!("a"),
            texture: Some(model::Image {
                r#type: model::image::Type::Png as i32,
                data: vec![1, 2, 3],
            }),
            texture_points: vec![model::Point2 { x: 0.0, y: 0.0 }],
            faces: vec![model::element_view::Face {
                vertex1: 1,
                vertex2: 1,
                vertex3: 1,
                texture1: 1,
                texture2: 1,
                texture3: 1,
            }],
            ..Default::default()
        });

        let err = Err(Error::new(WebGlError, format!("foo")));
        controller.adapter().set_texture_mock.rets.push(err);

        let res = controller.add_record(rec);
        assert_eq!(res, Err(Error::new(WebGlError, format!("foo"))));

        let args = controller.adapter().set_texture_mock.args.pop();
        assert!(args.is_some());

        let (index, image) = args.unwrap();
        assert_eq!(index, 0);
        assert_eq!(image.r#type, model::image::Type::Png as i32);
        assert_eq!(image.data, vec![1, 2, 3]);
    }

    #[test]
    fn test_too_many_verticies() {
        let mut controller = Controller::new(TestAdapter::new());

        let mut view = model::ElementView {
            element: format!("a"),
            texture_points: vec![model::Point2 { x: 0.0, y: 0.0 }],
            ..Default::default()
        };

        for i in 1..(u16::MAX - 2) as u32 {
            view.faces.push(model::element_view::Face {
                vertex1: i,
                vertex2: i + 1,
                vertex3: i + 2,
                texture1: 1,
                texture2: 1,
                texture3: 1,
            });
        }

        let rec = element_view_record(view);
        let res = controller.add_record(rec);
        assert_eq!(res, Result::Ok(()));

        let rec = element_view_record(model::ElementView {
            element: format!("b"),
            texture_points: vec![model::Point2 { x: 0.0, y: 0.0 }],
            faces: vec![model::element_view::Face {
                vertex1: u16::MAX as u32 + 1,
                vertex2: 1,
                vertex3: 1,
                texture1: 1,
                texture2: 1,
                texture3: 1,
            }],
            ..Default::default()
        });

        let res = controller.add_record(rec);
        assert_eq!(res, inconsistent_state_result("too many vertices"));
    }

    #[test]
    fn test_zero_vertex_number() {
        let mut controller = Controller::new(TestAdapter::new());

        let rec = element_view_record(model::ElementView {
            element: format!("a"),
            texture_points: vec![model::Point2 { x: 0.0, y: 0.0 }],
            faces: vec![model::element_view::Face {
                vertex1: 1,
                vertex2: 1,
                vertex3: 0,
                texture1: 1,
                texture2: 1,
                texture3: 1,
            }],
            ..Default::default()
        });

        let res = controller.add_record(rec);
        assert_eq!(
            res,
            inconsistent_state_result(
                "zero vertex number in view face for element 'a'"
            ),
        );
    }
}
