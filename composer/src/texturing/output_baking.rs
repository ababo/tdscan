use crate::texturing::{
    input_selection::{
        Metrics,
        FrameMetrics,
    },
    misc::*,
};

fn copy_triangle(
    img0: &RgbImage,
    uvs0: [UV; 3],  // desired coordinate system: [i,j] normalized to [0,1]
    img1: &mut RgbImage,
    uvs1: [UV; 3],
    emask: &mut EmptinessMask,
) -> Option<()> {
    let ij00 = uv_to_ij(uvs0[0], img0);
    let ij01 = uv_to_ij(uvs0[1], img0);
    let ij02 = uv_to_ij(uvs0[2], img0);
    let ij10 = uv_to_ij(uvs1[0], img1);
    let ij11 = uv_to_ij(uvs1[1], img1);
    let ij12 = uv_to_ij(uvs1[2], img1);
    
    let bcs0 = BarycentricCoordinateSystem::try_new([ij00, ij01, ij02])?;
    let bcs1 = BarycentricCoordinateSystem::try_new([ij10, ij11, ij12])?;
    let g = |ij: Vector2| [ij[0] as u32, ij[1] as u32];
    let rect1 = Rectangle::<u32>::bounding(&vec![g(ij10), g(ij11), g(ij12)]);
    let mut dbg_any = false;
    for i1 in rect1.pos[0]..=rect1.pos[0]+rect1.size[0] {
        for j1 in rect1.pos[1]..=rect1.pos[1]+rect1.size[1] {
            let ij1 = Vector2::new(i1 as f64, j1 as f64);
            let bary = bcs1.infer(ij1);
            let ij0 = bcs0.apply(bary);
            let uv0 = ij_to_uv(ij0, img0);
            let color = sample_pixel(uv0, img0);
            
            if all_nonneg(bary) {
                if i1 < img1.height() && j1 < img1.width() {
                    set_pixel_ij_as_vector3(i1, j1, color, img1);
                    emask[i1 as usize][j1 as usize] = false;
                    dbg_any = true;
                }
            }
        }
    }
    if dbg_any {
        Some(())
    } else {
        None
    }
}

pub type EmptinessMask = Vec<Vec<bool>>;  // rectangular, image-shaped grid

pub fn bake_texture(
    mesh: &Mesh,
    images: &Vec<Option<RgbImage>>,
    chosen_cameras: &Vec<Option<FrameIdx>>,
    vertex_metrics: &Vec<FrameMetrics>,
    uv_coords_tri: &Vec<[UV; 3]>,
    image_res: usize,
) -> (RgbImage, EmptinessMask) {
    let mut buffer = RgbImage::new(image_res as u32, image_res as u32);
    let mut emask: EmptinessMask = vec![vec![true; image_res]; image_res];

    //let mut dbg_no_pixels = 0;
    let dbg_dummy_image_source_magenta: RgbImage
        = dbg_dummy_image_source(Rgb([255, 0, 255]));
    
    for face_idx in 0..mesh.faces.len() {
        let img0: &RgbImage;
        let uvs0: [UV; 3];
        if let Some(frame_idx) = chosen_cameras[face_idx] {
            // load image source
            img0 = images[frame_idx].as_ref().unwrap();

            // define coordinates for image source
            let [v0, v1, v2] = mesh.faces[face_idx];
            let frame_metrics = vertex_metrics[frame_idx].as_ref().unwrap();
            let f = |v| (frame_metrics[v] as Metrics).pixel;
            uvs0 = [f(v0), f(v1), f(v2)];
        } else {
            img0 = &dbg_dummy_image_source_magenta;
            uvs0 = [UV::new(0.0, 0.0), UV::new(1.0, 0.0), UV::new(0.0, 1.0)];
            //println!("mising 9");
        }

        // define coordinates for output buffer
        let uvs1 = uv_coords_tri[face_idx];

        // copy triangle
        if copy_triangle(img0, uvs0, &mut buffer, uvs1, &mut emask)
            .is_none()
        {
            //println!("face {face_idx} does not contain any pixels");
            //dbg_no_pixels += 1;
        }
    }

    //println!("baking finished. {dbg_no_pixels} faces contained no pixels");
    
    (buffer, emask)
}



pub fn compress_uv_coords(
    uv_coords: &Vec<[UV; 3]>
) -> (Vec<UV>, Vec<[UVIdx; 3]>) {
    const EPS: f64 = 1e-6;  // round coordinates to this size, then merge them
    let up0 = |x| (x / EPS) as u64;
    let up1 = |uv: Vector2| [up0(uv[0]), up0(uv[1])];
    let down0 = |x| x as f64 * EPS;
    let down1 = |uv: [u64; 2]| Vector2::new(down0(uv[0]), down0(uv[1]));
    
    let mut uv_unique: HashMap<[u64; 2], usize> = HashMap::new();
    let mut uv_ordered: Vec<UV> = vec![];
    let mut uv_idxs: Vec<[UVIdx; 3]> = vec![];
    
    for i in 0..uv_coords.len() {
        let mut idxs = [0, 0, 0];
        for j in 0..3 {
            let uv = up1(uv_coords[i][j]);
            if !uv_unique.contains_key(&uv) {
                uv_unique.insert(uv, uv_unique.len());
                uv_ordered.push(down1(uv));
            }
            idxs[j] = uv_unique[&uv];
        }
        uv_idxs.push(idxs);
    }
    
    (uv_ordered, uv_idxs)
}


pub fn extrapolate_gutter(
    buffer: &mut RgbImage,
    emask: &mut EmptinessMask,
    gutter_size: usize
) {
    for _ in 0..gutter_size {
        for (i, j, i1, j1) in resolve_gutter_source(emask) {
            // beware that the image is indexed as (j, i)
            buffer[(j, i)] = buffer[(j1, i1)];
            emask[i as usize][j as usize] = false;
        }
    }
}

fn resolve_gutter_source(
    emask: &EmptinessMask,
) -> Vec<(u32, u32, u32, u32)> {
    let mut idxs = vec![];
    let height = emask.len() as u32;
    for i in 0..height as i32 {
        let width = emask[i as usize].len() as u32;
        for j in 0..width as i32 {
            if emask[i as usize][j as usize] {
                for (i1, j1) in [(i-1, j), (i+1, j), (i, j-1), (i, j+1)] {
                    if 0 <= i1 && (i1 as u32) < height
                        && 0 <= j1 && (j1 as u32) < width
                        && !emask[i1 as usize][j1 as usize]
                    {
                        idxs.push((i as u32, j as u32, i1 as u32, j1 as u32));
                    }
                }
            }
        }
    }
    idxs
}

fn dbg_dummy_image_source(color: Rgb<u8>) -> RgbImage {
    let mut img = RgbImage::new(1, 1);
    img[(0, 0)] = color;
    img
}
