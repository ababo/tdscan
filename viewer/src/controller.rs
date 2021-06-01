use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::ops::Bound::*;
use std::rc::Rc;

use arrayvec::ArrayVec;
use async_trait::async_trait;
use glam::{EulerRot, Quat, Vec3};

use crate::util::sync::Mutex;
use base::defs::{Error, ErrorKind::*, Result};
use base::model;
use base::util::glam::{point3_to_vec3, vec3_to_point3};

const DEFAULT_EYE_POSITION: model::Point3 = model::Point3 {
    x: 100.0,
    y: 100.0,
    z: 100.0,
};

const MOUSE_MOVE_ANGLE_FACTOR: f32 = 0.01;
const MOUSE_WHEEL_SCALE_FACTOR: f32 = -0.001;

pub type Time = i64;

#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct Vertex {
    pub texture: model::Point2,
    pub position: model::Point3,
    pub normal: model::Point3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct Face {
    pub vertex1: u16,
    pub vertex2: u16,
    pub vertex3: u16,
}

#[derive(Debug, Default)]
pub struct MouseEvent {
    pub dx: f32,
    pub dy: f32,
    pub primary_button: bool,
}

#[async_trait(?Send)]
pub trait Adapter {
    type Subscription; // Should unsubscribe when dropped.

    fn destroy(self: &Rc<Self>);

    fn render_frame(self: &Rc<Self>) -> Result<()>;

    fn set_faces(self: &Rc<Self>, faces: &[Face]) -> Result<()>;

    async fn set_texture(
        self: &Rc<Self>,
        index: usize,
        image: model::Image,
    ) -> Result<()>;

    fn set_texture_index(self: &Rc<Self>, index: &[u16]) -> Result<()>;

    fn set_vertices(self: &Rc<Self>, vertices: &[Vertex]) -> Result<()>;

    fn set_eye_position(self: &Rc<Self>, eye: &model::Point3) -> Result<()>;

    fn subscribe_to_mouse_move<F: Fn(&MouseEvent) + 'static>(
        self: &Rc<Self>,
        handler: F,
    ) -> Result<Self::Subscription>;

    fn subscribe_to_mouse_wheel<F: Fn(&MouseEvent) + 'static>(
        self: &Rc<Self>,
        handler: F,
    ) -> Result<Self::Subscription>;
}

#[derive(Default)]
struct ElementIndex {
    base: u16,
    vertices: Vec<(u16, u16)>,
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
struct ControllerData {
    index: HashMap<String, ElementIndex>,
    faces: Vec<Face>,
    states: BTreeMap<TimeElement, ElementState>,
    eye_pos: model::Point3,
}

impl ControllerData {
    fn states_at_time<'a>(
        &'a self,
        time: Time,
    ) -> HashMap<&'a String, &'a ElementState> {
        let mut states = HashMap::new();

        let max = self.index.keys().max().cloned().unwrap_or(String::new());
        let range = (Unbounded, Included(TimeElement(time, max)));

        for (TimeElement(_, element), state) in self.states.range(range).rev() {
            if !states.contains_key(element) {
                states.insert(element, state);
                if states.len() == self.index.len() {
                    break;
                }
            }
        }

        states
    }
}

pub struct Controller<A: Adapter> {
    mutex: Mutex,
    adapter: Rc<A>,
    data: RefCell<ControllerData>,
    vertices: RefCell<Vec<Vertex>>,
    mouse_move_sub: RefCell<Option<A::Subscription>>,
    mouse_wheel_sub: RefCell<Option<A::Subscription>>,
}

impl<A: Adapter + 'static> Controller<A> {
    pub fn create(adapter: Rc<A>) -> Result<Rc<Self>> {
        let controller = Rc::new(Self {
            mutex: Mutex::new(),
            adapter: adapter.clone(),
            data: RefCell::new(ControllerData::default()),
            vertices: RefCell::new(Vec::new()),
            mouse_move_sub: RefCell::new(None),
            mouse_wheel_sub: RefCell::new(None),
        });

        let cloned = controller.clone();
        let mouse_move_sub = adapter.subscribe_to_mouse_move(move |e| {
            let _ = cloned.handle_mouse_move(e);
        })?;
        controller
            .mouse_move_sub
            .borrow_mut()
            .get_or_insert(mouse_move_sub);

        let cloned = controller.clone();
        let mouse_wheel_sub = adapter.subscribe_to_mouse_wheel(move |e| {
            let _ = cloned.handle_mouse_wheel(e);
        })?;
        controller
            .mouse_wheel_sub
            .borrow_mut()
            .get_or_insert(mouse_wheel_sub);

        Ok(controller)
    }

    pub fn destroy(self: &Rc<Self>) {
        let _guard = self.mutex.try_lock().unwrap();

        self.mouse_move_sub.borrow_mut().take();
        self.mouse_wheel_sub.borrow_mut().take();

        let mut data = self.data.borrow_mut();
        data.index = HashMap::new();
        data.faces = Vec::new();
        data.states = BTreeMap::new();

        self.adapter.destroy();
    }

    pub fn clear(self: &Rc<Self>) {}

    pub async fn add_record(
        self: &Rc<Self>,
        record: model::Record,
    ) -> Result<()> {
        let _guard = self.mutex.try_lock()?;

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
        let mut data = self.data.borrow_mut();
        let mut all_vertices = self.vertices.borrow_mut();

        if !data.states.is_empty() {
            let desc = format!(
                "view for element '{}' after element view states",
                view.element
            );
            return Err(Error::new(InconsistentState, desc));
        }

        if data.index.contains_key(&view.element) {
            let desc = format!("duplicate view for element '{}'", view.element);
            return Err(Error::new(InconsistentState, desc));
        }

        #[derive(Eq, PartialEq, PartialOrd, Ord)]
        struct VertexDesc(u32, u32, u32);

        let mut vertex_descs: Vec<_> = view
            .faces
            .iter()
            .flat_map(|f| {
                ArrayVec::from([
                    VertexDesc(f.vertex1, f.texture1, f.normal1),
                    VertexDesc(f.vertex2, f.texture2, f.normal2),
                    VertexDesc(f.vertex3, f.texture3, f.normal3),
                ])
            })
            .collect();
        vertex_descs.sort();
        vertex_descs.dedup();

        if all_vertices.len() + vertex_descs.len() > u16::MAX as usize {
            let desc = format!("too many vertices");
            return Err(Error::new(UnsupportedFeature, desc));
        }

        let mut index = ElementIndex {
            base: all_vertices.len() as u16,
            vertices: Vec::with_capacity(vertex_descs.len()),
        };
        let mut vertices = Vec::with_capacity(vertex_descs.len());
        let mut faces = Vec::with_capacity(vertex_descs.len());

        for face in &view.faces {
            let v1 = VertexDesc(face.vertex1, face.texture1, face.normal1);
            let v2 = VertexDesc(face.vertex2, face.texture2, face.normal2);
            let v3 = VertexDesc(face.vertex3, face.texture3, face.normal3);
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

        let check_not_zero = |num, what| {
            if num == 0 {
                return in_face_err_res(what);
            }
            Ok(())
        };

        for VertexDesc(vn, tn, nn) in vertex_descs {
            check_not_zero(vn, "zero vertex number")?;
            check_not_zero(tn, "zero texture point number")?;
            check_not_zero(nn, "zero normal number")?;

            let tn = tn as usize;
            if tn > view.texture_points.len() {
                return in_face_err_res("unknown texture point number");
            }

            vertices.push(Vertex {
                texture: view.texture_points[tn - 1],
                ..Default::default()
            });

            index.vertices.push((vn as u16, nn as u16));
        }

        if let Some(img) = view.texture {
            self.adapter.set_texture(data.index.len(), img).await?;
        } else {
            let desc = format!("textureless element '{}'", view.element);
            return Err(Error::new(UnsupportedFeature, desc));
        }

        data.index.insert(view.element, index);
        all_vertices.append(&mut vertices);
        data.faces.append(&mut faces);
        Ok(())
    }

    async fn add_element_view_state(
        self: &Rc<Self>,
        view_state: model::ElementViewState,
    ) -> Result<()> {
        let mut data = self.data.borrow_mut();

        let index = data.index.get(&view_state.element).ok_or_else(|| {
            let desc = format!(
                "view state for unknown element '{}'",
                &view_state.element
            );
            return Error::new(InconsistentState, desc);
        })?;

        let bad_number_res = |what, expected, actual| {
            let desc = format!(
                "expected {} view state {} for element '{}', encountered {}",
                expected, what, &view_state.element, actual
            );
            return Err(Error::new(InconsistentState, desc));
        };

        let num_vertices = index.vertices.last().map(|d| d.0).unwrap_or(0);
        if view_state.vertices.len() != num_vertices as usize {
            return bad_number_res(
                "vertices",
                num_vertices,
                view_state.vertices.len(),
            );
        }

        let num_normals = index.vertices.iter().map(|d| d.1).max().unwrap_or(0);
        if view_state.normals.len() != num_normals as usize {
            return bad_number_res(
                "normals",
                num_normals,
                view_state.normals.len(),
            );
        }

        let key = TimeElement(view_state.time, view_state.element);

        let view_state_time_err_res = |prop: &str| {
            let desc = format!(
                "{} view state time {} for element '{}'",
                prop, key.0, key.1
            );
            return Err(Error::new(InconsistentState, desc));
        };

        if data.states.contains_key(&key) {
            return view_state_time_err_res("duplicate");
        }

        let last = data.states.iter().next_back();
        if last.map_or(false, |(&TimeElement(t, _), _)| t > key.0) {
            return view_state_time_err_res("non-monotonic");
        }

        if data.states.is_empty() {
            self.adapter.set_faces(&data.faces)?;
            data.faces = Vec::new(); // It's not used anymore, so deallocate.

            let mut index: Vec<u16> = data
                .index
                .iter()
                .map(|(_, i)| i.base + i.vertices.len() as u16)
                .collect();
            index.sort();
            self.adapter.set_texture_index(&index)?;
        }

        data.states.insert(
            key,
            ElementState {
                vertices: view_state.vertices,
                normals: view_state.normals,
            },
        );

        Ok(())
    }

    fn handle_mouse_move(self: &Rc<Self>, event: &MouseEvent) -> Result<()> {
        if !event.primary_button {
            return Ok(());
        }

        let _guard = self.mutex.try_lock()?;

        let mut data = self.data.borrow_mut();

        let hor_rot_angle = -event.dx * MOUSE_MOVE_ANGLE_FACTOR;
        let hor_rot = Quat::from_euler(EulerRot::YZX, 0.0, hor_rot_angle, 0.0);
        let eye_pos = point3_to_vec3(&data.eye_pos);
        data.eye_pos = vec3_to_point3(&hor_rot.mul_vec3(eye_pos));

        let vert_rot_axis = if data.eye_pos.y != 0.0 {
            let slope = -data.eye_pos.x / data.eye_pos.y;
            let x = 1.0 / (1.0 + slope * slope).sqrt();
            let y = slope * x;
            Vec3::new(x, y, 0.0)
        } else {
            Vec3::new(0.0, 1.0, 0.0)
        };

        let vert_rot_angle =
            data.eye_pos.y.signum() * event.dy * MOUSE_MOVE_ANGLE_FACTOR;
        let vert_rot = Quat::from_axis_angle(vert_rot_axis, vert_rot_angle);
        let eye_pos = point3_to_vec3(&data.eye_pos);
        let eye_pos = vec3_to_point3(&vert_rot.mul_vec3(eye_pos));

        let angle_z = (eye_pos.z
            / (eye_pos.x * eye_pos.x
                + eye_pos.y * eye_pos.y
                + eye_pos.z * eye_pos.z)
                .sqrt())
        .acos();

        if eye_pos.z >= 0.0 && vert_rot_angle.abs() < angle_z {
            data.eye_pos = eye_pos;
        }

        self.adapter.set_eye_position(&data.eye_pos)?;
        self.adapter.render_frame()
    }

    fn handle_mouse_wheel(self: &Rc<Self>, event: &MouseEvent) -> Result<()> {
        let _guard = self.mutex.try_lock()?;

        let mut data = self.data.borrow_mut();

        let scale = 1.0 + event.dy * MOUSE_WHEEL_SCALE_FACTOR;
        data.eye_pos.x *= scale;
        data.eye_pos.y *= scale;
        data.eye_pos.z *= scale;

        self.adapter.set_eye_position(&data.eye_pos)?;
        self.adapter.render_frame()
    }

    pub fn move_to_scene(self: &Rc<Self>, time: Time) -> Result<()> {
        let _guard = self.mutex.try_lock()?;

        let mut data = self.data.borrow_mut();
        let mut vertices = self.vertices.borrow_mut();

        let states = data.states_at_time(time);

        for (element, index) in &data.index {
            let element_state = states.get(&element);
            for (i, (vn, nn)) in index.vertices.iter().enumerate() {
                let j = index.base as usize + i;
                match element_state {
                    Some(s) => {
                        let k = vn.clone() as usize - 1;
                        vertices[j].position = s.vertices[k].clone();
                        let k = nn.clone() as usize - 1;
                        vertices[j].normal = s.normals[k].clone();
                    }
                    None => {
                        vertices[j].position = model::Point3::default();
                        vertices[j].normal = model::Point3::default();
                    }
                }
            }
        }

        self.adapter.set_vertices(vertices.as_ref())?;
        data.eye_pos = DEFAULT_EYE_POSITION;
        self.adapter.set_eye_position(&data.eye_pos)?;
        self.adapter.render_frame()
    }
}

#[cfg(test)]
mod tests {
    use async_attributes::test;

    use super::*;
    use base::util::test::{
        new_element_view_rec, new_element_view_state_rec, new_ev_face,
        new_point2, new_point3, MethodMock,
    };

    struct TestAdapterData {
        destroy_mock: MethodMock<(), Result<()>>,
        render_frame_mock: MethodMock<(), Result<()>>,
        set_faces_mock: MethodMock<Vec<Face>, Result<()>>,
        set_texture_mock: MethodMock<(usize, model::Image), Result<()>>,
        set_texture_index_mock: MethodMock<Vec<u16>, Result<()>>,
        set_vertices_mock: MethodMock<Vec<Vertex>, Result<()>>,
        set_eye_position_mock: MethodMock<model::Point3, Result<()>>,
        subscribe_to_mouse_move_mock:
            MethodMock<Box<dyn Fn(&MouseEvent)>, Result<String>>,
        subscribe_to_mouse_wheel_mock:
            MethodMock<Box<dyn Fn(&MouseEvent)>, Result<String>>,
    }

    struct TestAdapter {
        data: RefCell<TestAdapterData>,
    }

    impl TestAdapter {
        pub fn new() -> Rc<Self> {
            Rc::new(TestAdapter {
                data: RefCell::new(TestAdapterData {
                    destroy_mock: MethodMock::new(),
                    render_frame_mock: MethodMock::new(),
                    set_faces_mock: MethodMock::new(),
                    set_texture_mock: MethodMock::new(),
                    set_texture_index_mock: MethodMock::new(),
                    set_vertices_mock: MethodMock::new(),
                    set_eye_position_mock: MethodMock::new(),
                    subscribe_to_mouse_move_mock: MethodMock::new(),
                    subscribe_to_mouse_wheel_mock: MethodMock::new(),
                }),
            })
        }

        pub fn finish(&self) {
            let data = self.data.borrow();
            data.destroy_mock.finish();
            data.render_frame_mock.finish();
            data.set_faces_mock.finish();
            data.set_texture_mock.finish();
            data.set_texture_index_mock.finish();
            data.set_vertices_mock.finish();
            data.set_eye_position_mock.finish();
            data.subscribe_to_mouse_move_mock.finish();
            data.subscribe_to_mouse_wheel_mock.finish();
        }
    }

    #[async_trait(?Send)]
    impl Adapter for TestAdapter {
        type Subscription = String;

        fn destroy(self: &Rc<Self>) {
            let _ = self.data.borrow_mut().destroy_mock.call(());
        }

        fn render_frame(self: &Rc<Self>) -> Result<()> {
            self.data.borrow_mut().render_frame_mock.call(())
        }

        fn set_faces(self: &Rc<Self>, faces: &[Face]) -> Result<()> {
            self.data.borrow_mut().set_faces_mock.call(faces.to_vec())
        }

        async fn set_texture(
            self: &Rc<Self>,
            index: usize,
            image: model::Image,
        ) -> Result<()> {
            self.data.borrow_mut().set_texture_mock.call((index, image))
        }

        fn set_texture_index(self: &Rc<Self>, index: &[u16]) -> Result<()> {
            self.data
                .borrow_mut()
                .set_texture_index_mock
                .call(index.to_vec())
        }

        fn set_vertices(self: &Rc<Self>, vertices: &[Vertex]) -> Result<()> {
            self.data
                .borrow_mut()
                .set_vertices_mock
                .call(vertices.to_vec())
        }

        fn set_eye_position(
            self: &Rc<Self>,
            eye: &model::Point3,
        ) -> Result<()> {
            self.data
                .borrow_mut()
                .set_eye_position_mock
                .call(eye.clone())
        }

        fn subscribe_to_mouse_move<F: Fn(&MouseEvent) + 'static>(
            self: &Rc<Self>,
            handler: F,
        ) -> Result<Self::Subscription> {
            let mut data = self.data.borrow_mut();
            data.subscribe_to_mouse_move_mock.call(Box::new(handler))
        }

        fn subscribe_to_mouse_wheel<F: Fn(&MouseEvent) + 'static>(
            self: &Rc<Self>,
            handler: F,
        ) -> Result<Self::Subscription> {
            let mut data = self.data.borrow_mut();
            data.subscribe_to_mouse_wheel_mock.call(Box::new(handler))
        }
    }

    fn create_controller() -> Rc<Controller<TestAdapter>> {
        let adapter = TestAdapter::new();

        {
            let mut data = adapter.data.borrow_mut();
            let ret = Ok(format!("mouse_move_sub"));
            data.subscribe_to_mouse_move_mock.rets.push(ret);
            let ret = Ok(format!("mouse_wheel_sub"));
            data.subscribe_to_mouse_wheel_mock.rets.push(ret);
        }

        let controller = Controller::create(adapter).unwrap();

        {
            let mut data = controller.adapter.data.borrow_mut();
            let _ = data.subscribe_to_mouse_move_mock.args.pop().unwrap();
            let _ = data.subscribe_to_mouse_wheel_mock.args.pop().unwrap();
        }

        controller
    }

    fn new_simple_view(element: &str) -> model::Record {
        new_element_view_rec(model::ElementView {
            element: format!("{}", element),
            texture: Some(model::Image::default()),
            texture_points: vec![new_point2(0.0, 0.0)],
            faces: vec![new_ev_face(1, 1, 1, 1, 1, 1, 1, 1, 1)],
            ..Default::default()
        })
    }

    async fn add_simple_view(
        controller: &Rc<Controller<TestAdapter>>,
        element: &str,
    ) {
        {
            let mut data = controller.adapter.data.borrow_mut();
            data.set_texture_mock.rets.push(Ok(()));
        }

        let rec = new_simple_view(element);
        controller.add_record(rec).await.unwrap();

        {
            let mut data = controller.adapter.data.borrow_mut();
            data.set_texture_mock.args.pop().unwrap();
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
        let controller = create_controller();
        add_simple_view(&controller, "a").await;

        let rec = new_element_view_state_rec(model::ElementViewState {
            element: format!("a"),
            time: 0,
            vertices: vec![(new_point3(0.0, 0.0, 0.0))],
            normals: vec![(new_point3(0.0, 0.0, 0.0))],
        });

        {
            let mut data = controller.adapter.data.borrow_mut();
            data.set_faces_mock.rets.push(Ok(()));
            data.set_texture_index_mock.rets.push(Ok(()));
        }

        controller.add_record(rec).await.unwrap();

        {
            let mut data = controller.adapter.data.borrow_mut();
            data.set_faces_mock.args.pop().unwrap();
            data.set_texture_index_mock.args.pop().unwrap();
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
        let controller = create_controller();

        {
            let mut data = controller.adapter.data.borrow_mut();
            data.set_texture_mock.rets.push(Ok(()));
        }

        let rec = new_simple_view("a");
        controller.add_record(rec.clone()).await.unwrap();

        {
            let mut data = controller.adapter.data.borrow_mut();
            let image = data.set_texture_mock.args.pop();
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
        let controller = create_controller();
        add_simple_view(&controller, "a").await;

        let rec = new_element_view_state_rec(model::ElementViewState {
            element: format!("a"),
            time: 0,
            vertices: vec![
                new_point3(0.0, 0.0, 0.0),
                new_point3(0.0, 0.0, 0.0),
            ],
            normals: vec![(new_point3(0.0, 0.0, 0.0))],
        });

        let err_res = inconsistent_state_result(
            "expected 1 view state vertices for element 'a', encountered 2",
        );
        assert_eq!(controller.add_record(rec).await, err_res);

        let rec = new_element_view_state_rec(model::ElementViewState {
            element: format!("a"),
            time: 0,
            vertices: vec![new_point3(0.0, 0.0, 0.0)],
            normals: vec![new_point3(0.0, 0.0, 0.0), new_point3(0.0, 0.0, 0.0)],
        });

        let err_res = inconsistent_state_result(
            "expected 1 view state normals for element 'a', encountered 2",
        );
        assert_eq!(controller.add_record(rec).await, err_res);

        controller.adapter.finish();
    }

    #[test]
    async fn test_add_view_state_duplicate() {
        let controller = create_controller();
        add_simple_view(&controller, "a").await;

        {
            let mut data = controller.adapter.data.borrow_mut();
            data.set_faces_mock.rets.push(Ok(()));
            data.set_texture_index_mock.rets.push(Ok(()));
        }

        let rec = new_element_view_state_rec(model::ElementViewState {
            element: format!("a"),
            time: 123,
            vertices: vec![new_point3(0.0, 0.0, 0.0)],
            normals: vec![(new_point3(0.0, 0.0, 0.0))],
        });
        controller.add_record(rec.clone()).await.unwrap();

        {
            let mut data = controller.adapter.data.borrow_mut();
            data.set_faces_mock.args.pop().unwrap();
            data.set_texture_index_mock.args.pop().unwrap();
        }

        assert_eq!(
            controller.add_record(rec.clone()).await,
            inconsistent_state_result(
                "duplicate view state time 123 for element 'a'"
            ),
        );

        controller.adapter.finish();
    }

    #[test]
    async fn test_add_view_state_non_monotonic() {
        let controller = create_controller();
        add_simple_view(&controller, "a").await;

        {
            let mut data = controller.adapter.data.borrow_mut();
            data.set_faces_mock.rets.push(Ok(()));
            data.set_texture_index_mock.rets.push(Ok(()));
        }

        let mut state = model::ElementViewState {
            element: format!("a"),
            time: 123,
            vertices: vec![new_point3(0.0, 0.0, 0.0)],
            normals: vec![(new_point3(0.0, 0.0, 0.0))],
        };

        let rec = new_element_view_state_rec(state.clone());
        controller.add_record(rec.clone()).await.unwrap();

        {
            let mut data = controller.adapter.data.borrow_mut();
            data.set_faces_mock.args.pop().unwrap();
            data.set_texture_index_mock.args.pop().unwrap();
        }

        state.time = 122;
        let rec = new_element_view_state_rec(state);

        assert_eq!(
            controller.add_record(rec.clone()).await,
            inconsistent_state_result(
                "non-monotonic view state time 122 for element 'a'"
            ),
        );

        controller.adapter.finish();
    }

    #[test]
    async fn test_add_view_state_unknown_element() {
        let controller = create_controller();

        let rec = new_element_view_state_rec(model::ElementViewState {
            element: format!("a"),
            time: 0,
            vertices: vec![(new_point3(0.0, 0.0, 0.0))],
            normals: vec![(new_point3(0.0, 0.0, 0.0))],
        });

        assert_eq!(
            controller.add_record(rec).await,
            inconsistent_state_result("view state for unknown element 'a'"),
        );

        controller.adapter.finish();
    }

    #[test]
    async fn test_add_view_unknown_texture_point_reference() {
        let controller = create_controller();

        let rec = new_element_view_rec(model::ElementView {
            element: format!("a"),
            texture: Some(model::Image::default()),
            texture_points: vec![new_point2(0.0, 0.0)],
            faces: vec![new_ev_face(1, 1, 1, 2, 1, 1, 1, 1, 2)],
            ..Default::default()
        });

        assert_eq!(
            controller.add_record(rec).await,
            inconsistent_state_result(concat!(
                "unknown texture point number in view face for element 'a'"
            ))
        );

        let rec = new_element_view_rec(model::ElementView {
            element: format!("b"),
            faces: vec![new_ev_face(1, 1, 1, 0, 1, 1, 1, 1, 0)],
            ..Default::default()
        });

        assert_eq!(
            controller.add_record(rec).await,
            inconsistent_state_result(concat!(
                "zero texture point number in view face for element 'b'"
            ))
        );

        controller.adapter.finish();
    }

    #[test]
    async fn test_add_view_valid() {
        let controller = create_controller();

        let png = model::image::Type::Png as i32;
        let image = model::Image {
            r#type: png,
            data: vec![1, 2, 3],
        };

        let rec = new_element_view_rec(model::ElementView {
            element: format!("a"),
            texture: Some(image),
            texture_points: vec![
                new_point2(0.1, 0.2),
                new_point2(0.3, 0.4),
                new_point2(0.5, 0.6),
            ],
            faces: vec![
                new_ev_face(1, 2, 3, 1, 2, 3, 1, 2, 3),
                new_ev_face(2, 3, 4, 2, 3, 1, 2, 3, 1),
                new_ev_face(3, 4, 5, 3, 2, 1, 3, 1, 2),
            ],
            ..Default::default()
        });

        {
            let mut data = controller.adapter.data.borrow_mut();
            data.set_texture_mock.rets.push(Ok(()));
        }

        controller.add_record(rec).await.unwrap();

        {
            let data = controller.data.borrow();
            let vertices = controller.vertices.borrow();

            let index = &data.index;
            assert_eq!(index.len(), 1);
            assert_eq!(index["a"].base, 0);
            assert_eq!(
                index["a"].vertices,
                vec![(1, 1), (2, 2), (3, 3), (4, 1), (4, 1), (5, 2)]
            );

            assert_eq!(vertices.len(), 6);
            assert_eq!(vertices[0].texture, new_point2(0.1, 0.2));
            assert_eq!(vertices[1].texture, new_point2(0.3, 0.4));
            assert_eq!(vertices[2].texture, new_point2(0.5, 0.6));

            let faces = &data.faces;
            assert_eq!(faces.len(), 3);
            assert_eq!(faces[0], new_face(0, 1, 2));
            assert_eq!(faces[1], new_face(1, 2, 3));
            assert_eq!(faces[2], new_face(2, 4, 5));
        }

        {
            let mut data = controller.adapter.data.borrow_mut();
            let (index, image) = data.set_texture_mock.args.pop().unwrap();
            assert_eq!(index, 0);
            assert_eq!(image.r#type, png);
            assert_eq!(image.data, vec![1, 2, 3]);
        }

        controller.adapter.finish();
    }

    #[test]
    async fn test_add_view_zero_normal_number() {
        let controller = create_controller();

        let rec = new_element_view_rec(model::ElementView {
            element: format!("a"),
            texture: Some(model::Image::default()),
            texture_points: vec![new_point2(0.0, 0.0)],
            faces: vec![new_ev_face(1, 1, 1, 1, 1, 1, 1, 0, 1)],
            ..Default::default()
        });

        assert_eq!(
            controller.add_record(rec).await,
            inconsistent_state_result(
                "zero normal number in view face for element 'a'"
            ),
        );

        controller.adapter.finish();
    }

    #[test]
    async fn test_add_view_zero_texture_point_number() {
        let controller = create_controller();

        let rec = new_element_view_rec(model::ElementView {
            element: format!("a"),
            texture: Some(model::Image::default()),
            texture_points: vec![new_point2(0.0, 0.0)],
            faces: vec![new_ev_face(1, 1, 1, 0, 1, 1, 1, 1, 1)],
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
        let controller = create_controller();

        let rec = new_element_view_rec(model::ElementView {
            element: format!("a"),
            texture: Some(model::Image::default()),
            texture_points: vec![new_point2(0.0, 0.0)],
            faces: vec![new_ev_face(1, 1, 0, 1, 1, 1, 1, 1, 1)],
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

    #[test]
    async fn test_destroy() {
        let controller = create_controller();
        {
            let mut data = controller.adapter.data.borrow_mut();
            data.destroy_mock.rets.push(Ok(()));
        }
        controller.destroy();
        {
            let mut data = controller.adapter.data.borrow_mut();
            data.destroy_mock.args.pop().unwrap();
        }
    }

    #[test]
    async fn test_render_frame() {
        let controller = create_controller();
        add_simple_view(&controller, "a").await;
        add_simple_view(&controller, "b").await;
        add_simple_view(&controller, "c").await;

        {
            let mut data = controller.adapter.data.borrow_mut();
            data.set_faces_mock.rets.push(Ok(()));
            data.set_texture_index_mock.rets.push(Ok(()));
            data.set_vertices_mock.rets.push(Ok(()));
            data.set_eye_position_mock.rets.push(Ok(()));
            data.render_frame_mock.rets.push(Ok(()));
        }

        let rec = new_element_view_state_rec(model::ElementViewState {
            element: format!("a"),
            time: 123,
            vertices: vec![new_point3(0.123, 0.234, 0.345)],
            normals: vec![new_point3(0.456, 0.567, 0.678)],
        });
        controller.add_record(rec).await.unwrap();

        let rec = new_element_view_state_rec(model::ElementViewState {
            element: format!("b"),
            time: 234,
            vertices: vec![new_point3(0.789, 0.890, 0.901)],
            normals: vec![new_point3(0.012, 0.123, 0.234)],
        });
        controller.add_record(rec).await.unwrap();

        let rec = new_element_view_state_rec(model::ElementViewState {
            element: format!("b"),
            time: 345,
            vertices: vec![new_point3(0.345, 0.456, 0.567)],
            normals: vec![new_point3(0.678, 0.789, 0.890)],
        });
        controller.add_record(rec).await.unwrap();

        controller.move_to_scene(456).unwrap();

        let texture_index;
        let vertices;
        let view;
        {
            let mut data = controller.adapter.data.borrow_mut();
            data.set_faces_mock.args.pop().unwrap();
            texture_index = data.set_texture_index_mock.args.pop();
            vertices = data.set_vertices_mock.args.pop().unwrap();
            view = data.set_eye_position_mock.args.pop().unwrap();
            data.render_frame_mock.args.pop().unwrap();
        }

        assert_eq!(texture_index, Some(vec![1, 2, 3]));

        assert_eq!(vertices.len(), 3);
        assert_eq!(vertices[0].position, new_point3(0.123, 0.234, 0.345));
        assert_eq!(vertices[0].normal, new_point3(0.456, 0.567, 0.678));
        assert_eq!(vertices[1].position, new_point3(0.345, 0.456, 0.567));
        assert_eq!(vertices[1].normal, new_point3(0.678, 0.789, 0.890));
        assert_eq!(vertices[2].position, model::Point3::default());
        assert_eq!(vertices[2].normal, model::Point3::default());

        assert_eq!(view, DEFAULT_EYE_POSITION);

        controller.adapter.finish();
    }
}
