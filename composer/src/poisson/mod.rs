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

#[repr(C)]
pub struct Cloud<F: Float> {
    pub vertices: Vec<[F; 3]>,
}

#[repr(C)]
pub struct Mesh<F: Float> {
    pub vertices: Vec<[F; 3]>,
    pub faces: Vec<[usize; 3]>,
}

impl<F: Float> Mesh<F> {
    fn new() -> Self {
        Mesh {
            vertices: vec![],
            faces: vec![],
        }
    }
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

pub trait Reconstruct<F: Float> {
    fn reconstruct(cloud: &Cloud<F>, params: &Params) -> Option<Mesh<F>>;
}

impl Reconstruct<f32> for f32 {
    fn reconstruct(cloud: &Cloud<f32>, params: &Params) -> Option<Mesh<f32>> {
        let mut mesh = Mesh::new();
        if unsafe {
            poisson_reconstruct32(
                params,
                cloud as *const Cloud<f32> as *const raw::c_void,
                &mut mesh as *mut Mesh<f32> as *mut raw::c_void,
            )
        } {
            Some(mesh)
        } else {
            None
        }
    }
}

impl Reconstruct<f64> for f64 {
    fn reconstruct(cloud: &Cloud<f64>, params: &Params) -> Option<Mesh<f64>> {
        let mut mesh = Mesh::new();
        if unsafe {
            poisson_reconstruct64(
                params,
                cloud as *const Cloud<f64> as *const raw::c_void,
                &mut mesh as *mut Mesh<f64> as *mut raw::c_void,
            )
        } {
            Some(mesh)
        } else {
            None
        }
    }
}

pub fn reconstruct<F: Float + Reconstruct<F>>(
    cloud: &Cloud<F>,
    params: &Params,
) -> Option<Mesh<F>> {
    F::reconstruct(cloud, params)
}

extern "C" {
    fn poisson_reconstruct32(
        params: &Params,
        cloud: *const raw::c_void,
        mesh: *mut raw::c_void,
    ) -> bool;

    fn poisson_reconstruct64(
        params: &Params,
        cloud: *const raw::c_void,
        mesh: *mut raw::c_void,
    ) -> bool;
}
