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

        unimplemented!()
    }
}
