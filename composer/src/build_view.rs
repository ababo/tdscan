use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::ImageEncoder;
use log::info;
use structopt::StructOpt;
use uuid::Uuid;

use crate::mesh::Mesh;
use crate::point_cloud::{build_frame_clouds, PointCloudParams, PointNormal};
use crate::poisson;
use crate::scan::{read_scans, ScanParams};
use crate::texture::{TextureParams, TexturedMesh};
use base::defs::{Error, ErrorKind::*, Result};
use base::fm;
use base::fm::record::Type::*;
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

    #[structopt(
        help = "Disable texturing",
        long,
        short = "g",
        conflicts_with = "texture", // TODO: Make it work somehow.
    )]
    pub disable_texturing: bool,

    #[structopt(help = "Output element name", long, short = "e")]
    pub element: Option<String>,

    #[structopt(flatten)]
    pub texture: TextureParams,

    #[structopt(help = "Texture image type", long, default_value = "jpeg")]
    pub texture_image_type: fm::image::Type,

    #[structopt(
        help = "Texture JPEG quality (1-100)",
        long,
        default_value = "80" // TODO: Make it conflicting with non-jpeg.
    )]
    pub texture_jpeg_quality: u8,
}

pub fn build_view(
    reader: &mut dyn fm::Read,
    writer: &mut dyn fm::Write,
    params: &BuildViewParams,
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

    let (view, state) = if params.disable_texturing {
        create_non_textured_element(params, &mesh)?
    } else {
        info!(
            "texturing mesh of {} vertices and {} faces...",
            mesh.vertices.len(),
            mesh.faces.len()
        );
        let tmesh =
            TexturedMesh::new(&scans, &scan_frames, mesh, &params.texture)?;
        create_textured_element(params, &tmesh)?
    };

    info!("writing generated model...");
    writer.write_record(&fm::Record {
        r#type: Some(ElementView(view)),
    })?;
    writer.write_record(&fm::Record {
        r#type: Some(ElementViewState(state)),
    })?;

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

fn create_non_textured_element(
    params: &BuildViewParams,
    mesh: &Mesh,
) -> Result<(fm::ElementView, fm::ElementViewState)> {
    let element = params
        .element
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let mut view = fm::ElementView {
        element,
        ..Default::default()
    };

    view.faces = mesh
        .faces
        .iter()
        .map(|f| fm::element_view::Face {
            vertex1: f[0] as u32 + 1,
            vertex2: f[1] as u32 + 1,
            vertex3: f[2] as u32 + 1,
            normal1: f[0] as u32 + 1,
            normal2: f[1] as u32 + 1,
            normal3: f[2] as u32 + 1,
            ..Default::default()
        })
        .collect();

    let mut state = fm::ElementViewState {
        element: view.element.clone(),
        ..Default::default()
    };

    state.vertices = mesh
        .vertices
        .iter()
        .map(|p| fm::Point3 {
            x: p[0] as f32,
            y: p[1] as f32,
            z: p[2] as f32,
        })
        .collect();

    state.normals = mesh
        .normals
        .iter()
        .map(|p| fm::Point3 {
            x: p[0] as f32,
            y: p[1] as f32,
            z: p[2] as f32,
        })
        .collect();

    Ok((view, state))
}

fn create_textured_element(
    params: &BuildViewParams,
    mesh: &TexturedMesh,
) -> Result<(fm::ElementView, fm::ElementViewState)> {
    let (mut view, state) = create_non_textured_element(params, &mesh.mesh)?;

    let mut data = Vec::new();
    match params.texture_image_type {
        fm::image::Type::Png => {
            let encoder = PngEncoder::new_with_quality(
                &mut data,
                CompressionType::Best,
                FilterType::default(),
            );
            encoder
                .write_image(
                    mesh.image.as_ref(),
                    mesh.image.width(),
                    mesh.image.height(),
                    image::ColorType::Rgb8,
                )
                .unwrap();
            view.texture = Some(fm::Image {
                r#type: fm::image::Type::Png as i32,
                data,
            });
        }
        fm::image::Type::Jpeg => {
            let encoder = JpegEncoder::new_with_quality(
                &mut data,
                params.texture_jpeg_quality,
            );
            encoder
                .write_image(
                    mesh.image.as_ref(),
                    mesh.image.width(),
                    mesh.image.height(),
                    image::ColorType::Rgb8,
                )
                .unwrap();
            view.texture = Some(fm::Image {
                r#type: fm::image::Type::Jpeg as i32,
                data,
            });
        }
        fm::image::Type::None => {
            panic!("unsupported texture image type");
        }
    }

    view.texture_points = mesh
        .uv_coords
        .iter()
        .map(|p| fm::Point2 {
            x: p.y as f32,
            y: p.x as f32,
        })
        .collect();

    for (i, idxs) in mesh.uv_idxs.iter().enumerate() {
        view.faces[i].texture1 = idxs[0] as u32 + 1;
        view.faces[i].texture2 = idxs[1] as u32 + 1;
        view.faces[i].texture3 = idxs[2] as u32 + 1;
    }

    Ok((view, state))
}
