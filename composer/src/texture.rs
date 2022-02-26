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
    pub uv_coords: Vec<UV>,
    pub uv_idxs: Vec<[UVIdx; 3]>,
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

    pub fn write(
        &self,
        //base_path: &str,
        mtlpath: &str,
        objpath: &str,
        texpath: &str,
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

        // Write MTL.
        let file = File::create(mtlpath).ok().unwrap();
        let mut writer = io::BufWriter::new(file);
        writeln!(&mut writer, "newmtl Default_OBJ").unwrap();
        writeln!(&mut writer, "Ns 225.000000").unwrap();
        writeln!(&mut writer, "Ka 1.000000 1.000000 1.000000").unwrap();
        writeln!(&mut writer, "Kd 0.800000 0.800000 0.800000").unwrap();
        writeln!(&mut writer, "Ks 0.500000 0.500000 0.500000").unwrap();
        writeln!(&mut writer, "Ke 0.000000 0.000000 0.000000").unwrap();
        writeln!(&mut writer, "Ni 1.450000").unwrap();
        writeln!(&mut writer, "d 1.000000").unwrap();
        writeln!(&mut writer, "illum 2").unwrap();
        writeln!(&mut writer, "map_Kd {texpath_local}").unwrap();

        // Write OBJ.
        let file = File::create(objpath).ok().unwrap();
        let mut writer = io::BufWriter::new(file);
        writeln!(&mut writer, "mtllib {mtlpath_local}").unwrap();
        for v in &self.mesh.vertices {
            writeln!(&mut writer, "v {:.6} {:.6} {:.6}", v[0], v[1], v[2])
                .unwrap();
        }
        writeln!(&mut writer, "usemtl Default_OBJ\ns 1").unwrap();
        for vt in &self.uv_coords {
            writeln!(
                &mut writer,
                "vt {:.6} {:.6}",
                // Note: Changing coordinate system.
                vt[1],
                1.0 - vt[0]
            )
            .unwrap();
        }
        for vn in &self.mesh.normals {
            writeln!(
                &mut writer,
                // Note: Using the same precision as blender.
                "vn {:.4} {:.4} {:.4}",
                vn[0], vn[1], vn[2]
            )
            .unwrap();
        }
        for (f, t) in self.mesh.faces.iter().zip(self.uv_idxs.iter()) {
            writeln!(
                &mut writer,
                // (vertex / texture / normal)
                "f {}/{}/{} {}/{}/{} {}/{}/{}",
                // Note: Indexing starts at 1.
                f[0] + 1,
                t[0] + 1,
                f[0] + 1,
                f[1] + 1,
                t[1] + 1,
                f[1] + 1,
                f[2] + 1,
                t[2] + 1,
                f[2] + 1,
            )
            .unwrap();
        }

        // Write PNG.
        self.image.save(texpath).unwrap();
    }
}
