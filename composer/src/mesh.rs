use std::cmp::{Ord, Ordering};
use std::collections::{BinaryHeap, HashMap, HashSet};

use derive_more::{Add, AddAssign};
use petgraph::unionfind::UnionFind;

use crate::point_cloud::{
    validate_point_bounds, Matrix4, Point3, PointCloudParams, Vector3, Vector4,
};
use crate::poisson;

#[derive(Default)]
pub struct Mesh {
    pub vertices: Vec<Point3>,
    pub normals: Vec<Vector3>,
    pub faces: Vec<[usize; 3]>,
}

impl poisson::Mesh<f64> for Mesh {
    fn add_vertex(&mut self, vertex: &[f64; 3]) {
        self.vertices.push(Point3::from(*vertex));
    }

    fn add_normal(&mut self, normal: &[f64; 3]) {
        self.normals.push(Vector3::from(*normal));
    }

    fn add_triangle(&mut self, triangle: &[usize; 3]) {
        self.faces.push(*triangle);
    }
}

impl Mesh {
    pub fn apply_bounds(&mut self, params: &PointCloudParams) {
        assert_eq!(self.vertices.len(), self.normals.len());
        let mut mappings = HashMap::with_capacity(self.vertices.len());

        let mut j = 0;
        for i in 0..self.vertices.len() {
            if validate_point_bounds(
                &self.vertices[i],
                params.min_z,
                params.max_z,
                params.max_z_distance,
            ) {
                mappings.insert(i, j);
                self.vertices.swap(i, j);
                self.normals.swap(i, j);
                j += 1;
            }
        }
        self.vertices.truncate(j);
        self.normals.truncate(j);

        let mut j = 0;
        'next: for i in 0..self.faces.len() {
            for k in 0..self.faces[i].len() {
                if let Some(l) = mappings.get(&self.faces[i][k]) {
                    self.faces[j][k] = *l;
                } else {
                    continue 'next;
                }
            }
            j += 1;
        }
        self.faces.truncate(j);
    }

    pub fn smoothen(&mut self, num_iters: usize) {
        assert!(self.vertices.len() == self.normals.len());

        let mut sums =
            vec![(Vector3::zeros(), Vector3::zeros(), 0); self.vertices.len()];

        for _ in 0..num_iters {
            for sum in sums.iter_mut() {
                *sum = (Vector3::zeros(), Vector3::zeros(), 0);
            }

            for triangle in self.faces.iter() {
                sums[triangle[0]].0 += self.vertices[triangle[1]].coords;
                sums[triangle[0]].0 += self.vertices[triangle[2]].coords;
                sums[triangle[0]].1 += self.normals[triangle[1]];
                sums[triangle[0]].1 += self.normals[triangle[2]];
                sums[triangle[0]].2 += 2;

                sums[triangle[1]].0 += self.vertices[triangle[0]].coords;
                sums[triangle[1]].0 += self.vertices[triangle[2]].coords;
                sums[triangle[1]].1 += self.normals[triangle[0]];
                sums[triangle[1]].1 += self.normals[triangle[2]];
                sums[triangle[1]].2 += 2;

                sums[triangle[2]].0 += self.vertices[triangle[0]].coords;
                sums[triangle[2]].0 += self.vertices[triangle[1]].coords;
                sums[triangle[2]].1 += self.normals[triangle[0]];
                sums[triangle[2]].1 += self.normals[triangle[1]];
                sums[triangle[2]].2 += 2;
            }

            #[allow(clippy::needless_range_loop)]
            for i in 0..self.vertices.len() {
                self.vertices[i].coords = sums[i].0 / sums[i].2 as f64;
                self.normals[i] = sums[i].1 / sums[i].2 as f64;
            }
        }
    }

    pub fn decimate(self, ratio: f64) -> Mesh {
        Decimator::execute(self, ratio)
    }
}

#[derive(Add, AddAssign, Copy, Clone)]
struct Quadric(Matrix4);

impl Quadric {
    // Chosen by the Blender devs and represents a value
    // below which the optimization problem is too unstable.
    const OPTIMIZE_EPS: f64 = 1e-8;

    pub fn make_planar(p: Vector3, n: Vector3) -> Quadric {
        let plane = Vector4::new(n[0], n[1], n[2], -p.dot(&n));
        Quadric(plane * plane.transpose())
    }

    pub fn optimum(&self) -> Option<Vector3> {
        let a = self.0.fixed_slice::<3, 3>(0, 0);
        if a.determinant().abs() > Self::OPTIMIZE_EPS {
            let b = self.0.fixed_slice::<3, 1>(0, 3);
            Some(-a.cholesky().unwrap().solve(&b))
        } else {
            None
        }
    }

    pub fn eval(&self, p: Vector3) -> f64 {
        let v = Vector4::new(p[0], p[1], p[2], 1.0);
        v.dot(&(self.0 * v))
    }

    pub fn zero() -> Quadric {
        Quadric(nalgebra::zero())
    }
}

struct Candidate {
    cost: f64,             // Cost of contraction.
    edge: [usize; 2],      // Edge to be contracted.
    point: Vector3,        // Optimal point of contraction.
    timestamp: [usize; 2], // Used to identify and discard obsolete candidates.
}

impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> Ordering {
        self.cost.partial_cmp(&other.cost).unwrap().reverse()
    }
}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Candidate {
    fn eq(&self, other: &Self) -> bool {
        self.cost == other.cost
    }
}

impl Eq for Candidate {}

pub struct Decimator {
    mesh: Mesh,
    vertex_quadrics: Vec<Quadric>,
    vertex_partition: UnionFind<usize>,
    vertex_partition_sizes: Vec<usize>,
    vertices_around_vertex: Vec<HashSet<usize>>,
    edge_heap: BinaryHeap<Candidate>,
}

impl Decimator {
    pub fn execute(mesh: Mesh, ratio: f64) -> Mesh {
        assert!(0.0 < ratio && ratio <= 1.0);

        let mut d = Decimator::new(mesh);

        let mut faces_to_remove =
            (d.mesh.faces.len() as f64 * (1.0 - ratio)) as isize;
        while faces_to_remove > 0 && !d.edge_heap.is_empty() {
            let c = d.edge_heap.pop().unwrap();
            if d.try_contract(c.point, c.edge, c.timestamp) {
                faces_to_remove -= 2;
            }
        }

        d.finalize()
    }

    fn new(mesh: Mesh) -> Decimator {
        // Build metrics for cost computations.
        let mut vertex_quadrics = vec![Quadric::zero(); mesh.vertices.len()];
        for &[v0, v1, v2] in &mesh.faces {
            let v_mid = (mesh.vertices[v0].coords
                + mesh.vertices[v1].coords
                + mesh.vertices[v2].coords)
                / 3.0;
            let diff1 = mesh.vertices[v1] - mesh.vertices[v0];
            let diff2 = mesh.vertices[v2] - mesh.vertices[v0];
            let n_mid = diff1.cross(&diff2).normalize();
            let q = Quadric::make_planar(v_mid, n_mid);
            vertex_quadrics[v0] += q;
            vertex_quadrics[v1] += q;
            vertex_quadrics[v2] += q;
        }

        // Initialize partition of the vertex indices.
        let vertex_partition = UnionFind::new(mesh.vertices.len());
        let vertex_partition_sizes = vec![1; mesh.vertices.len()];

        // Initialize a simple topology cache.
        let mut vertices_around_vertex =
            vec![HashSet::new(); mesh.vertices.len()];
        for &[v0, v1, v2] in &mesh.faces {
            vertices_around_vertex[v0].insert(v1);
            vertices_around_vertex[v0].insert(v2);
            vertices_around_vertex[v1].insert(v0);
            vertices_around_vertex[v1].insert(v2);
            vertices_around_vertex[v2].insert(v0);
            vertices_around_vertex[v2].insert(v1);
        }

        let mut decimator = Decimator {
            mesh,
            vertex_quadrics,
            vertex_partition,
            vertex_partition_sizes,
            vertices_around_vertex,
            edge_heap: BinaryHeap::new(),
        };

        // Put all edges in the decimator queue.
        for &[v0, v1, v2] in &decimator.mesh.faces {
            assert!(v0 == decimator.vertex_partition.find(v0));
            assert!(v1 == decimator.vertex_partition.find(v1));
            assert!(v2 == decimator.vertex_partition.find(v2));

            for edge in [[v0, v1], [v0, v2], [v1, v2]] {
                let (point, cost) = decimator.optimize_single_edge(edge);
                let timestamp = decimator.edge_timestamp(edge);
                decimator.edge_heap.push(Candidate {
                    cost,
                    edge,
                    point,
                    timestamp,
                });
            }
        }

        decimator
    }

    fn optimize_single_edge(&self, e: [usize; 2]) -> (Vector3, f64) {
        let quadric = self.vertex_quadrics[e[0]] + self.vertex_quadrics[e[1]];
        let point = if let Some(p) = quadric.optimum() {
            p
        } else {
            let p0 = self.mesh.vertices[e[0]];
            let p1 = self.mesh.vertices[e[1]];
            (p0.coords + p1.coords) / 2.0
        };
        (point, quadric.eval(point))
    }

    fn edge_timestamp(&self, e: [usize; 2]) -> [usize; 2] {
        [
            self.vertex_partition_sizes[e[0]],
            self.vertex_partition_sizes[e[1]],
        ]
    }

    fn try_contract(
        &mut self,
        point: Vector3,
        edge: [usize; 2],
        ts: [usize; 2],
    ) -> bool {
        if !self.validate_timestamp(edge, ts) {
            return false;
        }

        let [v0, v1] = edge;
        assert!(v0 != v1);
        assert!(v0 == self.vertex_partition.find(v0));
        assert!(v1 == self.vertex_partition.find(v1));

        // One of the memory locations v0 and v1 will be reused
        // for the new vertex v. The other location will be unused.
        self.vertex_partition.union(v0, v1);
        let v = self.vertex_partition.find(v0);

        self.vertex_partition_sizes[v] =
            self.vertex_partition_sizes[v0] + self.vertex_partition_sizes[v1];
        self.mesh.vertices[v] = Point3::from(point);
        self.mesh.normals[v] =
            (self.mesh.normals[v0] + self.mesh.normals[v1]).normalize();
        self.vertex_quadrics[v] =
            self.vertex_quadrics[v0] + self.vertex_quadrics[v1];
        self.vertices_around_vertex[v] = self.vertices_around_vertex[v0]
            .union(&self.vertices_around_vertex[v1])
            .map(|&w| self.vertex_partition.find(w))
            .collect();
        self.vertices_around_vertex[v].remove(&v0);
        self.vertices_around_vertex[v].remove(&v1);

        for &v2 in &self.vertices_around_vertex[v] {
            assert!(v == self.vertex_partition.find(v));
            assert!(v2 == self.vertex_partition.find(v2));
            let edge = [v, v2];
            let (point, cost) = self.optimize_single_edge(edge);
            let timestamp = self.edge_timestamp(edge);
            self.edge_heap.push(Candidate {
                cost,
                edge,
                point,
                timestamp,
            });
        }

        true
    }

    fn validate_timestamp(&self, edge: [usize; 2], ts: [usize; 2]) -> bool {
        edge[0] == self.vertex_partition.find(edge[0])
            && edge[1] == self.vertex_partition.find(edge[1])
            && self.edge_timestamp(edge) == ts
    }

    fn finalize(self) -> Mesh {
        let mut kept_indices = self.vertex_partition.clone().into_labeling();
        kept_indices.sort_unstable();
        kept_indices.dedup();
        let new_indices = HashMap::<usize, usize>::from_iter(
            kept_indices.iter().enumerate().map(|(i, &j)| (j, i)),
        );

        let vertices: Vec<Point3> = kept_indices
            .iter()
            .map(|&i| self.mesh.vertices[i])
            .collect();
        let normals: Vec<Vector3> =
            kept_indices.iter().map(|&i| self.mesh.normals[i]).collect();
        let mut faces: Vec<[usize; 3]> = self
            .mesh
            .faces
            .iter()
            .map(|&[v0, v1, v2]| {
                [
                    new_indices[&self.vertex_partition.find(v0)],
                    new_indices[&self.vertex_partition.find(v1)],
                    new_indices[&self.vertex_partition.find(v2)],
                ]
            })
            .filter(|[v0, v1, v2]| v0 != v1 && v0 != v2 && v1 != v2)
            .collect();
        faces.sort_unstable();
        faces.dedup();

        Mesh {
            vertices,
            normals,
            faces,
        }
    }
}
