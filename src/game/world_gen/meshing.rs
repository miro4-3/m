//! Binary greedy meshing, after <https://github.com/cgerikj/binary-greedy-meshing>.
//!
//! Solid voxels are packed into bit columns, so finding exposed faces and merging
//! them into rectangles works on a whole row of voxels per machine instruction.

use macroquad::{
    color::Color,
    math::{Vec3, vec3},
    models::{Mesh, Vertex},
};

use crate::{
    constant::{CHUNK_WIDTH as N, CHUNK_WIDTH_I32 as W, FACES, MAX_FACES_PER_MESH},
    game::world_gen::{chunk::STRIDE, voxel::Voxel},
};

/// One bit per voxel along a row or column.
type Bits = u32;
const _: () = assert!(N <= Bits::BITS as usize && Voxel::KINDS <= Bits::BITS as usize);
const _: () = assert!(STRIDE[0] == 1);

/// Voxels of the six face-adjacent chunks in `FACES` order. `None` (not loaded) counts as air.
pub type Neighbours<'a> = [Option<&'a [Voxel]>; 6];

static AIR_CHUNK: [Voxel; N * N * N] = [Voxel::AIR; N * N * N];

/// For each normal axis: the two other axes, fastest-varying in memory first.
const TANGENTS: [(usize, usize); 3] = [(2, 1), (0, 2), (0, 1)];

/// `columns[axis][v * N + u]`: bit `s` is set when the voxel at slice `s` along `axis` is solid.
type Columns = [[Bits; N * N]; 3];

/// Exposed faces of one slice, one plane per voxel type: `planes[slice][kind][v]` has bit `u` set.
type Planes = [[[Bits; N]; Voxel::KINDS]; N];

pub fn greedy_mesh(voxels: &[Voxel], key: (i32, i32, i32), neighbours: Neighbours) -> Vec<Mesh> {
    let Some(columns) = solid_columns(voxels) else {
        return Vec::new();
    };
    let mut mesher = Mesher {
        voxels,
        neighbours,
        columns,
        origin: [key.0 * W, key.1 * W, key.2 * W],
        planes: [[[0; N]; Voxel::KINDS]; N],
        touched: [0; N],
        out: MeshBuilder::default(),
    };
    for (index, (normal, corners, shade)) in FACES.iter().enumerate() {
        let face = Face::new(index, *normal, *corners, *shade);
        mesher.collect_exposed(&face);
        mesher.emit_quads(&face);
    }
    mesher.out.finish()
}

/// Packs solidity into bit columns along all three axes. `None` if the chunk is empty.
fn solid_columns(voxels: &[Voxel]) -> Option<Columns> {
    let mut columns = [[0; N * N]; 3];
    let mut any = 0;
    for y in 0..N {
        for z in 0..N {
            let row = &voxels[y * STRIDE[1] + z * STRIDE[2]..][..N];
            let mut along_x = 0;
            for (x, voxel) in row.iter().enumerate() {
                let solid = voxel.is_solid() as Bits;
                along_x |= solid << x;
                columns[1][z * N + x] |= solid << y;
                columns[2][y * N + x] |= solid << z;
            }
            columns[0][y * N + z] = along_x;
            any |= along_x;
        }
    }
    (any != 0).then_some(columns)
}

struct Face {
    index: usize,
    d: usize,
    u: usize,
    v: usize,
    step: i32,
    corners: [[f32; 3]; 4],
    shade: f32,
}

impl Face {
    fn new(index: usize, normal: [i32; 3], corners: [[f32; 3]; 4], shade: f32) -> Self {
        let d = normal.iter().position(|&c| c != 0).unwrap();
        let (u, v) = TANGENTS[d];
        Face { index, d, u, v, step: normal[d], corners, shade }
    }

    /// Stretches the unit face to a `w` x `h` rectangle starting at (`a`, `b`) in slice `s`.
    fn quad(&self, origin: [i32; 3], s: usize, a: usize, b: usize, w: usize, h: usize) -> [Vec3; 4] {
        self.corners.map(|c| {
            let mut p = [0.0; 3];
            p[self.d] = (origin[self.d] + s as i32) as f32 + c[self.d];
            p[self.u] = (origin[self.u] + a as i32) as f32 + c[self.u] * w as f32;
            p[self.v] = (origin[self.v] + b as i32) as f32 + c[self.v] * h as f32;
            vec3(p[0], p[1], p[2])
        })
    }
}

struct Mesher<'a> {
    voxels: &'a [Voxel],
    neighbours: Neighbours<'a>,
    columns: Columns,
    origin: [i32; 3],
    planes: Planes,
    /// Per slice: bit `k` set when `planes[slice][k]` holds faces.
    touched: [Bits; N],
    out: MeshBuilder,
}

impl Mesher<'_> {
    /// Sorts every exposed face of this direction into the plane of its slice and voxel type.
    fn collect_exposed(&mut self, f: &Face) {
        let (su, sv, sd) = (STRIDE[f.u], STRIDE[f.v], STRIDE[f.d]);
        let positive = f.step > 0;
        let behind = self.neighbours[f.index].unwrap_or(&AIR_CHUNK[..]);
        let touching_slice = if positive { 0 } else { N - 1 };

        for b in 0..N {
            for a in 0..N {
                let column = self.columns[f.d][b * N + a];
                let beyond = behind[touching_slice * sd + a * su + b * sv].is_solid() as Bits;
                let covered = if positive { column >> 1 | beyond << (N - 1) } else { column << 1 | beyond };

                let mut exposed = column & !covered;
                while exposed != 0 {
                    let s = exposed.trailing_zeros() as usize;
                    exposed &= exposed - 1;
                    let kind = self.voxels[s * sd + a * su + b * sv].kind();
                    self.planes[s][kind][b] |= 1 << a;
                    self.touched[s] |= 1 << kind;
                }
            }
        }
    }

    fn emit_quads(&mut self, f: &Face) {
        for s in 0..N {
            let mut kinds = std::mem::take(&mut self.touched[s]);
            while kinds != 0 {
                let kind = kinds.trailing_zeros() as usize;
                kinds &= kinds - 1;
                let color = shaded(Voxel::from_kind(kind).color(), f.shade);
                self.merge_plane(f, s, kind, color);
            }
        }
    }

    /// Takes the widest run of faces from a row, then grows it down while the rows below match.
    fn merge_plane(&mut self, f: &Face, s: usize, kind: usize, color: Color) {
        let plane = &mut self.planes[s][kind];
        for b in 0..N {
            let mut row = std::mem::take(&mut plane[b]);
            while row != 0 {
                let a = row.trailing_zeros();
                let w = (row >> a).trailing_ones();
                let run = (Bits::MAX >> (Bits::BITS - w)) << a;
                row &= !run;

                let mut h = 1;
                while b + h < N && plane[b + h] & run == run {
                    plane[b + h] &= !run;
                    h += 1;
                }
                let corners = f.quad(self.origin, s, a as usize, b, w as usize, h);
                self.out.quad(corners, color);
            }
        }
    }
}

fn shaded(c: Color, shade: f32) -> Color {
    Color::new(c.r * shade, c.g * shade, c.b * shade, 1.0)
}

/// Collects quads, starting a new mesh whenever one is full.
#[derive(Default)]
struct MeshBuilder {
    meshes: Vec<Mesh>,
    vertices: Vec<Vertex>,
    indices: Vec<u16>,
}

impl MeshBuilder {
    fn quad(&mut self, corners: [Vec3; 4], color: Color) {
        if self.indices.len() / 6 >= MAX_FACES_PER_MESH {
            self.flush();
        }
        let base = self.vertices.len() as u16;
        self.vertices
            .extend(corners.map(|p| Vertex::new(p.x, p.y, p.z, 0.0, 0.0, color)));
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    fn flush(&mut self) {
        self.meshes.push(Mesh {
            vertices: std::mem::take(&mut self.vertices),
            indices: std::mem::take(&mut self.indices),
            texture: None,
        });
    }

    fn finish(mut self) -> Vec<Mesh> {
        if !self.indices.is_empty() {
            self.flush();
        }
        self.meshes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::world_gen::chunk::Chunk;
    use std::collections::HashMap;

    const LAST: usize = N - 1;
    const KEY: (i32, i32, i32) = (1, -2, 3);
    const NO_NEIGHBOURS: Neighbours<'static> = [None; 6];

    /// (voxel position relative to the chunk, index into FACES)
    type Cell = ([i32; 3], usize);

    fn meshes(c: &Chunk) -> Vec<Mesh> {
        c.build_meshes((0, 0, 0), NO_NEIGHBOURS)
    }

    fn quads(m: &[Mesh]) -> usize {
        m.iter().map(|m| m.indices.len() / 6).sum()
    }

    fn vertex_color(color: Color) -> [u8; 4] {
        Vertex::new(0.0, 0.0, 0.0, 0.0, 0.0, color).color.into()
    }

    fn full_chunk() -> Chunk {
        let mut c = Chunk::new();
        for y in 0..N {
            for z in 0..N {
                for x in 0..N {
                    c.set_voxel(x, y, z, Voxel::DIRT);
                }
            }
        }
        c
    }

    /// Every visible unit face, found the slow obvious way.
    fn reference(c: &Chunk, neighbours: &Neighbours) -> HashMap<Cell, [u8; 4]> {
        let mut faces = HashMap::new();
        for y in 0..N {
            for z in 0..N {
                for x in 0..N {
                    let voxel = c.get_voxel(x, y, z);
                    if !voxel.is_solid() {
                        continue;
                    }
                    for (f, (n, _, shade)) in FACES.iter().enumerate() {
                        let p = [x as i32 + n[0], y as i32 + n[1], z as i32 + n[2]];
                        let inside = p.iter().all(|i| (0..W).contains(i));
                        let wrapped = p.map(|i| i.rem_euclid(W) as usize);
                        let index = (0..3).map(|i| wrapped[i] * STRIDE[i]).sum::<usize>();
                        let hidden = if inside {
                            c.voxels()[index].is_solid()
                        } else {
                            neighbours[f].is_some_and(|g| g[index].is_solid())
                        };
                        if !hidden {
                            let color = vertex_color(shaded(voxel.color(), *shade));
                            faces.insert(([x as i32, y as i32, z as i32], f), color);
                        }
                    }
                }
            }
        }
        faces
    }

    /// Splits every quad back into unit faces, panicking if two quads overlap.
    fn unit_faces(m: &[Mesh]) -> HashMap<Cell, [u8; 4]> {
        let origin = [KEY.0 * W, KEY.1 * W, KEY.2 * W];
        let mut faces = HashMap::new();
        for mesh in m {
            for q in mesh.indices.chunks(6) {
                let base = q[0] as usize;
                let v = &mesh.vertices[base..base + 4];
                let normal = (v[1].position - v[0].position).cross(v[2].position - v[0].position).normalize();
                let n = [normal.x.round() as i32, normal.y.round() as i32, normal.z.round() as i32];
                let f = FACES.iter().position(|face| face.0 == n).expect("quad normal is axis-aligned");
                let d = n.iter().position(|&c| c != 0).unwrap();
                let (u, w) = TANGENTS[d];

                let pos = |i: usize, axis: usize| v[i].position[axis].round() as i32 - origin[axis];
                let min = |axis: usize| (0..4).map(|i| pos(i, axis)).min().unwrap();
                let max = |axis: usize| (0..4).map(|i| pos(i, axis)).max().unwrap();
                let plane = if n[d] > 0 { min(d) - 1 } else { min(d) };

                for a in min(u)..max(u) {
                    for b in min(w)..max(w) {
                        let mut cell = [0; 3];
                        cell[d] = plane;
                        cell[u] = a;
                        cell[w] = b;
                        let color: [u8; 4] = v[0].color.into();
                        assert!(faces.insert((cell, f), color).is_none(), "overlapping quads at {cell:?}");
                    }
                }
            }
        }
        faces
    }

    /// Voxel type chosen per `cell`-sized cube, so larger cells give larger mergeable regions.
    fn random_chunk(seed: u64, cell: usize, solid_percent: u64) -> Chunk {
        let hash = |x: usize, y: usize, z: usize| {
            let mut h = seed ^ (x as u64) << 40 ^ (y as u64) << 20 ^ z as u64;
            h = h.wrapping_mul(0x9E37_79B9_7F4A_7C15);
            h ^ (h >> 29)
        };
        let mut c = Chunk::new();
        for y in 0..N {
            for z in 0..N {
                for x in 0..N {
                    let h = hash(x / cell, y / cell, z / cell);
                    if h % 100 < solid_percent {
                        let voxel = [Voxel::DIRT, Voxel::GRASS, Voxel::STONE][(h >> 8) as usize % 3];
                        c.set_voxel(x, y, z, voxel);
                    }
                }
            }
        }
        c
    }

    #[test]
    fn single_voxel_is_six_quads() {
        let mut c = Chunk::new();
        c.set_voxel(5, 5, 5, Voxel::DIRT);
        let m = meshes(&c);
        assert_eq!((m[0].vertices.len(), m[0].indices.len()), (24, 36));
    }

    #[test]
    fn empty_chunk_has_no_meshes() {
        assert!(meshes(&Chunk::new()).is_empty());
    }

    #[test]
    fn full_chunk_is_one_quad_per_side() {
        let m = meshes(&full_chunk());
        assert_eq!((m.len(), quads(&m)), (1, 6));
    }

    #[test]
    fn adjacent_same_type_voxels_merge() {
        let mut c = Chunk::new();
        c.set_voxel(5, 5, 5, Voxel::DIRT);
        c.set_voxel(6, 5, 5, Voxel::DIRT);
        assert_eq!(quads(&meshes(&c)), 6); // 4 stretched sides + 2 end caps
    }

    #[test]
    fn different_types_do_not_merge() {
        let mut c = Chunk::new();
        c.set_voxel(5, 5, 5, Voxel::DIRT);
        c.set_voxel(6, 5, 5, Voxel::GRASS);
        assert_eq!(quads(&meshes(&c)), 10);
    }

    #[test]
    fn solid_neighbour_chunk_hides_border_faces() {
        let wall = full_chunk();
        let mut c = Chunk::new();
        c.set_voxel(LAST, 0, 0, Voxel::DIRT);
        let mut neighbours = NO_NEIGHBOURS;
        assert_eq!(quads(&c.build_meshes((0, 0, 0), neighbours)), 6);
        neighbours[0] = Some(wall.voxels()); // +X
        assert_eq!(quads(&c.build_meshes((0, 0, 0), neighbours)), 5);
    }

    #[test]
    fn unmergeable_voxels_split_into_meshes_of_at_most_max_faces() {
        let mut c = Chunk::new();
        for y in 0..N {
            for z in 0..N {
                for x in 0..N {
                    if (x + y + z) % 2 == 0 {
                        c.set_voxel(x, y, z, Voxel::DIRT); // 3D checkerboard: nothing touches
                    }
                }
            }
        }
        let m = meshes(&c);
        assert_eq!(m.len(), (N * N * N / 2 * 6).div_ceil(MAX_FACES_PER_MESH));
        assert!(m.iter().all(|m| m.indices.len() / 6 <= MAX_FACES_PER_MESH));
    }

    #[test]
    fn quads_are_placed_at_chunk_origin() {
        let mut c = Chunk::new();
        c.set_voxel(0, 0, 0, Voxel::DIRT);
        let m = c.build_meshes(KEY, NO_NEIGHBOURS);
        let min = |f: fn(&Vertex) -> f32| m[0].vertices.iter().map(f).fold(f32::MAX, f32::min);
        let w = W as f32;
        let expected = (KEY.0 as f32 * w, KEY.1 as f32 * w, KEY.2 as f32 * w);
        assert_eq!((min(|v| v.position.x), min(|v| v.position.y), min(|v| v.position.z)), expected);
    }

    #[test]
    fn covers_exactly_the_visible_faces_with_correct_winding_and_colour() {
        let around: Vec<Chunk> = (0..6).map(|i| random_chunk(100 + i, 4, 70)).collect();
        // one neighbour is missing (+Y) to cover the "not loaded" case
        let neighbours: Neighbours = std::array::from_fn(|i| (i != 2).then(|| around[i].voxels()));

        for cell in [1, 2, 4, 8, 32] {
            for solid in [20, 60, 95, 100] {
                let c = random_chunk(cell as u64 * 31 + solid, cell, solid);
                let actual = unit_faces(&c.build_meshes(KEY, neighbours));
                let expected = reference(&c, &neighbours);
                assert!(actual == expected, "cell={cell} solid={solid}%: greedy mesh differs from reference");
            }
        }
    }

    #[test]
    fn merging_beats_per_voxel_faces_on_blocky_terrain() {
        let c = random_chunk(7, 8, 60);
        let naive = reference(&c, &NO_NEIGHBOURS).len();
        assert!(quads(&meshes(&c)) * 4 < naive);
    }
}