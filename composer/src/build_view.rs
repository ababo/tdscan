use std::collections::HashMap;

use log::info;
use structopt::StructOpt;

use crate::point_cloud::{
    build_frame_clouds, validate_point_bounds, Point3, PointCloudParams,
    PointNormal, Vector3,
};
use crate::poisson;
use crate::scan::{read_scans, ScanParams};
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

        build_view(reader.as_mut(), writer.as_mut(), &self.params)
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
}

pub fn build_view(
    reader: &mut dyn fm::Read,
    _writer: &mut dyn fm::Write,
    params: &BuildViewParams,
) -> Result<()> {
    info!("reading scans...");
    let (scans, scan_frames) = read_scans(reader, &params.scan)?;

    params.point_cloud.validate(scans.keys().map(String::as_str))?;

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

    info!(
        "writing mesh of {} vertices and {} faces...",
        mesh.vertices.len(),
        mesh.triangles.len()
    );

    use std::io::Write;
    let mut file =
        std::fs::File::create("/Users/ababo/Desktop/foo.obj").unwrap();
    for vertex in mesh.vertices {
        file.write_all(
            format!("v {} {} {}\n", vertex.x, vertex.y, vertex.z)
                .into_bytes()
                .as_slice(),
        )
        .unwrap();
    }
    for normal in mesh.normals {
        file.write_all(
            format!("vn {} {} {}\n", normal.x, normal.y, normal.z)
                .into_bytes()
                .as_slice(),
        )
        .unwrap();
    }
    for triangle in mesh.triangles {
        file.write_all(
            format!(
                "f {0}//{0} {1}//{1} {2}//{2}\n",
                triangle[0] + 1,
                triangle[1] + 1,
                triangle[2] + 1
            )
            .into_bytes()
            .as_slice(),
        )
        .unwrap();
    }

    info!("done");
    Ok(())
}

struct Cloud(Vec<PointNormal>);

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

#[derive(Default)]
struct Mesh {
    vertices: Vec<Point3>,
    normals: Vec<Vector3>,
    triangles: Vec<[usize; 3]>,
}

impl Mesh {
    fn apply_bounds(&mut self, params: &PointCloudParams) {
        assert_eq!(self.vertices.len(), self.normals.len());
        let mut mappings = HashMap::with_capacity(self.vertices.len());

        let mut j = 0;
        for i in 0..self.vertices.len() {
            if validate_point_bounds(
                &self.vertices[i],
                params.min_z,
                params.max_z,
                params.max_z_distance,
            ) {
                mappings.insert(i, j);
                self.vertices.swap(i, j);
                self.normals.swap(i, j);
                j += 1;
            }
        }
        self.vertices.truncate(j);
        self.normals.truncate(j);

        let mut j = 0;
        'next: for i in 0..self.triangles.len() {
            for k in 0..self.triangles[i].len() {
                if let Some(l) = mappings.get(&self.triangles[i][k]) {
                    self.triangles[j][k] = *l;
                } else {
                    continue 'next;
                }
            }
            j += 1;
        }
        self.triangles.truncate(j);
    }
}

impl poisson::Mesh<f64> for Mesh {
    fn add_vertex(&mut self, vertex: &[f64; 3]) {
        self.vertices.push(Point3::from(*vertex));
    }

    fn add_normal(&mut self, normal: &[f64; 3]) {
        self.normals.push(Vector3::from(*normal));
    }

    fn add_triangle(&mut self, triangle: &[usize; 3]) {
        self.triangles.push(*triangle);
    }
}
