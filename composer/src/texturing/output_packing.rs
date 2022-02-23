use crate::texturing::misc::*;

use crate::texturing::output_patching::{
    LocalPatch,
};

const ACCURACY: f64 = 1e6;

pub fn pack_rectangles_with_automatic_stretching(
    sizes: &Vec<[f64; 2]>,
    spacing: f64
) -> (Vec<Rectangle<f64>>, f64) {

    let biggest: f64 = *sizes
        .iter()
        .flatten()
        .max_by(|p, q| p.partial_cmp(&q).unwrap())
        .unwrap();
    let bound_failing = 1.01 / biggest;  // makes the biggest one not fit at all
    let bound_succeeding = 0.0;
    let bounds = [bound_failing, bound_succeeding];
    let rtol = 1e-3;

    let f = |s| try_pack_rectangles_with_given_stretching(sizes, spacing, s);
    let (scale, rectangles): (f64, Vec<Rectangle<f64>>) =
        bisect(f, bounds, rtol);  // <-- main loop of packing algorithm

    (rectangles, scale)
}

pub fn try_pack_rectangles_with_given_stretching(
    sizes: &Vec<[f64; 2]>,
    spacing: f64,
    scale: f64
) -> Option<Vec<Rectangle<f64>>> {
    let f2 = |x| spacing + x*scale + spacing;
    let sizes_transformed: Vec<[f64; 2]> =
        sizes.iter().map(|&[a, b]| [f2(a), f2(b)]).collect();
    let positions_scaled: Vec<[f64; 2]> =
        try_pack_rectangles(&sizes_transformed)?;
    let f1 = |r| spacing + r;
    let positions: Vec<[f64; 2]> =
        positions_scaled.iter().map(|&[a, b]| [f1(a), f1(b)]).collect();
    
    let mut rectangles: Vec<Rectangle<f64>> = vec![];
    let f0 = |x| x*scale;
    for i in 0..sizes.len() {
        let size = { let [a, b] = sizes[i]; [f0(a), f0(b)] };
        let pos = positions[i];
        rectangles.push(Rectangle { pos, size });
    }
    
    Some(rectangles)
}

fn try_pack_rectangles(sizes: &Vec<[f64; 2]>) -> Option<Vec<[f64; 2]>> {
    // try to pack into unit box [0,1]x[0,1]
    let f = |x| (x * ACCURACY) as u32;
    let sizes_discrete: Vec<[u32; 2]> =
        sizes.iter().map(|&[a, b]| [f(a), f(b)]).collect();
    let positions_discrete =
        try_pack_rectangles_u32(&sizes_discrete, [f(1.0), f(1.0)])?;
    let f = |x| (x as f64) / ACCURACY;
    let positions =
        positions_discrete.iter().map(|&[a, b]| [f(a), f(b)]).collect();
    Some(positions)
}

fn try_pack_rectangles_u32(
    sizes: &Vec<[u32; 2]>,
    bounding_size: [u32; 2]
) -> Option<Vec<[u32; 2]>> { 
    use rectangle_pack::{
        GroupedRectsToPlace,
        RectToInsert,
        pack_rects,
        TargetBin,
        volume_heuristic,
        contains_smallest_box,
        PackedLocation
    };
    use std::collections::BTreeMap;
    
    let mut rects_to_place: GroupedRectsToPlace<i32, ()> =
        GroupedRectsToPlace::new();
    for (i, size) in sizes.iter().enumerate() {
        rects_to_place.push_rect(
            i as i32,
            None,
            RectToInsert::new(size[0], size[1], 1)
        );
    }

    let mut target_bins = BTreeMap::new();
    target_bins.insert((), TargetBin::new(
        bounding_size[0], bounding_size[1], 1));

    let rectangle_placements = pack_rects(
        &rects_to_place,
        &mut target_bins,
        &volume_heuristic,
        &contains_smallest_box
    ).ok()?;
    let packed_locations = rectangle_placements.packed_locations();

    // Beware: These names "x()" "y()" "width()" "height()" are just names used
    // by `rectangle_pack`. They don't reflect the actual image coordinate
    // system that is used for texturing at a higher level.
    let f = |pl: PackedLocation| [pl.x(), pl.y()];
    let positions_u32: Vec<[u32; 2]> =
        (0..sizes.len())
        .map(|i| f(packed_locations[&(i as i32)].1))
        .collect();
    
    Some(positions_u32)
}

pub fn globalize_uv(
    patches: &Vec<LocalPatch>,
    placements: &Vec<Rectangle<f64>>,
    mesh: &Mesh
) -> Vec<[UV; 3]> {
    let mut uv_coords: Vec<[UV; 3]> = vec![[UV::zeros(); 3]; mesh.faces.len()];
    for (patch, &rect) in patches.iter().zip(placements.iter()) {
        for (i, &uvs) in patch.to_global_coords(rect).iter().enumerate() {
            let j = patch.chunk[i];
            uv_coords[j] = uvs;
        }
    }
    uv_coords
}

pub fn dbg_show_packing(rects: &Vec<Rectangle<f64>>) -> RgbImage {
    let mut buffer = RgbImage::new(4096 as u32, 4096 as u32);

    let mut count_good = 0;
    let mut count_bad = 0;
    let mut count_really_bad = 0;

    let mut maxi = 0;
    let mut maxj = 0;
    
    for rect in rects {
        let i0 = ( rect.pos[0]*4096.0 ).floor() as u32;
        let j0 = ( rect.pos[1]*4096.0 ).floor() as u32;
        let i1 = ( (rect.pos[0] + rect.size[0])*4096.0 ).ceil() as u32;
        let j1 = ( (rect.pos[1] + rect.size[1])*4096.0 ).ceil() as u32;
        for i in i0..=i1 {
            for j in j0..=j1 {
                if i < 4096 && j < 4096 {
                    let color =
                        if i == i0 || i == i1 || j == j0 || j == j1 {
                            Rgb([255, 0, 0])
                        } else {
                            Rgb([255, 100, 0])
                        };
                    buffer.put_pixel(j, i, color);
                    count_good += 1;
                } else {
                    count_bad += 1;
                    if i > 4100 || j > 4100 {
                        count_really_bad += 1;
                    }
                }
                if maxi < i {
                    maxi = i;
                }
                if maxj < j {
                    maxj = j;
                }
            }
        }
    }
    
    dbg!(count_good);
    dbg!(count_bad);
    dbg!(count_really_bad);

    dbg!(maxi);
    dbg!(maxj);

    buffer
}
