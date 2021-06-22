use async_trait::async_trait;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::mem;
use std::ops::Bound::*;
use std::rc::Rc;

use arrayvec::ArrayVec;
use base::util::glam::{point3_to_vec3, vec3_to_point3};
use glam::{EulerRot, Quat, Vec3};

use crate::util::sync::LevelLock;
use base::defs::{Error, ErrorKind::*, Result};
use base::fm;
use base::model;

const DEFAULT_EYE_POSITION: model::Point3 = model::Point3 {
    x: 1.0,
    y: 1.0,
    z: 1.0,
};

const MOUSE_MOVE_ANGLE_FACTOR: f32 = 0.01;
const MOUSE_WHEEL_SCALE_FACTOR: f32 = -0.001;

#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct VertexData {
    pub element: u8,
    pub normal: model::Point3,
    pub texture: model::Point2,
    pub vertex: model::Point3,
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
    type Subscription; // Will unsubscribe when dropped.

    fn destroy(self: &Rc<Self>);

    async fn next_frame(self: &Rc<Self>) -> model::Time;

    fn render_frame(self: &Rc<Self>) -> Result<()>;

    fn set_faces(self: &Rc<Self>, faces: &[Face]) -> Result<()>;

    async fn set_now(self: &Rc<Self>, now: model::Time);

    async fn set_texture(
        self: &Rc<Self>,
        index: usize,
        image: model::Image,
    ) -> Result<()>;

    fn set_vertices(self: &Rc<Self>, vertices: &[VertexData]) -> Result<()>;

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

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum ControllerState {
    Idle,
    HandlingOp,
    HandlingEvent,
}

#[derive(Default)]
struct ElementData {
    index: usize,
    vertex_base: u16,
    vertices: Vec<(u16, u16)>,
}

#[derive(Clone, Default)]
struct ElementState {
    vertices: Vec<model::Point3>,
    normals: Vec<model::Point3>,
}

#[derive(Default)]
struct ControllerData {
    elements: HashMap<String, ElementData>,
    eye_pos: model::Point3,
    faces: Vec<Face>,
    states: Vec<BTreeMap<model::Time, ElementState>>,
}

impl ControllerData {
    fn interpolate_linear(
        at: model::Time,
        a: (&model::Time, &ElementState),
        b: (&model::Time, &ElementState),
    ) -> ElementState {
        #[inline]
        fn interpolate(at: f32, a: (f32, f32), b: (f32, f32)) -> f32 {
            (b.1 - a.1) / (b.0 - a.0) * (at - b.0) + b.1
        }

        fn interpolate_points(
            at: f32,
            a: (f32, &Vec<model::Point3>),
            b: (f32, &Vec<model::Point3>),
        ) -> Vec<model::Point3> {
            a.1.iter()
                .zip(b.1)
                .map(|(a1, b1)| model::Point3 {
                    x: interpolate(at, (a.0, a1.x), (b.0, b1.x)),
                    y: interpolate(at, (a.0, a1.y), (b.0, b1.y)),
                    z: interpolate(at, (a.0, a1.z), (b.0, b1.z)),
                })
                .collect()
        }

        let (atf, a0, b0) = (at as f32, *a.0 as f32, *b.0 as f32);

        ElementState {
            vertices: interpolate_points(
                atf,
                (a0, &a.1.vertices),
                (b0, &b.1.vertices),
            ),
            normals: interpolate_points(
                atf,
                (a0, &a.1.normals),
                (b0, &b.1.normals),
            ),
        }
    }

    fn interpolate_quadratic(
        at: model::Time,
        a: (&model::Time, &ElementState),
        b: (&model::Time, &ElementState),
        c: (&model::Time, &ElementState),
    ) -> ElementState {
        #[inline]
        fn interpolate(
            at: f32,
            a: (f32, f32),
            b: (f32, f32),
            c: (f32, f32),
        ) -> f32 {
            ((at - c.0)
                * ((at - b.0) * (b.0 - c.0) * a.1
                    + (at - a.0) * (-a.0 + c.0) * b.1)
                + (at - a.0) * (at - b.0) * (a.0 - b.0) * c.1)
                / ((a.0 - b.0) * (a.0 - c.0) * (b.0 - c.0))
        }

        fn interpolate_points(
            at: f32,
            a: (f32, &Vec<model::Point3>),
            b: (f32, &Vec<model::Point3>),
            c: (f32, &Vec<model::Point3>),
        ) -> Vec<model::Point3> {
            a.1.iter()
                .zip(b.1)
                .zip(c.1)
                .map(|((a1, b1), c1)| model::Point3 {
                    x: interpolate(at, (a.0, a1.x), (b.0, b1.x), (c.0, c1.x)),
                    y: interpolate(at, (a.0, a1.y), (b.0, b1.y), (c.0, c1.y)),
                    z: interpolate(at, (a.0, a1.z), (b.0, b1.z), (c.0, c1.z)),
                })
                .collect()
        }

        let (atf, a0, b0, c0) =
            (at as f32, *a.0 as f32, *b.0 as f32, *c.0 as f32);

        ElementState {
            vertices: interpolate_points(
                atf,
                (a0, &a.1.vertices),
                (b0, &b.1.vertices),
                (c0, &c.1.vertices),
            ),
            normals: interpolate_points(
                atf,
                (a0, &a.1.normals),
                (b0, &b.1.normals),
                (c0, &c.1.normals),
            ),
        }
    }

    pub fn no_states(&self) -> bool {
        self.states.iter().map(|s| s.len()).max().unwrap_or(0) == 0
    }

    fn state_at(
        states: &BTreeMap<model::Time, ElementState>,
        at: model::Time,
    ) -> Option<ElementState> {
        if let Some(state) = states.get(&at) {
            return Some(state.clone());
        }

        let mut prange = states.range((Unbounded, Excluded(at)));
        let prev = prange.next_back()?;

        let mut nrange = states.range((Excluded(at), Unbounded));
        let next = if let Some(next) = nrange.next() {
            next
        } else {
            return Some(prev.1.clone());
        };

        Some(if let Some(nnext) = nrange.next() {
            ControllerData::interpolate_quadratic(at, prev, next, nnext)
        } else if let Some(pprev) = prange.next_back() {
            ControllerData::interpolate_quadratic(at, pprev, prev, next)
        } else {
            ControllerData::interpolate_linear(at, prev, next)
        })
    }

    pub fn states_at(&self, at: model::Time) -> Vec<Option<ElementState>> {
        let mut states = Vec::with_capacity(self.elements.len());
        for element_states in &self.states {
            states.push(Self::state_at(element_states, at));
        }
        states
    }
}

pub struct Controller<A: Adapter> {
    adapter: Rc<A>,
    data: RefCell<ControllerData>,
    mouse_move_sub: RefCell<Option<A::Subscription>>,
    mouse_wheel_sub: RefCell<Option<A::Subscription>>,
    state: LevelLock<ControllerState>,
    vertices: RefCell<Vec<VertexData>>,
}

impl<A: Adapter + 'static> Controller<A> {
    pub fn create(adapter: Rc<A>) -> Result<Rc<Self>> {
        let controller = Rc::new(Self {
            adapter: adapter.clone(),
            data: RefCell::new(ControllerData::default()),
            mouse_move_sub: RefCell::new(None),
            mouse_wheel_sub: RefCell::new(None),
            state: LevelLock::new(ControllerState::Idle),
            vertices: RefCell::new(Vec::new()),
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

        {
            let mut data = controller.data.borrow_mut();
            data.eye_pos = DEFAULT_EYE_POSITION;
            controller.adapter.set_eye_position(&data.eye_pos)?;
        }

        Ok(controller)
    }

    pub fn destroy(self: &Rc<Self>) {
        let guard = self.state.try_lock(ControllerState::HandlingOp).unwrap();
        mem::forget(guard); // Make the object unusable.

        self.mouse_move_sub.borrow_mut().take();
        self.mouse_wheel_sub.borrow_mut().take();

        self.reset();

        self.adapter.destroy();
    }

    fn handle_mouse_move(self: &Rc<Self>, event: &MouseEvent) -> Result<()> {
        if !event.primary_button {
            return Ok(());
        }

        let _guard = self.state.try_lock(ControllerState::HandlingEvent)?;

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
        let _guard = self.state.try_lock(ControllerState::HandlingEvent)?;

        let mut data = self.data.borrow_mut();

        let scale = 1.0 + event.dy * MOUSE_WHEEL_SCALE_FACTOR;
        data.eye_pos.x *= scale;
        data.eye_pos.y *= scale;
        data.eye_pos.z *= scale;

        self.adapter.set_eye_position(&data.eye_pos)?;
        self.adapter.render_frame()
    }

    pub async fn load(
        self: &Rc<Self>,
        reader: &mut dyn fm::Read,
    ) -> Result<()> {
        let _guard = self.state.try_lock(ControllerState::HandlingOp).unwrap();

        self.reset();

        loop {
            let rec = reader.read_record()?;
            if rec.is_none() {
                break;
            }

            use model::record::Type::*;
            match rec.unwrap().r#type {
                Some(ElementView(v)) => self.load_element_view(v).await?,
                Some(ElementViewState(s)) => self.load_element_view_state(s)?,
                _ => (),
            }
        }

        Ok(())
    }

    async fn load_element_view(
        self: &Rc<Self>,
        view: model::ElementView,
    ) -> Result<()> {
        let mut data = self.data.borrow_mut();
        let mut all_vertices = self.vertices.borrow_mut();

        if data.elements.len() + 1 > u8::MAX as usize {
            let desc = format!("too many elements");
            return Err(Error::new(UnsupportedFeature, desc));
        }

        if !data.no_states() {
            let desc = format!(
                "view for element '{}' after element view states",
                view.element
            );
            return Err(Error::new(InconsistentState, desc));
        }

        if data.elements.contains_key(&view.element) {
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

        let mut element = ElementData {
            index: data.elements.len(),
            vertex_base: all_vertices.len() as u16,
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
                vertex1: element.vertex_base + v1i as u16,
                vertex2: element.vertex_base + v2i as u16,
                vertex3: element.vertex_base + v3i as u16,
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

            vertices.push(VertexData {
                element: data.elements.len() as u8,
                texture: view.texture_points[tn - 1],
                ..Default::default()
            });

            element.vertices.push((vn as u16, nn as u16));
        }

        if let Some(img) = view.texture {
            self.adapter.set_texture(data.elements.len(), img).await?;
        } else {
            let desc = format!("textureless element '{}'", view.element);
            return Err(Error::new(UnsupportedFeature, desc));
        }

        all_vertices.append(&mut vertices);
        data.elements.insert(view.element, element);
        data.faces.append(&mut faces);
        data.states.push(BTreeMap::new());
        Ok(())
    }

    fn load_element_view_state(
        self: &Rc<Self>,
        view_state: model::ElementViewState,
    ) -> Result<()> {
        let mut data = self.data.borrow_mut();

        let element =
            data.elements.get(&view_state.element).ok_or_else(|| {
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

        let num_vertices = element.vertices.last().map(|d| d.0).unwrap_or(0);
        if view_state.vertices.len() != num_vertices as usize {
            return bad_number_res(
                "vertices",
                num_vertices,
                view_state.vertices.len(),
            );
        }

        let num_normals =
            element.vertices.iter().map(|d| d.1).max().unwrap_or(0);
        if view_state.normals.len() != num_normals as usize {
            return bad_number_res(
                "normals",
                num_normals,
                view_state.normals.len(),
            );
        }

        let view_state_time_err_res = |prop: &str| {
            let desc = format!(
                "{} view state time {} for element '{}'",
                prop, view_state.time, view_state.element
            );
            return Err(Error::new(InconsistentState, desc));
        };

        let index = element.index.clone();
        if data.states[index].contains_key(&view_state.time) {
            return view_state_time_err_res("duplicate");
        }

        let last = data.states[index].iter().next_back();
        if last.map_or(false, |(&t, _)| t > view_state.time) {
            return view_state_time_err_res("non-monotonic");
        }

        if data.no_states() {
            self.adapter.set_faces(&data.faces)?;
            data.faces = Vec::new(); // It's not used anymore, so deallocate.
        }

        data.states[index].insert(
            view_state.time,
            ElementState {
                vertices: view_state.vertices,
                normals: view_state.normals,
            },
        );

        Ok(())
    }

    async fn render(
        self: &Rc<Self>,
        from: model::Time,
        to: model::Time,
    ) -> Result<()> {
        self.adapter.set_now(from).await;

        self.set_vertices(from)?;
        self.adapter.render_frame()?;

        loop {
            let now = self.adapter.next_frame().await;
            if now > to {
                break;
            }
            self.set_vertices(now)?;
            self.adapter.render_frame()?;
        }

        Ok(())
    }

    pub async fn render_all(self: &Rc<Self>) -> Result<()> {
        let _guard = self.state.try_lock(ControllerState::HandlingOp).unwrap();

        let from;
        let to;
        {
            let data = self.data.borrow();
            if data.states.is_empty() {
                return Ok(());
            }

            from = *data
                .states
                .iter()
                .map(|s| s.keys().next().unwrap_or(&model::Time::MAX))
                .min()
                .unwrap();

            to = *data
                .states
                .iter()
                .map(|s| s.keys().next_back().unwrap_or(&model::Time::MIN))
                .max()
                .unwrap();
        }

        self.render(from, to).await
    }

    pub fn render_moment(self: &Rc<Self>, at: model::Time) -> Result<()> {
        let _guard = self.state.try_lock(ControllerState::HandlingOp).unwrap();
        self.set_vertices(at)?;
        self.adapter.render_frame()
    }

    pub async fn render_period(
        self: &Rc<Self>,
        from: model::Time,
        to: model::Time,
    ) -> Result<()> {
        let _guard = self.state.try_lock(ControllerState::HandlingOp).unwrap();
        self.render(from, to).await
    }

    fn reset(self: &Rc<Self>) {
        let mut data = self.data.borrow_mut();
        data.elements = HashMap::new();
        data.faces = Vec::new();
        data.states = Vec::new();
    }

    pub fn reset_eye_position(self: &Rc<Self>) -> Result<()> {
        let _guard = self.state.try_lock(ControllerState::HandlingOp).unwrap();
        let mut data = self.data.borrow_mut();
        data.eye_pos = DEFAULT_EYE_POSITION;
        self.adapter.set_eye_position(&data.eye_pos)?;
        self.adapter.render_frame()
    }

    fn set_vertices(self: &Rc<Self>, at: model::Time) -> Result<()> {
        let data = self.data.borrow();
        let mut vertices = self.vertices.borrow_mut();

        let states = data.states_at(at);

        for (_, element) in &data.elements {
            let element_state = &states[element.index];
            for (i, (vn, nn)) in element.vertices.iter().enumerate() {
                let j = element.vertex_base as usize + i;
                match element_state {
                    Some(s) => {
                        let k = vn.clone() as usize - 1;
                        vertices[j].vertex = s.vertices[k].clone();
                        let k = nn.clone() as usize - 1;
                        vertices[j].normal = s.normals[k].clone();
                    }
                    None => {
                        vertices[j].vertex = model::Point3::default();
                        vertices[j].normal = model::Point3::default();
                    }
                }
            }
        }

        self.adapter.set_vertices(vertices.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use async_attributes::test;

    use super::*;
    use base::assert_eq_point3;
    use base::util::test::{
        create_reader_with_records, new_element_view_rec,
        new_element_view_state_rec, new_ev_face, new_point2, new_point3,
        MethodMock,
    };

    struct TestAdapterData {
        destroy_mock: MethodMock<(), Result<()>>,
        next_frame_mock: MethodMock<(), model::Time>,
        render_moment_mock: MethodMock<(), Result<()>>,
        set_eye_position_mock: MethodMock<model::Point3, Result<()>>,
        set_faces_mock: MethodMock<Vec<Face>, Result<()>>,
        set_now_mock: MethodMock<model::Time, ()>,
        set_texture_mock: MethodMock<(usize, model::Image), Result<()>>,
        set_vertices_mock: MethodMock<Vec<VertexData>, Result<()>>,
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
                    next_frame_mock: MethodMock::new(),
                    render_moment_mock: MethodMock::new(),
                    set_eye_position_mock: MethodMock::new(),
                    set_faces_mock: MethodMock::new(),
                    set_now_mock: MethodMock::new(),
                    set_texture_mock: MethodMock::new(),
                    set_vertices_mock: MethodMock::new(),
                    subscribe_to_mouse_move_mock: MethodMock::new(),
                    subscribe_to_mouse_wheel_mock: MethodMock::new(),
                }),
            })
        }

        pub fn finish(&self) {
            let data = self.data.borrow();
            data.destroy_mock.finish();
            data.next_frame_mock.finish();
            data.render_moment_mock.finish();
            data.set_eye_position_mock.finish();
            data.set_faces_mock.finish();
            data.set_now_mock.finish();
            data.set_texture_mock.finish();
            data.set_vertices_mock.finish();
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

        async fn next_frame(self: &Rc<Self>) -> model::Time {
            self.data.borrow_mut().next_frame_mock.call(())
        }

        fn render_frame(self: &Rc<Self>) -> Result<()> {
            self.data.borrow_mut().render_moment_mock.call(())
        }

        fn set_faces(self: &Rc<Self>, faces: &[Face]) -> Result<()> {
            self.data.borrow_mut().set_faces_mock.call(faces.to_vec())
        }

        async fn set_now(self: &Rc<Self>, now: model::Time) {
            self.data.borrow_mut().set_now_mock.call(now)
        }

        async fn set_texture(
            self: &Rc<Self>,
            index: usize,
            image: model::Image,
        ) -> Result<()> {
            self.data.borrow_mut().set_texture_mock.call((index, image))
        }

        fn set_vertices(
            self: &Rc<Self>,
            vertices: &[VertexData],
        ) -> Result<()> {
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
            data.set_eye_position_mock.rets.push(Ok(()));
        }

        let controller = Controller::create(adapter).unwrap();

        {
            let mut data = controller.adapter.data.borrow_mut();
            let _ = data.subscribe_to_mouse_move_mock.args.pop().unwrap();
            let _ = data.subscribe_to_mouse_wheel_mock.args.pop().unwrap();
            let args = data.set_eye_position_mock.args.pop().unwrap();
            assert_eq!(args, DEFAULT_EYE_POSITION);
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

        let view = new_simple_view("a");
        let state = new_element_view_state_rec(model::ElementViewState {
            element: format!("a"),
            time: 0,
            vertices: vec![(new_point3(0.0, 0.0, 0.0))],
            normals: vec![(new_point3(0.0, 0.0, 0.0))],
        });
        let view2 = new_simple_view("b");
        let mut reader = create_reader_with_records(&vec![view, state, view2]);

        {
            let mut data = controller.adapter.data.borrow_mut();
            data.set_texture_mock.rets.push(Ok(()));
            data.set_faces_mock.rets.push(Ok(()));
        }

        assert_eq!(
            controller.load(&mut reader).await,
            inconsistent_state_result(
                "view for element 'b' after element view states"
            )
        );

        {
            let mut data = controller.adapter.data.borrow_mut();
            data.set_texture_mock.args.pop().unwrap();
            data.set_faces_mock.args.pop().unwrap();
        }

        controller.adapter.finish();
    }

    #[test]
    async fn test_add_view_duplicate() {
        let controller = create_controller();

        let mut reader = create_reader_with_records(&vec![
            new_simple_view("a"),
            new_simple_view("a"),
        ]);

        {
            let mut data = controller.adapter.data.borrow_mut();
            data.set_texture_mock.rets.push(Ok(()));
        }

        assert_eq!(
            controller.load(&mut reader).await,
            inconsistent_state_result("duplicate view for element 'a'"),
        );

        {
            let mut data = controller.adapter.data.borrow_mut();
            data.set_texture_mock.args.pop().unwrap();
        }

        controller.adapter.finish();
    }

    #[test]
    async fn test_add_view_state_bad_num_of_vertices_normals() {
        async fn test(
            vertices: Vec<model::Point3>,
            normals: Vec<model::Point3>,
            err_desc: &str,
        ) {
            let controller = create_controller();

            let view = new_simple_view("a");
            let state = new_element_view_state_rec(model::ElementViewState {
                element: format!("a"),
                time: 0,
                vertices: vertices,
                normals: normals,
            });
            let mut reader = create_reader_with_records(&vec![view, state]);

            {
                let mut data = controller.adapter.data.borrow_mut();
                data.set_texture_mock.rets.push(Ok(()));
            }

            let err_res = inconsistent_state_result(err_desc);
            assert_eq!(controller.load(&mut reader).await, err_res);

            {
                let mut data = controller.adapter.data.borrow_mut();
                data.set_texture_mock.args.pop().unwrap();
            }

            controller.adapter.finish();
        }

        test(
            vec![new_point3(0.0, 0.0, 0.0), new_point3(0.0, 0.0, 0.0)],
            vec![new_point3(0.0, 0.0, 0.0)],
            "expected 1 view state vertices for element 'a', encountered 2",
        )
        .await;

        test(
            vec![new_point3(0.0, 0.0, 0.0)],
            vec![new_point3(0.0, 0.0, 0.0), new_point3(0.0, 0.0, 0.0)],
            "expected 1 view state normals for element 'a', encountered 2",
        )
        .await;
    }

    #[test]
    async fn test_add_view_state_duplicate() {
        let controller = create_controller();

        let view = new_simple_view("a");
        let state = new_element_view_state_rec(model::ElementViewState {
            element: format!("a"),
            time: 123,
            vertices: vec![new_point3(0.0, 0.0, 0.0)],
            normals: vec![(new_point3(0.0, 0.0, 0.0))],
        });
        let mut reader =
            create_reader_with_records(&vec![view, state.clone(), state]);

        {
            let mut data = controller.adapter.data.borrow_mut();
            data.set_texture_mock.rets.push(Ok(()));
            data.set_faces_mock.rets.push(Ok(()));
        }

        assert_eq!(
            controller.load(&mut reader).await,
            inconsistent_state_result(
                "duplicate view state time 123 for element 'a'"
            ),
        );

        {
            let mut data = controller.adapter.data.borrow_mut();
            data.set_texture_mock.args.pop().unwrap();
            data.set_faces_mock.args.pop().unwrap();
        }

        controller.adapter.finish();
    }

    #[test]
    async fn test_add_view_state_non_monotonic() {
        let controller = create_controller();

        let view = new_simple_view("a");
        let state = model::ElementViewState {
            element: format!("a"),
            time: 123,
            vertices: vec![new_point3(0.0, 0.0, 0.0)],
            normals: vec![(new_point3(0.0, 0.0, 0.0))],
        };
        let mut state2 = state.clone();
        state2.time = 122;
        let mut reader = create_reader_with_records(&vec![
            view,
            new_element_view_state_rec(state),
            new_element_view_state_rec(state2),
        ]);

        {
            let mut data = controller.adapter.data.borrow_mut();
            data.set_texture_mock.rets.push(Ok(()));
            data.set_faces_mock.rets.push(Ok(()));
        }

        assert_eq!(
            controller.load(&mut reader).await,
            inconsistent_state_result(
                "non-monotonic view state time 122 for element 'a'"
            ),
        );

        {
            let mut data = controller.adapter.data.borrow_mut();
            data.set_texture_mock.args.pop().unwrap();
            data.set_faces_mock.args.pop().unwrap();
        }

        controller.adapter.finish();
    }

    #[test]
    async fn test_add_view_state_unknown_element() {
        let controller = create_controller();

        let state = new_element_view_state_rec(model::ElementViewState {
            element: format!("a"),
            time: 0,
            vertices: vec![(new_point3(0.0, 0.0, 0.0))],
            normals: vec![(new_point3(0.0, 0.0, 0.0))],
        });
        let mut reader = create_reader_with_records(&vec![state]);

        assert_eq!(
            controller.load(&mut reader).await,
            inconsistent_state_result("view state for unknown element 'a'"),
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

        let view = new_element_view_rec(model::ElementView {
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

        let mut reader = create_reader_with_records(&vec![view]);

        {
            let mut data = controller.adapter.data.borrow_mut();
            data.set_texture_mock.rets.push(Ok(()));
        }

        controller.load(&mut reader).await.unwrap();

        {
            let data = controller.data.borrow();
            let vertices = controller.vertices.borrow();

            let elements = &data.elements;
            assert_eq!(elements.len(), 1);
            assert_eq!(elements["a"].vertex_base, 0);
            assert_eq!(
                elements["a"].vertices,
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

        let view = new_element_view_rec(model::ElementView {
            element: format!("a"),
            texture: Some(model::Image::default()),
            texture_points: vec![new_point2(0.0, 0.0)],
            faces: vec![new_ev_face(1, 1, 1, 1, 1, 1, 1, 0, 1)],
            ..Default::default()
        });

        let mut reader = create_reader_with_records(&vec![view]);

        assert_eq!(
            controller.load(&mut reader).await,
            inconsistent_state_result(
                "zero normal number in view face for element 'a'"
            ),
        );

        controller.adapter.finish();
    }

    #[test]
    async fn test_add_view_zero_texture_point_number() {
        let controller = create_controller();

        let view = new_element_view_rec(model::ElementView {
            element: format!("a"),
            texture: Some(model::Image::default()),
            texture_points: vec![new_point2(0.0, 0.0)],
            faces: vec![new_ev_face(1, 1, 1, 0, 1, 1, 1, 1, 1)],
            ..Default::default()
        });

        let mut reader = create_reader_with_records(&vec![view]);

        assert_eq!(
            controller.load(&mut reader).await,
            inconsistent_state_result(
                "zero texture point number in view face for element 'a'"
            ),
        );

        controller.adapter.finish();
    }

    #[test]
    async fn test_add_view_zero_vertex_number() {
        let controller = create_controller();

        let view = new_element_view_rec(model::ElementView {
            element: format!("a"),
            texture: Some(model::Image::default()),
            texture_points: vec![new_point2(0.0, 0.0)],
            faces: vec![new_ev_face(1, 1, 0, 1, 1, 1, 1, 1, 1)],
            ..Default::default()
        });

        let mut reader = create_reader_with_records(&vec![view]);

        assert_eq!(
            controller.load(&mut reader).await,
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
    async fn test_interpolate() {
        let controller = create_controller();
        let view_a = new_simple_view("a");
        let view_b = new_simple_view("b");
        let view_c = new_simple_view("c");

        let state_a1 = new_element_view_state_rec(model::ElementViewState {
            element: format!("a"),
            time: 0,
            vertices: vec![new_point3(3.0, 6.0, 12.0)],
            normals: vec![new_point3(6.0, 12.0, 24.0)],
        });

        let state_b1 = new_element_view_state_rec(model::ElementViewState {
            element: format!("b"),
            time: 0,
            vertices: vec![new_point3(1.0, 2.0, 4.0)],
            normals: vec![new_point3(2.0, 4.0, 8.0)],
        });

        let state_a2 = new_element_view_state_rec(model::ElementViewState {
            element: format!("a"),
            time: 10,
            vertices: vec![new_point3(6.0, 12.0, 24.0)],
            normals: vec![new_point3(12.0, 24.0, 48.0)],
        });

        let state_b2 = new_element_view_state_rec(model::ElementViewState {
            element: format!("b"),
            time: 10,
            vertices: vec![new_point3(2.0, 4.0, 8.0)],
            normals: vec![new_point3(4.0, 8.0, 16.0)],
        });

        let state_a3 = new_element_view_state_rec(model::ElementViewState {
            element: format!("a"),
            time: 20,
            vertices: vec![new_point3(11.0, 22.0, 44.0)],
            normals: vec![new_point3(22.0, 44.0, 88.0)],
        });

        let mut reader = create_reader_with_records(&vec![
            view_a, view_b, view_c, state_a1, state_b1, state_a2, state_b2,
            state_a3,
        ]);

        {
            let mut data = controller.adapter.data.borrow_mut();
            data.set_texture_mock.rets.push(Ok(()));
            data.set_texture_mock.rets.push(Ok(()));
            data.set_texture_mock.rets.push(Ok(()));
            data.set_faces_mock.rets.push(Ok(()));
            data.set_vertices_mock.rets.push(Ok(()));
            data.render_moment_mock.rets.push(Ok(()));
        }

        controller.load(&mut reader).await.unwrap();
        controller.render_moment(5).unwrap();

        let vertices;
        {
            let mut data = controller.adapter.data.borrow_mut();
            data.set_texture_mock.args.pop().unwrap();
            data.set_texture_mock.args.pop().unwrap();
            data.set_texture_mock.args.pop().unwrap();
            data.set_faces_mock.args.pop().unwrap();
            vertices = data.set_vertices_mock.args.pop().unwrap();
            data.render_moment_mock.args.pop().unwrap();

            data.set_vertices_mock.rets.push(Ok(()));
            data.render_moment_mock.rets.push(Ok(()));
        }

        controller.render_moment(15).unwrap();

        assert_eq!(vertices.len(), 3);
        assert_eq_point3!(vertices[0].vertex, new_point3(4.25, 8.5, 17.0));
        assert_eq_point3!(vertices[0].normal, new_point3(8.5, 17.0, 34.0));
        assert_eq_point3!(vertices[1].vertex, new_point3(1.5, 3.0, 6.0));
        assert_eq_point3!(vertices[1].normal, new_point3(3.0, 6.0, 12.0));
        assert_eq_point3!(vertices[2].vertex, new_point3(0.0, 0.0, 0.0));
        assert_eq_point3!(vertices[2].normal, new_point3(0.0, 0.0, 0.0));

        let vertices;
        {
            let mut data = controller.adapter.data.borrow_mut();
            vertices = data.set_vertices_mock.args.pop().unwrap();
            data.render_moment_mock.args.pop().unwrap();
        }

        assert_eq!(vertices.len(), 3);
        assert_eq_point3!(vertices[0].vertex, new_point3(8.25, 16.5, 33.0));
        assert_eq_point3!(vertices[0].normal, new_point3(16.5, 33.0, 66.0));
        assert_eq_point3!(vertices[1].vertex, new_point3(2.0, 4.0, 8.0));
        assert_eq_point3!(vertices[1].normal, new_point3(4.0, 8.0, 16.0));
        assert_eq_point3!(vertices[2].vertex, new_point3(0.0, 0.0, 0.0));
        assert_eq_point3!(vertices[2].normal, new_point3(0.0, 0.0, 0.0));
    }

    #[test]
    async fn test_render_moment() {
        let controller = create_controller();

        let view = new_simple_view("a");
        let view2 = new_simple_view("b");
        let view3 = new_simple_view("c");

        let state = new_element_view_state_rec(model::ElementViewState {
            element: format!("a"),
            time: 123,
            vertices: vec![new_point3(0.123, 0.234, 0.345)],
            normals: vec![new_point3(0.456, 0.567, 0.678)],
        });

        let state2 = new_element_view_state_rec(model::ElementViewState {
            element: format!("b"),
            time: 234,
            vertices: vec![new_point3(0.789, 0.890, 0.901)],
            normals: vec![new_point3(0.012, 0.123, 0.234)],
        });

        let state3 = new_element_view_state_rec(model::ElementViewState {
            element: format!("b"),
            time: 345,
            vertices: vec![new_point3(0.345, 0.456, 0.567)],
            normals: vec![new_point3(0.678, 0.789, 0.890)],
        });

        let mut reader = create_reader_with_records(&vec![
            view, view2, view3, state, state2, state3,
        ]);

        {
            let mut data = controller.adapter.data.borrow_mut();
            data.set_texture_mock.rets.push(Ok(()));
            data.set_texture_mock.rets.push(Ok(()));
            data.set_texture_mock.rets.push(Ok(()));
            data.set_faces_mock.rets.push(Ok(()));
            data.set_vertices_mock.rets.push(Ok(()));
            data.render_moment_mock.rets.push(Ok(()));
        }

        controller.load(&mut reader).await.unwrap();
        controller.render_moment(456).unwrap();

        let vertices;
        {
            let mut data = controller.adapter.data.borrow_mut();
            data.set_texture_mock.args.pop().unwrap();
            data.set_texture_mock.args.pop().unwrap();
            data.set_texture_mock.args.pop().unwrap();
            data.set_faces_mock.args.pop().unwrap();
            vertices = data.set_vertices_mock.args.pop().unwrap();
            data.render_moment_mock.args.pop().unwrap();
        }

        assert_eq!(vertices.len(), 3);
        assert_eq!(vertices[0].element, 0);
        assert_eq!(vertices[0].normal, new_point3(0.456, 0.567, 0.678));
        assert_eq!(vertices[0].vertex, new_point3(0.123, 0.234, 0.345));
        assert_eq!(vertices[1].element, 1);
        assert_eq!(vertices[1].normal, new_point3(0.678, 0.789, 0.890));
        assert_eq!(vertices[1].vertex, new_point3(0.345, 0.456, 0.567));
        assert_eq!(vertices[2].element, 2);
        assert_eq!(vertices[2].normal, model::Point3::default());
        assert_eq!(vertices[2].vertex, model::Point3::default());

        controller.adapter.finish();
    }
}
