use std::collections::hash_map::Entry::Vacant;

use image::{Rgb, RgbImage};
use nalgebra::Dim;

use crate::texture::{
    input_selection::{FrameMetrics, Metrics},
    *,
};

fn copy_triangle(
    image0: &RgbImage,
    uv_coords0: [Vector2; 3],
    image1: &mut RgbImage,
    uv_coords1: [Vector2; 3],
    emptiness_mask: &mut ImageMask,
    face_idx: usize,
    color_correction: &ColorCorrection,
) -> Option<()> {
    // Rescale coordinates 0 <= [u,v] <= 1 to 0 <= [i,j] <= [h,w].
    let ij00 = uv_to_ij(uv_coords0[0], image0);
    let ij01 = uv_to_ij(uv_coords0[1], image0);
    let ij02 = uv_to_ij(uv_coords0[2], image0);
    let ij10 = uv_to_ij(uv_coords1[0], image1);
    let ij11 = uv_to_ij(uv_coords1[1], image1);
    let ij12 = uv_to_ij(uv_coords1[2], image1);

    // Create local coordinate systems in both source and target images.
    let bcs0 = BarycentricCoordinateSystem::new([ij00, ij01, ij02])?;
    let bcs1 = BarycentricCoordinateSystem::new([ij10, ij11, ij12])?;

    // Create a bounding box for the triangle in the target image.
    let g = |ij: Vector2| [ij[0] as u32, ij[1] as u32];
    let rect1 = Rectangle::bounding(&[g(ij10), g(ij11), g(ij12)]);

    // Iterate over pixels inside the bounding box and fetch color values.
    let mut dbg_any = false;
    for i1 in rect1.pos[0]..=rect1.pos[0] + rect1.size[0] {
        for j1 in rect1.pos[1]..=rect1.pos[1] + rect1.size[1] {
            let ij1 = Vector2::new(i1 as f64, j1 as f64);
            let bary = bcs1.infer(ij1);
            let ij0 = bcs0.apply(bary);
            let uv0 = ij_to_uv(ij0, image0);

            let sampled_color = sample_pixel(uv0, image0);
            let offset = color_correction.sample_color_offset(face_idx, bary);
            let color = sampled_color + 1.0 * offset;

            if all_nonneg(bary) && i1 < image1.height() && j1 < image1.width() {
                set_pixel_ij_as_vector3(i1, j1, color, image1);
                emptiness_mask[(i1 as usize, j1 as usize)] = false;
                dbg_any = true;
            }
        }
    }
    if dbg_any {
        Some(())
    } else {
        None
    }
}

// Rectangular, image-shaped grid of booleans, that represents whether
// any given pixel has not been written to yet during baking.
//pub type ImageMask = OMatrix<bool, Dynamic, Dynamic>;

pub fn bake_texture(
    mesh: &Mesh,
    images: &[Option<RgbImage>],
    chosen_cameras: &[Option<usize>],
    vertex_metrics: &[FrameMetrics],
    uv_coords_tri: &[[Vector2; 3]],
    image_res: usize,
    color_correction: &ColorCorrection,
) -> (RgbImage, ImageMask) {
    let mut buffer = RgbImage::new(image_res as u32, image_res as u32);
    let dim = Dim::from_usize(image_res);
    let mut emask = ImageMask::from_element_generic(dim, dim, true);

    let dummy_image_source_black = dummy_image_source(Rgb([0, 0, 0]));

    for face_idx in 0..mesh.faces.len() {
        let img0;
        let uvs0;
        if let Some(frame_idx) = chosen_cameras[face_idx] {
            // Load image source.
            img0 = images[frame_idx].as_ref().unwrap();

            // Define coordinates for image source.
            uvs0 = uv_coords_from_metrics(
                face_idx,
                frame_idx,
                vertex_metrics,
                mesh,
            );
        } else {
            img0 = &dummy_image_source_black;
            uvs0 = [
                Vector2::new(0.0, 0.0),
                Vector2::new(1.0, 0.0),
                Vector2::new(0.0, 1.0),
            ];
        }

        // Define coordinates for output buffer.
        let uvs1 = uv_coords_tri[face_idx];

        // Copy triangle.
        copy_triangle(
            img0,
            uvs0,
            &mut buffer,
            uvs1,
            &mut emask,
            face_idx,
            color_correction,
        );
    }

    (buffer, emask)
}

pub fn uv_coords_from_metrics(
    face_idx: usize,
    frame_idx: usize,
    vertex_metrics: &[FrameMetrics],
    mesh: &Mesh,
) -> [Vector2; 3] {
    let [v0, v1, v2] = mesh.faces[face_idx];
    let single_frame_metrics = vertex_metrics[frame_idx].as_ref().unwrap();
    let f = |v| (single_frame_metrics[v] as Metrics).pixel;
    [f(v0), f(v1), f(v2)]
}

pub fn compress_uv_coords(
    uv_coords: &[[Vector2; 3]],
) -> (Vec<Vector2>, Vec<[usize; 3]>) {
    const EPS: f64 = 1e-6; // Round coordinates to this size, then merge them.
    let up0 = |x| (x / EPS) as u64;
    let up1 = |uv: Vector2| [up0(uv[0]), up0(uv[1])];
    let down0 = |x| x as f64 * EPS;
    let down1 = |uv: [u64; 2]| Vector2::new(down0(uv[0]), down0(uv[1]));

    let mut uv_unique = HashMap::new();
    let mut uv_ordered = vec![];
    let mut uv_idxs = vec![];

    for uvs in uv_coords {
        let mut idxs = [0, 0, 0];
        for j in 0..3 {
            let uv = up1(uvs[j]);
            if let Vacant(e) = uv_unique.entry(uv) {
                e.insert(uv_ordered.len());
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
    emask: &mut ImageMask,
    gutter_size: usize,
) {
    for _ in 0..gutter_size {
        for (i, j, i1, j1) in resolve_gutter_source(emask) {
            // Beware that the image is indexed as (j, i).
            buffer[(j, i)] = buffer[(j1, i1)];
            emask[(i as usize, j as usize)] = false;
        }
    }
}

fn resolve_gutter_source(emask: &ImageMask) -> Vec<(u32, u32, u32, u32)> {
    let mut idxs = vec![];
    let (height, width) = emask.shape();
    for i in 0..height as i32 {
        for j in 0..width as i32 {
            if emask[(i as usize, j as usize)] {
                for (i1, j1) in [(i - 1, j), (i + 1, j), (i, j - 1), (i, j + 1)]
                {
                    if 0 <= i1
                        && (i1 as usize) < height
                        && 0 <= j1
                        && (j1 as usize) < width
                        && !emask[(i1 as usize, j1 as usize)]
                    {
                        idxs.push((i as u32, j as u32, i1 as u32, j1 as u32));
                    }
                }
            }
        }
    }
    idxs
}

fn dummy_image_source(color: Rgb<u8>) -> RgbImage {
    let mut img = RgbImage::new(1, 1);
    img[(0, 0)] = color;
    img
}
