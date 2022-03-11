use std::result::Result as StdResult;
use std::collections::HashMap;

use argmin::core::{
    ArgminKV, ArgminOp, Error as ArgminError, Executor, IterState, Observe,
    ObserverMode,
};
use argmin::solver::gradientdescent::SteepestDescent;
use argmin::solver::linesearch::MoreThuenteLineSearch;
use indexmap::IndexMap;
use log::info;
use structopt::StructOpt;

use crate::point_cloud::{
    build_frame_clouds, distance_between_point_clouds, Matrix4,
    PointCloudParams, PointNormal, Vector3, Vector4,
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

        optimize_scan_geometry(reader.as_mut(), writer.as_mut(), &self.params)
    }
}

#[derive(StructOpt)]
pub struct OptimizeScanGeometryParams {
    #[structopt(
        help = "Match scan clouds without altering their shapes",
        long,
        short = "i"
    )]
    match_scans: bool,

    #[structopt(
        help = "Number of iterations",
        long,
        short = "n",
        default_value = "100"
    )]
    num_iters: usize,

    #[structopt(
        help = "Scan to optimize (all the scans if not specified)",
        long = "optimized-scan",
        number_of_values = 1,
        short = "s"
    )]
    pub optimized_scans: Vec<String>,

    #[structopt(flatten)]
    point_cloud: PointCloudParams,

    #[structopt(flatten)]
    scan: ScanParams,
}

#[allow(clippy::too_many_arguments)]
pub fn optimize_scan_geometry(
    reader: &mut dyn fm::Read,
    writer: &mut dyn fm::Write,
    params: &OptimizeScanGeometryParams,
) -> Result<()> {
    info!("reading scans...");
    let (mut scans, scan_frames) = read_scans(reader, &params.scan)?;

    params
        .point_cloud
        .validate(scans.keys().map(String::as_str))?;

    let optimized: Vec<_> = if params.optimized_scans.is_empty() {
        scans.keys().cloned().collect()
    } else {
        if let Some(target) = params
            .optimized_scans
            .iter()
            .find(|t| !scans.contains_key(t.as_str()))
        {
            let desc = format!("unknown target scan '{}'", target);
            return Err(Error::new(InconsistentState, desc));
        }
        params.optimized_scans.clone()
    };

    let mut init_params = Vec::new();
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

    let res = if params.match_scans {
        match_scans(params, &scans, &scan_frames, &optimized, init_params)
    } else {
        match_frames(params, &scans, &scan_frames, &optimized, init_params)
    };

    match res {
        Ok(best_params) => {
            info!("writing scans with updated geometry...");

            apply_geometry_params(&mut scans, &optimized, &best_params);

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
            let desc = format!("failed to find optimum: {}", err);
            Err(Error::new(ArgminError, desc))
        }
    }
}

#[allow(clippy::ptr_arg)]
fn match_frames(
    params: &OptimizeScanGeometryParams,
    scans: &IndexMap<String, fm::Scan>,
    scan_frames: &Vec<fm::ScanFrame>,
    optimized: &Vec<String>,
    init_params: Vec<f32>,
) -> StdResult<Vec<f32>, ArgminError> {
    let op = FrameOp {
        point_cloud_params: &params.point_cloud,
        scans,
        scan_frames,
        optimized: optimized.clone(),
    };

    let linesearch = MoreThuenteLineSearch::new();
    let solver = SteepestDescent::new(linesearch);
    let observer = FrameObserver(optimized.clone());
    let res = Executor::new(op, solver, init_params)
        .add_observer(observer, ObserverMode::NewBest)
        .max_iters(params.num_iters as u64)
        .run()?;

    Ok(res.state.best_param)
}

fn apply_geometry_params(
    scans: &mut IndexMap<String, fm::Scan>,
    optimized: &[String],
    params: &[f32],
) {
    for (i, target) in optimized.iter().enumerate() {
        let base = i * 7;

        let scan = scans.get_mut(target).unwrap();
        let pos = scan.camera_initial_position.as_mut().unwrap();
        let dir = scan.camera_initial_direction.as_mut().unwrap();

        pos.x = params[base];
        pos.y = params[base + 1];
        pos.z = params[base + 2];

        dir.x = params[base + 3];
        dir.y = params[base + 4];
        dir.z = params[base + 5];

        scan.camera_up_angle = params[base + 6];
    }
}

fn log_geometry_params(scans: &[String], iter: u64, best: f32, params: &[f32]) {
    let mut param_str = String::new();
    for (i, target) in scans.iter().enumerate() {
        let base = i * 7;
        param_str += &format!(
            " -y {}={},{},{}",
            target,
            params[base],
            params[base + 1],
            params[base + 2]
        );
        param_str += &format!(
            " -c {}={},{},{}",
            target,
            params[base + 3],
            params[base + 4],
            params[base + 5]
        );
        param_str += &format!(" -l {}={}", target, params[base + 6]);
    }
    info!("iter {}, best {}, params{}", iter, best, param_str);
}

#[allow(clippy::ptr_arg)]
fn gradient<F>(apply: F, p: &Vec<f32>) -> StdResult<Vec<f32>, ArgminError>
where
    F: Fn(&Vec<f32>) -> StdResult<f32, ArgminError>,
{
    const DELTA: f32 = 0.001;
    let mut params = p.clone();
    let base = apply(p).unwrap();
    let mut grad = Vec::with_capacity(p.len());

    for (i, param) in p.iter().enumerate() {
        params[i] = *param + DELTA;
        grad.push((apply(&params).unwrap() - base) / DELTA);
        params[i] = *param;
    }

    Ok(grad)
}

const PENALTY_SCORE: f32 = 1.0;

struct FrameOp<'a> {
    point_cloud_params: &'a PointCloudParams,
    scans: &'a IndexMap<String, fm::Scan>,
    scan_frames: &'a Vec<fm::ScanFrame>,
    optimized: Vec<String>,
}

impl<'a> ArgminOp for FrameOp<'a> {
    type Param = Vec<f32>;
    type Output = f32;
    type Hessian = ();
    type Jacobian = ();
    type Float = f32;

    fn apply(&self, p: &Self::Param) -> StdResult<Self::Output, ArgminError> {
        let mut scans = self.scans.clone();
        apply_geometry_params(&mut scans, &self.optimized, p);

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
                return Ok(PENALTY_SCORE);
            }
        }

        Ok(sum as f32 / num as f32)
    }

    fn gradient(&self, p: &Self::Param) -> StdResult<Self::Param, ArgminError> {
        gradient(|p| self.apply(p), p)
    }
}

struct FrameObserver(Vec<String>);

impl<'a> Observe<FrameOp<'a>> for FrameObserver {
    fn observe_iter(
        &mut self,
        state: &IterState<FrameOp>,
        _kv: &ArgminKV,
    ) -> StdResult<(), ArgminError> {
        log_geometry_params(
            &self.0,
            state.iter,
            state.best_cost,
            &state.best_param,
        );
        Ok(())
    }
}

fn match_scans(
    params: &OptimizeScanGeometryParams,
    scans: &IndexMap<String, fm::Scan>,
    scan_frames: &[fm::ScanFrame],
    optimized: &[String],
    mut init_params: Vec<f32>,
) -> StdResult<Vec<f32>, ArgminError> {
    let frame_clouds =
        build_frame_clouds(scans, scan_frames, &params.point_cloud);
    let mut scan_clouds = HashMap::<String, Vec<_>>::new();
    for (i, cloud) in frame_clouds.into_iter().enumerate() {
        let name = &scan_frames[i].scan;
        if let Some(points) = scan_clouds.get_mut(name) {
            points.extend(cloud);
        } else {
            scan_clouds.insert(name.to_string(), cloud);
        }
    }

    let op = ScanOp {
        all: scans.keys().cloned().collect(),
        optimized: optimized.to_vec(),
        scan_clouds,
    };

    let linesearch = MoreThuenteLineSearch::new();
    let solver = SteepestDescent::new(linesearch);
    let observer = ScanObserver(optimized.to_vec(), init_params.clone());
    let icp_init_params = vec![0.0; optimized.len() * 2];
    let res = Executor::new(op, solver, icp_init_params)
        .add_observer(observer, ObserverMode::NewBest)
        .max_iters(params.num_iters as u64)
        .run()?;

    update_geometry_params(&mut init_params, &res.state.best_param);
    Ok(init_params)
}

struct IcpTransform(Matrix4);

impl IcpTransform {
    fn new(z_rot: f64, z_off: f64) -> Self {
        let cos = z_rot.cos();
        let sin = z_rot.sin();
        // TODO: Remove when
        // https://github.com/rust-lang/rust/issues/88591 is fixed.
        #[allow(clippy::deprecated_cfg_attr)]
        #[cfg_attr(rustfmt, rustfmt_skip)]
        IcpTransform(Matrix4::from_row_slice(&[
            cos, -sin, 0.0, 0.0,
            sin, cos, 0.0, 0.0,
            0.0, 0.0, 1.0, z_off,
            0.0, 0.0, 0.0, 1.0,
        ]))
    }

    #[inline]
    fn apply(&self, v: &Vector3) -> Vector3 {
        let v = self.0 * Vector4::new(v[0], v[1], v[2], 1.0);
        Vector3::new(v[0], v[1], v[2])
    }
}

fn update_geometry_params(params: &mut [f32], icp_params: &[f32]) {
    for i in 0..icp_params.len() / 2 {
        let transform = IcpTransform::new(
            icp_params[i * 2] as f64,
            icp_params[i * 2 + 1] as f64,
        );

        let base = i * 7;

        let v = transform.apply(&Vector3::new(
            params[base] as f64,
            params[base + 1] as f64,
            params[base + 2] as f64,
        ));
        params[base] = v[0] as f32;
        params[base + 1] = v[1] as f32;
        params[base + 2] = v[2] as f32;

        let v = transform.apply(&Vector3::new(
            params[base + 3] as f64,
            params[base + 4] as f64,
            params[base + 5] as f64,
        ));
        params[base + 3] = v[0] as f32;
        params[base + 4] = v[1] as f32;
        params[base + 5] = v[2] as f32;
    }
}

struct ScanOp {
    all: Vec<String>,
    optimized: Vec<String>,
    scan_clouds: HashMap<String, Vec<PointNormal>>,
}

impl ArgminOp for ScanOp {
    type Param = Vec<f32>;
    type Output = f32;
    type Hessian = ();
    type Jacobian = ();
    type Float = f32;

    fn apply(&self, p: &Self::Param) -> StdResult<Self::Output, ArgminError> {
        let mut clouds = self.scan_clouds.clone();
        for (i, name) in self.optimized.iter().enumerate() {
            let cloud = clouds.get_mut(name.as_str()).unwrap();

            let transform =
                IcpTransform::new(p[i * 2] as f64, p[i * 2 + 1] as f64);
            for p in cloud.iter_mut() {
                p.0.coords = transform.apply(&p.0.coords);
            }
        }

        let mut sum = 0.0;
        let mut num = 0;
        for i in 0..self.all.len() {
            if let Some(dist) = distance_between_point_clouds(
                clouds.get(&self.all[i]).unwrap(),
                clouds.get(&self.all[(i + 1) % self.all.len()]).unwrap(),
            ) {
                sum += dist;
                num += 1;
            } else {
                return Ok(PENALTY_SCORE);
            }
        }

        Ok(sum as f32 / num as f32)
    }

    fn gradient(&self, p: &Self::Param) -> StdResult<Self::Param, ArgminError> {
        gradient(|p| self.apply(p), p)
    }
}

struct ScanObserver(Vec<String>, Vec<f32>);

impl<'a> Observe<ScanOp> for ScanObserver {
    fn observe_iter(
        &mut self,
        state: &IterState<ScanOp>,
        _kv: &ArgminKV,
    ) -> StdResult<(), ArgminError> {
        let mut params = self.1.clone();
        update_geometry_params(&mut params, &state.best_param);
        log_geometry_params(&self.0, state.iter, state.best_cost, &params);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use base::assert_eq_point3;

    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn test_icp_transform() {
        let t = IcpTransform::new(PI/2.0, 1.0);
        let v = t.apply(&Vector3::new(2.0, 3.0, 4.0));
        assert_eq_point3!(v, &Vector3::new(-3.0, 2.0, 5.0));

        let t = IcpTransform::new(-PI/2.0, -1.0);
        let v = t.apply(&Vector3::new(2.0, 3.0, 4.0));
        assert_eq_point3!(v, &Vector3::new(3.0, -2.0, 3.0));
    }
}
