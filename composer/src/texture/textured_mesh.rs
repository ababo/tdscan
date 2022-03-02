use indexmap::IndexMap;

use crate::mesh::Mesh;
use crate::texture::*;
use base::fm;

pub struct TexturedMesh {
    pub mesh: Mesh,
    pub uv_coords: Vec<Vector2>,
    pub uv_idxs: Vec<[usize; 3]>,
    pub image: RgbImage,
}

// Spacing between patches in the final texture.
// Measured as a fraction of the total texture size, e.g. 0.005 = 0.5%.
const PATCH_SPACING: f64 = 0.005;

// Amount of colored pixels added around each patch to avoid rendering issues.
pub const GUTTER_SIZE: usize = 3;

// Output texture image resolution.
pub const IMAGE_RES: usize = 4096;

impl TexturedMesh {
    pub fn new(
        scans: &IndexMap<String, fm::Scan>,
        scan_frames: &[fm::ScanFrame],
        mesh: Mesh,
    ) -> TexturedMesh {
        let (vertex_metrics, face_metrics) =
            make_all_frame_metrics(scans, scan_frames, &mesh);
        let chosen_cameras = select_cameras(&face_metrics, &mesh);

        let topo = BasicMeshTopology::new(&mesh);
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
                PATCH_SPACING,
            );
        let uv_coords_tri =
            globalize_uv(&local_patches, &rectangle_placements_vec, &mesh);
        let (uv_coords, uv_idxs_tri) = compress_uv_coords(&uv_coords_tri);

        let images = load_all_frame_images(scan_frames);
        let (mut buffer, mut emask) = bake_texture(
            &mesh,
            &images,
            &chosen_cameras,
            &vertex_metrics,
            &uv_coords_tri,
            IMAGE_RES,
        );
        extrapolate_gutter(&mut buffer, &mut emask, GUTTER_SIZE);

        TexturedMesh {
            mesh,
            uv_coords,
            uv_idxs: uv_idxs_tri,
            image: buffer,
        }
    }
}
