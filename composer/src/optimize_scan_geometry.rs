use std::cell::Cell;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::result::Result as StdResult;

use argmin::core::{
    ArgminKV, ArgminOp, Error as ArgminError, Executor, IterState, Observe,
    ObserverMode,
};
use argmin::solver::gradientdescent::SteepestDescent;
use argmin::solver::linesearch::MoreThuenteLineSearch;
use glam::Vec3;
use structopt::StructOpt;
use rand::Rng;

use crate::misc::{
    fm_reader_from_file_or_stdin, fm_writer_to_file_or_stdout, read_scans,
};
use crate::point_cloud::{
    build_point_clouds, clouds_distance, PointCloudParams,
};
use base::defs::{Error, ErrorKind, Result};
use base::fm;

#[derive(StructOpt)]
#[structopt(about = "Optimize scan geometry parameters")]
pub struct OptimizeScanGeometryParams {
    #[structopt(help = "Input scan .fm file (STDIN if omitted)")]
    in_path: Option<PathBuf>,

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
        params.num_iters,
        &params.point_cloud_params,
        writer.as_mut(),
    )
}

pub fn optimize_scan_geometry(
    reader: &mut dyn fm::Read,
    num_iters: usize,
    point_cloud_params: &PointCloudParams,
    writer: &mut dyn fm::Write,
) -> Result<()> {
    let (scans, scan_frames) = read_scans(reader)?;

    let opt = ScanOpt {
        point_cloud_params,
        scans: &scans,
        scan_frames: &scan_frames,
        num_points: Cell::new(None),
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
            let scans = opt.apply_params(&ares.state.best_param);

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
    point_cloud_params: &'a PointCloudParams,
    scans: &'a BTreeMap<String, fm::Scan>,
    scan_frames: &'a Vec<fm::ScanFrame>,
    num_points: Cell<Option<usize>>,
}

impl<'a> ScanOpt<'a> {
    fn apply_params(&self, params: &Vec<f32>) -> BTreeMap<String, fm::Scan> {
        let mut scans = self.scans.clone();

        for (i, scan) in scans.values_mut().enumerate() {
            let base = i * 5;

            let pos = scan.camera_initial_position.as_mut().unwrap();
            pos.x = params[base + 0];
            pos.y = params[base + 1];
            pos.z = params[base + 2];

            scan.camera_view_elevation = params[base + 3];
            scan.camera_landscape_angle = params[base + 4];
        }

        scans
    }

    fn select_random_points(points: &mut Vec<Vec3>, num: usize) {
        if num >= points.len() {
            return;
        }

        let mut rng = rand::thread_rng();
        for i in 0..num {
            let j = rng.gen_range(i..points.len());
            points.swap(i, j);
        }

        points.resize(num, Vec3::default());
    }
}

impl<'a> ArgminOp for ScanOpt<'a> {
    type Param = Vec<f32>;
    type Output = f32;
    type Hessian = ();
    type Jacobian = ();
    type Float = f32;

    fn apply(&self, p: &Self::Param) -> StdResult<Self::Output, ArgminError> {
        let scans = self.apply_params(p);

        let mut clouds = build_point_clouds(
            &scans,
            self.scan_frames,
            &self.point_cloud_params,
        );

        for cloud in clouds.iter_mut() {
            ScanOpt::select_random_points(cloud, 500);
        }

        let num_points = clouds.iter().fold(0, |b, c| b + c.len());
        if let Some(num) = self.num_points.get() {
            if num > num_points {
                // Fine for point loss.
                return Ok(1.0);
            }
        } else {
            self.num_points.set(Some(num_points));
        }

        let mut sum = 0.0;
        let mut num = 0;
        for i in 0..clouds.len() - 1 {
            if let Some(dist) = clouds_distance(&clouds[i], &clouds[i + 1], 5) {
                sum += dist;
                num += 1;
            } else {
                return Ok(1.0);
            }
        }

        Ok(sum / num as f32)
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
