use std::collections::BTreeMap;
use std::f32::consts::PI;
use std::path::PathBuf;
use std::result::Result as StdResult;

use argmin::core::{
    ArgminKV, ArgminOp, Error as ArgminError, Executor, IterState, Observe,
    ObserverMode,
};
use argmin::solver::gradientdescent::SteepestDescent;
use argmin::solver::linesearch::MoreThuenteLineSearch;
use log::info;
use structopt::StructOpt;

use crate::misc::{
    fm_reader_from_file_or_stdin, fm_writer_to_file_or_stdout, read_scans,
    ScanParams,
};
use crate::point_cloud::{
    build_frame_clouds, distance_between_point_clouds, PointCloudParams,
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
    scan_params: ScanParams,

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
        &params.scan_params,
        &params.point_cloud_params,
        writer.as_mut(),
    )
}

pub fn optimize_scan_geometry(
    reader: &mut dyn fm::Read,
    num_iters: usize,
    scan_params: &ScanParams,
    point_cloud_params: &PointCloudParams,
    writer: &mut dyn fm::Write,
) -> Result<()> {
    info!("reading scans...");
    let (scans, scan_frames) = read_scans(reader, scan_params)?;

    let opt = ScanOpt {
        point_cloud_params,
        scans: &scans,
        scan_frames: &scan_frames,
    };

    let mut init_params: Vec<f32> = Vec::new();
    for scan in scans.values() {
        let pos = scan.camera_initial_position.unwrap();
        init_params.push(pos.x);
        init_params.push(pos.y);
        init_params.push(pos.z);

        let dir = scan.camera_initial_direction.unwrap();
        init_params.push(dir.x);
        init_params.push(dir.y);
        init_params.push(dir.z);

        init_params.push(scan.camera_up_angle);
    }

    info!("starting more-thuente line search...");
    let linesearch = MoreThuenteLineSearch::new();
    let solver = SteepestDescent::new(linesearch);
    let observer = Observer(scans.keys().cloned().collect());
    let res = Executor::new(opt, solver, init_params)
        .add_observer(observer, ObserverMode::NewBest)
        .max_iters(num_iters as u64)
        .run();

    match res {
        Ok(ares) => {
            info!("writing scans with updated geometry...");

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

            info!("done");
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
}

impl<'a> ScanOpt<'a> {
    fn apply_params(
        &self,
        params: &[f32],
    ) -> (BTreeMap<String, fm::Scan>, bool) {
        let mut scans = self.scans.clone();
        let mut ok = true;

        let check_distance = |init: f32, val: f32| (val - init).abs() < 0.1;
        let check_angle =
            |init: f32, val: f32| (val - init).abs() < 10.0 * PI / 180.0;

        for (i, scan) in scans.values_mut().enumerate() {
            let base = i * 7;

            let pos = scan.camera_initial_position.as_mut().unwrap();
            let dir = scan.camera_initial_direction.as_mut().unwrap();

            ok &= check_distance(pos.x, params[base])
                & check_distance(pos.y, params[base + 1])
                & check_distance(pos.z, params[base + 2])
                & check_distance(dir.x, params[base + 3])
                & check_distance(dir.y, params[base + 4])
                & check_distance(dir.z, params[base + 5])
                & check_angle(scan.camera_up_angle, params[base + 6]);

            pos.x = params[base];
            pos.y = params[base + 1];
            pos.z = params[base + 2];

            dir.x = params[base + 3];
            dir.y = params[base + 4];
            dir.z = params[base + 5];

            scan.camera_up_angle = params[base + 6];
        }

        (scans, ok)
    }
}

const PENALTY: f32 = 1.0;

impl<'a> ArgminOp for ScanOpt<'a> {
    type Param = Vec<f32>;
    type Output = f32;
    type Hessian = ();
    type Jacobian = ();
    type Float = f32;

    fn apply(&self, p: &Self::Param) -> StdResult<Self::Output, ArgminError> {
        let (scans, ok) = self.apply_params(p);
        if !ok {
            return Ok(PENALTY);
        }

        let clouds = build_frame_clouds(
            &scans,
            self.scan_frames,
            self.point_cloud_params,
        );

        let mut sum = 0.0;
        let mut num = 0;
        for i in 0..clouds.len() {
            if let Some(dist) = distance_between_point_clouds(
                &clouds[i],
                &clouds[(i + 1) % clouds.len()],
            ) {
                sum += dist;
                num += 1;
            } else {
                return Ok(PENALTY);
            }
        }

        Ok(sum as f32 / num as f32)
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
        let mut params = String::new();
        for (i, scan) in self.0.iter().enumerate() {
            let base = i * 7;
            params += &format!(
                " -y {}={},{},{}",
                scan,
                state.best_param[base],
                state.best_param[base + 1],
                state.best_param[base + 2]
            );
            params += &format!(
                " -c {}={},{},{}",
                scan,
                state.best_param[base + 3],
                state.best_param[base + 4],
                state.best_param[base + 5]
            );
            params += &format!(" -l {}={}", scan, state.best_param[base + 6]);
        }
        info!(
            "iter {}, best {}, params{}",
            state.iter, state.best_cost, params
        );
        Ok(())
    }
}
