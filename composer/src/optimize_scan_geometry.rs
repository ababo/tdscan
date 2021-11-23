use std::collections::{BTreeMap};
use std::result::Result as StdResult;

use argmin::core::{
    ArgminKV, ArgminOp, Error as ArgminError, Executor, IterState, Observe,
    ObserverMode,
};
use argmin::solver::gradientdescent::SteepestDescent;
use argmin::solver::linesearch::MoreThuenteLineSearch;
use log::{info, warn};
use structopt::StructOpt;

use crate::point_cloud::{
    build_frame_clouds, distance_between_point_clouds, PointCloudParams,
};
use crate::scan::{read_scans, ScanParams};
use base::defs::{Error, ErrorKind::*, Result};
use base::fm;
use base::util::cli;

#[derive(StructOpt)]
#[structopt(about = "Optimize scan geometry parameters")]
pub struct OptimizeScanGeometryCommand {
    #[structopt(flatten)]
    input: cli::FmInput,

    #[structopt(flatten)]
    output: cli::FmOutput,

    #[structopt(flatten)]
    params: OptimizeScanGeometryParams,
}

impl OptimizeScanGeometryCommand {
    pub fn run(&self) -> Result<()> {
        let mut reader = self.input.get()?;
        let mut writer = self.output.get()?;

        optimize_scan_geometry(
            reader.as_mut(),
            writer.as_mut(),
            &self.params,
        )
    }
}

#[derive(StructOpt)]
pub struct OptimizeScanGeometryParams {
    #[structopt(help = "Angle variability range", long, default_value = "0.2")]
    angle_range: f32,

    #[structopt(
        help = "Distance variability range",
        long,
        default_value = "0.1"
    )]
    distance_range: f32,

    #[structopt(
        help = "Number of iterations",
        long,
        short = "n",
        default_value = "100"
    )]
    num_iters: usize,

    #[structopt(flatten)]
    scan: ScanParams,

    #[structopt(
        help = "Target scan to optimize (all scans if not specified)",
        long = "target-scan",
        number_of_values = 1,
        short = "s"
    )]
    pub target_scans: Vec<String>,

    #[structopt(flatten)]
    point_cloud: PointCloudParams,
}

#[allow(clippy::too_many_arguments)]
pub fn optimize_scan_geometry(
    reader: &mut dyn fm::Read,
    writer: &mut dyn fm::Write,
    params: &OptimizeScanGeometryParams,
) -> Result<()> {
    info!("reading scans...");
    let (scans, scan_frames) = read_scans(reader, &params.scan)?;

    let optimized: Vec<_> = if params.target_scans.is_empty() {
        scans.keys().cloned().collect()
    } else {
        if let Some(target) = params.target_scans
            .iter()
            .find(|t| !scans.contains_key(t.as_str()))
        {
            let desc = format!("unknown target scan '{}'", target);
            return Err(Error::new(InconsistentState, desc));
        }
        params.target_scans.clone()
    };

    let opt = ScanOpt {
        point_cloud_params: &params.point_cloud,
        scans: &scans,
        scan_frames: &scan_frames,
        optimized: optimized.clone(),
        angle_range: params.angle_range,
        distance_range: params.distance_range,
    };

    let mut init_params: Vec<f32> = Vec::new();
    for target in optimized.iter() {
        let scan = scans.get(target).unwrap();

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
    let observer = Observer(optimized);
    let res = Executor::new(opt, solver, init_params)
        .add_observer(observer, ObserverMode::NewBest)
        .max_iters(params.num_iters as u64)
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
            Err(Error::new(ArgminError, desc))
        }
    }
}

struct ScanOpt<'a> {
    point_cloud_params: &'a PointCloudParams,
    scans: &'a BTreeMap<String, fm::Scan>,
    scan_frames: &'a Vec<fm::ScanFrame>,
    optimized: Vec<String>,
    angle_range: f32,
    distance_range: f32,
}

impl<'a> ScanOpt<'a> {
    fn apply_params(
        &self,
        params: &[f32],
    ) -> (BTreeMap<String, fm::Scan>, bool) {
        let mut scans = self.scans.clone();
        let mut ok = true;

        let check_angle =
            |init: f32, val: f32| (val - init).abs() < self.angle_range;
        let check_distance =
            |init: f32, val: f32| (val - init).abs() < self.distance_range;

        for (i, target) in self.optimized.iter().enumerate() {
            let base = i * 7;

            let scan = scans.get_mut(target).unwrap();
            let pos = scan.camera_initial_position.as_mut().unwrap();
            let dir = scan.camera_initial_direction.as_mut().unwrap();

            ok &= check_distance(pos.x, params[base])
                & check_distance(pos.y, params[base + 1])
                & check_distance(pos.z, params[base + 2])
                & check_distance(dir.x, params[base + 3])
                & check_distance(dir.y, params[base + 4])
                & check_distance(dir.z, params[base + 5])
                & check_angle(scan.camera_up_angle, params[base + 6]);
            if !ok {
                warn!("params out of bounds");
            }

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
        for (i, target) in self.0.iter().enumerate() {
            let base = i * 7;
            params += &format!(
                " -y {}={},{},{}",
                target,
                state.best_param[base],
                state.best_param[base + 1],
                state.best_param[base + 2]
            );
            params += &format!(
                " -c {}={},{},{}",
                target,
                state.best_param[base + 3],
                state.best_param[base + 4],
                state.best_param[base + 5]
            );
            params += &format!(" -l {}={}", target, state.best_param[base + 6]);
        }
        info!(
            "iter {}, best {}, params{}",
            state.iter, state.best_cost, params
        );
        Ok(())
    }
}
