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

impl TexturedMesh {
    pub fn new(
        scans: &IndexMap<String, fm::Scan>,
        scan_frames: &[fm::ScanFrame],
        mesh: Mesh,
    ) -> TexturedMesh {
        let (_vertex_metrics, face_metrics) =
            make_all_frame_metrics(scans, scan_frames, &mesh);
        let _chosen_cameras = select_cameras(&face_metrics, &mesh);

        let topo = BasicMeshTopology::make(&mesh);
        let local_patches: Vec<LocalPatch> = choose_uv_patches(&mesh, &topo)
            .iter()
            .map(|(chunk, major)| {
                LocalPatch::calculate_from(chunk, *major, &mesh)
            })
            .collect();

        // (Dummy calls to avoid compiler warnings for now.)
        local_patches[0]
            .to_global_coords(Rectangle { pos: [0.0, 0.0], size: [0.0, 0.0] });
        BarycentricCoordinateSystem::try_new([Vector2::new(0.0, 0.0); 3])
            .unwrap()
            .apply(Vector3::new(0.0, 0.0, 0.0));

        unimplemented!()
    }
}
