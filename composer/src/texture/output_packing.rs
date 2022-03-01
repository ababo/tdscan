use std::collections::BTreeMap;

use rectangle_pack::{
    contains_smallest_box, pack_rects, volume_heuristic,
    GroupedRectsToPlace, PackedLocation, RectToInsert, TargetBin,
};

use crate::texture::{*, output_patching::LocalPatch};

const ACCURACY: f64 = 1e6;

pub fn pack_rectangles_with_automatic_stretching(
    sizes: &[[f64; 2]],
    spacing: f64,
) -> (Vec<Rectangle<f64>>, f64) {
    let biggest: f64 =
        extremum(sizes.iter().flatten().cloned(), Iterator::max_by);

    // Represents how closely to find the optimum stretching.
    const RTOL: f64 = 1e-3;

    // Makes the biggest rectangle not fit at all.
    let bound_failing = (1.0 + RTOL) / biggest;

    // Rectangles that don't take up any space can be packed trivially.
    let bound_succeeding = 0.0;

    // Use bisection to find the optimal stretching.
    let bounds = [bound_failing, bound_succeeding];
    let f = |s| try_pack_rectangles_with_given_stretching(sizes, spacing, s);
    let (scale, rectangles) = bisect(f, bounds, RTOL);

    (rectangles, scale)
}

pub fn try_pack_rectangles_with_given_stretching(
    sizes: &[[f64; 2]],
    spacing: f64,
    scale: f64,
) -> Option<Vec<Rectangle<f64>>> {
    let f2 = |x| spacing + x * scale + spacing;
    let sizes_transformed: Vec<[f64; 2]> =
        sizes.iter().map(|&[a, b]| [f2(a), f2(b)]).collect();
    let positions_scaled: Vec<[f64; 2]> =
        try_pack_rectangles(&sizes_transformed)?;
    let f1 = |r| spacing + r;
    let positions: Vec<[f64; 2]> = positions_scaled
        .iter()
        .map(|&[a, b]| [f1(a), f1(b)])
        .collect();

    let mut rectangles = vec![];
    let f0 = |x| x * scale;
    for i in 0..sizes.len() {
        let size = {
            let [a, b] = sizes[i];
            [f0(a), f0(b)]
        };
        let pos = positions[i];
        rectangles.push(Rectangle { pos, size });
    }

    Some(rectangles)
}

fn try_pack_rectangles(sizes: &[[f64; 2]]) -> Option<Vec<[f64; 2]>> {
    // Try to pack into unit box [0,1]x[0,1].
    let f = |x| (x * ACCURACY) as u32;
    let sizes_discrete: Vec<[u32; 2]> =
        sizes.iter().map(|&[a, b]| [f(a), f(b)]).collect();
    let positions_discrete =
        try_pack_rectangles_u32(&sizes_discrete, [f(1.0), f(1.0)])?;
    let f = |x| (x as f64) / ACCURACY;
    let positions = positions_discrete
        .iter()
        .map(|&[a, b]| [f(a), f(b)])
        .collect();
    Some(positions)
}

fn try_pack_rectangles_u32(
    sizes: &[[u32; 2]],
    bounding_size: [u32; 2],
) -> Option<Vec<[u32; 2]>> {
    let mut rects_to_place = GroupedRectsToPlace::<usize, ()>::new();
    for (i, size) in sizes.iter().enumerate() {
        rects_to_place.push_rect(
            i,
            None,
            RectToInsert::new(size[0], size[1], 1),
        );
    }

    let mut target_bins = BTreeMap::new();
    target_bins
        .insert((), TargetBin::new(bounding_size[0], bounding_size[1], 1));

    let rectangle_placements = pack_rects(
        &rects_to_place,
        &mut target_bins,
        &volume_heuristic,
        &contains_smallest_box,
    )
    .ok()?;
    let packed_locations = rectangle_placements.packed_locations();

    // Beware: These names "x()" "y()" "width()" "height()" are just names used
    // by `rectangle_pack`. They don't reflect the actual image coordinate
    // system that is used for texturing at a higher level.
    let f = |pl: PackedLocation| [pl.x(), pl.y()];
    let positions_u32: Vec<[u32; 2]> = (0..sizes.len())
        .map(|i| f(packed_locations[&i].1))
        .collect();

    Some(positions_u32)
}

pub fn globalize_uv(
    patches: &[LocalPatch],
    placements: &[Rectangle<f64>],
    mesh: &Mesh,
) -> Vec<[Vector2; 3]> {
    let mut uv_coords = vec![[Vector2::zeros(); 3]; mesh.faces.len()];
    for (patch, &rect) in patches.iter().zip(placements.iter()) {
        for (i, &uvs) in patch.to_global_coords(rect).iter().enumerate() {
            let j = patch.chunk[i];
            uv_coords[j] = uvs;
        }
    }
    uv_coords
}
