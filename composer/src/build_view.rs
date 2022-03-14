use std::path::PathBuf;

use log::info;
use structopt::StructOpt;

use crate::export_obj::write_textured_mesh;
use crate::mesh::Mesh;
use crate::point_cloud::{build_frame_clouds, PointCloudParams, PointNormal};
use crate::poisson;
use crate::scan::{read_scans, ScanParams};
use crate::texture::{TextureParams, TexturedMesh};
use base::defs::{Error, ErrorKind::*, Result};
use base::fm;
use base::util::cli;

#[derive(StructOpt)]
#[structopt(about = "Build element view from scan .fm file")]
pub struct BuildViewCommand {
    #[structopt(flatten)]
    input: cli::FmInput,

    #[structopt(flatten)]
    output: cli::FmOutput,

    #[structopt(flatten)]
    params: BuildViewParams,
}

impl BuildViewCommand {
    pub fn run(&self) -> Result<()> {
        let mut reader = self.input.get()?;
        let mut writer = self.output.get()?;

        build_view(
            reader.as_mut(),
            writer.as_mut(),
            &self.params,
            &self.output.path,
        )
    }
}

#[derive(StructOpt)]
pub struct BuildViewParams {
    #[structopt(flatten)]
    pub scan: ScanParams,

    #[structopt(flatten)]
    pub point_cloud: PointCloudParams,

    #[structopt(flatten)]
    pub poisson: poisson::Params,

    #[structopt(
        help = "Number of Laplacian smoothing iterations",
        long,
        short = "s",
        default_value = "0"
    )]
    pub num_smooth_iters: usize,

    #[structopt(
        help = "Surface decimation ratio",
        long,
        default_value = "0.1"
    )]
    pub decimate_ratio: f64,

    #[structopt(flatten)]
    pub texture: TextureParams,
}

pub fn build_view(
    reader: &mut dyn fm::Read,
    _writer: &mut dyn fm::Write,
    params: &BuildViewParams,
    output_path: &Option<PathBuf>,
) -> Result<()> {
    info!("reading scans...");
    let (scans, scan_frames) = read_scans(reader, &params.scan)?;

    params
        .point_cloud
        .validate(scans.keys().map(String::as_str))?;

    info!(
        "building point clouds from {} scans ({} frames)...",
        scans.len(),
        scan_frames.len()
    );
    let cloud = Cloud(
        build_frame_clouds(&scans, &scan_frames, &params.point_cloud)
            .into_iter()
            .flatten()
            .collect(),
    );

    let mut mesh = Mesh::default();

    info!(
        "reconstructing mesh from cloud of {} points...",
        cloud.0.len()
    );
    if !poisson::reconstruct(&params.poisson, &cloud, &mut mesh) {
        return Err(Error::new(
            PoissonError,
            "failed to reconstruct surface".to_string(),
        ));
    }
    mesh.apply_bounds(&params.point_cloud);
    for vn in mesh.normals.iter_mut() {
        vn.normalize_mut();
    }
    mesh.clean();

    if params.num_smooth_iters > 0 {
        info!("smoothing mesh...");
        mesh.smoothen(params.num_smooth_iters);
    }

    if params.decimate_ratio > 0.0 && params.decimate_ratio < 1.0 {
        info!(
            "decimating mesh of {} vertices and {} faces...",
            mesh.vertices.len(),
            mesh.faces.len()
        );
        mesh = mesh.decimate(params.decimate_ratio);
    }

    info!(
        "texturing mesh of {} vertices and {} faces...",
        mesh.vertices.len(),
        mesh.faces.len()
    );
    let tmesh = TexturedMesh::new(&scans, &scan_frames, mesh, &params.texture)?;

    info!("writing textured mesh...");
    let f = |s| {
        if let Some(p) = output_path {
            String::from(p.as_path().join(s).to_str().unwrap())
        } else {
            String::from(s)
        }
    };
    write_textured_mesh(&tmesh, &f("foo.mtl"), &f("foo.obj"), &f("foo.png"));

    info!("done");
    Ok(())
}

pub struct Cloud(Vec<PointNormal>);

impl poisson::Cloud<f64> for Cloud {
    fn len(&self) -> usize {
        self.0.len()
    }

    fn has_normals(&self) -> bool {
        true
    }

    fn point(&self, index: usize) -> [f64; 3] {
        *self.0[index].0.coords.as_ref()
    }

    fn normal(&self, index: usize) -> [f64; 3] {
        *self.0[index].1.as_ref()
    }
}
