use std::path::PathBuf;

use structopt::StructOpt;

use crate::misc::{
    fm_reader_from_file_or_stdin, fm_writer_to_file_or_stdout, read_scans,
    ScanParams,
};
use crate::point_cloud::{
    build_frame_clouds, Point3, PointCloudParams, PointNormal, Vector3,
};
use crate::poisson;
use base::defs::{Error, ErrorKind, Result};
use base::fm;

#[derive(StructOpt)]
#[structopt(about = "Build element view from scan .fm file")]
pub struct BuildViewParams {
    #[structopt(help = "Input scan .fm file (STDIN if omitted)")]
    in_path: Option<PathBuf>,

    #[structopt(flatten)]
    scan_params: ScanParams,

    #[structopt(flatten)]
    point_cloud_params: PointCloudParams,

    #[structopt(flatten)]
    poisson_params: poisson::Params,

    #[structopt(
        help = "Output element view .fm file (STDOUT if omitted)",
        long,
        short = "o"
    )]
    out_path: Option<PathBuf>,

    #[structopt(flatten)]
    fm_write_params: fm::WriterParams,
}

pub fn build_view_with_params(params: &BuildViewParams) -> Result<()> {
    let mut reader = fm_reader_from_file_or_stdin(&params.in_path)?;

    let mut writer =
        fm_writer_to_file_or_stdout(&params.out_path, &params.fm_write_params)?;

    build_view(
        reader.as_mut(),
        &params.scan_params,
        &params.point_cloud_params,
        &params.poisson_params,
        writer.as_mut(),
    )
}

pub fn build_view(
    reader: &mut dyn fm::Read,
    scan_params: &ScanParams,
    point_cloud_params: &PointCloudParams,
    poisson_params: &poisson::Params,
    _writer: &mut dyn fm::Write,
) -> Result<()> {
    let (scans, scan_frames) = read_scans(reader, scan_params)?;

    eprintln!("building point clouds...");
    let cloud = Cloud(
        build_frame_clouds(&scans, &scan_frames, point_cloud_params)
            .into_iter()
            .flatten()
            .collect(),
    );

    let mut mesh = Mesh::default();

    eprintln!("reconstructing mesh...");
    if !poisson::reconstruct(poisson_params, &cloud, &mut mesh) {
        return Err(Error::new(
            ErrorKind::PoissonError,
            "failed to reconstruct surface".to_string(),
        ));
    }
    mesh.apply_bounds(point_cloud_params);

    eprintln!("writing resulting view...");
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
    fn apply_bounds(&mut self, _params: &PointCloudParams) {
        assert_eq!(self.vertices.len(), self.normals.len());

        // TODO: implement this.
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
