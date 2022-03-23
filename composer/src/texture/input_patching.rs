use core::cmp::Ordering;
use std::collections::BinaryHeap;

use petgraph::unionfind::UnionFind;

use crate::mesh::Mesh;
use crate::misc::{vec_inv, vec_inv_many};
use crate::texture::*;

struct TmpPatch {
    face_idxs: Vec<usize>,
    frame_idx: usize,
}


impl Ord for TmpPatch {
    fn cmp(&self, other: &Self) -> Ordering {
        self.face_idxs.len().partial_cmp(&other.face_idxs.len()).unwrap()
    }
}

impl PartialOrd for TmpPatch {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for TmpPatch {
    fn eq(&self, other: &Self) -> bool {
        self.face_idxs.len() == other.face_idxs.len()
    }
}

impl Eq for TmpPatch {}

fn make_provisory_partition(
    face_idxs: &[usize],
    _mesh: &Mesh,
    topo: &BasicMeshTopology,
    chosen_cameras: &[Option<usize>],
    force_camera: Option<usize>,
) -> Vec<TmpPatch> {
    let carrier = face_idxs; //mask_to_idxs(faces_mask);
    let carrier_inv = vec_inv(carrier);
    let mut partition = UnionFind::new(carrier.len());

    for &f0 in carrier {
        if let Some(&c0) = carrier_inv.get(&f0) {
            for &f1 in &topo.neighbouring_faces[f0] {
                if chosen_cameras[f0] != chosen_cameras[f1] && force_camera.is_none() {
                    continue;
                }
                if let Some(&c1) = carrier_inv.get(&f1) {
                    partition.union(c0, c1);
                }
            }
        }
    }
    
    let family = vec_inv_many(&partition.into_labeling());
    
    family.iter().map(|(&repr, part)| TmpPatch {
        face_idxs: part.iter().map(|&c| carrier[c]).collect(),
        frame_idx: force_camera.or_else(|| chosen_cameras[carrier[repr]]).unwrap()
    }).collect()
}

fn face_is_acceptable(metrics: &Metrics, angle_limit: f64) -> bool {
    metrics.within_bounds
        && !metrics.is_occluded
        //&& !metrics.is_background
        && metrics.depth > 0.0
    //build_cost_for_single_face(metrics) < f64::INFINITY
        && metrics.dot_product > f64::cos(angle_limit)
    // TODO: try a metrics ratio instead, to make top faces patch more
}

fn build_acceptability_record(
    mesh: &Mesh,
    face_metrics: &[FrameMetrics],
    chosen_cameras: &[Option<usize>],
    angle_limit: f64,
) -> Vec<Option<Vec<usize>>> {
    face_metrics.iter().enumerate().map(|(frame_idx, face_metrics)| {
        face_metrics.as_ref().map(|face_metrics| {
            (0..mesh.faces.len()).filter(|&face_idx| {
                let condition = face_is_acceptable(&face_metrics[face_idx], angle_limit);
                /*if chosen_cameras[face_idx] == Some(frame_idx) {
                    // A face which was chosen before patching must be acceptable.
                    assert!(condition);
                }*/
                condition
            }).collect::<Vec<usize>>()
        })
    }).collect()
}

pub fn form_patches(
    chosen_cameras: &mut [Option<usize>],
    face_metrics: &[FrameMetrics],
    mesh: &Mesh,
    topo: &BasicMeshTopology,
    angle_limit: f64
) {
    //if true { return; }  // DEBUG
    
    // For each image frame and each mesh face, record whether it is acceptably well-visible.
    let mut acceptable = build_acceptability_record(mesh, face_metrics, chosen_cameras, angle_limit);
    
    // Collection of available faces that have only been assigned to a provisory patch so far.
    let mut remaining: Vec<bool> = chosen_cameras.iter().map(|i| i.is_some()).collect();
    
    // Collection of patches that are still subject to change.
    let mut provisory: BinaryHeap<TmpPatch> = BinaryHeap::new();
    for patch in make_provisory_partition(&mask_to_idxs(&remaining), mesh, topo, chosen_cameras, None) {
        provisory.push(patch);
    }
    
    // Go through the provisory patches from biggest to smallest,
    // and let each grow to encompass reasonably well-visible nearby faces,
    // unless some of its faces have been "stolen" from it, in which case
    // the patch is re-partitioned and each fragment is put back into the heap.
    while !provisory.is_empty() {
        let TmpPatch { mut face_idxs, frame_idx } = provisory.pop().unwrap();
        
        let len0 = face_idxs.len();
        face_idxs.retain(|&face_idx| remaining[face_idx]);
        if face_idxs.len() == len0 {
            // The patch is grown to encompass reasonably good nearby faces.
            let acceptable_here: &mut Vec<usize> = acceptable.get_mut(frame_idx).unwrap().as_mut().unwrap();
            acceptable_here.retain(|&face_idx| remaining[face_idx]);
            
            for larger_patch in make_provisory_partition(acceptable_here, mesh, topo, chosen_cameras, Some(frame_idx)) {
                if larger_patch.face_idxs.contains(&face_idxs[0]) {
                    for face_idx in larger_patch.face_idxs {
                        chosen_cameras[face_idx] = Some(frame_idx);
                        remaining[face_idx] = false;
                    }
                    break;
                }
            }
        } else {
            // The patch is caused to fragment. The fragments will be dealt with later.
            for patch in make_provisory_partition(&face_idxs, mesh, topo, chosen_cameras, None) {
                provisory.push(patch);
            }
        }
    }
}
