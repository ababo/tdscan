pub mod input_selection;
pub mod misc;

use log::info;

use input_selection::{
    make_all_frame_metrics,
    select_cameras,
};
use misc::*;

pub struct TexturedMesh {
    pub mesh: Mesh,
    pub uv_coords: Vec<Vector2>,
    pub uv_idxs: Vec<[usize; 3]>,
    pub image: RgbImage,
}

impl TexturedMesh {
    pub fn make(
        scans: &IndexMap<String, fm::Scan>,
        scan_frames: &[fm::ScanFrame],
        mesh: Mesh,
    ) -> TexturedMesh {
        // Choose image source for each mesh face.
        info!("  choosing texture sources for mesh faces...");
        let (_vertex_metrics, face_metrics) =
            make_all_frame_metrics(scans, scan_frames, &mesh);
        let chosen_cameras = select_cameras(&face_metrics, &mesh);
        info!(
            "  texture sources obtained for {:.1}% of the faces",
            chosen_cameras
                .iter()
                .map(|c| c.is_some() as usize)
                .sum::<usize>() as f64
                / mesh.faces.len() as f64
                * 100.0
        );
        
        unimplemented!()
    }
}
