use std::path::PathBuf;

use structopt::StructOpt;

use crate::poisson;

use crate::misc::{
    fm_reader_from_file_or_stdin, fm_writer_to_file_or_stdout, read_scans,
    ScanParams,
};
use crate::point_cloud::{build_frame_clouds, PointCloudParams};
use base::defs::Result;
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
        writer.as_mut(),
    )
}

pub fn build_view(
    reader: &mut dyn fm::Read,
    scan_params: &ScanParams,
    point_cloud_params: &PointCloudParams,
    _writer: &mut dyn fm::Write,
) -> Result<()> {
    let (scans, scan_frames) = read_scans(reader, scan_params)?;

    let clouds = build_frame_clouds(&scans, &scan_frames, point_cloud_params);

    use std::io::Write;
    let mut file =
        std::fs::File::create("/Users/ababo/Desktop/foo.obj").unwrap();
    for cloud in clouds {
        for point in cloud {
            file.write_all(
                format!("v {} {} {}\n", point[0], point[1], point[2])
                    .into_bytes()
                    .as_slice(),
            )
            .unwrap();
        }
    }

    let cloud = poisson::Cloud::<f64>{ vertices: vec![]};
    let _ = poisson::reconstruct(&cloud, &poisson::Params::default());
    let cloud = poisson::Cloud::<f32>{ vertices: vec![]};
    let _ = poisson::reconstruct(&cloud, &poisson::Params::default());

    Ok(())
}
