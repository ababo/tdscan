use std::mem::transmute;
use std::os::raw;

use num::traits::Float;

#[allow(dead_code)]
#[repr(C)]
pub enum BoundaryType {
    Free,
    Dirichlet,
    Neumann,
}

#[repr(C)]
pub struct Params {
    // Boundary type for the finite elements.
    pub boundary: BoundaryType,

    // The maximum depth of the tree that will be used for surface
    // reconstruction. Running at depth d corresponds to solving on
    // a 2^d x 2^d x 2^d. Note that since the reconstructor adapts
    // the octree to the sampling density, the specified reconstruction
    // depth is only an upper bound.
    pub depth: raw::c_int,

    // The target width of the finest level octree cells (ignored if depth is specified).
    pub finest_cell_width: f32,
    // The ratio between the diameter of the cube used for reconstruction
    // and the diameter of the samples' bounding cube. .Specifies the factor
    // of the bounding cube that the input samples should fit into.
    pub scale: f32,

    // The minimum number of sample points that should fall within an octree
    // node as the octree construction is adapted to sampling density. This
    // parameter specifies the minimum number of points that should fall
    // within an octree node. For noise-free samples, small values in the
    // range [1.0 - 5.0] can be used. For more noisy samples, larger values
    // in the range [15.0 - 20.0] may be needed to provide a smoother
    // noise-reduced, reconstruction.
    pub sample_per_node: f32,

    // The importance that interpolation of the point samples is given
    // in the formulation of the screened Poisson equation. The results
    // of the original (unscreened) Poisson Reconstruction can be obtained
    // by setting this value to 0.
    pub point_weight: f32,

    // The number of solver iterations. Number of Gauss-Seidel relaxations
    // to be performed at each level of the octree hierarchy.
    pub iters: raw::c_int,

    // If this flag is enabled, the sampling density is written out with
    // the vertices.
    pub density: bool,

    // This flag tells the reconstructor to read in color values with
    // the input points and extrapolate those to the vertices of the output.
    pub with_colors: bool,

    // Data pull factor. If withColors is rue, this floating point value
    // specifies the relative importance of finer color estimates over
    // lower ones.
    pub color_pull_factor: f32,

    // Normal confidence exponent. Exponent to be applied to a point's
    // confidence to adjust its weight. A point's confidence is defined
    // by the magnitude of its normal.
    pub normal_confidence: f32,

    // Normal confidence bias exponent. Exponent to be applied to a point's
    // confidence to bias the resolution at which the sample contributes to
    // the linear system. Points with lower confidence are biased to
    // contribute at coarser resolutions.
    pub normal_confidence_bias: f32,

    // Enabling this flag has the reconstructor use linear interpolation to
    // estimate the positions of iso-vertices.
    pub linear_fit: bool,

    // This parameter specifies the number of threads across which the solver
    // should be parallelized.
    pub threads: raw::c_int,

    // The depth beyond which the octree will be adapted. At coarser depths,
    // the octree will be complete, containing all 2^d x 2^d x 2^d nodes.
    pub full_depth: raw::c_int,

    // Coarse MG solver depth.
    pub base_depth: raw::c_int,

    // Coarse MG solver v-cycles.
    pub base_v_cycles: raw::c_int,

    // This flag specifies the accuracy cut-off to be used for CG.
    pub cg_accuracy: f32,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            boundary: BoundaryType::Neumann,
            depth: 8,
            finest_cell_width: 0.0,
            scale: 1.1,
            sample_per_node: 1.5,
            point_weight: 2.0,
            iters: 8,
            density: false,
            with_colors: true,
            color_pull_factor: 32.0,
            normal_confidence: 0.0,
            normal_confidence_bias: 0.0,
            linear_fit: false,
            threads: 1,
            full_depth: 5,
            base_depth: 0,
            base_v_cycles: 1,
            cg_accuracy: 1.0e-3,
        }
    }
}

pub trait Cloud<F: Float> {
    fn len(&self) -> usize;
    fn has_normals(&self) -> bool {
        false
    }
    fn has_colors(&self) -> bool {
        false
    }
    fn point(&self, _index: usize) -> [F; 3];
    fn normal(&self, _index: usize) -> [F; 3] {
        [F::nan(), F::nan(), F::nan()]
    }
    fn color(&self, _index: usize) -> [F; 3] {
        [F::nan(), F::nan(), F::nan()]
    }
}

pub trait Mesh<F: Float> {
    fn add_vertex(&mut self, vertex: &[F; 3]);
    fn add_normal(&mut self, _normal: &[F; 3]) {}
    fn add_color(&mut self, _color: &[F; 3]) {}
    fn add_density(&mut self, _density: f64) {}
    fn add_triangle(&mut self, triangle: &[usize; 3]);
}

pub trait Reconstruct<F: Float> {
    fn reconstruct(
        params: &Params,
        cloud: &dyn Cloud<F>,
        mesh: &mut dyn Mesh<F>,
    ) -> bool;
}

impl Reconstruct<f32> for f32 {
    fn reconstruct(
        params: &Params,
        cloud: &dyn Cloud<f32>,
        mesh: &mut dyn Mesh<f32>,
    ) -> bool {
        unsafe {
            poisson_reconstruct32(params, transmute(cloud), transmute(mesh))
        }
    }
}

impl Reconstruct<f64> for f64 {
    fn reconstruct(
        params: &Params,
        cloud: &dyn Cloud<f64>,
        mesh: &mut dyn Mesh<f64>,
    ) -> bool {
        unsafe {
            poisson_reconstruct64(params, transmute(cloud), transmute(mesh))
        }
    }
}

pub fn reconstruct<F: Float + Reconstruct<F>>(
    params: &Params,
    cloud: &dyn Cloud<F>,
    mesh: &mut dyn Mesh<F>,
) -> bool {
    F::reconstruct(params, cloud, mesh)
}

#[repr(C)]
pub struct TraitObj(usize, usize);

extern "C" {
    fn poisson_reconstruct32(
        params: &Params,
        cloud: TraitObj,
        mesh: TraitObj,
    ) -> bool;

    fn poisson_reconstruct64(
        params: &Params,
        cloud: TraitObj,
        mesh: TraitObj,
    ) -> bool;
}

#[no_mangle]
pub unsafe extern "C" fn poisson_cloud32_size(cloud: TraitObj) -> usize {
    transmute::<TraitObj, &dyn Cloud<f32>>(cloud).len()
}

#[no_mangle]
pub unsafe extern "C" fn poisson_cloud32_has_normals(cloud: TraitObj) -> bool {
    transmute::<TraitObj, &dyn Cloud<f32>>(cloud).has_normals()
}

#[no_mangle]
pub unsafe extern "C" fn poisson_cloud32_has_colors(cloud: TraitObj) -> bool {
    transmute::<TraitObj, &dyn Cloud<f32>>(cloud).has_colors()
}

#[no_mangle]
pub unsafe extern "C" fn poisson_cloud32_get_point(
    cloud: TraitObj,
    index: usize,
    coords: &mut [f32; 3],
) {
    *coords = transmute::<TraitObj, &dyn Cloud<f32>>(cloud).point(index);
}

#[no_mangle]
pub unsafe extern "C" fn poisson_cloud32_get_normal(
    cloud: TraitObj,
    index: usize,
    coords: &mut [f32; 3],
) {
    *coords = transmute::<TraitObj, &dyn Cloud<f32>>(cloud).normal(index);
}

#[no_mangle]
pub unsafe extern "C" fn poisson_cloud32_get_color(
    cloud: TraitObj,
    index: usize,
    rgb: &mut [f32; 3],
) {
    *rgb = transmute::<TraitObj, &dyn Cloud<f32>>(cloud).color(index);
}

#[no_mangle]
pub unsafe extern "C" fn poisson_mesh32_add_vertex(
    mesh: TraitObj,
    coords: &[f32; 3],
) {
    transmute::<TraitObj, &mut dyn Mesh<f32>>(mesh).add_vertex(coords);
}

#[no_mangle]
pub unsafe extern "C" fn poisson_mesh32_add_normal(
    mesh: TraitObj,
    coords: &[f32; 3],
) {
    transmute::<TraitObj, &mut dyn Mesh<f32>>(mesh).add_normal(coords);
}

#[no_mangle]
pub unsafe extern "C" fn poisson_mesh32_add_color(
    mesh: TraitObj,
    rgb: &[f32; 3],
) {
    transmute::<TraitObj, &mut dyn Mesh<f32>>(mesh).add_color(rgb);
}

#[no_mangle]
pub unsafe extern "C" fn poisson_mesh32_add_density(mesh: TraitObj, d: f64) {
    transmute::<TraitObj, &mut dyn Mesh<f32>>(mesh).add_density(d);
}

#[no_mangle]
pub unsafe extern "C" fn poisson_mesh32_add_triangle(
    mesh: TraitObj,
    i1: usize,
    i2: usize,
    i3: usize,
) {
    transmute::<TraitObj, &mut dyn Mesh<f32>>(mesh).add_triangle(&[i1, i2, i3]);
}

#[no_mangle]
pub unsafe extern "C" fn poisson_cloud64_size(cloud: TraitObj) -> usize {
    transmute::<TraitObj, &dyn Cloud<f64>>(cloud).len()
}

#[no_mangle]
pub unsafe extern "C" fn poisson_cloud64_has_normals(cloud: TraitObj) -> bool {
    transmute::<TraitObj, &dyn Cloud<f64>>(cloud).has_normals()
}

#[no_mangle]
pub unsafe extern "C" fn poisson_cloud64_has_colors(cloud: TraitObj) -> bool {
    transmute::<TraitObj, &dyn Cloud<f64>>(cloud).has_colors()
}

#[no_mangle]
pub unsafe extern "C" fn poisson_cloud64_get_point(
    cloud: TraitObj,
    index: usize,
    coords: &mut [f64; 3],
) {
    *coords = transmute::<TraitObj, &dyn Cloud<f64>>(cloud).point(index);
}

#[no_mangle]
pub unsafe extern "C" fn poisson_cloud64_get_normal(
    cloud: TraitObj,
    index: usize,
    coords: &mut [f64; 3],
) {
    *coords = transmute::<TraitObj, &dyn Cloud<f64>>(cloud).normal(index);
}

#[no_mangle]
pub unsafe extern "C" fn poisson_cloud64_get_color(
    cloud: TraitObj,
    index: usize,
    rgb: &mut [f64; 3],
) {
    *rgb = transmute::<TraitObj, &dyn Cloud<f64>>(cloud).color(index);
}

#[no_mangle]
pub unsafe extern "C" fn poisson_mesh64_add_vertex(
    mesh: TraitObj,
    coords: &[f64; 3],
) {
    transmute::<TraitObj, &mut dyn Mesh<f64>>(mesh).add_vertex(coords);
}

#[no_mangle]
pub unsafe extern "C" fn poisson_mesh64_add_normal(
    mesh: TraitObj,
    coords: &[f64; 3],
) {
    transmute::<TraitObj, &mut dyn Mesh<f64>>(mesh).add_normal(coords);
}

#[no_mangle]
pub unsafe extern "C" fn poisson_mesh64_add_color(
    mesh: TraitObj,
    rgb: &[f64; 3],
) {
    transmute::<TraitObj, &mut dyn Mesh<f64>>(mesh).add_color(rgb);
}

#[no_mangle]
pub unsafe extern "C" fn poisson_mesh64_add_density(mesh: TraitObj, d: f64) {
    transmute::<TraitObj, &mut dyn Mesh<f64>>(mesh).add_density(d);
}

#[no_mangle]
pub unsafe extern "C" fn poisson_mesh64_add_triangle(
    mesh: TraitObj,
    i1: usize,
    i2: usize,
    i3: usize,
) {
    transmute::<TraitObj, &mut dyn Mesh<f64>>(mesh).add_triangle(&[i1, i2, i3]);
}
