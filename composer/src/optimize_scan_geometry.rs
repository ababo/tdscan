use std::collections::{BTreeMap, HashMap};
use std::f32::INFINITY;
use std::path::PathBuf;
use std::result::Result as StdResult;

use argmin::core::{
    ArgminKV, ArgminOp, Error as ArgminError, Executor, IterState, Observe,
    ObserverMode,
};
use argmin::solver::gradientdescent::SteepestDescent;
use argmin::solver::linesearch::MoreThuenteLineSearch;
use glam::Vec3;
use nalgebra::DMatrix;
use structopt::StructOpt;

use crate::misc::{
    fm_reader_from_file_or_stdin, fm_writer_to_file_or_stdout, read_scans,
};
use crate::point_cloud::{build_point_clouds, PointCloudParams};
use base::defs::{Error, ErrorKind, Result};
use base::fm;

#[derive(StructOpt)]
#[structopt(about = "Optimize scan geometry parameters")]
pub struct OptimizeScanGeometryParams {
    #[structopt(help = "Input scan .fm file (STDIN if omitted)")]
    in_path: Option<PathBuf>,

    #[structopt(
        help = "Angle parameter variability",
        long,
        short = "a",
        default_value = "0.174533" // 10 degrees.
    )]
    angle_variability: f32,

    #[structopt(
        help = "Distance parameter variability",
        long,
        short = "i",
        default_value = "0.1"
    )]
    distance_variability: f32,

    #[structopt(
        help = "Size of cell for roughness calculation",
        long,
        short = "c",
        default_value = "0.02"
    )]
    cell_size: f32,

    #[structopt(
        help = "Number of iterations",
        long,
        short = "n",
        default_value = "100"
    )]
    num_iters: usize,

    #[structopt(flatten)]
    point_cloud_params: PointCloudParams,

    #[structopt(
        help = "Output scan view .fm file (STDOUT if omitted)",
        long,
        short = "o"
    )]
    out_path: Option<PathBuf>,

    #[structopt(flatten)]
    fm_write_params: fm::WriterParams,
}

pub fn optimize_scan_geometry_with_params(
    params: &OptimizeScanGeometryParams,
) -> Result<()> {
    let mut reader = fm_reader_from_file_or_stdin(&params.in_path)?;

    let mut writer =
        fm_writer_to_file_or_stdout(&params.out_path, &params.fm_write_params)?;

    optimize_scan_geometry(
        reader.as_mut(),
        params.angle_variability,
        params.distance_variability,
        params.cell_size,
        params.num_iters,
        &params.point_cloud_params,
        writer.as_mut(),
    )
}

pub fn optimize_scan_geometry(
    reader: &mut dyn fm::Read,
    angle_variability: f32,
    distance_variability: f32,
    cell_size: f32,
    num_iters: usize,
    point_cloud_params: &PointCloudParams,
    writer: &mut dyn fm::Write,
) -> Result<()> {
    let (scans, scan_frames) = read_scans(reader)?;

    let opt = ScanOpt {
        angle_variability,
        distance_variability,
        cell_size,
        point_cloud_params,
        scans: &scans,
        scan_frames: &scan_frames,
    };

    let mut init_params: Vec<f32> = Vec::new();
    for (_, scan) in &scans {
        let pos = scan.camera_initial_position.unwrap();
        init_params.push(pos.x);
        init_params.push(pos.y);
        init_params.push(pos.z);
        init_params.push(scan.camera_view_elevation);
        init_params.push(scan.camera_landscape_angle);
    }

    let linesearch = MoreThuenteLineSearch::new();
    let solver = SteepestDescent::new(linesearch);
    let observer = Observer(scans.keys().cloned().collect());
    let res = Executor::new(opt, solver, init_params)
        .add_observer(observer, ObserverMode::NewBest)
        .max_iters(num_iters as u64)
        .run();

    match res {
        Ok(ares) => {
            let opt = ares.operator;
            let (scans, _) = opt.apply_params(&ares.state.best_param);

            use fm::record::Type;
            for (_, scan) in scans {
                writer.write_record(&fm::Record {
                    r#type: Some(Type::Scan(scan)),
                })?;
            }

            for frame in scan_frames {
                writer.write_record(&fm::Record {
                    r#type: Some(Type::ScanFrame(frame)),
                })?;
            }

            Ok(())
        }
        Err(err) => {
            let desc = format!("failed to find scan geometry optimum: {}", err);
            Err(Error::new(ErrorKind::ArgminError, desc))
        }
    }
}

struct ScanOpt<'a> {
    angle_variability: f32,
    distance_variability: f32,
    cell_size: f32,
    point_cloud_params: &'a PointCloudParams,
    scans: &'a BTreeMap<String, fm::Scan>,
    scan_frames: &'a Vec<fm::ScanFrame>,
}

impl<'a> ScanOpt<'a> {
    fn apply_param(src: f32, dst: &mut f32, variability: f32) -> f32 {
        let diff = (*dst - src).abs();
        *dst = src;
        if diff > variability {
            diff - variability
        } else {
            0.0
        }
    }

    fn apply_params(
        &self,
        params: &Vec<f32>,
    ) -> (BTreeMap<String, fm::Scan>, f32) {
        let (avar, dvar) = (self.angle_variability, self.distance_variability);
        let mut scans = self.scans.clone();
        let mut deviation = 0.0;

        for (i, scan) in scans.values_mut().enumerate() {
            let base = i * 5;

            let pos = scan.camera_initial_position.as_mut().unwrap();
            deviation += Self::apply_param(params[base + 0], &mut pos.x, dvar);
            deviation += Self::apply_param(params[base + 1], &mut pos.y, dvar);
            deviation += Self::apply_param(params[base + 2], &mut pos.z, dvar);

            let elev = &mut scan.camera_view_elevation;
            deviation += Self::apply_param(params[base + 3], elev, dvar);

            let lang = &mut scan.camera_landscape_angle;
            deviation += Self::apply_param(params[base + 4], lang, avar);
        }

        (scans, deviation)
    }
}

impl<'a> ArgminOp for ScanOpt<'a> {
    type Param = Vec<f32>;
    type Output = f32;
    type Hessian = ();
    type Jacobian = ();
    type Float = f32;

    fn apply(&self, p: &Self::Param) -> StdResult<Self::Output, ArgminError> {
        let (scans, param_deviation) = self.apply_params(p);

        let mut point_cloud_params = self.point_cloud_params.clone();
        point_cloud_params.min_z = -INFINITY;
        point_cloud_params.max_z = INFINITY;

        let clouds =
            build_point_clouds(&scans, self.scan_frames, &point_cloud_params);
        let points: Vec<_> = clouds.into_iter().flatten().collect();

        let (min_z, max_z) =
            points.iter().fold((INFINITY, -INFINITY), |mut b, p| {
                if p[2] < b.0 {
                    b.0 = p[2];
                } else if p[2] > b.1 {
                    b.1 = p[2];
                }
                b
            });

        let mut z_deviation = 0.0;
        if min_z < self.point_cloud_params.min_z {
            z_deviation += self.point_cloud_params.min_z - min_z;
        }
        if max_z > self.point_cloud_params.max_z {
            z_deviation += max_z - self.point_cloud_params.max_z;
        }

        const NAN_ROUGHNESS: f32 = 1000.0;
        const PARAM_PENALTY_FACTOR: f32 = 1000.0;
        const Z_PENALTY_FACTOR: f32 = 10.0;

        let roughness = compute_roughness(&points, self.cell_size);
        Ok(if roughness.is_nan() {
            NAN_ROUGHNESS
        } else {
            let penalty = 1.0
                + param_deviation * PARAM_PENALTY_FACTOR
                + z_deviation * Z_PENALTY_FACTOR;
            roughness * penalty
        })
    }

    fn gradient(&self, p: &Self::Param) -> StdResult<Self::Param, ArgminError> {
        const DELTA: f32 = 0.001;
        let mut params = p.clone();
        let base = self.apply(p).unwrap();
        let mut grad = Vec::with_capacity(p.len());

        for (i, param) in p.iter().enumerate() {
            params[i] = *param + DELTA;
            grad.push((self.apply(&params).unwrap() - base) / DELTA);
            params[i] = *param;
        }

        Ok(grad)
    }
}

struct Observer(Vec<String>);

impl<'a> Observe<ScanOpt<'a>> for Observer {
    fn observe_iter(
        &mut self,
        state: &IterState<ScanOpt>,
        _kv: &ArgminKV,
    ) -> StdResult<(), ArgminError> {
        eprint!("{} {}", state.iter, state.best_cost);
        for (i, scan) in self.0.iter().enumerate() {
            let base = i * 5;
            eprint!(
                " -y {}={},{},{}",
                scan,
                state.best_param[base + 0],
                state.best_param[base + 1],
                state.best_param[base + 2]
            );
            eprint!(" -e {}={}", scan, state.best_param[base + 3]);
            eprint!(" -l {}={}", scan, state.best_param[base + 4]);
        }
        eprintln!();
        Ok(())
    }
}

// Plane defined in Hessian normal form.
struct Plane {
    n: Vec3,
    p: f32,
}

fn compute_best_fitting_plane(points: &[Vec3]) -> Option<Plane> {
    let data = points.iter().map(|p| p.as_ref()).flatten().cloned();
    let points = DMatrix::from_iterator(points.len(), 3, data);

    let means = points.row_mean();
    let mut points = points.transpose();
    points.row_mut(0).add_scalar_mut(-means[0]);
    points.row_mut(1).add_scalar_mut(-means[1]);
    points.row_mut(2).add_scalar_mut(-means[2]);

    if let Some(u) = points.svd(true, false).u {
        let normal = u.column(0);
        let normal = Vec3::new(normal[0], normal[1], normal[2]);
        let centroid = Vec3::new(means[0], means[1], means[2]);
        Some(Plane {
            n: Vec3::new(normal[0], normal[1], normal[2]),
            p: -normal.dot(centroid),
        })
    } else {
        None
    }
}

fn compute_roughness(points: &[Vec3], cell_size: f32) -> f32 {
    let init = Vec3::new(INFINITY, INFINITY, INFINITY);
    let base = points.iter().fold(init, |mut b, p| {
        if p.x < b.x {
            b.x = p.x;
        }
        if p.y < b.y {
            b.y = p.y;
        }
        if p.z < b.z {
            b.z = p.z;
        }
        b
    });

    let mut cells = HashMap::new();
    for p in points {
        let key = (
            ((p.x - base.x) / cell_size) as usize,
            ((p.y - base.y) / cell_size) as usize,
            ((p.z - base.z) / cell_size) as usize,
        );
        let cell = cells.entry(key).or_insert_with(|| Vec::new());
        cell.push(*p);
    }

    let mut sum = 0.0;
    let mut total = 0;
    for cell in cells.values() {
        if let Some(plane) = compute_best_fitting_plane(cell) {
            for p in cell {
                let distance = plane.n.dot(*p) + plane.p;
                sum += distance.abs();
                total += 1;
            }
        }
    }

    sum / total as f32
}
