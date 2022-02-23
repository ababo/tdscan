pub mod input_projection;
pub mod input_selection;
//pub mod input_patching;

//pub mod color_correction;

pub mod output_patching;
pub mod output_packing;
pub mod output_baking;

pub mod misc;
use misc::*;

use input_selection::{
    //Metrics,
    FrameMetrics,
    make_all_frame_metrics,
    select_cameras,
};
use output_patching::{
    choose_uv_patches,
    LocalPatch,
};
use output_packing::{
    pack_rectangles_with_automatic_stretching,
    globalize_uv,
};
use output_baking::{
    compress_uv_coords,
    bake_texture,
    EmptinessMask,
    extrapolate_gutter,
};

use log::info;


pub struct TexturedMesh {
    pub mesh: Mesh,
    pub uv_coords: Vec<UV>,
    pub uv_idxs: Vec<[UVIdx; 3]>,
    pub image: RgbImage
}

pub const PATCH_SPACING: f64 = 0.005;
pub const GUTTER_SIZE: usize = 3;
pub const IMAGE_RES: usize = 4096;

impl TexturedMesh {
    pub fn make(
        scans: &IndexMap<String, fm::Scan>,
        scan_frames: &Vec<fm::ScanFrame>,
        mesh: Mesh,
    ) -> TexturedMesh {
        // choose image source for each mesh face
        info!("  choosing texture sources for mesh faces...");
        let (vertex_metrics, face_metrics)
            : (Vec<FrameMetrics>, Vec<FrameMetrics>) =
            make_all_frame_metrics(scans, scan_frames, &mesh);
        let chosen_cameras: Vec<Option<FrameIdx>> =
            select_cameras(&face_metrics, &mesh);
        info!("  texture sources obtained for {:.1}% of the faces",
              chosen_cameras.iter().map(|c| c.is_some() as usize).
              sum::<usize>() as f64 / mesh.faces.len() as f64 * 100.0);

        // choose patches for output texture baking
        info!("  choosing patches for baking...");
        let topo = BasicMeshTopology::make(&mesh);
        let local_patches: Vec<LocalPatch> = choose_uv_patches(&mesh, &topo)
            .iter()
            .map(|(chunk, major)| LocalPatch::calculate_from(
                chunk, *major, &mesh)
            )
            .collect();
        let local_patch_sizes: Vec<[f64; 2]> =
            local_patches.iter().map(|patch| patch.size).collect();
        let (rectangle_placements_vec, scale): (Vec<Rectangle<f64>>, f64) =
            pack_rectangles_with_automatic_stretching(
                &local_patch_sizes, PATCH_SPACING);
        let uv_coords_tri: Vec<[UV; 3]> =
            globalize_uv(&local_patches, &rectangle_placements_vec, &mesh);
        let (uv_coords, uv_idxs_tri): (Vec<UV>, Vec<[UVIdx; 3]>) =
            compress_uv_coords(&uv_coords_tri);

        // bake texture
        let density = scale * IMAGE_RES as f64 / 100.0;
        info!("  baking texture with density: {:.1} pixels per centimeter...",
              density);
        
        let images: Vec<Option<RgbImage>> =
            try_load_all_frame_images(&scan_frames);
        let (mut buffer, mut emask): (RgbImage, EmptinessMask) = bake_texture(
            &mesh,
            &images,
            &chosen_cameras,
            &vertex_metrics,
            &uv_coords_tri,
            IMAGE_RES 
        );
        info!("  extrapolating gutter...");
        extrapolate_gutter(&mut buffer, &mut emask, GUTTER_SIZE);
        info!("  finished with total texture packing efficiency: {:.1}%",
              emask.iter().flatten().map(|b| !b as usize).sum::<usize>() as f64
              / (IMAGE_RES * IMAGE_RES) as f64 * 100.0);
        
        TexturedMesh {
            mesh,
            uv_coords,
            uv_idxs: uv_idxs_tri,
            image: buffer
        }
    }
    
    pub fn write(
        &self,
        //base_path: &str,
        mtlpath: &str,
        objpath: &str,
        texpath: &str
    ) {
        //let mtlpath = format!("{base_path}.mtl");
        //let objpath = format!("{base_path}.obj");
        //let texpath = format!("{base_path}.png");
        let mtlpath_local =
            Path::new(mtlpath).file_name().unwrap().to_str().unwrap();
        let _objpath_local =
            Path::new(objpath).file_name().unwrap().to_str().unwrap();
        let texpath_local =
            Path::new(texpath).file_name().unwrap().to_str().unwrap();
        
        // write MTL
        let file = File::create(mtlpath).ok().unwrap();
        let mut writer = io::BufWriter::new(file);
        write!(&mut writer, "newmtl Default_OBJ\n").unwrap();
        write!(&mut writer, "Ns 225.000000\n").unwrap();
        write!(&mut writer, "Ka 1.000000 1.000000 1.000000\n").unwrap();
        write!(&mut writer, "Kd 0.800000 0.800000 0.800000\n").unwrap();
        write!(&mut writer, "Ks 0.500000 0.500000 0.500000\n").unwrap();
        write!(&mut writer, "Ke 0.000000 0.000000 0.000000\n").unwrap();
        write!(&mut writer, "Ni 1.450000\n").unwrap();
        write!(&mut writer, "d 1.000000\n").unwrap();
        write!(&mut writer, "illum 2\n").unwrap();
        write!(&mut writer, "map_Kd {texpath_local}\n").unwrap();
        
        // write OBJ
        let file = File::create(objpath).ok().unwrap();
        let mut writer = io::BufWriter::new(file);
        write!(&mut writer, "mtllib {mtlpath_local}\n").unwrap();
        for v in &self.mesh.vertices {
            write!(&mut writer,
                   "v {:.6} {:.6} {:.6}\n",
                   v[0], v[1], v[2]).unwrap();
        }
        write!(&mut writer, "usemtl Default_OBJ\ns 1\n").unwrap();
        for vt in &self.uv_coords {
            write!(&mut writer,
                   "vt {:.6} {:.6}\n",
                   // note: changing coordinate system
                   vt[1], 1.0 - vt[0]).unwrap();
        }
        for vn in &self.mesh.normals {
            write!(&mut writer,
                   // note: using the same precision as blender
                   "vn {:.4} {:.4} {:.4}\n",
                   vn[0], vn[1], vn[2]).unwrap();
        }
        for (f, t) in self.mesh.faces.iter().zip(self.uv_idxs.iter()) {
            write!(&mut writer,
                   // (vertex / texture / normal)
                   "f {}/{}/{} {}/{}/{} {}/{}/{}\n",
                   // note: indexing starts at 1
                   f[0]+1, t[0]+1, f[0]+1,
                   f[1]+1, t[1]+1, f[1]+1,
                   f[2]+1, t[2]+1, f[2]+1,
            ).unwrap();
        }
        
        // write PNG
        self.image.save(texpath).unwrap();
    }
}
