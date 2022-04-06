use std::f64::consts::PI;

use indexmap::IndexMap;
use structopt::StructOpt;

use crate::mesh::Mesh;
use crate::texture::*;
use base::defs::Result;
use base::fm;
use base::util::cli::parse_color;

#[derive(Clone, StructOpt)]
pub struct TextureParams {
    #[structopt(
        help = "Spacing between patches in the final texture",
        long,
        default_value = "0.005"
    )]
    // Measured as a fraction of the total texture size, e.g. 0.005 = 0.5%.
    pub patch_spacing: f64,

    #[structopt(
        help = "Amount of colored pixels added around each texturing patch",
        long,
        default_value = "3"
    )]
    pub gutter_size: usize,

    #[structopt(
        help = "Output texture image resolution",
        long,
        default_value = "4096"
    )]
    pub image_resolution: usize,

    #[structopt(
        help = "Threshold beyond which a mesh face is deemed not visible",
        long,
        default_value = "10.0"
    )]
    pub selection_cost_limit: f64,

    #[structopt(
        help = "Mean color for background detection",
        long,
        parse(try_from_str = parse_color_into_vector3),
        default_value = "#00b140" // Common chroma key green color.
    )]
    pub background_color: Vector3,

    #[structopt(
        help = "Allowed color deviation for background detection",
        long,
        default_value = "-1.0" // Disable background extraction by default.
    )]
    pub background_deviation: f64,

    #[structopt(
        help = "Number of steps of color correction",
        long,
        default_value = "10"
    )]
    pub color_correction_steps: usize,

    #[structopt(
        help = "Whether to apply a constant offset after color correction",
        long
    )]
    pub color_correction_final_offset: bool,

    #[structopt(
        help = "Maximum cost ratio during input patch formation",
        long,
        default_value = "2.0"
    )]
    pub input_patching_threshold: f64,

    #[structopt(
        help = "Denoising for background detection (measured in pixels)",
        long,
        default_value = "-5.0,10.0",
        use_delimiter = true
    )]
    pub background_dilations: Vec<f64>,

    #[structopt(
        help = "Radius for avoiding corners when choosing texture source",
        long,
        default_value = "0"
    )]
    pub selection_corner_radius: usize,

    #[structopt(
        help = "Threshold for background detection consensus",
        long,
        default_value = "0.5"
    )]
    pub background_consensus_threshold: f64,

    #[structopt(
        help = "Radius of spread of background detection consensus",
        long,
        default_value = "2"
    )]
    pub background_consensus_spread: usize,
}

fn parse_color_into_vector3(src: &str) -> Result<Vector3> {
    let [r, g, b] = parse_color(src)?;
    Ok(Vector3::new(r as f64, g as f64, b as f64))
}

pub struct TexturedMesh {
    pub mesh: Mesh,
    pub uv_coords: Vec<Vector2>,
    pub uv_idxs: Vec<[usize; 3]>,
    pub image: RgbImage,
}

impl TexturedMesh {
    pub fn new(
        scans: &IndexMap<String, fm::Scan>,
        scan_frames: &[fm::ScanFrame],
        mesh: Mesh,
        params: &TextureParams,
    ) -> Result<TexturedMesh> {
        let topo = BasicMeshTopology::new(&mesh);

        let VertexAndFaceMetricsOfAllFrames {
            vertex_metrics,
            face_metrics,
        } = make_all_frame_metrics(
            scans,
            scan_frames,
            &mesh,
            params.background_color,
            params.background_deviation,
            &params.background_dilations,
        );
        let all_costs = build_all_costs(
            &face_metrics,
            &mesh,
            &topo,
            params.selection_corner_radius,
        );
        let mut chosen_cameras =
            select_cameras(&all_costs, &mesh, params.selection_cost_limit);
        form_patches(
            &mut chosen_cameras,
            &face_metrics,
            &all_costs,
            &mesh,
            &topo,
            params.input_patching_threshold,
        );
        disqualify_background_faces(
            &mut chosen_cameras,
            &face_metrics,
            &all_costs,
            &mesh,
            &topo,
            params.selection_cost_limit,
            params.background_consensus_threshold,
            params.background_consensus_spread,
        );

        let local_patches: Vec<LocalPatch> = choose_uv_patches(&mesh, &topo)
            .iter()
            .map(|(chunk, major)| {
                LocalPatch::calculate_from(chunk, *major, &mesh)
            })
            .collect();
        let local_patch_sizes: Vec<[f64; 2]> =
            local_patches.iter().map(|patch| patch.size).collect();
        let (rectangle_placements_vec, _scale) =
            pack_rectangles_with_automatic_stretching(
                &local_patch_sizes,
                params.patch_spacing,
            );
        let uv_coords_tri =
            globalize_uv(&local_patches, &rectangle_placements_vec, &mesh);
        let (uv_coords, uv_idxs_tri) = compress_uv_coords(&uv_coords_tri);

        let images = load_all_frame_images(scan_frames);
        let color_correction = ColorCorrection::new(
            &mesh,
            &topo,
            &vertex_metrics,
            &chosen_cameras,
            &images,
            params.color_correction_steps,
            params.color_correction_final_offset,
        );
        let (mut buffer, mut emask) = bake_texture(
            &mesh,
            &images,
            &chosen_cameras,
            &vertex_metrics,
            &uv_coords_tri,
            params.image_resolution,
            &color_correction,
        );
        extrapolate_gutter(&mut buffer, &mut emask, params.gutter_size);

        Ok(TexturedMesh {
            mesh,
            uv_coords,
            uv_idxs: uv_idxs_tri,
            image: buffer,
        })
    }
}
