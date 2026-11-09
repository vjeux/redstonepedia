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

/// `HumanoidModel.createMesh(NONE, 0)` (the zombie's layer, texture 64 x 64) posed by
/// `HumanoidModel.setupAnim` and `ZombieModel.setupAttackAnimation`: the head's yaw and pitch, the
/// walk, a passenger's legs forward (`isPassenger`), the zombie's arms held out
/// (`AnimationUtils.animateZombieArms`, not aggressive: the farm's zombie targets nothing it can
/// reach) and bobbing with its age (`bobModelPart`).
fn zombie(d: &crate::mesh::MobDraw) -> Vec<EPart> {
    let (pos, speed) = (d.walk_pos, d.walk_speed);
    let (hx, hy) = (d.head_pitch.to_radians(), d.head_yaw.to_radians());
    let mut right_leg = [(pos * 0.6662).cos() * 1.4 * speed, 0.005, 0.005];
    let mut left_leg = [(pos * 0.6662 + PI).cos() * 1.4 * speed, -0.005, -0.005];
    if d.riding {
        right_leg = [-1.4137167, PI / 10.0, 0.07853982];
        left_leg = [-1.4137167, -PI / 10.0, -0.07853982];
    }
    // animateZombieArms (an adult: arms raised to -pi / 2.25; no swing), then the arms' bob
    let arm_x = -PI / 2.25;
    let bob = |sign: f32| [sign * ((d.age * 0.067).sin() * 0.05), 0.0, sign * ((d.age * 0.09).cos() * 0.05 + 0.05)];
    let (rb, lb) = (bob(1.0), bob(-1.0));
    let right_arm = [arm_x + rb[0], -0.1, rb[2]];
    let left_arm = [arm_x + lb[0], 0.1, lb[2]];
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

/// The game texture each mob is drawn with (with its overlay layers composited in once the atlas
/// is packed, `Atlas::add_game_entities`): an iron golem's base under its crackiness
/// (`IronGolemCrackinessLayer`), a villager's base under its plains type (`VillagerProfessionLayer`:
/// no profession, so the type's layer covers the whole model, hat included), a zombie's own.
pub fn mob_texture(kind: &str, crack: u8) -> Option<(String, [f32; 2])> {
    match kind.strip_prefix("minecraft:").unwrap_or(kind) {
        "iron_golem" => Some((format!("mob/iron_golem/{}", crack.min(3)), [128.0, 128.0])),
        "villager" => Some(("mob/villager/plains".to_string(), [64.0, 64.0])),
        "zombie" => Some(("entity/zombie/zombie".to_string(), [64.0, 64.0])),
        _ => None,
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

/// A mob drawn like the game (`LivingEntityRenderer.submit` with its renderer's model and
/// `setupRotations`): a sleeper moved toward its pillow and laid on its bed, a dying mob falling
/// sideways over 20 ticks, a walking golem's sway, the model flipped and lowered 1.501, then the
/// red overlay of a hurt or dying mob (`OverlayTexture`: 30% red, [`crate::atlas::GAME_LAYER_RED`]).
/// None: not a mob the viewer has, or its texture is not packed.
pub fn mob_triangles(g: &GameAssets, kind: &str, pos: [f32; 3], d: &crate::mesh::MobDraw) -> Option<Vec<Vertex>> {
    let k = kind.strip_prefix("minecraft:").unwrap_or(kind);
    let (tex, size) = mob_texture(k, d.crack)?;
    let r = rect(g, &tex)?;
    let parts = match k {
        "iron_golem" => iron_golem(d),
        "villager" => villager(d),
        "zombie" => zombie(d),
        _ => return None,
    };
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
        m = m.rot(2, (fall * 90.0).to_radians());
    } else if let Some(dir) = d.sleeping {
        m = m.rot(1, sleep_rotation(dir).to_radians()).rot(2, PI / 2.0).rot(1, 270f32.to_radians());
    }
    // IronGolemRenderer.setupRotations: a walking golem sways
    if k == "iron_golem" && d.walk_speed >= 0.01 {
        let wp = d.walk_pos + 6.0;
        let tri = ((wp % 13.0 - 6.5).abs() - 3.25) / 3.25;
        m = m.rot(2, (6.5 * tri).to_radians());
    }
    let m = m.scale([-1.0, -1.0, 1.0]).translate([0.0, -1.501, 0.0]);
    let (tw, th) = (g.width as f32, g.height as f32);
    let mut out = Vec::new();
    emit(&mut out, &parts, &m, size, r, tw, th);
    if d.red {
        for v in &mut out {
            v.layer = crate::atlas::GAME_LAYER_RED;
            v.color[3] = 178;
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
