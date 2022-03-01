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

impl TexturedMesh {
    pub fn new(
        scans: &IndexMap<String, fm::Scan>,
        scan_frames: &[fm::ScanFrame],
        mesh: Mesh,
    ) -> TexturedMesh {
        let (_vertex_metrics, face_metrics) =
            make_all_frame_metrics(scans, scan_frames, &mesh);
        let _chosen_cameras = select_cameras(&face_metrics, &mesh);

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
        let _uv_coords_tri =
            globalize_uv(&local_patches, &rectangle_placements_vec, &mesh);

        // Dummy calls to avoid compiler warnings for now.
        BarycentricCoordinateSystem::new([Vector2::new(0.0, 0.0); 3])
            .unwrap()
            .apply(Vector3::new(0.0, 0.0, 0.0));

        unimplemented!()
    }
}
