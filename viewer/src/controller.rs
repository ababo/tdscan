use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

use arrayvec::ArrayVec;
use async_trait::async_trait;

use base::defs::{Error, ErrorKind::*, Result};
use base::model;

pub type Time = i64;

#[derive(Default)]
pub struct Vertex {
    #[allow(dead_code)]
    pub texture: model::Point2,
    #[allow(dead_code)]
    pub position: model::Point3,
    #[allow(dead_code)]
    pub normal: model::Point3,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Face {
    #[allow(dead_code)]
    vertex1: u16,
    #[allow(dead_code)]
    vertex2: u16,
    #[allow(dead_code)]
    vertex3: u16,
}

#[async_trait(?Send)]
pub trait Adapter {
    async fn set_faces(self: &Rc<Self>, faces: &[Face]) -> Result<()>;

    async fn set_texture(
        self: &Rc<Self>,
        index: usize,
        image: model::Image,
    ) -> Result<()>;
}

#[derive(Default)]
struct ElementIndex {
    base: u16,
    vertices: Vec<u16>,
}

#[derive(Default)]
struct ElementState {
    #[allow(dead_code)]
    vertices: Vec<model::Point3>,
    #[allow(dead_code)]
    normals: Vec<model::Point3>,
}

#[derive(PartialEq, PartialOrd, Ord, Eq)]
struct TimeElement(Time, String);

#[derive(Default)]
struct ControllerState {
    index: HashMap<String, ElementIndex>,
    vertices: Vec<Vertex>,
    faces: Vec<Face>,
    states: BTreeMap<TimeElement, ElementState>,
}

pub struct Controller<A: Adapter> {
    adapter: Rc<A>,
    state: Rc<RefCell<ControllerState>>,
}

impl<A: Adapter> Controller<A> {
    pub async fn new(adapter: Rc<A>) -> Result<Rc<Self>> {
        Ok(Rc::new(Self {
            adapter: adapter.clone(),
            state: Rc::new(RefCell::new(ControllerState::default())),
        }))
    }

    #[allow(dead_code)]
    pub fn adapter(self: &Rc<Self>) -> Rc<A> {
        self.adapter.clone()
    }

    pub fn clear(self: &Rc<Self>) {}

    pub async fn add_record(
        self: &Rc<Self>,
        record: model::Record,
    ) -> Result<()> {
        use model::record::Type::*;
        match record.r#type {
            Some(ElementView(v)) => self.add_element_view(v).await?,
            Some(ElementViewState(s)) => self.add_element_view_state(s).await?,
            _ => (),
        }
        Ok(())
    }

    async fn add_element_view(
        self: &Rc<Self>,
        view: model::ElementView,
    ) -> Result<()> {
        let mut state = self.state.borrow_mut();

        if !state.states.is_empty() {
            let desc = format!(
                "view for element '{}' after element view states",
                view.element
            );
            return Err(Error::new(InconsistentState, desc));
        }

        if state.index.contains_key(&view.element) {
            let desc = format!("duplicate view for element '{}'", view.element);
            return Err(Error::new(InconsistentState, desc));
        }

        #[derive(PartialEq, PartialOrd, Ord, Eq)]
        struct VertexDesc(u32, u32);

        let mut vertex_descs: Vec<_> = view
            .faces
            .iter()
            .flat_map(|f| {
                ArrayVec::from([
                    VertexDesc(f.vertex1, f.texture1),
                    VertexDesc(f.vertex2, f.texture2),
                    VertexDesc(f.vertex3, f.texture3),
                ])
            })
            .collect();
        vertex_descs.sort();
        vertex_descs.dedup();

        if state.vertices.len() + vertex_descs.len() > u16::MAX as usize {
            let desc = format!("too many vertices");
            return Err(Error::new(UnsupportedFeature, desc));
        }

        let mut index = ElementIndex {
            base: state.vertices.len() as u16,
            vertices: Vec::with_capacity(vertex_descs.len()),
        };
        let mut vertices = Vec::with_capacity(vertex_descs.len());
        let mut faces = Vec::with_capacity(vertex_descs.len());

        for face in &view.faces {
            let v1 = VertexDesc(face.vertex1, face.texture1);
            let v2 = VertexDesc(face.vertex2, face.texture2);
            let v3 = VertexDesc(face.vertex3, face.texture3);
            let v1i = vertex_descs.binary_search(&v1).unwrap();
            let v2i = vertex_descs.binary_search(&v2).unwrap();
            let v3i = vertex_descs.binary_search(&v3).unwrap();
            faces.push(Face {
                vertex1: index.base + v1i as u16,
                vertex2: index.base + v2i as u16,
                vertex3: index.base + v3i as u16,
            })
        }

        let in_face_err_res = |what| {
            let desc =
                format!("{} in view face for element '{}'", what, view.element);
            Err(Error::new(InconsistentState, desc))
        };

        let unknown_texture_point_ref_err_res =
            || in_face_err_res("reference to unknown texture point number");

        for VertexDesc(vn, tn) in vertex_descs {
            if vn == 0 {
                return in_face_err_res("zero vertex number");
            }

            let tp = if view.texture.is_some() {
                if tn == 0 {
                    return in_face_err_res("zero texture point number");
                } else if tn as usize > view.texture_points.len() {
                    return unknown_texture_point_ref_err_res();
                }
                view.texture_points[tn as usize - 1].clone()
            } else {
                if tn != 0 {
                    return unknown_texture_point_ref_err_res();
                }
                model::Point2::default()
            };

            vertices.push(Vertex {
                texture: tp,
                ..Default::default()
            });
            index.vertices.push(vn as u16);
        }

        if let Some(img) = view.texture {
            self.adapter.set_texture(state.index.len(), img).await?;
        }

        state.index.insert(view.element, index);
        state.vertices.append(&mut vertices);
        state.faces.append(&mut faces);
        Ok(())
    }

    async fn add_element_view_state(
        self: &Rc<Self>,
        view_state: model::ElementViewState,
    ) -> Result<()> {
        let mut state = self.state.borrow_mut();

        let index = state.index.get(&view_state.element).ok_or_else(|| {
            let desc = format!(
                "view state for unknown element '{}'",
                &view_state.element
            );
            return Error::new(InconsistentState, desc);
        })?;

        let num_vertices = *index.vertices.last().unwrap_or(&0) as usize;
        if view_state.vertices.len() != num_vertices
            || view_state.normals.len() != num_vertices
        {
            let desc = format!(
                "bad number of view state vertices or normals for element '{}'",
                &view_state.element
            );
            return Err(Error::new(InconsistentState, desc));
        }

        let key = TimeElement(view_state.time, view_state.element);

        let view_state_time_err_res = |prop: &str| {
            let desc = format!(
                "{} view state time {} for element '{}'",
                prop, key.0, key.1
            );
            return Err(Error::new(InconsistentState, desc));
        };

        if state.states.contains_key(&key) {
            return view_state_time_err_res("duplicate");
        }

        let last = state.states.iter().next_back();
        if last.map_or(false, |(&TimeElement(t, _), _)| t > key.0) {
            return view_state_time_err_res("non-monotonic");
        }

        if state.states.is_empty() {
            self.adapter.set_faces(&state.faces).await?;
        }

        state.states.insert(
            key,
            ElementState {
                vertices: view_state.vertices,
                normals: view_state.normals,
            },
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use async_attributes::test;

    use super::*;
    use base::util::test::{new_ev_face, new_point2, new_point3, MethodMock};

    struct TestAdapterState {
        set_faces_mock: MethodMock<Vec<Face>, Result<()>>,
        set_texture_mock: MethodMock<(usize, model::Image), Result<()>>,
    }

    struct TestAdapter {
        state: RefCell<TestAdapterState>,
    }

    impl TestAdapter {
        pub fn new() -> Rc<Self> {
            Rc::new(TestAdapter {
                state: RefCell::new(TestAdapterState {
                    set_faces_mock: MethodMock::new(),
                    set_texture_mock: MethodMock::new(),
                }),
            })
        }

        pub fn finish(&self) {
            let state = self.state.borrow();
            state.set_faces_mock.finish();
            state.set_texture_mock.finish();
        }
    }

    #[async_trait(?Send)]
    impl Adapter for TestAdapter {
        async fn set_faces(self: &Rc<Self>, faces: &[Face]) -> Result<()> {
            let mut state = self.state.borrow_mut();
            state.set_faces_mock.call(faces.to_vec())
        }

        async fn set_texture(
            self: &Rc<Self>,
            index: usize,
            image: model::Image,
        ) -> Result<()> {
            let mut state = self.state.borrow_mut();
            state.set_texture_mock.call((index, image))
        }
    }

    fn new_simple_view(element: &str) -> model::Record {
        element_view_record(model::ElementView {
            element: format!("{}", element),
            texture: Some(model::Image::default()),
            texture_points: vec![new_point2(0.0, 0.0)],
            faces: vec![new_ev_face(1, 1, 1, 1, 1, 1)],
            ..Default::default()
        })
    }

    async fn add_simple_view(
        controller: &Rc<Controller<TestAdapter>>,
        element: &str,
    ) {
        {
            let adapter = controller.adapter();
            let mut state = adapter.state.borrow_mut();
            state.set_texture_mock.rets.push(Ok(()));
        }

        let rec = new_simple_view(element);
        controller.add_record(rec).await.unwrap();

        {
            let adapter = controller.adapter();
            let mut state = adapter.state.borrow_mut();
            state.set_texture_mock.args.pop();
        }
    }

    fn element_view_record(view: model::ElementView) -> model::Record {
        model::Record {
            r#type: Some(base::model::record::Type::ElementView(view)),
        }
    }

    fn element_view_state_record(
        view_state: model::ElementViewState,
    ) -> model::Record {
        use base::model::record::Type;
        model::Record {
            r#type: Some(Type::ElementViewState(view_state)),
        }
    }

    fn new_face(vertex1: u16, vertex2: u16, vertex3: u16) -> Face {
        Face {
            vertex1,
            vertex2,
            vertex3,
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
    async fn test_add_view_after_state() {
        let controller = Controller::new(TestAdapter::new()).await.unwrap();
        add_simple_view(&controller, "a").await;

        let rec = element_view_state_record(model::ElementViewState {
            element: format!("a"),
            time: 0,
            vertices: vec![(new_point3(0.0, 0.0, 0.0))],
            normals: vec![(new_point3(0.0, 0.0, 0.0))],
        });

        {
            let adapter = controller.adapter();
            let mut state = adapter.state.borrow_mut();
            state.set_faces_mock.rets.push(Ok(()));
        }

        controller.add_record(rec).await.unwrap();

        {
            let adapter = controller.adapter();
            let mut state = adapter.state.borrow_mut();
            state.set_faces_mock.args.pop();
        }

        let rec = new_simple_view("b");
        assert_eq!(
            controller.add_record(rec).await,
            inconsistent_state_result(
                "view for element 'b' after element view states"
            ),
        );

        controller.adapter.finish();
    }

    #[test]
    async fn test_add_view_duplicate() {
        let controller = Controller::new(TestAdapter::new()).await.unwrap();

        {
            let adapter = controller.adapter();
            let mut state = adapter.state.borrow_mut();
            state.set_texture_mock.rets.push(Ok(()));
        }

        let rec = new_simple_view("a");
        controller.add_record(rec.clone()).await.unwrap();

        {
            let adapter = controller.adapter();
            let mut state = adapter.state.borrow_mut();
            let image = state.set_texture_mock.args.pop();
            assert_eq!(image, Some((0, model::Image::default())));
        }

        assert_eq!(
            controller.add_record(rec).await,
            inconsistent_state_result("duplicate view for element 'a'"),
        );

        controller.adapter.finish();
    }

    #[test]
    async fn test_add_view_state_bad_num_of_vertices_normals() {
        let controller = Controller::new(TestAdapter::new()).await.unwrap();
        add_simple_view(&controller, "a").await;

        let rec = element_view_state_record(model::ElementViewState {
            element: format!("a"),
            time: 0,
            vertices: vec![
                new_point3(0.0, 0.0, 0.0),
                new_point3(0.0, 0.0, 0.0),
            ],
            normals: vec![(new_point3(0.0, 0.0, 0.0))],
        });

        let err_res = inconsistent_state_result(
            "bad number of view state vertices or normals for element 'a'",
        );

        assert_eq!(controller.add_record(rec).await, err_res);

        let rec = element_view_state_record(model::ElementViewState {
            element: format!("a"),
            time: 0,
            vertices: vec![new_point3(0.0, 0.0, 0.0)],
            normals: vec![new_point3(0.0, 0.0, 0.0), new_point3(0.0, 0.0, 0.0)],
        });

        assert_eq!(controller.add_record(rec).await, err_res);
    }

    #[test]
    async fn test_add_view_state_duplicate() {
        let controller = Controller::new(TestAdapter::new()).await.unwrap();
        add_simple_view(&controller, "a").await;

        {
            let adapter = controller.adapter();
            let mut state = adapter.state.borrow_mut();
            state.set_faces_mock.rets.push(Ok(()));
        }

        let rec = element_view_state_record(model::ElementViewState {
            element: format!("a"),
            time: 123,
            vertices: vec![new_point3(0.0, 0.0, 0.0)],
            normals: vec![(new_point3(0.0, 0.0, 0.0))],
        });
        controller.add_record(rec.clone()).await.unwrap();

        {
            let adapter = controller.adapter();
            let mut state = adapter.state.borrow_mut();
            state.set_faces_mock.args.pop();
        }

        assert_eq!(
            controller.add_record(rec.clone()).await,
            inconsistent_state_result(
                "duplicate view state time 123 for element 'a'"
            ),
        );
    }

    #[test]
    async fn test_add_view_state_non_monotonic() {
        let controller = Controller::new(TestAdapter::new()).await.unwrap();
        add_simple_view(&controller, "a").await;

        {
            let adapter = controller.adapter();
            let mut state = adapter.state.borrow_mut();
            state.set_faces_mock.rets.push(Ok(()));
        }

        let mut state = model::ElementViewState {
            element: format!("a"),
            time: 123,
            vertices: vec![new_point3(0.0, 0.0, 0.0)],
            normals: vec![(new_point3(0.0, 0.0, 0.0))],
        };

        let rec = element_view_state_record(state.clone());
        controller.add_record(rec.clone()).await.unwrap();

        {
            let adapter = controller.adapter();
            let mut state = adapter.state.borrow_mut();
            state.set_faces_mock.args.pop();
        }

        state.time = 122;
        let rec = element_view_state_record(state);

        assert_eq!(
            controller.add_record(rec.clone()).await,
            inconsistent_state_result(
                "non-monotonic view state time 122 for element 'a'"
            ),
        );
    }

    #[test]
    async fn test_add_view_state_unknown_element() {
        let controller = Controller::new(TestAdapter::new()).await.unwrap();

        let rec = element_view_state_record(model::ElementViewState {
            element: format!("a"),
            time: 0,
            vertices: vec![(new_point3(0.0, 0.0, 0.0))],
            normals: vec![(new_point3(0.0, 0.0, 0.0))],
        });

        assert_eq!(
            controller.add_record(rec).await,
            inconsistent_state_result("view state for unknown element 'a'"),
        );
    }

    #[test]
    async fn test_add_view_unknown_texture_point_reference() {
        let controller = Controller::new(TestAdapter::new()).await.unwrap();

        let rec = element_view_record(model::ElementView {
            element: format!("a"),
            texture: Some(model::Image::default()),
            texture_points: vec![new_point2(0.0, 0.0)],
            faces: vec![new_ev_face(1, 1, 1, 2, 1, 1)],
            ..Default::default()
        });

        assert_eq!(
            controller.add_record(rec).await,
            inconsistent_state_result(concat!(
                "reference to unknown texture point ",
                "number in view face for element 'a'"
            ))
        );

        let rec = element_view_record(model::ElementView {
            element: format!("b"),
            faces: vec![new_ev_face(1, 1, 1, 0, 1, 1)],
            ..Default::default()
        });

        assert_eq!(
            controller.add_record(rec).await,
            inconsistent_state_result(concat!(
                "reference to unknown texture point ",
                "number in view face for element 'b'"
            ))
        );

        controller.adapter.finish();
    }

    #[test]
    async fn test_add_view_valid() {
        let controller = Controller::new(TestAdapter::new()).await.unwrap();

        let png = model::image::Type::Png as i32;
        let image = model::Image {
            r#type: png,
            data: vec![1, 2, 3],
        };

        let rec = element_view_record(model::ElementView {
            element: format!("a"),
            texture: Some(image),
            texture_points: vec![
                new_point2(0.1, 0.2),
                new_point2(0.3, 0.4),
                new_point2(0.5, 0.6),
            ],
            faces: vec![
                new_ev_face(1, 2, 3, 1, 2, 3),
                new_ev_face(2, 3, 4, 2, 3, 1),
                new_ev_face(3, 4, 5, 3, 2, 1),
            ],
            ..Default::default()
        });

        {
            let adapter = controller.adapter();
            let mut state = adapter.state.borrow_mut();
            state.set_texture_mock.rets.push(Ok(()));
        }

        controller.add_record(rec).await.unwrap();

        {
            let state = controller.state.borrow_mut();

            let index = &state.index;
            assert_eq!(index.len(), 1);
            assert_eq!(index["a"].base, 0);
            assert_eq!(index["a"].vertices, vec![1, 2, 3, 4, 4, 5]);

            let vertices = &state.vertices;
            assert_eq!(vertices.len(), 6);
            assert_eq!(vertices[0].texture, new_point2(0.1, 0.2));
            assert_eq!(vertices[1].texture, new_point2(0.3, 0.4));
            assert_eq!(vertices[2].texture, new_point2(0.5, 0.6));

            let faces = &state.faces;
            assert_eq!(faces.len(), 3);
            assert_eq!(faces[0], new_face(0, 1, 2));
            assert_eq!(faces[1], new_face(1, 2, 3));
            assert_eq!(faces[2], new_face(2, 4, 5));
        }

        {
            let adapter = controller.adapter();
            let mut state = adapter.state.borrow_mut();
            let (index, image) = state.set_texture_mock.args.pop().unwrap();
            assert_eq!(index, 0);
            assert_eq!(image.r#type, png);
            assert_eq!(image.data, vec![1, 2, 3]);
        }

        controller.adapter.finish();
    }

    #[test]
    async fn test_add_view_zero_texture_point_number() {
        let controller = Controller::new(TestAdapter::new()).await.unwrap();

        let rec = element_view_record(model::ElementView {
            element: format!("a"),
            texture: Some(model::Image::default()),
            texture_points: vec![new_point2(0.0, 0.0)],
            faces: vec![new_ev_face(1, 1, 1, 0, 1, 1)],
            ..Default::default()
        });

        assert_eq!(
            controller.add_record(rec).await,
            inconsistent_state_result(
                "zero texture point number in view face for element 'a'"
            ),
        );

        controller.adapter.finish();
    }

    #[test]
    async fn test_add_view_zero_vertex_number() {
        let controller = Controller::new(TestAdapter::new()).await.unwrap();

        let rec = element_view_record(model::ElementView {
            element: format!("a"),
            texture: Some(model::Image::default()),
            texture_points: vec![new_point2(0.0, 0.0)],
            faces: vec![new_ev_face(1, 1, 0, 1, 1, 1)],
            ..Default::default()
        });

        assert_eq!(
            controller.add_record(rec).await,
            inconsistent_state_result(
                "zero vertex number in view face for element 'a'"
            ),
        );

        controller.adapter.finish();
    }
}
