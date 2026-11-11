//! The game's own entity models (its client code: `BoatModel`, `MinecartModel`,
//! `ArmorStandModel`, read in the decompiled 26.3 client) with their renderers' transforms
//! (`AbstractBoatRenderer`, `AbstractMinecartRenderer`, `ArmorStandRenderer`,
//! `ItemEntityRenderer`, `FallingBlockRenderer`), drawn with the game's entity and item textures
//! (the vendored atlases, packed below the block atlas in the game texture) and the game's
//! entity lighting (two directional lights, `light.glsl`).

use crate::atlas::GameAssets;
use crate::mesh::Vertex;

/// A box of a model part (`CubeListBuilder.texOffs(u, v).addBox(x, y, z, w, h, d[, grow])`): a
/// `CubeDeformation` grows the box on every side, never its texture.
struct Cube {
    tex: [f32; 2],
    from: [f32; 3],
    size: [f32; 3],
    mirror: bool,
    grow: f32,
}

/// A model part: its boxes, its pose (offset in pixels, rotations in radians, scale: `PartPose`)
/// and its children (`PartDefinition.addOrReplaceChild`), posed relative to it.
struct EPart {
    cubes: Vec<Cube>,
    offset: [f32; 3],
    rot: [f32; 3],
    scale: f32,
    children: Vec<EPart>,
}

fn cube(u: f32, v: f32, from: [f32; 3], size: [f32; 3]) -> Cube {
    Cube { tex: [u, v], from, size, mirror: false, grow: 0.0 }
}

/// A box with a `CubeDeformation` of `grow` on every side.
fn cube_grown(u: f32, v: f32, from: [f32; 3], size: [f32; 3], grow: f32) -> Cube {
    Cube { grow, ..cube(u, v, from, size) }
}

/// A mirrored box (`.mirror()`).
fn cube_mirrored(u: f32, v: f32, from: [f32; 3], size: [f32; 3]) -> Cube {
    Cube { mirror: true, ..cube(u, v, from, size) }
}

fn part(cubes: Vec<Cube>, offset: [f32; 3], rot: [f32; 3]) -> EPart {
    EPart { cubes, offset, rot, scale: 1.0, children: Vec::new() }
}

/// A part with its children.
fn part_with(cubes: Vec<Cube>, offset: [f32; 3], rot: [f32; 3], children: Vec<EPart>) -> EPart {
    EPart { children, ..part(cubes, offset, rot) }
}

const PI: f32 = std::f32::consts::PI;

/// `BoatModel.addCommonParts` (+ the chest boat's chest), textures 128 x 64 (128 x 128 chest),
/// the paddles where `AbstractBoatModel.animatePaddle` holds them at rowing time 0 (an idle boat).
fn boat(chest: bool) -> Vec<EPart> {
    let lerp = |f: f32, a: f32, b: f32| a + f.clamp(0.0, 1.0) * (b - a);
    let paddle_x = lerp(((-0.0f32).sin() + 1.0) / 2.0, -PI / 3.0, -PI / 12.0);
    let paddle_y = lerp(((1.0f32).sin() + 1.0) / 2.0, -PI / 4.0, PI / 4.0);
    let mut v = vec![
        part(vec![cube(0.0, 0.0, [-14.0, -9.0, -3.0], [28.0, 16.0, 3.0])], [0.0, 3.0, 1.0], [PI / 2.0, 0.0, 0.0]),
        part(vec![cube(0.0, 19.0, [-13.0, -7.0, -1.0], [18.0, 6.0, 2.0])], [-15.0, 4.0, 4.0], [0.0, PI * 3.0 / 2.0, 0.0]),
        part(vec![cube(0.0, 27.0, [-8.0, -7.0, -1.0], [16.0, 6.0, 2.0])], [15.0, 4.0, 0.0], [0.0, PI / 2.0, 0.0]),
        part(vec![cube(0.0, 35.0, [-14.0, -7.0, -1.0], [28.0, 6.0, 2.0])], [0.0, 4.0, -9.0], [0.0, PI, 0.0]),
        part(vec![cube(0.0, 43.0, [-14.0, -7.0, -1.0], [28.0, 6.0, 2.0])], [0.0, 4.0, 9.0], [0.0, 0.0, 0.0]),
        part(vec![cube(62.0, 0.0, [-1.0, 0.0, -5.0], [2.0, 2.0, 18.0]), cube(62.0, 0.0, [-1.001, -3.0, 8.0], [1.0, 6.0, 7.0])], [3.0, -5.0, 9.0], [paddle_x, paddle_y, PI / 16.0]),
        part(vec![cube(62.0, 20.0, [-1.0, 0.0, -5.0], [2.0, 2.0, 18.0]), cube(62.0, 20.0, [0.001, -3.0, 8.0], [1.0, 6.0, 7.0])], [3.0, -5.0, -9.0], [paddle_x, PI - paddle_y, PI / 16.0]),
    ];
    if chest {
        v.push(part(vec![cube(0.0, 76.0, [0.0, 0.0, 0.0], [12.0, 8.0, 12.0])], [-2.0, -5.0, -6.0], [0.0, -PI / 2.0, 0.0]));
        v.push(part(vec![cube(0.0, 59.0, [0.0, 0.0, 0.0], [12.0, 4.0, 12.0])], [-2.0, -9.0, -6.0], [0.0, -PI / 2.0, 0.0]));
        v.push(part(vec![cube(0.0, 59.0, [0.0, 0.0, 0.0], [2.0, 4.0, 1.0])], [-1.0, -6.0, -1.0], [0.0, -PI / 2.0, 0.0]));
    }
    v
}

/// `MinecartModel.createBodyLayer`, texture 64 x 32.
fn minecart() -> Vec<EPart> {
    let side = |offset: [f32; 3], yrot: f32| part(vec![cube(0.0, 0.0, [-8.0, -9.0, -1.0], [16.0, 8.0, 2.0])], offset, [0.0, yrot, 0.0]);
    vec![
        part(vec![cube(0.0, 10.0, [-10.0, -8.0, -1.0], [20.0, 16.0, 2.0])], [0.0, 4.0, 0.0], [PI / 2.0, 0.0, 0.0]),
        side([-9.0, 4.0, 0.0], PI * 3.0 / 2.0),
        side([9.0, 4.0, 0.0], PI / 2.0),
        side([0.0, 4.0, -7.0], PI),
        side([0.0, 4.0, 7.0], 0.0),
    ]
}

/// `ArmorStandModel.createBodyLayer` in its default pose (`ArmorStand.DEFAULT_*_POSE`; arms hidden,
/// base plate shown and kept square to the world), texture 64 x 64.
fn armor_stand(yaw_deg: f32) -> Vec<EPart> {
    let d = PI / 180.0;
    let legs = |u: f32, v: f32, x: f32, pose: [f32; 3], mirror: bool| {
        let mut c = cube(u, v, [-1.0, 0.0, -1.0], [2.0, 11.0, 2.0]);
        c.mirror = mirror;
        part(vec![c], [x, 12.0, 0.0], [pose[0] * d, pose[1] * d, pose[2] * d])
    };
    vec![
        part(vec![cube(0.0, 0.0, [-1.0, -7.0, -1.0], [2.0, 7.0, 2.0])], [0.0, 1.0, 0.0], [0.0; 3]),
        part(vec![cube(0.0, 26.0, [-6.0, 0.0, -1.5], [12.0, 3.0, 3.0])], [0.0; 3], [0.0; 3]),
        legs(8.0, 0.0, -1.9, [1.0, 0.0, 1.0], false),
        legs(40.0, 16.0, 1.9, [-1.0, 0.0, -1.0], true),
        part(vec![cube(16.0, 0.0, [-3.0, 3.0, -1.0], [2.0, 7.0, 2.0])], [0.0; 3], [0.0; 3]),
        part(vec![cube(48.0, 16.0, [1.0, 3.0, -1.0], [2.0, 7.0, 2.0])], [0.0; 3], [0.0; 3]),
        part(vec![cube(0.0, 48.0, [-4.0, 10.0, -1.0], [8.0, 2.0, 2.0])], [0.0; 3], [0.0; 3]),
        part(vec![cube(0.0, 32.0, [-6.0, 11.0, -6.0], [12.0, 1.0, 12.0])], [0.0, 12.0, 0.0], [0.0, -yaw_deg * d, 0.0]),
    ]
}

/// A 4x4 affine transform, column vectors (`m * [x, y, z, 1]`).
#[derive(Clone, Copy)]
pub struct M([[f32; 4]; 4]);

impl M {
    pub fn id() -> M {
        M([[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]])
    }
    pub fn mul(&self, o: &M) -> M {
        let mut r = [[0.0f32; 4]; 4];
        for (i, row) in r.iter_mut().enumerate() {
            for (j, x) in row.iter_mut().enumerate() {
                *x = (0..4).map(|k| self.0[i][k] * o.0[k][j]).sum();
            }
        }
        M(r)
    }
    pub fn translate(&self, t: [f32; 3]) -> M {
        let mut m = M::id();
        m.0[0][3] = t[0];
        m.0[1][3] = t[1];
        m.0[2][3] = t[2];
        self.mul(&m)
    }
    pub fn scale(&self, s: [f32; 3]) -> M {
        let mut m = M::id();
        m.0[0][0] = s[0];
        m.0[1][1] = s[1];
        m.0[2][2] = s[2];
        self.mul(&m)
    }
    /// A right-handed rotation about axis 0 / 1 / 2 by `a` radians.
    pub fn rot(&self, axis: usize, a: f32) -> M {
        let (s, c) = a.sin_cos();
        let mut m = M::id();
        let (i, j) = match axis {
            0 => (1, 2),
            1 => (2, 0),
            _ => (0, 1),
        };
        m.0[i][i] = c;
        m.0[i][j] = -s;
        m.0[j][i] = s;
        m.0[j][j] = c;
        self.mul(&m)
    }
    pub fn point(&self, p: [f32; 3]) -> [f32; 3] {
        let m = &self.0;
        [0, 1, 2].map(|i| m[i][0] * p[0] + m[i][1] * p[1] + m[i][2] * p[2] + m[i][3])
    }
    pub fn dir(&self, p: [f32; 3]) -> [f32; 3] {
        let m = &self.0;
        [0, 1, 2].map(|i| m[i][0] * p[0] + m[i][1] * p[1] + m[i][2] * p[2])
    }
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-9);
    [v[0] / l, v[1] / l, v[2] / l]
}

/// The game's entity light for a face normal (`light.glsl`: two diffuse lights, 0.6 power, 0.4
/// ambient).
pub fn entity_light(n: [f32; 3]) -> f32 {
    let l0 = normalize([0.2, 1.0, -0.7]);
    let l1 = normalize([-0.2, 1.0, 0.7]);
    let d = |l: [f32; 3]| (l[0] * n[0] + l[1] * n[1] + l[2] * n[2]).max(0.0);
    ((d(l0) + d(l1)) * 0.6 + 0.4).min(1.0)
}

/// The triangles of a model's parts (`ModelPart.Cube`'s polygons and UVs, mirror and growth
/// included; children posed relative to their parent, `ModelPart.translateAndRotate`) under `m`,
/// textured from `rect` (x, y, w, h in the game texture) of a `tex_size` texture.
fn emit(out: &mut Vec<Vertex>, parts: &[EPart], m: &M, tex_size: [f32; 2], rect: [u32; 4], tw: f32, th: f32) {
    for p in parts {
        let mut pm = m.translate([p.offset[0] / 16.0, p.offset[1] / 16.0, p.offset[2] / 16.0]).rot(2, p.rot[2]).rot(1, p.rot[1]).rot(0, p.rot[0]);
        if p.scale != 1.0 {
            pm = pm.scale([p.scale, p.scale, p.scale]);
        }
        for c in &p.cubes {
            let g = c.grow;
            let (mut x0, y0, z0) = (c.from[0] - g, c.from[1] - g, c.from[2] - g);
            let (mut x1, y1, z1) = (c.from[0] + c.size[0] + g, c.from[1] + c.size[1] + g, c.from[2] + c.size[2] + g);
            if c.mirror {
                std::mem::swap(&mut x0, &mut x1);
            }
            let (w, h, d) = (c.size[0], c.size[1], c.size[2]);
            let t0 = [x0, y0, z0];
            let t1 = [x1, y0, z0];
            let t2 = [x1, y1, z0];
            let t3 = [x0, y1, z0];
            let l0 = [x0, y0, z1];
            let l1 = [x1, y0, z1];
            let l2 = [x1, y1, z1];
            let l3 = [x0, y1, z1];
            let (u, v) = (c.tex[0], c.tex[1]);
            let (u0, u1, u2, u22, u3, u4) = (u, u + d, u + d + w, u + d + w + w, u + d + w + d, u + d + w + d + w);
            let (v0, v1, v2) = (v, v + d, v + d + h);
            let polys: [([[f32; 3]; 4], [f32; 4]); 6] = [
                ([l1, l0, t0, t1], [u1, v0, u2, v1]),
                ([t2, t3, l3, l2], [u2, v1, u22, v0]),
                ([t0, l0, l3, t3], [u0, v1, u1, v2]),
                ([t1, t0, t3, t2], [u1, v1, u2, v2]),
                ([l1, t1, t2, l2], [u2, v1, u3, v2]),
                ([l0, l1, l2, l3], [u3, v1, u4, v2]),
            ];
            for (verts, [pu0, pv0, pu1, pv1]) in polys {
                let mut uv = [[pu1, pv0], [pu0, pv0], [pu0, pv1], [pu1, pv1]];
                let mut vs = verts;
                if c.mirror {
                    vs.reverse();
                    uv.reverse();
                }
                let world: [[f32; 3]; 4] = vs.map(|q| pm.point([q[0] / 16.0, q[1] / 16.0, q[2] / 16.0]));
                let e1 = [world[1][0] - world[0][0], world[1][1] - world[0][1], world[1][2] - world[0][2]];
                let e2 = [world[2][0] - world[0][0], world[2][1] - world[0][1], world[2][2] - world[0][2]];
                let n = normalize([e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]]);
                if n.iter().all(|x| x.is_nan()) {
                    continue;
                }
                let l = (entity_light(n) * 255.0).round() as u8;
                let tex = |t: [f32; 2]| -> [f32; 2] {
                    let x = rect[0] as f32 + (t[0] / tex_size[0]).clamp(0.0, 1.0) * rect[2] as f32;
                    let y = rect[1] as f32 + (t[1] / tex_size[1]).clamp(0.0, 1.0) * rect[3] as f32;
                    [(x.clamp(rect[0] as f32 + 0.02, (rect[0] + rect[2]) as f32 - 0.02)) / tw, (y.clamp(rect[1] as f32 + 0.02, (rect[1] + rect[3]) as f32 - 0.02)) / th]
                };
                let vx: [Vertex; 4] = std::array::from_fn(|k| Vertex { pos: world[k], uv: tex(uv[k]), layer: crate::atlas::GAME_LAYER, color: [l, l, l, 255], light: crate::mesh::UNLIT });
                out.extend_from_slice(&[vx[0], vx[1], vx[2], vx[0], vx[2], vx[3]]);
            }
        }
        emit(out, &p.children, &pm, tex_size, rect, tw, th);
    }
}

/// A minecart's place and rotation like `AbstractMinecartRenderer.oldRender`: on a rail, the
/// rail's line under it and the direction between the points 0.3 ahead and behind (pitch
/// `atan(dy) * 73`); off rails, its own yaw and pitch.
pub fn minecart_pose(w: &redsim::World, pos: [f64; 3], yaw: f32, pitch: f32) -> ([f64; 3], f32, f32) {
    use redsim::BlockPos;
    use redsim::rails;
    let floor = |v: f64| v.floor() as i32;
    let rail_at = |x: f64, y: f64, z: f64| -> Option<(i32, i32, i32, ([i32; 3], [i32; 3]))> {
        let (xt, mut yt, zt) = (floor(x), floor(y), floor(z));
        if rails::is_rail(&w.get(BlockPos::new(xt, yt - 1, zt))) {
            yt -= 1;
        }
        let s = w.get(BlockPos::new(xt, yt, zt));
        rails::is_rail(&s).then(|| (xt, yt, zt, rails::exits(rails::shape_of(&s))))
    };
    // OldMinecartBehavior.getPos
    let get_pos = |x: f64, y: f64, z: f64| -> Option<[f64; 3]> {
        let (xt, yt, zt, (e0, e1)) = rail_at(x, y, z)?;
        let p0 = [xt as f64 + 0.5 + e0[0] as f64 * 0.5, yt as f64 + 0.0625 + e0[1] as f64 * 0.5, zt as f64 + 0.5 + e0[2] as f64 * 0.5];
        let p1 = [xt as f64 + 0.5 + e1[0] as f64 * 0.5, yt as f64 + 0.0625 + e1[1] as f64 * 0.5, zt as f64 + 0.5 + e1[2] as f64 * 0.5];
        let (xd, yd, zd) = (p1[0] - p0[0], (p1[1] - p0[1]) * 2.0, p1[2] - p0[2]);
        let progress = if xd == 0.0 {
            z - zt as f64
        } else if zd == 0.0 {
            x - xt as f64
        } else {
            ((x - p0[0]) * xd + (z - p0[2]) * zd) * 2.0
        };
        let mut y = p0[1] + yd * progress;
        if yd < 0.0 {
            y += 1.0;
        } else if yd > 0.0 {
            y += 0.5;
        }
        Some([p0[0] + xd * progress, y, p0[2] + zd * progress])
    };
    // OldMinecartBehavior.getPosOffs
    let get_pos_offs = |x: f64, y: f64, z: f64, offs: f64| -> Option<[f64; 3]> {
        let (xt, yt, zt, (e0, e1)) = rail_at(x, y, z)?;
        let slope = e0[1] != 0 || e1[1] != 0;
        let mut yy = if slope { yt as f64 + 1.0 } else { yt as f64 };
        let (mut xd, mut zd) = ((e1[0] - e0[0]) as f64, (e1[2] - e0[2]) as f64);
        let dd = (xd * xd + zd * zd).sqrt();
        xd /= dd;
        zd /= dd;
        let (x, z) = (x + xd * offs, z + zd * offs);
        if e0[1] != 0 && floor(x) - xt == e0[0] && floor(z) - zt == e0[2] {
            yy += e0[1] as f64;
        } else if e1[1] != 0 && floor(x) - xt == e1[0] && floor(z) - zt == e1[2] {
            yy += e1[1] as f64;
        }
        get_pos(x, yy, z)
    };
    let (x, y, z) = (pos[0], pos[1], pos[2]);
    if let (Some(on), Some(front), Some(back)) = (get_pos(x, y, z), get_pos_offs(x, y, z, 0.3), get_pos_offs(x, y, z, -0.3)) {
        let d = [back[0] - front[0], back[1] - front[1], back[2] - front[2]];
        let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        let p = [on[0], (front[1] + back[1]) / 2.0, on[2]];
        if l != 0.0 {
            let d = [d[0] / l, d[1] / l, d[2] / l];
            return (p, (d[2].atan2(d[0]) * 180.0 / std::f64::consts::PI) as f32, (d[1].atan() * 73.0) as f32);
        }
        return (p, yaw, pitch);
    }
    (pos, yaw, pitch)
}

/// The game texture rect of a texture, if packed.
fn rect(g: &GameAssets, name: &str) -> Option<[u32; 4]> {
    g.tex.get(name).map(|t| t.rect)
}

/// `ChestModel.createSingleBodyLayer` (closed), texture 64 x 64, in block pixels (y up).
fn chest() -> Vec<EPart> {
    vec![
        part(vec![cube(0.0, 19.0, [1.0, 0.0, 1.0], [14.0, 10.0, 14.0])], [0.0; 3], [0.0; 3]),
        part(vec![cube(0.0, 0.0, [1.0, 0.0, 0.0], [14.0, 5.0, 14.0])], [0.0, 9.0, 1.0], [0.0; 3]),
        part(vec![cube(0.0, 0.0, [7.0, -2.0, 14.0], [2.0, 4.0, 1.0])], [0.0, 9.0, 1.0], [0.0; 3]),
    ]
}

/// `ShulkerModel.createBoxLayer` (closed: the lid at 24, not turned), texture 64 x 64.
fn shulker() -> Vec<EPart> {
    vec![
        part(vec![cube(0.0, 0.0, [-8.0, -16.0, -8.0], [16.0, 12.0, 16.0])], [0.0, 24.0, 0.0], [0.0; 3]),
        part(vec![cube(0.0, 28.0, [-8.0, -8.0, -8.0], [16.0, 8.0, 16.0])], [0.0, 24.0, 0.0], [0.0; 3]),
    ]
}

/// A chest or shulker box at `cell` drawn like the game's block-entity renderers
/// (`ChestRenderer`: turned about the cell's centre by its facing; `ShulkerBoxRenderer`: turned
/// by `Direction.getRotation`, flipped, 0.9995 scale), with the game's entity textures. None:
/// not one of them, or its texture is not packed.
pub fn block_entity_triangles(g: &GameAssets, state: &str, cell: [i32; 3]) -> Option<Vec<Vertex>> {
    let name = state.split('[').next()?.trim().trim_start_matches("minecraft:");
    let facing = state.split("facing=").nth(1).and_then(|r| r.split([',', ']']).next()).unwrap_or("");
    let (tw, th) = (g.width as f32, g.height as f32);
    let base = M::id().translate([cell[0] as f32, cell[1] as f32, cell[2] as f32]);
    let mut out = Vec::new();
    if name == "chest" {
        let r = rect(g, "entity/chest/normal")?;
        // Direction.toYRot
        let yrot: f32 = match facing {
            "south" => 0.0,
            "west" => 90.0,
            "east" => 270.0,
            _ => 180.0,
        };
        let m = base.translate([0.5, 0.0, 0.5]).rot(1, (-yrot).to_radians()).translate([-0.5, 0.0, -0.5]);
        emit(&mut out, &chest(), &m, [64.0, 64.0], r, tw, th);
        return Some(out);
    }
    if name.ends_with("shulker_box") {
        let colour = name.trim_end_matches("shulker_box").trim_end_matches('_');
        let r = if colour.is_empty() { rect(g, "entity/shulker/shulker") } else { rect(g, &format!("entity/shulker/shulker_{colour}")).or_else(|| rect(g, "entity/shulker/shulker")) }?;
        // Direction.getRotation
        let turn = match facing {
            "down" => M::id().rot(0, PI),
            "north" => M::id().rot(0, PI / 2.0).rot(2, PI),
            "south" => M::id().rot(0, PI / 2.0),
            "west" => M::id().rot(0, PI / 2.0).rot(2, PI / 2.0),
            "east" => M::id().rot(0, PI / 2.0).rot(2, -PI / 2.0),
            _ => M::id(),
        };
        let m = base.translate([0.5, 0.5, 0.5]).scale([0.9995, 0.9995, 0.9995]).mul(&turn).scale([1.0, -1.0, -1.0]).translate([0.0, -1.0, 0.0]);
        emit(&mut out, &shulker(), &m, [64.0, 64.0], r, tw, th);
        return Some(out);
    }
    None
}

/// The triangles of an entity drawn like the game (None: not one of the game's models the viewer
/// has, or its texture is not loaded: the viewer's own stand-in is drawn).
pub fn entity_triangles(g: &GameAssets, kind: &str, pos: [f32; 3], yaw: f32, pitch: f32) -> Option<Vec<Vertex>> {
    let k = kind.strip_prefix("minecraft:").unwrap_or(kind);
    let (tw, th) = (g.width as f32, g.height as f32);
    let mut out = Vec::new();
    let at = M::id().translate(pos);
    if k.ends_with("_boat") || k.ends_with("_raft") {
        let chest = k.contains("chest");
        let wood = k.trim_end_matches("_chest_boat").trim_end_matches("_boat").trim_end_matches("_chest_raft").trim_end_matches("_raft");
        let name = if chest { format!("entity/chest_boat/{wood}") } else { format!("entity/boat/{wood}") };
        let r = rect(g, &name).or_else(|| rect(g, if chest { "entity/chest_boat/oak" } else { "entity/boat/oak" }))?;
        // AbstractBoatRenderer.submit
        let m = at.translate([0.0, 0.375, 0.0]).rot(1, (180.0 - yaw).to_radians()).scale([-1.0, -1.0, 1.0]).rot(1, PI / 2.0);
        emit(&mut out, &boat(chest), &m, if chest { [128.0, 128.0] } else { [128.0, 64.0] }, r, tw, th);
        return Some(out);
    }
    if k.contains("minecart") {
        let r = rect(g, "entity/minecart/minecart")?;
        // AbstractMinecartRenderer.oldRender (the caller passes the rail-aligned pose)
        let m = at.translate([0.0, 0.375, 0.0]).rot(1, (180.0 - yaw).to_radians()).rot(2, (-pitch).to_radians());
        emit(&mut out, &minecart(), &m.scale([-1.0, -1.0, 1.0]), [64.0, 32.0], r, tw, th);
        return Some(out);
    }
    if k == "armor_stand" {
        let r = rect(g, "entity/armorstand/armorstand")?;
        // LivingEntityRenderer.submit + ArmorStandRenderer.setupRotations
        let m = at.rot(1, (180.0 - yaw).to_radians()).scale([-1.0, -1.0, 1.0]).translate([0.0, -1.501, 0.0]);
        emit(&mut out, &armor_stand(yaw), &m, [64.0, 64.0], r, tw, th);
        return Some(out);
    }
    None
}

/// `Mth.triangleWave`.
fn triangle_wave(index: f32, period: f32) -> f32 {
    ((index % period - period * 0.5).abs() - period * 0.25) / (period * 0.25)
}

/// `IronGolemModel.createBodyLayer` (texture 128 x 128) posed by `setupAnim`: the head's yaw and
/// pitch, the arms and legs swinging with the walk (no attack, no flower offered: the farms' golems
/// never fight).
fn iron_golem(d: &crate::mesh::MobDraw) -> Vec<EPart> {
    let (pos, speed) = (d.walk_pos, d.walk_speed);
    let tri = triangle_wave(pos, 13.0);
    let (hx, hy) = (d.head_pitch.to_radians(), d.head_yaw.to_radians());
    vec![
        part(vec![cube(0.0, 0.0, [-4.0, -12.0, -5.5], [8.0, 10.0, 8.0]), cube(24.0, 0.0, [-1.0, -5.0, -7.5], [2.0, 4.0, 2.0])], [0.0, -7.0, -2.0], [hx, hy, 0.0]),
        part(vec![cube(0.0, 40.0, [-9.0, -2.0, -6.0], [18.0, 12.0, 11.0]), cube_grown(0.0, 70.0, [-4.5, 10.0, -3.0], [9.0, 5.0, 6.0], 0.5)], [0.0, -7.0, 0.0], [0.0; 3]),
        part(vec![cube(60.0, 21.0, [-13.0, -2.5, -3.0], [4.0, 30.0, 6.0])], [0.0, -7.0, 0.0], [(-0.2 + 1.5 * tri) * speed, 0.0, 0.0]),
        part(vec![cube(60.0, 58.0, [9.0, -2.5, -3.0], [4.0, 30.0, 6.0])], [0.0, -7.0, 0.0], [(-0.2 - 1.5 * tri) * speed, 0.0, 0.0]),
        part(vec![cube(37.0, 0.0, [-3.5, -3.0, -3.0], [6.0, 16.0, 5.0])], [-4.0, 11.0, 0.0], [-1.5 * tri * speed, 0.0, 0.0]),
        part(vec![cube_mirrored(60.0, 0.0, [-3.5, -3.0, -3.0], [6.0, 16.0, 5.0])], [5.0, 11.0, 0.0], [1.5 * tri * speed, 0.0, 0.0]),
    ]
}

/// `VillagerModel.createBodyModel` (texture 64 x 64) with the villagers' `MeshTransformer.scaling
/// (0.9375)` (`LayerDefinitions`: the root part scaled, 1.501 px down), posed by `setupAnim`: the
/// head's yaw and pitch, the legs swinging with the walk (never unhappy: no trades).
fn villager(d: &crate::mesh::MobDraw) -> Vec<EPart> {
    let (pos, speed) = (d.walk_pos, d.walk_speed);
    let (hx, hy) = (d.head_pitch.to_radians(), d.head_yaw.to_radians());
    let hat = part_with(vec![cube_grown(32.0, 0.0, [-4.0, -10.0, -4.0], [8.0, 10.0, 8.0], 0.51)], [0.0; 3], [0.0; 3], vec![part(vec![cube(30.0, 47.0, [-8.0, -8.0, -6.0], [16.0, 16.0, 1.0])], [0.0; 3], [-PI / 2.0, 0.0, 0.0])]);
    let nose = part(vec![cube(24.0, 0.0, [-1.0, -1.0, -6.0], [2.0, 4.0, 2.0])], [0.0, -2.0, 0.0], [0.0; 3]);
    let head = part_with(vec![cube(0.0, 0.0, [-4.0, -10.0, -4.0], [8.0, 10.0, 8.0])], [0.0; 3], [hx, hy, 0.0], vec![hat, nose]);
    let body = part_with(vec![cube(16.0, 20.0, [-4.0, 0.0, -3.0], [8.0, 12.0, 6.0])], [0.0; 3], [0.0; 3], vec![part(vec![cube_grown(0.0, 38.0, [-4.0, 0.0, -3.0], [8.0, 20.0, 6.0], 0.5)], [0.0; 3], [0.0; 3])]);
    let arms = part(vec![cube(44.0, 22.0, [-8.0, -2.0, -2.0], [4.0, 8.0, 4.0]), cube_mirrored(44.0, 22.0, [4.0, -2.0, -2.0], [4.0, 8.0, 4.0]), cube(40.0, 38.0, [-4.0, 2.0, -2.0], [8.0, 4.0, 4.0])], [0.0, 3.0, -1.0], [-0.75, 0.0, 0.0]);
    let swing = |phase: f32| (pos * 0.6662 + phase).cos() * 1.4 * speed * 0.5;
    let right_leg = part(vec![cube(0.0, 22.0, [-2.0, 0.0, -2.0], [4.0, 12.0, 4.0])], [-2.0, 12.0, 0.0], [swing(0.0), 0.0, 0.0]);
    let left_leg = part(vec![cube_mirrored(0.0, 22.0, [-2.0, 0.0, -2.0], [4.0, 12.0, 4.0])], [2.0, 12.0, 0.0], [swing(PI), 0.0, 0.0]);
    let scale = 0.9375f32;
    vec![EPart { scale, ..part_with(Vec::new(), [0.0, 24.016 * (1.0 - scale), 0.0], [0.0; 3], vec![head, body, arms, right_leg, left_leg]) }]
}

/// `HumanoidModel.createMesh(NONE, 0)` (the zombie's layer, texture 64 x 64), or a baby's
/// `BabyZombieModel` (its own texture, `zombie_baby`: 64 x 64), posed by `HumanoidModel.setupAnim`
/// and `ZombieModel.setupAttackAnimation`: the head's yaw and pitch, the walk, a passenger's legs
/// forward (`isPassenger`), the arms held out (`AnimationUtils.animateZombieArms`: raised to
/// -pi / 2.25, or -pi / 1.5 aggressive; a baby holding an item keeps them down; no swing) and
/// bobbing with its age (`bobModelPart`).
fn zombie(d: &crate::mesh::MobDraw, baby: bool) -> Vec<EPart> {
    let (pos, speed) = (d.walk_pos, d.walk_speed);
    let (hx, hy) = (d.head_pitch.to_radians(), d.head_yaw.to_radians());
    let mut right_leg = [(pos * 0.6662).cos() * 1.4 * speed, 0.005, 0.005];
    let mut left_leg = [(pos * 0.6662 + PI).cos() * 1.4 * speed, -0.005, -0.005];
    if d.riding {
        right_leg = [-1.4137167, PI / 10.0, 0.07853982];
        left_leg = [-1.4137167, -PI / 10.0, -0.07853982];
    }
    let raise = !baby || d.held.is_none();
    let arm_x = if raise { -PI / if d.aggressive { 1.5 } else { 2.25 } } else { 0.0 };
    let arm_y = if raise { 0.1 } else { -0.1 };
    let bob = |sign: f32| [sign * ((d.age * 0.067).sin() * 0.05), 0.0, sign * ((d.age * 0.09).cos() * 0.05 + 0.05)];
    let (rb, lb) = (bob(1.0), bob(-1.0));
    let right_arm = [arm_x + rb[0], -arm_y, rb[2]];
    let left_arm = [arm_x + lb[0], arm_y, lb[2]];
    if baby {
        let head = part(vec![cube(3.0, 3.0, [-3.0, -6.25, -3.0], [6.0, 6.0, 6.0]), cube_grown(35.0, 3.0, [-3.0, -6.15, -3.0], [6.0, 6.0, 6.0], 0.25)], [0.0, 15.25, 0.0], [hx, hy, 0.0]);
        return vec![
            part(vec![cube(16.0, 16.0, [-2.0, -2.5, -1.0], [4.0, 5.0, 2.0])], [0.0, 17.5, 0.0], [0.0; 3]),
            head,
            part(vec![cube(36.0, 16.0, [-1.0, -0.5, -1.0], [2.0, 5.0, 2.0])], [-3.0, 15.5, 0.0], right_arm),
            part(vec![cube(28.0, 16.0, [-1.0, -0.5, -1.0], [2.0, 5.0, 2.0])], [3.0, 15.5, 0.0], left_arm),
            part(vec![cube(8.0, 16.0, [-1.0, 0.0, -1.0], [2.0, 4.0, 2.0])], [-1.0, 20.0, 0.0], right_leg),
            part(vec![cube(0.0, 16.0, [-1.0, 0.0, -1.0], [2.0, 4.0, 2.0])], [1.0, 20.0, 0.0], left_leg),
        ];
    }
    let head = part_with(vec![cube(0.0, 0.0, [-4.0, -8.0, -4.0], [8.0, 8.0, 8.0])], [0.0; 3], [hx, hy, 0.0], vec![part(vec![cube_grown(32.0, 0.0, [-4.0, -8.0, -4.0], [8.0, 8.0, 8.0], 0.5)], [0.0; 3], [0.0; 3])]);
    vec![
        head,
        part(vec![cube(16.0, 16.0, [-4.0, 0.0, -2.0], [8.0, 12.0, 4.0])], [0.0; 3], [0.0; 3]),
        part(vec![cube(40.0, 16.0, [-3.0, -2.0, -2.0], [4.0, 12.0, 4.0])], [-5.0, 2.0, 0.0], right_arm),
        part(vec![cube_mirrored(40.0, 16.0, [-1.0, -2.0, -2.0], [4.0, 12.0, 4.0])], [5.0, 2.0, 0.0], left_arm),
        part(vec![cube(0.0, 16.0, [-2.0, 0.0, -2.0], [4.0, 12.0, 4.0])], [-1.9, 12.0, 0.0], right_leg),
        part(vec![cube_mirrored(0.0, 16.0, [-2.0, 0.0, -2.0], [4.0, 12.0, 4.0])], [1.9, 12.0, 0.0], left_leg),
    ]
}

/// `CreeperModel.createBodyLayer(NONE)` (texture 64 x 32) posed by `setupAnim`: the head's yaw
/// and pitch, the legs walking (its fields cross the parts' names: the part `right_hind_leg`
/// swings as its `leftHindLeg`). Never swelling (`CreeperRenderer.scale`: a creeper swells at a
/// player within reach, and the views have none) nor charged.
fn creeper(d: &crate::mesh::MobDraw) -> Vec<EPart> {
    let (hx, hy) = (d.head_pitch.to_radians(), d.head_yaw.to_radians());
    let swing = |phase: f32| (d.walk_pos * 0.6662 + phase).cos() * 1.4 * d.walk_speed;
    let leg = || vec![cube(0.0, 16.0, [-2.0, 0.0, -2.0], [4.0, 6.0, 4.0])];
    vec![
        part(vec![cube(0.0, 0.0, [-4.0, -8.0, -4.0], [8.0, 8.0, 8.0])], [0.0, 6.0, 0.0], [hx, hy, 0.0]),
        part(vec![cube(16.0, 16.0, [-4.0, 0.0, -2.0], [8.0, 12.0, 4.0])], [0.0, 6.0, 0.0], [0.0; 3]),
        part(leg(), [-2.0, 18.0, 4.0], [swing(PI), 0.0, 0.0]),
        part(leg(), [2.0, 18.0, 4.0], [swing(0.0), 0.0, 0.0]),
        part(leg(), [-2.0, 18.0, -4.0], [swing(0.0), 0.0, 0.0]),
        part(leg(), [2.0, 18.0, -4.0], [swing(PI), 0.0, 0.0]),
    ]
}

/// `SkeletonModel.createBodyLayer` (texture 64 x 32: `HumanoidModel.createMesh` with its thin
/// arms and legs) posed by `HumanoidModel.setupAnim`: the head's yaw and pitch, the arms and legs
/// walking, a passenger's legs forward; the arms hanging (26.3's `HumanoidMobRenderer.getArmPose`:
/// EMPTY, a bow included) or, aggressive with its bow, both raised along its look
/// (`AbstractSkeletonRenderer.getArmPose`: BOW_AND_ARROW; right-handed: redsim draws
/// `setLeftHanded`'s roll but does not keep it), then bobbing with its age (`bobModelPart`).
fn skeleton(d: &crate::mesh::MobDraw) -> Vec<EPart> {
    let (hx, hy) = (d.head_pitch.to_radians(), d.head_yaw.to_radians());
    let (pos, speed) = (d.walk_pos * 0.6662, d.walk_speed);
    let mut ra = [(pos + PI).cos() * speed, 0.0, 0.0];
    let mut la = [pos.cos() * speed, 0.0, 0.0];
    let mut rl = [pos.cos() * 1.4 * speed, 0.005, 0.005];
    let mut ll = [(pos + PI).cos() * 1.4 * speed, -0.005, -0.005];
    if d.riding {
        ra[0] -= PI / 5.0;
        la[0] -= PI / 5.0;
        rl = [-1.4137167, PI / 10.0, 0.07853982];
        ll = [-1.4137167, -PI / 10.0, -0.07853982];
    }
    let bow = d.held.as_deref().is_some_and(|h| h.trim_start_matches("minecraft:") == "bow");
    if d.aggressive && bow {
        ra = [-PI / 2.0 + hx, -0.1 + hy, ra[2]];
        la = [-PI / 2.0 + hx, 0.1 + hy + 0.4, la[2]];
    }
    let bob = |a: &mut [f32; 3], s: f32| {
        a[2] += s * ((d.age * 0.09).cos() * 0.05 + 0.05);
        a[0] += s * ((d.age * 0.067).sin() * 0.05);
    };
    bob(&mut ra, 1.0);
    bob(&mut la, -1.0);
    let hat = part(vec![cube_grown(32.0, 0.0, [-4.0, -8.0, -4.0], [8.0, 8.0, 8.0], 0.5)], [0.0; 3], [0.0; 3]);
    vec![
        part_with(vec![cube(0.0, 0.0, [-4.0, -8.0, -4.0], [8.0, 8.0, 8.0])], [0.0; 3], [hx, hy, 0.0], vec![hat]),
        part(vec![cube(16.0, 16.0, [-4.0, 0.0, -2.0], [8.0, 12.0, 4.0])], [0.0; 3], [0.0; 3]),
        part(vec![cube(40.0, 16.0, [-1.0, -2.0, -1.0], [2.0, 12.0, 2.0])], [-5.0, 2.0, 0.0], ra),
        part(vec![cube_mirrored(40.0, 16.0, [-1.0, -2.0, -1.0], [2.0, 12.0, 2.0])], [5.0, 2.0, 0.0], la),
        part(vec![cube(0.0, 16.0, [-1.0, 0.0, -1.0], [2.0, 12.0, 2.0])], [-2.0, 12.0, 0.0], rl),
        part(vec![cube_mirrored(0.0, 16.0, [-1.0, 0.0, -1.0], [2.0, 12.0, 2.0])], [2.0, 12.0, 0.0], ll),
    ]
}

/// `EndermanModel.createBodyLayer` (texture 64 x 32: `HumanoidModel.createMesh` 14 px up with its
/// long limbs) posed by `setupAnim`: `HumanoidModel`'s (the head, the walk, a passenger's legs,
/// the arms hanging and bobbing) with each limb's swing halved and held within 0.4, the arms
/// forward holding a carried block (drawn by [`carried_block_transform`]); never creepy (its jaw
/// drops at a player who looked at it, and the views have none).
fn enderman(d: &crate::mesh::MobDraw) -> Vec<EPart> {
    let (hx, hy) = (d.head_pitch.to_radians(), d.head_yaw.to_radians());
    let (pos, speed) = (d.walk_pos * 0.6662, d.walk_speed);
    let mut ra = [(pos + PI).cos() * speed, 0.0, 0.0];
    let mut la = [pos.cos() * speed, 0.0, 0.0];
    let mut rl = [pos.cos() * 1.4 * speed, 0.005, 0.005];
    let mut ll = [(pos + PI).cos() * 1.4 * speed, -0.005, -0.005];
    if d.riding {
        ra[0] -= PI / 5.0;
        la[0] -= PI / 5.0;
        rl = [-1.4137167, PI / 10.0, 0.07853982];
        ll = [-1.4137167, -PI / 10.0, -0.07853982];
    }
    let bob = |a: &mut [f32; 3], s: f32| {
        a[2] += s * ((d.age * 0.09).cos() * 0.05 + 0.05);
        a[0] += s * ((d.age * 0.067).sin() * 0.05);
    };
    bob(&mut ra, 1.0);
    bob(&mut la, -1.0);
    for a in [&mut ra, &mut la, &mut rl, &mut ll] {
        a[0] = (a[0] * 0.5).clamp(-0.4, 0.4);
    }
    if d.carried.is_some() {
        ra = [-0.5, ra[1], 0.05];
        la = [-0.5, la[1], -0.05];
    }
    let jaw = part(vec![cube_grown(0.0, 16.0, [-4.0, -8.0, -4.0], [8.0, 8.0, 8.0], -0.5)], [0.0; 3], [0.0; 3]);
    vec![
        part_with(vec![cube(0.0, 0.0, [-4.0, -8.0, -4.0], [8.0, 8.0, 8.0])], [0.0, -13.0, 0.0], [hx, hy, 0.0], vec![jaw]),
        part(vec![cube(32.0, 16.0, [-4.0, 0.0, -2.0], [8.0, 12.0, 4.0])], [0.0, -14.0, 0.0], [0.0; 3]),
        part(vec![cube(56.0, 0.0, [-1.0, -2.0, -1.0], [2.0, 30.0, 2.0])], [-5.0, -12.0, 0.0], ra),
        part(vec![cube_mirrored(56.0, 0.0, [-1.0, -2.0, -1.0], [2.0, 30.0, 2.0])], [5.0, -12.0, 0.0], la),
        part(vec![cube(56.0, 0.0, [-1.0, 0.0, -1.0], [2.0, 30.0, 2.0])], [-2.0, -5.0, 0.0], rl),
        part(vec![cube_mirrored(56.0, 0.0, [-1.0, 0.0, -1.0], [2.0, 30.0, 2.0])], [2.0, -5.0, 0.0], ll),
    ]
}

/// An enderman's carried block's transform (`CarriedBlockLayer`: under the model's, 0.6875 down
/// and 0.75 forward, tilted 20 degrees and turned 45, half size and flipped): the block model's
/// unit cube goes through it.
pub fn carried_block_transform(pos: [f32; 3], d: &crate::mesh::MobDraw) -> M {
    mob_transform("enderman", pos, d).translate([0.0, 0.6875, -0.75]).rot(0, 20f32.to_radians()).rot(1, 45f32.to_radians()).translate([0.25, 0.1875, 0.25]).scale([-0.5, -0.5, 0.5]).rot(1, PI / 2.0)
}

/// A keyframe channel of an animation (`AnimationChannel`, every keyframe LINEAR): its part,
/// whether it moves the part (`Targets.POSITION`: px, `posVec`'s y already flipped) or turns it
/// (`ROTATION`: degrees), and its keyframes (seconds, value).
type Channel = (&'static str, bool, &'static [(f32, [f32; 3])]);

/// `BatAnimation.BAT_RESTING` (looping, 0.5 s): hung upside down, wings folded.
const BAT_RESTING: &[Channel] = &[
    ("head", false, &[(0.0, [180.0, 0.0, 0.0])]),
    ("head", true, &[(0.0, [0.0, -0.5, 0.0])]),
    ("body", false, &[(0.0, [180.0, 0.0, 0.0])]),
    ("body", true, &[(0.0, [0.0, -0.5, 0.0])]),
    ("feet", false, &[(0.0, [0.0, 0.0, 0.0])]),
    ("right_wing", false, &[(0.0, [0.0, -10.0, 0.0])]),
    ("right_wing", true, &[(0.0, [0.0, 0.0, 1.0])]),
    ("right_wing_tip", false, &[(0.0, [0.0, -120.0, 0.0])]),
    ("left_wing", false, &[(0.0, [0.0, 10.0, 0.0])]),
    ("left_wing", true, &[(0.0, [0.0, 0.0, 1.0])]),
    ("left_wing_tip", false, &[(0.0, [0.0, 120.0, 0.0])]),
];

/// The flight's bob (`BAT_FLYING`'s head and body positions).
const BAT_BOB: &[(f32, [f32; 3])] = &[(0.0, [0.0; 3]), (0.125, [0.0, -2.0, 0.0]), (0.25, [0.0, -1.0, 0.0]), (0.375, [0.0; 3]), (0.4583, [0.0, 1.0, 0.0]), (0.5, [0.0; 3])];

/// `BatAnimation.BAT_FLYING` (looping, 0.5 s): the wings beating, the body bobbing.
const BAT_FLYING: &[Channel] = &[
    ("head", false, &[(0.0, [0.0; 3]), (0.125, [20.0, 0.0, 0.0]), (0.5, [0.0; 3])]),
    ("head", true, BAT_BOB),
    ("body", false, &[(0.0, [40.0, 0.0, 0.0]), (0.25, [52.5, 0.0, 0.0]), (0.5, [40.0, 0.0, 0.0])]),
    ("body", true, BAT_BOB),
    ("feet", false, &[(0.0, [10.0, 0.0, 0.0]), (0.125, [-21.25, 0.0, 0.0]), (0.25, [-12.5, 0.0, 0.0]), (0.5, [10.0, 0.0, 0.0])]),
    ("right_wing", false, &[(0.0, [0.0, 85.0, 0.0]), (0.125, [0.0, -55.0, 0.0]), (0.25, [0.0, 50.0, 0.0]), (0.375, [0.0, 70.0, 0.0]), (0.5, [0.0, 85.0, 0.0])]),
    ("right_wing_tip", false, &[(0.0, [0.0, 10.5, 0.0]), (0.0417, [0.0, 65.5, 0.0]), (0.2083, [0.0, -135.0, 0.0]), (0.5, [0.0, 10.5, 0.0])]),
    ("left_wing", false, &[(0.0, [0.0, -85.0, 0.0]), (0.125, [0.0, 55.0, 0.0]), (0.25, [0.0, -50.0, 0.0]), (0.375, [0.0, -70.0, 0.0]), (0.5, [0.0, -85.0, 0.0])]),
    ("left_wing_tip", false, &[(0.0, [0.0, -10.5, 0.0]), (0.0417, [0.0, -65.5, 0.0]), (0.2083, [0.0, 135.0, 0.0]), (0.5, [0.0, -10.5, 0.0])]),
];

/// A channel's value at `t` seconds (`KeyframeAnimation.Entry.apply`: the keyframe before `t`
/// and the next, linearly).
fn channel_at(k: &[(f32, [f32; 3])], t: f32) -> [f32; 3] {
    let prev = k.iter().position(|f| t <= f.0).unwrap_or(k.len()).saturating_sub(1);
    let next = (prev + 1).min(k.len() - 1);
    let a = if next != prev { ((t - k[prev].0) / (k[next].0 - k[prev].0)).clamp(0.0, 1.0) } else { 0.0 };
    [0, 1, 2].map(|i| k[prev].1[i] + (k[next].1[i] - k[prev].1[i]) * a)
}

/// `BatModel.createBodyLayer` (texture 32 x 32) posed by `setupAnim`: resting, its head turned by
/// its yaw; then its keyframe animation (`BAT_RESTING` or `BAT_FLYING`) at the time since it
/// started (`AnimationState.getTimeInMillis`: whole milliseconds, looped every 0.5 s), each
/// channel added to its part's offset or rotation.
fn bat(d: &crate::mesh::MobDraw) -> Vec<EPart> {
    let millis = ((d.age - d.anim_start as f32) * 50.0) as i64;
    let t = (millis as f32 / 1000.0) % 0.5;
    let anim = if d.resting { BAT_RESTING } else { BAT_FLYING };
    // (offset, rotation in radians) of a part: its pose plus the animation's channels
    let pose = |name: &str, offset: [f32; 3], rot: [f32; 3]| -> ([f32; 3], [f32; 3]) {
        let (mut o, mut r) = (offset, rot);
        for (part_name, moves, keys) in anim {
            if *part_name == name {
                let v = channel_at(keys, t);
                for i in 0..3 {
                    if *moves {
                        o[i] += v[i];
                    } else {
                        r[i] += v[i].to_radians();
                    }
                }
            }
        }
        (o, r)
    };
    let head_yaw = if d.resting { d.head_yaw.to_radians() } else { 0.0 };
    let posed = |name: &str, offset: [f32; 3], rot: [f32; 3], cubes: Vec<Cube>, children: Vec<EPart>| {
        let (o, r) = pose(name, offset, rot);
        part_with(cubes, o, r, children)
    };
    let tip_r = posed("right_wing_tip", [-2.0, 0.0, 0.0], [0.0; 3], vec![cube(16.0, 0.0, [-6.0, -2.0, 0.0], [6.0, 8.0, 0.0])], Vec::new());
    let tip_l = posed("left_wing_tip", [2.0, 0.0, 0.0], [0.0; 3], vec![cube(16.0, 8.0, [0.0, -2.0, 0.0], [6.0, 8.0, 0.0])], Vec::new());
    let wing_r = posed("right_wing", [-1.5, 0.0, 0.0], [0.0; 3], vec![cube(12.0, 0.0, [-2.0, -2.0, 0.0], [2.0, 7.0, 0.0])], vec![tip_r]);
    let wing_l = posed("left_wing", [1.5, 0.0, 0.0], [0.0; 3], vec![cube(12.0, 7.0, [0.0, -2.0, 0.0], [2.0, 7.0, 0.0])], vec![tip_l]);
    let feet = posed("feet", [0.0, 5.0, 0.0], [0.0; 3], vec![cube(16.0, 16.0, [-1.5, 0.0, 0.0], [3.0, 2.0, 0.0])], Vec::new());
    let ears = vec![
        part(vec![cube(1.0, 15.0, [-2.5, -4.0, 0.0], [3.0, 5.0, 0.0])], [-1.5, -2.0, 0.0], [0.0; 3]),
        part(vec![cube(8.0, 15.0, [-0.1, -3.0, 0.0], [3.0, 5.0, 0.0])], [1.1, -3.0, 0.0], [0.0; 3]),
    ];
    vec![
        posed("body", [0.0, 17.0, 0.0], [0.0; 3], vec![cube(0.0, 0.0, [-1.5, 0.0, -1.0], [3.0, 5.0, 2.0])], vec![wing_r, wing_l, feet]),
        posed("head", [0.0, 17.0, 0.0], [0.0, head_yaw, 0.0], vec![cube(0.0, 7.0, [-2.0, -3.0, -1.0], [4.0, 3.0, 2.0])], ears),
    ]
}

/// `WitchModel.createBodyLayer` (texture 64 x 128: `VillagerModel.createBodyModel` with its own
/// head and pointed hat; a part added again keeps its children, so the villager's hat rim and
/// nose stay, the nose with a mole) scaled 0.9375 like a villager (`LayerDefinitions`), posed by
/// `setupAnim`: the head's yaw and pitch, the legs at half the walk's swing, the nose wobbling at
/// `entityId % 10`'s pace, or lifted toward the mouth while it holds an item.
fn witch(d: &crate::mesh::MobDraw) -> Vec<EPart> {
    let (hx, hy) = (d.head_pitch.to_radians(), d.head_yaw.to_radians());
    let swing = |phase: f32| (d.walk_pos * 0.6662 + phase).cos() * 1.4 * d.walk_speed * 0.5;
    let pace = 0.01 * (d.entity_id % 10) as f32;
    let (nose_at, nose_rot) = if d.held.is_some() {
        ([0.0, 1.0, -1.5], [-0.9, 0.0, (d.age * pace).cos() * 2.5f32.to_radians()])
    } else {
        ([0.0, -2.0, 0.0], [(d.age * pace).sin() * 4.5f32.to_radians(), 0.0, (d.age * pace).cos() * 2.5f32.to_radians()])
    };
    let mole = part(vec![cube_grown(0.0, 0.0, [0.0, 3.0, -6.75], [1.0, 1.0, 1.0], -0.25)], [0.0, -2.0, 0.0], [0.0; 3]);
    let nose = part_with(vec![cube(24.0, 0.0, [-1.0, -1.0, -6.0], [2.0, 4.0, 2.0])], nose_at, nose_rot, vec![mole]);
    let hat4 = part(vec![cube_grown(0.0, 95.0, [0.0; 3], [1.0, 2.0, 1.0], 0.25)], [1.75, -2.0, 2.0], [-PI / 15.0, 0.0, 0.10471976]);
    let hat3 = part_with(vec![cube(0.0, 87.0, [0.0; 3], [4.0, 4.0, 4.0])], [1.75, -4.0, 2.0], [-0.10471976, 0.0, 0.05235988], vec![hat4]);
    let hat2 = part_with(vec![cube(0.0, 76.0, [0.0; 3], [7.0, 4.0, 7.0])], [1.75, -4.0, 2.0], [-0.05235988, 0.0, 0.02617994], vec![hat3]);
    let rim = part(vec![cube(30.0, 47.0, [-8.0, -8.0, -6.0], [16.0, 16.0, 1.0])], [0.0; 3], [-PI / 2.0, 0.0, 0.0]);
    let hat = part_with(vec![cube(0.0, 64.0, [0.0; 3], [10.0, 2.0, 10.0])], [-5.0, -10.03125, -5.0], [0.0; 3], vec![rim, hat2]);
    let head = part_with(vec![cube(0.0, 0.0, [-4.0, -10.0, -4.0], [8.0, 10.0, 8.0])], [0.0; 3], [hx, hy, 0.0], vec![hat, nose]);
    let body = part_with(vec![cube(16.0, 20.0, [-4.0, 0.0, -3.0], [8.0, 12.0, 6.0])], [0.0; 3], [0.0; 3], vec![part(vec![cube_grown(0.0, 38.0, [-4.0, 0.0, -3.0], [8.0, 20.0, 6.0], 0.5)], [0.0; 3], [0.0; 3])]);
    let arms = part(vec![cube(44.0, 22.0, [-8.0, -2.0, -2.0], [4.0, 8.0, 4.0]), cube_mirrored(44.0, 22.0, [4.0, -2.0, -2.0], [4.0, 8.0, 4.0]), cube(40.0, 38.0, [-4.0, 2.0, -2.0], [8.0, 4.0, 4.0])], [0.0, 3.0, -1.0], [-0.75, 0.0, 0.0]);
    let right_leg = part(vec![cube(0.0, 22.0, [-2.0, 0.0, -2.0], [4.0, 12.0, 4.0])], [-2.0, 12.0, 0.0], [swing(0.0), 0.0, 0.0]);
    let left_leg = part(vec![cube_mirrored(0.0, 22.0, [-2.0, 0.0, -2.0], [4.0, 12.0, 4.0])], [2.0, 12.0, 0.0], [swing(PI), 0.0, 0.0]);
    scaled(0.9375, vec![head, body, arms, right_leg, left_leg])
}

/// `AdultChickenModel.createBodyLayer` (texture 64 x 32; the plains' one variant, temperate:
/// redsim draws `selectVariantToSpawn`'s candidate but keeps no variant) posed by `setupAnim`: the
/// head's yaw and pitch, the legs walking, the wings beating while it is off the ground
/// (`(sin(flap) + 1) * flapSpeed`).
fn chicken(d: &crate::mesh::MobDraw) -> Vec<EPart> {
    let (hx, hy) = (d.head_pitch.to_radians(), d.head_yaw.to_radians());
    let swing = |phase: f32| (d.walk_pos * 0.6662 + phase).cos() * 1.4 * d.walk_speed;
    let wing = (d.flap[0].sin() + 1.0) * d.flap[1];
    let leg = || vec![cube(26.0, 0.0, [-1.0, 0.0, -3.0], [3.0, 5.0, 3.0])];
    let beak = part(vec![cube(14.0, 0.0, [-2.0, -4.0, -4.0], [4.0, 2.0, 2.0])], [0.0; 3], [0.0; 3]);
    let wattle = part(vec![cube(14.0, 4.0, [-1.0, -2.0, -3.0], [2.0, 2.0, 2.0])], [0.0; 3], [0.0; 3]);
    vec![
        part_with(vec![cube(0.0, 0.0, [-2.0, -6.0, -2.0], [4.0, 6.0, 3.0])], [0.0, 15.0, -4.0], [hx, hy, 0.0], vec![beak, wattle]),
        part(vec![cube(0.0, 9.0, [-3.0, -4.0, -3.0], [6.0, 8.0, 6.0])], [0.0, 16.0, 0.0], [PI / 2.0, 0.0, 0.0]),
        part(leg(), [-2.0, 19.0, 1.0], [swing(0.0), 0.0, 0.0]),
        part(leg(), [1.0, 19.0, 1.0], [swing(PI), 0.0, 0.0]),
        part(vec![cube(24.0, 13.0, [0.0, 0.0, -3.0], [1.0, 4.0, 6.0])], [-4.0, 13.0, 0.0], [0.0, 0.0, wing]),
        part(vec![cube(24.0, 13.0, [-1.0, 0.0, -3.0], [1.0, 4.0, 6.0])], [4.0, 13.0, 0.0], [0.0, 0.0, -wing]),
    ]
}

/// `SheepModel.createBodyLayer` (texture 64 x 32: `QuadrupedModel.createBodyMesh(12, false,
/// true)`, its right legs mirrored, with its own head and body), or with `fur` its wool's
/// `SheepFurModel.createFurLayer` (the same boxes grown: head 0.6, body 1.75, legs 0.5), posed
/// by `setupAnim`: `QuadrupedModel`'s (the head's yaw and pitch, the legs walking), the head
/// lowered and tilted while it eats grass.
fn sheep(d: &crate::mesh::MobDraw, fur: bool) -> Vec<EPart> {
    let (hx, hy) = (d.head_pitch.to_radians(), d.head_yaw.to_radians());
    let swing = |phase: f32| (d.walk_pos * 0.6662 + phase).cos() * 1.4 * d.walk_speed;
    let (head_y, head_x) = match d.eat {
        Some([drop, angle]) => (6.0 + drop * 9.0, angle),
        None => (6.0, hx),
    };
    let (gh, gb, gl) = if fur { (0.6, 1.75, 0.5) } else { (0.0, 0.0, 0.0) };
    let head = if fur { cube_grown(0.0, 0.0, [-3.0, -4.0, -4.0], [6.0, 6.0, 6.0], gh) } else { cube(0.0, 0.0, [-3.0, -4.0, -6.0], [6.0, 6.0, 8.0]) };
    let leg = |mirror: bool| vec![Cube { mirror: mirror && !fur, ..cube_grown(0.0, 16.0, [-2.0, 0.0, -2.0], [4.0, if fur { 6.0 } else { 12.0 }, 4.0], gl) }];
    vec![
        part(vec![head], [0.0, head_y, -8.0], [head_x, hy, 0.0]),
        part(vec![cube_grown(28.0, 8.0, [-4.0, -10.0, -7.0], [8.0, 16.0, 6.0], gb)], [0.0, 5.0, 2.0], [PI / 2.0, 0.0, 0.0]),
        part(leg(true), [-3.0, 12.0, 7.0], [swing(0.0), 0.0, 0.0]),
        part(leg(false), [3.0, 12.0, 7.0], [swing(PI), 0.0, 0.0]),
        part(leg(true), [-3.0, 12.0, -5.0], [swing(PI), 0.0, 0.0]),
        part(leg(false), [3.0, 12.0, -5.0], [swing(0.0), 0.0, 0.0]),
    ]
}

/// `DyeColor.textureDiffuseColor` of each dye by id (white, orange, ... black).
const DYE_DIFFUSE: [u32; 16] = [16383998, 16351261, 13061821, 3847130, 16701501, 8439583, 15961002, 4673362, 10329495, 1481884, 8991416, 3949738, 8606770, 6192150, 11546150, 1908001];

/// A sheep's wool tint (`ColorLerper.Type.SHEEP`: white 0xE6E6E6, any other dye its diffuse
/// colour x 0.75, floored).
fn wool_tint(color: u8) -> [u8; 3] {
    if color == 0 {
        return [230, 230, 230];
    }
    let c = DYE_DIFFUSE[(color as usize).min(15)];
    [(c >> 16) & 255, (c >> 8) & 255, c & 255].map(|v| (v as f32 * 0.75).floor() as u8)
}

/// `AbstractEquineModel.createBodyMesh(NONE)` (texture 64 x 64; an undead horse's layer is not
/// scaled, a living horse's is 1.1) posed by `setupAnim`: the neck at 30 degrees plus the head's
/// pitch (nodding when it trots), turned by its yaw within 20 degrees, the legs walking, the tail
/// lifting with the walk; never rearing, eating or feeding (redsim keeps no horse animation), the
/// tail not swishing (`moveTail` runs on the client's own random).
fn zombie_horse(d: &crate::mesh::MobDraw) -> Vec<EPart> {
    let (pos, speed) = (d.walk_pos, d.walk_speed);
    let mut head_x = d.head_pitch.to_radians();
    if speed > 0.2 {
        head_x += (pos * 0.8).cos() * 0.15 * speed;
    }
    let head_y = d.head_yaw.clamp(-20.0, 20.0).to_radians();
    let leg = (pos * 0.6662 + PI).cos();
    let front = leg * 0.8 * speed;
    let hind = leg * 0.5 * speed;
    let tail = part(vec![cube(42.0, 36.0, [-1.5, 0.0, 0.0], [3.0, 14.0, 4.0])], [0.0, -5.0 + speed, 2.0 + speed * 2.0], [PI / 6.0 + speed * 0.75, 0.0, 0.0]);
    let ears = vec![
        part(vec![cube_grown(19.0, 16.0, [0.55, -13.0, 4.0], [2.0, 3.0, 1.0], -0.001)], [0.0; 3], [0.0; 3]),
        part(vec![cube_grown(19.0, 16.0, [-2.55, -13.0, 4.0], [2.0, 3.0, 1.0], -0.001)], [0.0; 3], [0.0; 3]),
    ];
    let head = part_with(vec![cube(0.0, 13.0, [-3.0, -11.0, -2.0], [6.0, 5.0, 7.0])], [0.0; 3], [0.0; 3], ears);
    let mane = part(vec![cube(56.0, 36.0, [-1.0, -11.0, 5.01], [2.0, 16.0, 2.0])], [0.0; 3], [0.0; 3]);
    let mouth = part(vec![cube(0.0, 25.0, [-2.0, -11.0, -7.0], [4.0, 5.0, 5.0])], [0.0; 3], [0.0; 3]);
    vec![
        part_with(vec![cube_grown(0.0, 32.0, [-5.0, -8.0, -17.0], [10.0, 10.0, 22.0], 0.05)], [0.0, 11.0, 5.0], [0.0; 3], vec![tail]),
        part_with(vec![cube(0.0, 35.0, [-2.05, -6.0, -2.0], [4.0, 12.0, 7.0])], [0.0, 4.0, -12.0], [PI / 6.0 + head_x, head_y, 0.0], vec![head, mane, mouth]),
        part(vec![cube_mirrored(48.0, 21.0, [-3.0, -1.01, -1.0], [4.0, 11.0, 4.0])], [4.0, 14.0, 7.0], [-hind, 0.0, 0.0]),
        part(vec![cube(48.0, 21.0, [-1.0, -1.01, -1.0], [4.0, 11.0, 4.0])], [-4.0, 14.0, 7.0], [hind, 0.0, 0.0]),
        part(vec![cube_mirrored(48.0, 21.0, [-3.0, -1.01, -1.9], [4.0, 11.0, 4.0])], [4.0, 14.0, -10.0], [front, 0.0, 0.0]),
        part(vec![cube(48.0, 21.0, [-1.0, -1.01, -1.9], [4.0, 11.0, 4.0])], [-4.0, 14.0, -10.0], [-front, 0.0, 0.0]),
    ]
}

/// A layer definition through `MeshTransformer.scaling(scale)` (`LayerDefinitions`: the root part
/// scaled, `24.016 * (1 - scale)` px down so the feet stay on the ground).
fn scaled(scale: f32, parts: Vec<EPart>) -> Vec<EPart> {
    if scale == 1.0 {
        return parts;
    }
    vec![EPart { scale, ..part_with(Vec::new(), [0.0, 24.016 * (1.0 - scale), 0.0], [0.0; 3], parts) }]
}

/// `SpiderModel.createSpiderBodyLayer` (texture 64 x 32) posed by `setupAnim`: the head's yaw and
/// pitch, each pair of legs swinging (`yRot`) and stepping (`zRot`) with the walk, a quarter
/// period apart; the cave spider's layer is it scaled 0.7 (`LayerDefinitions`).
fn spider(d: &crate::mesh::MobDraw, scale: f32) -> Vec<EPart> {
    let (hx, hy) = (d.head_pitch.to_radians(), d.head_yaw.to_radians());
    let (pos, speed) = (d.walk_pos * 0.6662, d.walk_speed);
    let swing = |phase: f32| -((pos * 2.0 + phase).cos() * 0.4) * speed;
    let step = |phase: f32| ((pos + phase).sin() * 0.4).abs() * speed;
    let right = || vec![cube(18.0, 0.0, [-15.0, -1.0, -1.0], [16.0, 2.0, 2.0])];
    let left = || vec![cube_mirrored(18.0, 0.0, [-1.0, -1.0, -1.0], [16.0, 2.0, 2.0])];
    let (q, e, m) = (PI / 4.0, PI / 8.0, 0.58119464f32);
    // (z, the default yaw and roll of the right leg; the left one mirrors them) and its phase
    let legs = [(2.0, q, -q, 0.0), (1.0, e, -m, PI), (0.0, -e, -m, PI / 2.0), (-1.0, -q, -q, PI * 3.0 / 2.0)];
    let mut parts = vec![
        part(vec![cube(32.0, 4.0, [-4.0, -4.0, -8.0], [8.0, 8.0, 8.0])], [0.0, 15.0, -3.0], [hx, hy, 0.0]),
        part(vec![cube(0.0, 0.0, [-3.0, -3.0, -3.0], [6.0, 6.0, 6.0])], [0.0, 15.0, 0.0], [0.0; 3]),
        part(vec![cube(0.0, 12.0, [-5.0, -4.0, -6.0], [10.0, 8.0, 12.0])], [0.0, 15.0, 9.0], [0.0; 3]),
    ];
    for (z, yaw, roll, phase) in legs {
        let (s, t) = (swing(phase), step(phase));
        parts.push(part(right(), [-4.0, 15.0, z], [0.0, yaw + s, roll + t]));
        parts.push(part(left(), [4.0, 15.0, z], [0.0, -yaw - s, -roll - t]));
    }
    scaled(scale, parts)
}

/// The game texture each mob is drawn with (with its overlay layers composited in once the atlas
/// is packed, `Atlas::add_game_entities`): an iron golem's base under its crackiness
/// (`IronGolemCrackinessLayer`), a villager's base under its plains type (`VillagerProfessionLayer`:
/// no profession, so the type's layer covers the whole model, hat included), a zombie's own.
pub fn mob_texture(kind: &str, crack: u8) -> Option<(String, [f32; 2])> {
    match kind.strip_prefix("minecraft:").unwrap_or(kind) {
        "iron_golem" => Some((format!("mob/iron_golem/{}", crack.min(3)), [128.0, 128.0])),
        "villager" => Some(("mob/villager/plains".to_string(), [64.0, 64.0])),
        "zombie" => Some(("entity/zombie/zombie".to_string(), [64.0, 64.0])),
        "baby_zombie" => Some(("entity/zombie/zombie_baby".to_string(), [64.0, 64.0])),
        "spider" => Some(("entity/spider/spider".to_string(), [64.0, 32.0])),
        "cave_spider" => Some(("entity/spider/cave_spider".to_string(), [64.0, 32.0])),
        "creeper" => Some(("entity/creeper/creeper".to_string(), [64.0, 32.0])),
        "skeleton" => Some(("entity/skeleton/skeleton".to_string(), [64.0, 32.0])),
        "enderman" => Some(("entity/enderman/enderman".to_string(), [64.0, 32.0])),
        "bat" => Some(("entity/bat/bat".to_string(), [32.0, 32.0])),
        "witch" => Some(("entity/witch/witch".to_string(), [64.0, 128.0])),
        "chicken" => Some(("entity/chicken/chicken_temperate".to_string(), [64.0, 32.0])),
        "sheep" => Some(("entity/sheep/sheep".to_string(), [64.0, 32.0])),
        "zombie_horse" => Some(("entity/horse/horse_zombie".to_string(), [64.0, 64.0])),
        _ => None,
    }
}

/// The game textures the mob models draw as they are (packed from the vendored entity atlas by
/// `Atlas::add_game_entities`; the composited ones are [`MOB_COMPOSITES`]).
pub const MOB_TEXTURES: &[&str] = &[
    "entity/zombie/zombie",
    "entity/zombie/zombie_baby",
    "entity/spider/spider",
    "entity/spider/cave_spider",
    "entity/spider/spider_eyes",
    "entity/creeper/creeper",
    "entity/skeleton/skeleton",
    "entity/enderman/enderman",
    "entity/enderman/enderman_eyes",
    "entity/bat/bat",
    "entity/witch/witch",
    "entity/chicken/chicken_temperate",
    "entity/sheep/sheep",
    "entity/sheep/sheep_wool",
    "entity/sheep/sheep_wool_undercoat",
    "entity/horse/horse_zombie",
    // armor (`EquipmentLayerRenderer`: each material's `humanoid` and `humanoid_leggings` layers)
    "entity/equipment/humanoid/leather",
    "entity/equipment/humanoid/leather_overlay",
    "entity/equipment/humanoid/copper",
    "entity/equipment/humanoid/gold",
    "entity/equipment/humanoid/chainmail",
    "entity/equipment/humanoid/iron",
    "entity/equipment/humanoid/diamond",
    "entity/equipment/humanoid_leggings/leather",
    "entity/equipment/humanoid_leggings/leather_overlay",
    "entity/equipment/humanoid_leggings/copper",
    "entity/equipment/humanoid_leggings/gold",
    "entity/equipment/humanoid_leggings/chainmail",
    "entity/equipment/humanoid_leggings/iron",
    "entity/equipment/humanoid_leggings/diamond",
];

/// A mob's eyes layer (`EyesLayer`: the model again with this texture through the `eyes`
/// pipeline: the texel times white, no cardinal lighting, no lightmap, no overlay, alpha-blended
/// over the model). Its texels are opaque or empty (`a_spiders_eyes_are_opaque_or_empty`), so it is
/// drawn as solid faces over the model's, at full brightness.
fn mob_eyes(kind: &str) -> Option<&'static str> {
    match kind {
        // SpiderRenderer's SpiderEyesLayer (CaveSpiderRenderer extends it)
        "spider" | "cave_spider" => Some("entity/spider/spider_eyes"),
        // EndermanRenderer's EnderEyesLayer
        "enderman" => Some("entity/enderman/enderman_eyes"),
        _ => None,
    }
}

/// `LivingEntityRenderer.getFlipDegrees`: how far a dying mob falls over (a spider onto its back).
fn flip_degrees(kind: &str) -> f32 {
    match kind {
        "spider" | "cave_spider" => 180.0,
        _ => 90.0,
    }
}

/// The composited mob textures [`mob_texture`] names: (name, base, overlay), the overlay drawn
/// over the base wherever it is opaque (the game draws the layer again over the model with a
/// cutout: alpha under 0.1 discarded).
pub const MOB_COMPOSITES: [(&str, &str, Option<&str>); 5] = [
    ("mob/iron_golem/0", "entity/iron_golem/iron_golem", None),
    ("mob/iron_golem/1", "entity/iron_golem/iron_golem", Some("entity/iron_golem/iron_golem_crackiness_low")),
    ("mob/iron_golem/2", "entity/iron_golem/iron_golem", Some("entity/iron_golem/iron_golem_crackiness_medium")),
    ("mob/iron_golem/3", "entity/iron_golem/iron_golem", Some("entity/iron_golem/iron_golem_crackiness_high")),
    ("mob/villager/plains", "entity/villager/villager", Some("entity/villager/type/plains")),
];

/// `LivingEntityRenderer.sleepDirectionToRotation`.
fn sleep_rotation(d: redsim::Dir) -> f32 {
    match d {
        redsim::Dir::South => 90.0,
        redsim::Dir::West => 0.0,
        redsim::Dir::North => 270.0,
        redsim::Dir::East => 180.0,
        _ => 0.0,
    }
}

/// The transform a mob's model is drawn under (`LivingEntityRenderer.submit`: at its position, a
/// sleeper moved toward its pillow and laid on its bed, else turned by its body's yaw; a dying mob
/// falling over (`getFlipDegrees`), a walking golem's sway; the model flipped and lowered 1.501).
fn mob_transform(k: &str, pos: [f32; 3], d: &crate::mesh::MobDraw) -> M {
    let mut m = M::id().translate(pos);
    if let Some(dir) = d.sleeping {
        let off = d.eye_standing - 0.1;
        let s = dir.step();
        m = m.translate([-s[0] as f32 * off, 0.0, -s[2] as f32 * off]);
    }
    if d.sleeping.is_none() {
        m = m.rot(1, (180.0 - d.body_yaw).to_radians());
    }
    if d.death_time > 0.0 {
        let fall = ((d.death_time - 1.0) / 20.0 * 1.6).max(0.0).sqrt().min(1.0);
        m = m.rot(2, (fall * flip_degrees(k)).to_radians());
    } else if let Some(dir) = d.sleeping {
        m = m.rot(1, sleep_rotation(dir).to_radians()).rot(2, PI / 2.0).rot(1, 270f32.to_radians());
    }
    // IronGolemRenderer.setupRotations: a walking golem sways
    if k == "iron_golem" && d.walk_speed >= 0.01 {
        let wp = d.walk_pos + 6.0;
        let tri = ((wp % 13.0 - 6.5).abs() - 3.25) / 3.25;
        m = m.rot(2, (6.5 * tri).to_radians());
    }
    m.scale([-1.0, -1.0, 1.0]).translate([0.0, -1.501, 0.0])
}

/// The equipment asset of each armor material (redsim's `Armor::material` order: golden armor's
/// asset is `gold`), whose `humanoid` and `humanoid_leggings` layers draw it.
const ARMOR_ASSETS: [&str; 6] = ["leather", "copper", "gold", "chainmail", "iron", "diamond"];

/// A humanoid mob's parts' poses as its `setupAnim` left them (head, body, right arm, left arm,
/// right leg, left leg, from the built model): what its armor model, an instance of its own model
/// class posed by the same `setupAnim`, takes. None: not a humanoid (or a baby zombie, whose armor
/// is the `humanoid_baby` mesh: not drawn yet).
fn humanoid_pose(k: &str, parts: &[EPart]) -> Option<[[f32; 3]; 6]> {
    match k {
        "zombie" | "skeleton" if parts.len() >= 6 => Some(std::array::from_fn(|i| parts[i].rot)),
        _ => None,
    }
}

/// `HumanoidModel.createArmorMeshSet(0.5, 1.0)`'s model of an armor slot (0 head, 1 chest, 2 legs,
/// 3 feet: `ADULT_ARMOR_PARTS_PER_SLOT`; the legs' with the inner deformation 0.5, the others'
/// the outer 1.0, a leg's box 0.1 less: `createBaseArmorMesh`), each part at the humanoid mesh's
/// offset turned by the mob's pose ([`humanoid_pose`]); texture 64 x 32.
fn armor_parts(slot: usize, pose: &[[f32; 3]; 6]) -> Vec<EPart> {
    let g = if slot == 2 { 0.5 } else { 1.0 };
    let mirrored = |u: f32, v: f32, from: [f32; 3], size: [f32; 3], grow: f32| Cube { mirror: true, grow, ..cube(u, v, from, size) };
    let body = part(vec![cube_grown(16.0, 16.0, [-4.0, 0.0, -2.0], [8.0, 12.0, 4.0], g)], [0.0; 3], pose[1]);
    let legs = || {
        vec![
            part(vec![cube_grown(0.0, 16.0, [-2.0, 0.0, -2.0], [4.0, 12.0, 4.0], g - 0.1)], [-1.9, 12.0, 0.0], pose[4]),
            part(vec![mirrored(0.0, 16.0, [-2.0, 0.0, -2.0], [4.0, 12.0, 4.0], g - 0.1)], [1.9, 12.0, 0.0], pose[5]),
        ]
    };
    match slot {
        0 => {
            let hat = part(vec![cube_grown(32.0, 0.0, [-4.0, -8.0, -4.0], [8.0, 8.0, 8.0], g + 0.5)], [0.0; 3], [0.0; 3]);
            vec![part_with(vec![cube_grown(0.0, 0.0, [-4.0, -8.0, -4.0], [8.0, 8.0, 8.0], g)], [0.0; 3], pose[0], vec![hat])]
        }
        1 => vec![
            body,
            part(vec![cube_grown(40.0, 16.0, [-3.0, -2.0, -2.0], [4.0, 12.0, 4.0], g)], [-5.0, 2.0, 0.0], pose[2]),
            part(vec![mirrored(40.0, 16.0, [-1.0, -2.0, -2.0], [4.0, 12.0, 4.0], g)], [5.0, 2.0, 0.0], pose[3]),
        ],
        2 => {
            let mut v = legs();
            v.insert(0, body);
            v
        }
        _ => legs(),
    }
}

/// The layers of a slot's armor of a material (`EquipmentLayerRenderer.renderLayers` over the
/// asset's `humanoid` layers, the legs' `humanoid_leggings`): each texture and its colour
/// (leather's base dyeable, undyed `color_when_undyed` 0xA06540, then its overlay as drawn).
fn armor_layers(slot: usize, material: u8) -> Vec<(String, [u8; 3])> {
    let kind = if slot == 2 { "humanoid_leggings" } else { "humanoid" };
    let asset = ARMOR_ASSETS.get(material as usize).copied().unwrap_or("iron");
    let tex = |a: &str| format!("entity/equipment/{kind}/{a}");
    if asset == "leather" { vec![(tex("leather"), [0xA0, 0x65, 0x40]), (tex("leather_overlay"), [255; 3])] } else { vec![(tex(asset), [255; 3])] }
}

/// An item's `thirdperson_righthand` display (its model's or its parent's: rotation in degrees,
/// translation in pixels, scale) and the sprite its in-hand model shows: the bow's own; a spear's
/// `<spear>_in_hand` model (`spear_in_hand`); a tool's, a stick's, a bone's `item/handheld`; any
/// other item's `item/generated`.
fn held_display(id: &str) -> ([f32; 3], [f32; 3], [f32; 3], String) {
    let name = id.strip_prefix("minecraft:").unwrap_or(id);
    let handheld = ["_sword", "_shovel", "_axe", "_pickaxe", "_hoe"].iter().any(|s| name.ends_with(s)) || matches!(name, "stick" | "bone" | "blaze_rod" | "breeze_rod");
    if name == "bow" {
        ([-80.0, 260.0, -40.0], [-1.0, -2.0, 2.5], [0.9; 3], "item/bow".into())
    } else if name.ends_with("_spear") {
        ([5.0, 270.0, -40.0], [0.0, 2.0, 2.0], [1.7, 1.7, 0.85], format!("item/{name}_in_hand"))
    } else if handheld {
        ([0.0, -90.0, 55.0], [0.0, 4.0, 0.5], [0.85; 3], format!("item/{name}"))
    } else {
        ([0.0; 3], [0.0, 3.0, 1.0], [0.55; 3], format!("item/{name}"))
    }
}

/// Where the item in a humanoid's right hand is drawn (`ItemInHandLayer.submitArmWithItem`): the
/// mob's transform, `translateToHand` (the root, the arm: a skeleton's a pixel further in,
/// `SkeletonModel.translateToHand`), a quarter turn about x back, a half turn about y, the hand's
/// offset (1, 2, -10 pixels; a baby's 0, 1, -4.5), then the item's display (`ItemTransform.apply`:
/// translate, rotate x then y then z, scale, its unit cube centred) to the middle of its cube: the
/// sprite's plane.
fn held_transform(m: &M, arm_offset: [f32; 3], arm_rot: [f32; 3], baby: bool, display: &([f32; 3], [f32; 3], [f32; 3], String)) -> M {
    let (r, t, s) = (display.0, display.1, display.2);
    let hand = if baby { [0.0, 1.0, -4.5] } else { [1.0, 2.0, -10.0] };
    m.translate(arm_offset.map(|v| v / 16.0))
        .rot(2, arm_rot[2])
        .rot(1, arm_rot[1])
        .rot(0, arm_rot[0])
        .rot(0, -PI / 2.0)
        .rot(1, PI)
        .translate(hand.map(|v| v / 16.0))
        .translate(t.map(|v| v / 16.0))
        .rot(0, r[0].to_radians())
        .rot(1, r[1].to_radians())
        .rot(2, r[2].to_radians())
        .scale(s)
        .translate([-0.5, -0.5, 0.0])
}

/// A mob drawn like the game (`LivingEntityRenderer.submit` with its renderer's model and
/// `setupRotations`): a sleeper moved toward its pillow and laid on its bed, a dying mob falling
/// sideways over 20 ticks, a walking golem's sway, the model flipped and lowered 1.501, then the
/// red overlay of a hurt or dying mob (`OverlayTexture`: 30% red, [`crate::atlas::GAME_LAYER_RED`]),
/// then its layers without it: a humanoid's armor (`HumanoidArmorLayer`) and the item in its hand
/// (`ItemInHandLayer`), the eyes.
/// None: not a mob the viewer has, or its texture is not packed.
pub fn mob_triangles(g: &GameAssets, kind: &str, pos: [f32; 3], d: &crate::mesh::MobDraw) -> Option<Vec<Vertex>> {
    let k = kind.strip_prefix("minecraft:").unwrap_or(kind);
    let (tex, size) = mob_texture(k, d.crack)?;
    let r = rect(g, &tex)?;
    let parts = match k {
        "iron_golem" => iron_golem(d),
        "villager" => villager(d),
        "zombie" => zombie(d, false),
        "baby_zombie" => zombie(d, true),
        "spider" => spider(d, 1.0),
        "cave_spider" => spider(d, 0.7),
        "creeper" => creeper(d),
        "skeleton" => skeleton(d),
        "enderman" => enderman(d),
        "bat" => bat(d),
        "witch" => witch(d),
        "chicken" => chicken(d),
        "sheep" => sheep(d, false),
        "zombie_horse" => zombie_horse(d),
        _ => return None,
    };
    let m = mob_transform(k, pos, d);
    let (tw, th) = (g.width as f32, g.height as f32);
    let mut out = Vec::new();
    emit(&mut out, &parts, &m, size, r, tw, th);
    // its tinted layers over it (`coloredCutoutModelCopyLayerRender`: the model's pose, the
    // texture times the tint; a sheep's undercoat when dyed, then its wool)
    let layers: Vec<(Vec<EPart>, &str, [u8; 3])> = match k {
        "sheep" => {
            let tint = wool_tint(d.color);
            let mut l = Vec::new();
            if d.color != 0 {
                l.push((sheep(d, false), "entity/sheep/sheep_wool_undercoat", tint));
            }
            l.push((sheep(d, true), "entity/sheep/sheep_wool", tint));
            l
        }
        _ => Vec::new(),
    };
    for (lp, tex, tint) in layers {
        if let Some(lr) = rect(g, tex) {
            let n = out.len();
            emit(&mut out, &lp, &m, size, lr, tw, th);
            for v in &mut out[n..] {
                for c in 0..3 {
                    v.color[c] = (v.color[c] as u32 * tint[c] as u32 / 255) as u8;
                }
            }
        }
    }
    if d.red {
        for v in &mut out {
            v.layer = crate::atlas::GAME_LAYER_RED;
            v.color[3] = 178;
        }
    }
    // its armor (`HumanoidArmorLayer`: chest, legs, feet, head; `NO_OVERLAY`), each layer's texture
    // times its colour
    if let Some(pose) = humanoid_pose(k, &parts) {
        for slot in [1, 2, 3, 0] {
            let Some(material) = d.armor[slot] else { continue };
            for (tex, tint) in armor_layers(slot, material) {
                let Some(ar) = rect(g, &tex) else { continue };
                let n = out.len();
                emit(&mut out, &armor_parts(slot, &pose), &m, [64.0, 32.0], ar, tw, th);
                for v in &mut out[n..] {
                    for c in 0..3 {
                        v.color[c] = (v.color[c] as u32 * tint[c] as u32 / 255) as u8;
                    }
                }
            }
        }
    }
    // the item in its right hand (`ItemInHandLayer`, `NO_OVERLAY`): its sprite, flat (the game
    // extrudes a generated item's sprite a pixel deep)
    if let (Some(item), true) = (&d.held, matches!(k, "zombie" | "baby_zombie" | "skeleton") && parts.len() >= 3) {
        let arm = &parts[2];
        let offset = if k == "skeleton" { [arm.offset[0] + 1.0, arm.offset[1], arm.offset[2]] } else { arm.offset };
        let display = held_display(item);
        let hm = held_transform(&m, offset, arm.rot, k == "baby_zombie", &display);
        if let Some(t) = sprite_triangles_of(g, &display.3, &hm) {
            out.extend(t);
        }
    }
    // its eyes layer over it, unshaded, never lit (`RenderTypes.eyes`: no lightmap) and without
    // the red overlay
    if let Some(eyes) = mob_eyes(k).and_then(|e| rect(g, e)) {
        let n = out.len();
        emit(&mut out, &parts, &m, size, eyes, tw, th);
        for v in &mut out[n..] {
            v.color = [255; 4];
            v.light = crate::mesh::EMISSIVE;
        }
    }
    Some(out)
}

/// The game texture UVs of `block/fire_0` and `fire_1` (a burning entity's flames), if packed.
pub fn fire_uvs(g: &GameAssets) -> Option<[[f32; 4]; 2]> {
    let (tw, th) = (g.width as f32, g.height as f32);
    let uvs = |r: [u32; 4]| [(r[0] as f32 + 0.02) / tw, (r[1] as f32 + 0.02) / th, (r[0] as f32 + r[2] as f32 - 0.02) / tw, (r[1] as f32 + r[2].min(r[3]) as f32 - 0.02) / th];
    Some([uvs(rect(g, "block/fire_0")?), uvs(rect(g, "block/fire_1")?)])
}

/// A burning entity's flames (`FlameFeatureRenderer`): squares of `block/fire_0` and `fire_1`
/// turned about the vertical toward the camera (`right`: the camera's horizontal right, `back`:
/// toward the camera), scaled to 1.4 x the box's width, stacked every 0.45 up its height (each 0.9
/// as wide and 0.03 further back, every other pair mirrored), lit by full block light.
pub fn flame_triangles(f: &crate::mesh::Flame, right: [f32; 3], back: [f32; 3]) -> Vec<Vertex> {
    let (pos, s) = (f.pos, f.width * 1.4);
    let mut h = f.height / s;
    let at = |x: f32, y: f32, z: f32| [pos[0] + s * (x * right[0] + z * back[0]), pos[1] + s * y, pos[2] + s * (x * right[2] + z * back[2])];
    let (mut r, mut yo) = (0.5f32, 0.0f32);
    let mut zo = 0.3 - (h as i32) as f32 * 0.02;
    let mut out = Vec::new();
    let mut ss = 0;
    while h > 0.0 {
        let [mut u0, v0, mut u1, v1] = f.uv[ss % 2];
        if (ss / 2) % 2 == 0 {
            std::mem::swap(&mut u0, &mut u1);
        }
        let v = |p: [f32; 3], u: f32, vv: f32| Vertex { pos: p, uv: [u, vv], layer: crate::atlas::GAME_LAYER, color: [255, 255, 255, 255], light: crate::mesh::UNLIT };
        let q = [v(at(-r, -yo, zo), u1, v1), v(at(r, -yo, zo), u0, v1), v(at(r, 1.4 - yo, zo), u0, v0), v(at(-r, 1.4 - yo, zo), u1, v0)];
        out.extend_from_slice(&[q[0], q[1], q[2], q[0], q[2], q[3]]);
        h -= 0.45;
        yo -= 0.45;
        r *= 0.9;
        zo -= 0.03;
        ss += 1;
    }
    out
}

/// `TntRenderer`: a primed TNT's swell over the last 10 ticks of its fuse (`fuse` = the fuse left
/// as drawn: `getFuse() - partialTicks + 1`), and whether it flashes white (every 5 ticks).
pub fn tnt_look(fuse: f32) -> (f32, bool) {
    let swell = if fuse < 10.0 {
        let g = (1.0 - fuse / 10.0).clamp(0.0, 1.0);
        let g = g * g;
        g * g * 0.3
    } else {
        0.0
    };
    (1.0 + swell, fuse >= 0.0 && (fuse / 5.0) as i32 % 2 == 0)
}

/// A minecart's display block transform (`AbstractMinecartRenderer`: 0.75 scale, offset by the
/// display offset, a quarter turn), before the model's flip: the block model's unit cube goes
/// through it.
pub fn minecart_block_transform(pos: [f32; 3], yaw: f32, pitch: f32, display_offset: i32) -> M {
    M::id().translate(pos).translate([0.0, 0.375, 0.0]).rot(1, (180.0 - yaw).to_radians()).rot(2, (-pitch).to_radians()).scale([0.75, 0.75, 0.75]).translate([-0.5, (display_offset - 8) as f32 / 16.0, 0.5]).rot(1, PI / 2.0)
}

/// A dropped item's transform (`ItemEntityRenderer`: bob and spin with its age; `bobOffs` 0)
/// for a block item: the block model scaled by the item model's `ground` display (0.25, 3 px up).
pub fn item_block_transform(pos: [f32; 3], age: f32) -> M {
    let bob = (age / 10.0).sin() * 0.1 + 0.1;
    let spin = age / 20.0;
    // ItemRenderer: display transform (translation [0, 3, 0] px, scale 0.25), then the model's
    // unit cube centred
    M::id().translate(pos).translate([0.0, bob + 0.0625 + 0.125 - 0.1875, 0.0]).rot(1, spin).translate([0.0, 0.1875, 0.0]).scale([0.25, 0.25, 0.25]).translate([-0.5, -0.5, -0.5])
}

/// A dropped item that is not a block: its item sprite as a flat double-sided square (the
/// `ground` display: 0.5 scale, 2 px up), bobbing and spinning.
pub fn item_sprite_triangles(g: &GameAssets, item: &str, pos: [f32; 3], age: f32) -> Option<Vec<Vertex>> {
    let m = M::id().translate(pos).translate([0.0, (age / 10.0).sin() * 0.1 + 0.1 + 0.0625, 0.0]).rot(1, age / 20.0).translate([0.0, 0.125, 0.0]).scale([0.5, 0.5, 0.5]).translate([-0.5, -0.5, 0.0]);
    let name = item.strip_prefix("minecraft:").unwrap_or(item);
    // a flower's item is `item/generated` from its block texture (`models/item/poppy.json`:
    // layer0 `block/poppy`): a golem's poppy
    if !g.tex.contains_key(&format!("item/{name}")) && BLOCK_SPRITE_ITEMS.contains(&name) {
        return sprite_triangles_of(g, &format!("block/{name}"), &m);
    }
    sprite_triangles(g, item, &m)
}

/// Items whose model is `item/generated` from their block's texture (the small flowers), not an
/// `item/` sprite.
pub const BLOCK_SPRITE_ITEMS: &[&str] = &["poppy", "dandelion", "blue_orchid", "allium", "azure_bluet", "red_tulip", "orange_tulip", "white_tulip", "pink_tulip", "oxeye_daisy", "cornflower", "lily_of_the_valley", "wither_rose"];

/// An item frame's transform (`ItemFrameRenderer`) for a frame facing `d` hanging in the cell
/// whose centre is `center`: turned so the `item_frame` block model (built facing north, its back
/// on the south side) faces `d`.
pub fn item_frame_transform(center: [f32; 3], d: redsim::Dir) -> M {
    use redsim::Dir;
    let (x, y) = match d {
        Dir::Up => (-90.0f32, 180.0f32),
        Dir::Down => (90.0, 180.0),
        // 180 - `Direction.toYRot()` (south 0, west 90, north 180, east 270)
        Dir::South => (0.0, 180.0),
        Dir::West => (0.0, 90.0),
        Dir::North => (0.0, 0.0),
        Dir::East => (0.0, -90.0),
    };
    M::id().translate(center).rot(0, x.to_radians()).rot(1, y.to_radians())
}

/// A framed item's transform (`ItemFrameRenderer`, a frame with an item): 0.4375 toward the
/// back, turned `rotation` x 45 degrees about the frame's normal, half size; then the item
/// model's `fixed` display (a half turn about y) and its unit cube centred on x and y (a sprite's
/// plane is the cube's middle).
pub fn framed_item_transform(frame: &M, rotation: u8) -> M {
    frame.translate([0.0, 0.0, 0.4375]).rot(2, (rotation as f32 * 45.0).to_radians()).scale([0.5, 0.5, 0.5]).rot(1, PI).translate([-0.5, -0.5, 0.0])
}

/// An item's sprite as a flat double-sided square: the unit square (z 0, its texture upright
/// seen from +z) through `m`.
pub fn sprite_triangles(g: &GameAssets, item: &str, m: &M) -> Option<Vec<Vertex>> {
    sprite_triangles_of(g, &format!("item/{}", item.strip_prefix("minecraft:").unwrap_or(item)), m)
}

/// [`sprite_triangles`] of a texture of the game texture by its name (`item/...`, `block/...`).
pub fn sprite_triangles_of(g: &GameAssets, name: &str, m: &M) -> Option<Vec<Vertex>> {
    let r = rect(g, name)?;
    let (tw, th) = (g.width as f32, g.height as f32);
    let q = [[0.0, 1.0, 0.0], [0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]].map(|p| m.point(p));
    let uv = |u: f32, v: f32| [(r[0] as f32 + 0.02 + u * (r[2] as f32 - 0.04)) / tw, (r[1] as f32 + 0.02 + v * (r[3] as f32 - 0.04)) / th];
    let n = normalize(m.dir([0.0, 0.0, 1.0]));
    let mut out = Vec::new();
    for (side, nn) in [(false, n), (true, [-n[0], -n[1], -n[2]])] {
        let l = (entity_light(nn) * 255.0).round() as u8;
        let mk = |k: usize, u: f32, v: f32| Vertex { pos: q[k], uv: uv(u, v), layer: crate::atlas::GAME_LAYER, color: [l, l, l, 255], light: crate::mesh::UNLIT };
        let vs = [mk(0, 0.0, 0.0), mk(1, 0.0, 1.0), mk(2, 1.0, 1.0), mk(3, 1.0, 0.0)];
        if side {
            out.extend_from_slice(&[vs[0], vs[2], vs[1], vs[0], vs[3], vs[2]]);
        } else {
            out.extend_from_slice(&[vs[0], vs[1], vs[2], vs[0], vs[2], vs[3]]);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The vendored entity atlas (`web/textures/26.3`): its RGBA, width and index.
    fn vendored_entities() -> (Vec<u8>, u32, String) {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/textures/26.3");
        let dec = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(dir.join("entities.png")).unwrap()));
        let mut r = dec.read_info().unwrap();
        let mut buf = vec![0; r.output_buffer_size().unwrap()];
        let info = r.next_frame(&mut buf).unwrap();
        assert_eq!(info.color_type, png::ColorType::Rgba);
        (buf, info.width, std::fs::read_to_string(dir.join("entities.min.json")).unwrap())
    }

    /// A texture's rect in the vendored index.
    fn index_rect(index: &str, name: &str) -> [u32; 4] {
        let at = index.find(&format!("\"{name}\":[")).unwrap_or_else(|| panic!("{name} not in the index")) + name.len() + 4;
        let v: Vec<u32> = index[at..at + index[at..].find(']').unwrap()].split(',').map(|x| x.parse().unwrap()).collect();
        [v[0], v[1], v[2], v[3]]
    }

    #[test]
    fn a_spiders_eyes_are_opaque_or_empty() {
        // the eyes pipeline blends its layer over the model (TRANSLUCENT): an opaque texel replaces
        // the model's there, an empty one leaves it, so the viewer draws it as solid faces
        let (img, w, index) = vendored_entities();
        for (name, eyes_texels) in [("entity/spider/spider_eyes", 20), ("entity/enderman/enderman_eyes", 6)] {
            let eyes = index_rect(&index, name);
            let mut lit = 0;
            for y in 0..eyes[3] {
                for x in 0..eyes[2] {
                    let a = img[(((eyes[1] + y) * w + eyes[0] + x) * 4 + 3) as usize];
                    assert!(a == 0 || a == 255, "{name} ({x}, {y}): alpha {a}");
                    lit += (a == 255) as u32;
                }
            }
            assert_eq!(lit, eyes_texels, "{name}: its eyes' texels");
        }
    }

    /// The browser's game texture: the block atlas and the vendored entity and item atlases.
    fn browser_game() -> std::rc::Rc<GameAssets> {
        let mut atlas = crate::atlas::Atlas::build();
        let (png, idx, defs, models) = crate::tools::vanilla_assets().unwrap();
        atlas.apply_vanilla_png(&png, &idx).unwrap();
        let (rgba, w, h) = crate::tools::decode_png(&png).unwrap();
        atlas.set_game(&rgba, w, h, &idx, &defs, &models).unwrap();
        let ((ep, ei), (ip, ii)) = crate::tools::vanilla_entity_assets().unwrap();
        let (er, ew, eh) = crate::tools::decode_png(&ep).unwrap();
        let (ir, iw, ih) = crate::tools::decode_png(&ip).unwrap();
        atlas.add_game_entities((&er, ew, eh, &ei), (&ir, iw, ih, &ii)).unwrap();
        atlas.game.clone().unwrap()
    }

    /// A humanoid's armor and the item in its hand (the census: skeletons' bows, zombies' spears and
    /// shovels, leather, golden and copper armor in the terrain farms): each slot's pieces over the
    /// model (`ADULT_ARMOR_PARTS_PER_SLOT`: head 2 boxes, chest 3, legs 3, feet 2; leather twice,
    /// its base and overlay), never under the red overlay; the item a double-sided square at the
    /// right hand, below the shoulder and in front of the body.
    #[test]
    fn a_humanoid_wears_its_armor_and_holds_its_item() {
        let g = browser_game();
        let bare = crate::mesh::MobDraw { eye_standing: 1.74, ..Default::default() };
        let n0 = mob_triangles(&g, "minecraft:zombie", [0.0; 3], &bare).unwrap().len();
        let box_v = 36;
        let gold = crate::mesh::MobDraw { armor: [Some(2); 4], red: true, ..bare.clone() };
        let t = mob_triangles(&g, "minecraft:zombie", [0.0; 3], &gold).unwrap();
        assert_eq!(t.len(), n0 + (2 + 3 + 3 + 2) * box_v, "golden armor: 10 boxes");
        assert!(t[n0..].iter().all(|v| v.layer != crate::atlas::GAME_LAYER_RED) && t[..n0].iter().all(|v| v.layer == crate::atlas::GAME_LAYER_RED), "armor without the red overlay");
        let (lo, hi) = bounds(&t[n0..]);
        let (mlo, mhi) = bounds(&t[..n0]);
        assert!(hi[1] > mhi[1] && lo[0] < mlo[0], "the armor over the model: {lo:?}..{hi:?} vs {mlo:?}..{mhi:?}");
        let leather = crate::mesh::MobDraw { armor: [Some(0), None, None, None], ..bare.clone() };
        assert_eq!(mob_triangles(&g, "minecraft:skeleton", [0.0; 3], &leather).unwrap().len() - mob_triangles(&g, "minecraft:skeleton", [0.0; 3], &bare).unwrap().len(), 2 * 2 * box_v, "a leather helmet: its base and its overlay");
        for item in ["minecraft:iron_shovel", "minecraft:bow", "minecraft:iron_spear", "minecraft:iron_ingot"] {
            let holding = crate::mesh::MobDraw { held: Some(item.into()), ..bare.clone() };
            let t = mob_triangles(&g, "minecraft:zombie", [0.0; 3], &holding).unwrap();
            assert_eq!(t.len(), n0 + 12, "{item}: a double-sided square");
            let (lo, hi) = bounds(&t[n0..]);
            let c = [0, 1, 2].map(|k| (lo[k] + hi[k]) / 2.0);
            // the zombie faces south (+z), its right hand at x < 0, its arms forward
            assert!(c[0] < 0.0 && c[1] > 0.6 && c[1] < 1.9 && c[2] > 0.2, "{item}: at the right hand {c:?}");
        }
    }

    /// The browser's game texture (the blocks, the items, the entities, the particles: as
    /// `mcview-web` packs it) fits the renderer's limits (`Limits::downlevel_webgl2_defaults`:
    /// 2048 on each side, what every WebGL2 phone has): the armor's textures had made it 2080
    /// tall, and wgpu refused it (no picture at all).
    #[test]
    fn the_game_texture_fits_webgl2() {
        let mut atlas = crate::atlas::Atlas::build();
        let (png, idx, defs, models) = crate::tools::vanilla_assets().unwrap();
        atlas.apply_vanilla_png(&png, &idx).unwrap();
        let (rgba, w, h) = crate::tools::decode_png(&png).unwrap();
        atlas.set_game(&rgba, w, h, &idx, &defs, &models).unwrap();
        let blocks = (atlas.game.as_ref().unwrap().width, atlas.game.as_ref().unwrap().height);
        let ((ep, ei), (ip, ii)) = crate::tools::vanilla_entity_assets().unwrap();
        let (er, ew, eh) = crate::tools::decode_png(&ep).unwrap();
        let (ir, iw, ih) = crate::tools::decode_png(&ip).unwrap();
        atlas.add_game_entities((&er, ew, eh, &ei), (&ir, iw, ih, &ii)).unwrap();
        let with_entities = atlas.game.as_ref().unwrap().height;
        atlas.add_game_particles(&crate::tools::vanilla_particle_assets().unwrap()).unwrap();
        let g = atlas.game.clone().unwrap();
        let limit = wgpu::Limits::downlevel_webgl2_defaults().max_texture_dimension_2d;
        eprintln!("blocks {blocks:?}, items {iw}x{ih}, with the entities {with_entities} tall, with the particles {}x{}", g.width, g.height);
        assert!(g.width <= limit && g.height <= limit, "{}x{} over {limit}", g.width, g.height);
    }

    /// Bounds (min, max) of triangles.
    fn bounds(v: &[Vertex]) -> ([f32; 3], [f32; 3]) {
        let lo = [0, 1, 2].map(|k| v.iter().map(|x| x.pos[k]).fold(f32::MAX, f32::min));
        let hi = [0, 1, 2].map(|k| v.iter().map(|x| x.pos[k]).fold(f32::MIN, f32::max));
        (lo, hi)
    }

    /// Each natural mob's model stands like the game's: its lowest and highest points from its
    /// layer definition (the creeper's head top 26 px up; the skeleton's 32 px plus its hat's half
    /// pixel; the enderman's 45 px, its long legs one pixel into the ground, as the game's are).
    #[test]
    fn natural_mobs_stand_like_the_game() {
        let g = browser_game();
        let d = crate::mesh::MobDraw::default();
        for (kind, feet, top) in [
            ("minecraft:creeper", 0.0, 26.0 / 16.0),
            ("minecraft:skeleton", 0.0, 32.5 / 16.0),
            ("minecraft:enderman", -1.0 / 16.0, 45.0 / 16.0),
            ("minecraft:chicken", 0.0, 15.0 / 16.0),
            // its wool's head (grown 0.6) over the model
            ("minecraft:sheep", 0.0, 22.6 / 16.0),
            ("minecraft:baby_zombie", 0.0, 15.15 / 16.0),
        ] {
            let tris = mob_triangles(&g, kind, [0.0; 3], &d).unwrap_or_else(|| panic!("{kind}: no model"));
            let (lo, hi) = bounds(&tris);
            assert!((lo[1] - feet).abs() < 0.002 && (hi[1] - top).abs() < 0.002, "{kind}: {lo:?} .. {hi:?}");
        }
        // the witch's pointed hat; the horse's ears (the neck at 4 px tilted 30 degrees: the ear's
        // corner at y -13, z 5 rises to 33.76 px)
        for (kind, top) in [("minecraft:witch", 2.5..2.7), ("minecraft:zombie_horse", 2.1..2.12)] {
            let (lo, hi) = bounds(&mob_triangles(&g, kind, [0.0; 3], &d).unwrap_or_else(|| panic!("{kind}: no model")));
            eprintln!("{kind}: {lo:?} .. {hi:?}");
            assert!(lo[1].abs() < 0.002 && top.contains(&hi[1]), "{kind}: {lo:?} .. {hi:?}");
        }
        // a dyed sheep: its undercoat and wool tinted by the dye (orange: 249,128,29 x 0.75)
        let plain = mob_triangles(&g, "minecraft:sheep", [0.0; 3], &d).unwrap();
        let dyed = mob_triangles(&g, "minecraft:sheep", [0.0; 3], &crate::mesh::MobDraw { color: 1, ..Default::default() }).unwrap();
        assert_eq!(dyed.len(), plain.len() + 6 * 6 * 6, "the undercoat: the body's 6 boxes again");
        assert_eq!(wool_tint(1), [186, 96, 21]);
        assert_eq!(wool_tint(0), [230, 230, 230]);
        // a bat: flying, its wings beat over the half second; resting, hung upside down
        let at = |resting: bool, age: f32| mob_triangles(&g, "minecraft:bat", [0.0; 3], &crate::mesh::MobDraw { resting, age, ..Default::default() }).unwrap();
        let (a, b) = (at(false, 0.0), at(false, 2.5));
        assert!(a.iter().zip(&b).any(|(x, y)| x.pos != y.pos), "the wings move");
        assert_eq!(at(false, 0.0).iter().map(|v| v.pos).collect::<Vec<_>>(), at(false, 10.0).iter().map(|v| v.pos).collect::<Vec<_>>(), "a 10-tick loop");
        let (lo, hi) = bounds(&at(true, 0.0));
        eprintln!("resting bat: {lo:?} .. {hi:?}");
        assert!(hi[1] < 0.9 + 0.01 && lo[1] > 0.0, "inside its 0.9-high box: {lo:?} .. {hi:?}");
    }

    /// A spider and a cave spider drawn with the game's model: on their feet (the legs' tips at
    /// the ground), the spider's body 13 px tall (the head's and the abdomen's tops at 13 px), its
    /// legs reaching about a block to each side (the middle legs: 4 + 15 cos(0.58) cos(pi/8) px,
    /// wider than its 1.4-block box); the cave spider 0.7 of it; each with its eyes layer (the
    /// model again, unshaded) over it.
    #[test]
    fn a_spider_is_drawn_with_the_games_model() {
        let g = browser_game();
        let d = crate::mesh::MobDraw::default();
        for (kind, scale) in [("minecraft:spider", 1.0f32), ("minecraft:cave_spider", 0.7)] {
            let tris = mob_triangles(&g, kind, [0.0; 3], &d).unwrap_or_else(|| panic!("{kind}: no model"));
            // 11 boxes x 6 faces x 2 triangles, then the eyes layer's as many
            assert_eq!(tris.len(), 11 * 6 * 6 * 2, "{kind}");
            let (base, eyes) = tris.split_at(tris.len() / 2);
            assert!(eyes.iter().all(|v| v.color == [255; 4]) && base.iter().any(|v| v.color[0] < 255), "{kind}: the eyes unshaded over the shaded model");
            // the model lit by its light, the eyes never (`Builder::entity` lights only `UNLIT`)
            assert!(eyes.iter().all(|v| v.light == crate::mesh::EMISSIVE) && base.iter().all(|v| v.light == crate::mesh::UNLIT), "{kind}: the eyes emissive");
            assert!(base.iter().zip(eyes).all(|(a, b)| a.pos == b.pos), "{kind}: the eyes layer on the model's own faces");
            let (lo, hi) = bounds(base);
            eprintln!("{kind}: {lo:?} .. {hi:?}");
            assert!(lo[1].abs() < 0.04 * scale, "{kind}: feet at {}", lo[1]);
            assert!((hi[1] - 13.0 / 16.0 * scale).abs() < 0.01, "{kind}: top at {}", hi[1]);
            let span = hi[0] - lo[0];
            assert!((span - 2.056 * scale).abs() < 0.01, "{kind}: legs span {span}");
        }
        // walking: the legs move, the body stays
        let walk = crate::mesh::MobDraw { walk_pos: 3.0, walk_speed: 1.0, ..Default::default() };
        let (still, moving) = (mob_triangles(&g, "minecraft:spider", [0.0; 3], &d).unwrap(), mob_triangles(&g, "minecraft:spider", [0.0; 3], &walk).unwrap());
        let body = 3 * 6 * 6;
        assert_eq!(still[..body].iter().map(|v| v.pos).collect::<Vec<_>>(), moving[..body].iter().map(|v| v.pos).collect::<Vec<_>>());
        assert!(still[body..].iter().zip(&moving[body..]).any(|(a, b)| a.pos != b.pos));
    }

    #[test]
    fn transforms_like_joml() {
        // rotationZYX order: x first, then y, then z
        let m = M::id().rot(2, PI / 2.0).rot(1, 0.0).rot(0, PI / 2.0);
        let p = m.point([0.0, 1.0, 0.0]);
        // x turns +y to +z, then z leaves +z
        assert!((p[2] - 1.0).abs() < 1e-5, "{p:?}");
        assert!((entity_light([0.0, 1.0, 0.0]) - 1.0).abs() < 1e-6);
        assert!(entity_light([0.0, -1.0, 0.0]) < 0.41);
    }

    /// The boat's hull spans 1.75 blocks along its length, sits on y 0.375 - 3/16 .. 0.375 +
    /// 6/16 + ..., and its faces point outward (counter-clockwise from outside).
    #[test]
    fn boat_geometry() {
        let mut out = Vec::new();
        let m = M::id().translate([0.0, 0.375, 0.0]).rot(1, PI).scale([-1.0, -1.0, 1.0]).rot(1, PI / 2.0);
        emit(&mut out, &boat(false), &m, [128.0, 64.0], [0, 0, 128, 64], 1024.0, 1024.0);
        let lo = (0..3).map(|k| out.iter().map(|v| v.pos[k]).fold(f32::MAX, f32::min)).collect::<Vec<_>>();
        let hi = (0..3).map(|k| out.iter().map(|v| v.pos[k]).fold(f32::MIN, f32::max)).collect::<Vec<_>>();
        eprintln!("boat bounds {lo:?} .. {hi:?}, {} triangles", out.len() / 3);
        assert!(hi[1] > 0.5 && lo[1] > -0.2 && lo[1] < 0.3, "{lo:?} {hi:?}");
        // the hull's bottom plank alone: its lower face (y lowest) faces down
        let mut hull = Vec::new();
        emit(&mut hull, &boat(false)[..1], &m, [128.0, 64.0], [0, 0, 128, 64], 1024.0, 1024.0);
        let low = hull.iter().map(|v| v.pos[1]).fold(f32::MAX, f32::min);
        let bottom = hull.chunks(3).find(|t| t.iter().all(|v| (v.pos[1] - low).abs() < 1e-4)).expect("a bottom triangle");
        let (a, b, c) = (bottom[0].pos, bottom[1].pos, bottom[2].pos);
        let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let ny = e1[2] * e2[0] - e1[0] * e2[2];
        assert!(ny < 0.0, "the hull's underside faces down ({ny})");
        // the hull is 28 px long (along z at yaw 0: the boat faces south) and 16 px wide
        let span = |k: usize| hull.iter().map(|v| v.pos[k]).fold(f32::MIN, f32::max) - hull.iter().map(|v| v.pos[k]).fold(f32::MAX, f32::min);
        assert!((span(2) - 1.75).abs() < 1e-3 && (span(0) - 1.0).abs() < 1e-3, "{} x {}", span(0), span(2));
    }
}
