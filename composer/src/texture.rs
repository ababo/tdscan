pub mod input_selection;

pub mod misc;
use misc::*;

use input_selection::{
    project_like_camera,
};

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
        _mesh: Mesh,
    ) -> TexturedMesh {

        project_like_camera(&scans[""], &scan_frames[0], &[]);
        
        unimplemented!()
    }
}
