pub mod input_selection;
//pub mod input_patching;

//pub mod color_correction;

//pub mod output_baking;
//pub mod output_packing;
//pub mod output_patching;

pub mod misc;
use misc::*;

use input_selection::{
    project_like_camera,
};

//use log::info;

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
