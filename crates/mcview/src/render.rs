//! The wgpu renderer: one device, one pipeline pair (solid with alpha test, translucent with
//! blending), the procedural atlas with its mip levels, a sky gradient and distance fog. It draws
//! into any texture view: the browser's canvas surface (WebGL2) or, natively, an offscreen
//! texture read back for PNG shots (Vulkan, headless).

use crate::atlas::{Atlas, MIPS, TILE};
use crate::camera::{Camera, Mat4};
use crate::mesh::Vertex;
use wgpu::util::DeviceExt;

const SHADER: &str = r#"
struct U {
    vp: mat4x4<f32>,
    eye: vec4<f32>,
    fog: vec4<f32>,      // start, end
    sky_top: vec4<f32>,
    sky_low: vec4<f32>,
    inv_vp: mat4x4<f32>,
    sky_void: vec4<f32>,
    // the lightmap's inputs (`lightmap.fsh`): the sky light's colour and factor, the ambient
    // colour and the block factor, the block light's tint and the brightness
    sky_light: vec4<f32>,
    ambient: vec4<f32>,
    block_tint: vec4<f32>,
    // the sky's bodies (`SkyRenderer`): the sun's direction and the stars' brightness, the moon's
    // direction and whether they are drawn, the sunrise colour, the star field's turn (cos, sin)
    // and the rain's dimming of them
    sun: vec4<f32>,
    moon: vec4<f32>,
    sunrise: vec4<f32>,
    stars: vec4<f32>,
};
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var tex: texture_2d_array<f32>;
@group(0) @binding(2) var samp: sampler;
// the game's block atlas (faces of the game's own models: layer 65535)
@group(0) @binding(3) var gtex: texture_2d<f32>;

fn texel(uv: vec2<f32>, layer: u32) -> vec4<f32> {
    // layer 65535 (and 65534: the same with the game's red overlay) samples the game texture
    let game = layer >= 65534u;
    // both sampled (uniform control flow), one kept
    let a = textureSample(tex, samp, uv, select(layer, 0u, game));
    let b = textureSample(gtex, samp, uv);
    return select(a, b, game);
}

/// One texel of the game's lightmap at whole light levels (`lightmap.fsh`: the sky light scaled
/// by the day's factor and coloured, the block light warm and flickerless, the ambient colour,
/// then half way to the brightened curve).
fn lightmap_texel(sky: f32, block: f32) -> vec3<f32> {
    let bl = block / 15.0;
    let sl = sky / 15.0;
    let block_b = bl / (4.0 - 3.0 * bl) * u.ambient.w;
    let sky_b = sl / (4.0 - 3.0 * sl) * u.sky_light.w;
    let pm = (2.0 * bl - 1.0) * (2.0 * bl - 1.0);
    let blc = u.block_tint.rgb + (vec3<f32>(1.0) - u.block_tint.rgb) * 0.9 * pm;
    var c = clamp(u.ambient.rgb + u.sky_light.rgb * sky_b + blc * block_b, vec3<f32>(0.0), vec3<f32>(1.0));
    let mx = max(c.r, max(c.g, c.b));
    if (mx > 0.0) {
        let inv = 1.0 - mx;
        let scaled = 1.0 - inv * inv * inv * inv;
        c = c + (c * scaled / mx - c) * u.block_tint.w;
    }
    return c;
}

/// The lightmap at fractional levels (a smooth-lit corner's mean), between texels like the
/// game's linear sampler.
fn lightmap(sky: f32, block: f32) -> vec3<f32> {
    let s = clamp(sky, 0.0, 15.0);
    let b = clamp(block, 0.0, 15.0);
    let s0 = floor(s);
    let b0 = floor(b);
    let s1 = min(s0 + 1.0, 15.0);
    let b1 = min(b0 + 1.0, 15.0);
    let lo = mix(lightmap_texel(s0, b0), lightmap_texel(s0, b1), b - b0);
    let hi = mix(lightmap_texel(s1, b0), lightmap_texel(s1, b1), b - b0);
    return mix(lo, hi, s - s0);
}

struct VOut {
    @builtin(position) pos: vec4<f32>,
    // (sampled at the centroid of the samples a pixel's triangle covers, not at the pixel's centre
    // outside it: an edge pixel read the sprite next to its face in the atlas, a line of that
    // colour along block edges, or a texel the alpha test drops, a hole there)
    @location(0) @interpolate(perspective, centroid) uv: vec2<f32>,
    @location(1) @interpolate(perspective, centroid) color: vec4<f32>,
    @location(2) @interpolate(perspective, centroid) world: vec3<f32>,
    @location(3) @interpolate(flat) layer: u32,
    // the vertex's lightmap (1 unlit): the game mixes a mob's red overlay (a TNT's white) into its
    // colour before the lightmap (`entity.fsh`), so a hurt mob in the dark flashes dark red
    @location(4) @interpolate(perspective, centroid) lm: vec3<f32>,
};

@vertex
fn vs(@location(0) pos: vec3<f32>, @location(1) uv: vec2<f32>, @location(2) layer: u32, @location(3) color: vec4<f32>, @location(4) light: vec4<f32>) -> VOut {
    var o: VOut;
    o.pos = u.vp * vec4<f32>(pos, 1.0);
    // an X-ray face (the layer's top bit, `mesh::XRAY`: a mob farm's spawn spots) at the nearest
    // depth, so the blocks in front never hide it
    if ((layer & 0x80000000u) != 0u) {
        o.pos.z = o.pos.w * 0.0001;
    }
    o.uv = uv;
    o.color = color;
    o.lm = vec3<f32>(1.0, 1.0, 1.0);
    // a lit vertex (`mesh::lit`: its sky and block light x 16): the frame's lightmap
    if (light.z > 0.0) {
        o.lm = lightmap(light.x * 255.0 / 16.0, light.y * 255.0 / 16.0);
        o.color = vec4<f32>(color.rgb * o.lm, color.a);
    }
    o.world = pos;
    o.layer = layer & 0x7fffffffu;
    return o;
}

/// The game's fog (`fog.glsl`): the environment's (the overworld's: linear over the spherical
/// distance from 0 to 1024 blocks) and the render distance's (linear over the cylindrical
/// distance, `max(|xz|, |y|)`, from `FogRenderer`'s start to the render distance), the larger.
fn fogged(c: vec3<f32>, world: vec3<f32>) -> vec3<f32> {
    let rel = world - u.eye.xyz;
    let spherical = length(rel);
    let cylindrical = max(length(rel.xz), abs(rel.y));
    let env = clamp(spherical / max(u.fog.w, 0.001), 0.0, 1.0);
    let rd = clamp((cylindrical - u.fog.x) / max(u.fog.y - u.fog.x, 0.001), 0.0, 1.0);
    return mix(c, u.sky_low.rgb, max(env, rd));
}

@fragment
fn fs_solid(i: VOut) -> @location(0) vec4<f32> {
    let t = texel(i.uv, i.layer);
    if (t.a < 0.5) {
        discard;
    }
    // the game draws only the chunks within its render distance of the camera's chunk (a square):
    // the sky shows beyond (the pictures' band over the horizon)
    if (u.fog.z > 0.0) {
        let c = floor(u.eye.xz / 16.0);
        let f = floor(i.world.xz / 16.0);
        if (abs(f.x - c.x) > u.fog.z || abs(f.y - c.y) > u.fog.z) {
            discard;
        }
    }
    // a white flash (a primed TNT) rides in the vertex alpha of solid faces: 1 = none; a hurt or
    // dying mob's red overlay the same way (layer 65534: `OverlayTexture`'s red), lit like the
    // texel it mixes into
    let flash = select(vec3<f32>(1.0, 1.0, 1.0), vec3<f32>(1.0, 0.0, 0.0), i.layer == 65534u);
    let lit = mix(t.rgb * i.color.rgb, flash * i.lm, 1.0 - i.color.a);
    return vec4<f32>(fogged(lit, i.world), 1.0);
}

@fragment
fn fs_blend(i: VOut) -> @location(0) vec4<f32> {
    let t = texel(i.uv, i.layer);
    return vec4<f32>(fogged(t.rgb * i.color.rgb, i.world), t.a * i.color.a);
}

struct SkyOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

@vertex
fn vs_sky(@builtin(vertex_index) i: u32) -> SkyOut {
    var p = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
    var o: SkyOut;
    o.pos = vec4<f32>(p[i], 1.0, 1.0);
    o.ndc = p[i];
    return o;
}

@fragment
fn fs_sky(i: SkyOut) -> @location(0) vec4<f32> {
    // the view ray's elevation picks the gradient (horizon light, zenith deeper blue)
    let far = u.inv_vp * vec4<f32>(i.ndc, 1.0, 1.0);
    let dir = normalize(far.xyz / far.w - u.eye.xyz);
    let t = clamp(dir.y * 1.6 + 0.1, 0.0, 1.0);
    var c = mix(u.sky_low.rgb, u.sky_top.rgb, t);
    // the game's day (the live view of a run: `daylight::Env`): the sunrise fan on the sun's
    // side of the horizon, the stars, the sun and the moon (drawn additively, dimmed by rain)
    if (u.moon.w > 0.5) {
        let side = normalize(vec3<f32>(u.sun.x, 0.0, 0.0) + vec3<f32>(0.0001, 0.0, 0.0));
        let toward = max(dot(normalize(vec3<f32>(dir.x, 0.0, dir.z)), side), 0.0);
        let fan = u.sunrise.a * toward * toward * toward * (1.0 - smoothstep(0.0, 0.45, abs(dir.y)));
        c = mix(c, u.sunrise.rgb, clamp(fan, 0.0, 1.0));
        // the stars turn with the sky (about the north-south axis, like the sun): a fixed field,
        // hashed in the sky's own frame
        let sd = vec3<f32>(dir.x * u.stars.x + dir.y * u.stars.y, -dir.x * u.stars.y + dir.y * u.stars.x, dir.z);
        let cell = floor(sd * 180.0);
        let h = fract(sin(dot(cell, vec3<f32>(12.9898, 78.233, 37.719))) * 43758.5453);
        if (h > 0.9975) {
            c = c + vec3<f32>(u.sun.w * u.stars.z);
        }
        // the sun (30 wide at 100 away: a square of half-size 0.3 in its plane) and the moon (20)
        let bodies = array<vec4<f32>, 2>(vec4<f32>(u.sun.xyz, 0.3), vec4<f32>(u.moon.xyz, 0.2));
        for (var k = 0; k < 2; k = k + 1) {
            let b = bodies[k];
            let along = dot(dir, b.xyz);
            if (along > 0.0) {
                // its plane's axes: north-south, and the one across (`SkyRenderer`'s rotations)
                let ax = vec3<f32>(0.0, 0.0, 1.0);
                let ay = normalize(cross(b.xyz, ax));
                let p = vec2<f32>(dot(dir, ax), dot(dir, ay)) / along;
                let m = max(abs(p.x), abs(p.y)) / b.w;
                if (m < 1.0) {
                    if (k == 0) {
                        // the sun's texture: a bright core, its glow around
                        let core = 1.0 - smoothstep(0.42, 0.5, m);
                        let glow = (1.0 - m) * 0.55;
                        c = c + vec3<f32>(1.0, 0.95, 0.7) * max(core, glow) * u.stars.z;
                    } else {
                        let core = 1.0 - smoothstep(0.4, 0.5, m);
                        c = c + vec3<f32>(0.75, 0.78, 0.85) * core * u.stars.z;
                    }
                }
            }
        }
        c = min(c, vec3<f32>(1.0));
    }
    // under the horizon in the viewer (no ground under a design: the judges' void): the sky fades
    // into a quiet void tone, no surface
    if (u.sky_void.a > 0.5 && dir.y < 0.0) {
        return vec4<f32>(mix(c, u.sky_void.rgb, clamp(-dir.y * 1.4, 0.0, 1.0)), 1.0);
    }
    return vec4<f32>(c, 1.0);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    vp: Mat4,
    eye: [f32; 4],
    fog: [f32; 4],
    sky_top: [f32; 4],
    sky_low: [f32; 4],
    inv_vp: Mat4,
    /// The void's tone under the horizon (alpha 0: the pictures' sky, unchanged).
    sky_void: [f32; 4],
    sky_light: [f32; 4],
    ambient: [f32; 4],
    block_tint: [f32; 4],
    sun: [f32; 4],
    moon: [f32; 4],
    sunrise: [f32; 4],
    stars: [f32; 4],
}

/// What one frame draws.
pub struct Frame<'a> {
    pub solid: &'a [Vertex],
    pub translucent: &'a [Vertex],
    pub camera: Camera,
    pub width: u32,
    pub height: u32,
}

pub const SAMPLES: u32 = 4;

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub format: wgpu::TextureFormat,
    pub samples: u32,
    solid: wgpu::RenderPipeline,
    blend: wgpu::RenderPipeline,
    sky: wgpu::RenderPipeline,
    /// The sky drawn first under everything (the order of before: `sky_first`, for a check that
    /// drawing it last changes no pixel: `tools::frame_cost`).
    sky_always: wgpu::RenderPipeline,
    pub sky_first: bool,
    uniforms: wgpu::Buffer,
    bind: wgpu::BindGroup,
    /// Multisampled color + depth targets for the current size.
    targets: Option<(u32, u32, wgpu::Texture, wgpu::Texture)>,
    vbuf: Option<(wgpu::Buffer, u64)>,
    /// Solid triangles kept on the GPU between frames ([`Renderer::set_fixed`]: a mob farm's
    /// blocks), by their key, and how many vertices.
    fixed: Option<(u64, wgpu::Buffer, u32)>,
    /// The kept triangles a camera can see ([`Renderer::cull_fixed`]: [`crate::occlude`]), drawn
    /// in place of all of them while that camera, frame size and kept vertices hold: (the kept
    /// vertices' key, the camera, width, height), the index buffer and its count.
    pub(crate) fixed_visible: Option<((u64, Camera, u32, u32), wgpu::Buffer, u32)>,
    /// The same as a list (a cull carried to the next kept vertices: [`Renderer::carry_cull`]),
    /// and whether the one drawn is carried (a superset: the exact cull still runs).
    fixed_visible_cpu: Option<((u64, Camera, u32, u32), std::rc::Rc<Vec<u32>>)>,
    fixed_provisional: bool,
    /// The still translucent triangles kept on the GPU with them ([`Renderer::set_fixed_blend`]:
    /// a mob farm's water, sorted back to front for the camera), by their key, and how many
    /// vertices; and those a camera can see ([`Renderer::cull_fixed`]: by the kept vertices' key,
    /// theirs, the camera, width and height).
    fixed_blend: Option<(u64, wgpu::Buffer, u32)>,
    pub(crate) fixed_blend_visible: Option<((u64, u64, Camera, u32, u32), wgpu::Buffer, u32)>,
    /// The same as a list (carried with the kept vertices' cull), and the triangles the last
    /// exact cull and the last carried one kept (a carried cull close to the exact one is kept
    /// as it is: [`Renderer::cull_fixed`]).
    fixed_blend_visible_cpu: Option<std::rc::Rc<Vec<u32>>>,
    last_exact_kept: usize,
    carried_kept: usize,
    /// The occluders' side of the kept vertices ([`crate::occlude::Occluders`]), by their key.
    occluders: Option<(u64, std::rc::Rc<crate::occlude::Occluders>)>,
    /// The visible set being worked out, for its kept vertices, translucent ones, camera and size.
    cull: Option<((u64, u64, Camera, u32, u32), crate::occlude::Cull)>,
    /// The tiles' texture array.
    tiles: wgpu::Texture,
    tiles_view: wgpu::TextureView,
    bgl: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// The game texture and which game assets it holds (their address; 0 = none yet).
    game_tex: wgpu::Texture,
    game_of: usize,
    /// The client's render distance in chunks at least (None: the real-game pictures' 6, which
    /// `web compare` matches). The browser viewer draws farther: the fogged end of the ground
    /// stays at the horizon instead of a curved band under it.
    pub view_chunks: Option<f32>,
    /// The viewer's sky under the horizon: a void tone (it draws no ground under a design);
    /// false: the pictures' sky (`web compare`).
    pub void_below: bool,
    /// The game's day (a live view of a run: its time and weather, `daylight::at`): the sky's
    /// colours, the sun, the moon and the stars, and the lightmap of every lit vertex. None: the
    /// pictures' noon sky and light.
    pub env: Option<crate::daylight::Env>,
}

/// The game texture of an atlas (its game assets' mip levels), or a 1x1 placeholder.
fn game_texture(device: &wgpu::Device, queue: &wgpu::Queue, atlas: &Atlas) -> (wgpu::Texture, usize) {
    let (w, h, mips, of): (u32, u32, Vec<&[u8]>, usize) = match &atlas.game {
        Some(g) => (g.width, g.height, g.mips.iter().map(|m| m.as_slice()).collect(), std::rc::Rc::as_ptr(g) as usize),
        None => (1, 1, vec![&[255u8, 255, 255, 255][..]], 0),
    };
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("game"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: mips.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (k, data) in mips.iter().enumerate() {
        let (lw, lh) = ((w >> k).max(1), (h >> k).max(1));
        queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: k as u32, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            data,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * lw), rows_per_image: Some(lh) },
            wgpu::Extent3d { width: lw, height: lh, depth_or_array_layers: 1 },
        );
    }
    (tex, of)
}

/// 4x4 inverse (general; the view-projection is always invertible here).
fn invert(m: &Mat4) -> Mat4 {
    let a: Vec<f32> = (0..16).map(|i| m[i / 4][i % 4]).collect();
    let mut inv = [0f32; 16];
    inv[0] = a[5] * a[10] * a[15] - a[5] * a[11] * a[14] - a[9] * a[6] * a[15] + a[9] * a[7] * a[14] + a[13] * a[6] * a[11] - a[13] * a[7] * a[10];
    inv[4] = -a[4] * a[10] * a[15] + a[4] * a[11] * a[14] + a[8] * a[6] * a[15] - a[8] * a[7] * a[14] - a[12] * a[6] * a[11] + a[12] * a[7] * a[10];
    inv[8] = a[4] * a[9] * a[15] - a[4] * a[11] * a[13] - a[8] * a[5] * a[15] + a[8] * a[7] * a[13] + a[12] * a[5] * a[11] - a[12] * a[7] * a[9];
    inv[12] = -a[4] * a[9] * a[14] + a[4] * a[10] * a[13] + a[8] * a[5] * a[14] - a[8] * a[6] * a[13] - a[12] * a[5] * a[10] + a[12] * a[6] * a[9];
    inv[1] = -a[1] * a[10] * a[15] + a[1] * a[11] * a[14] + a[9] * a[2] * a[15] - a[9] * a[3] * a[14] - a[13] * a[2] * a[11] + a[13] * a[3] * a[10];
    inv[5] = a[0] * a[10] * a[15] - a[0] * a[11] * a[14] - a[8] * a[2] * a[15] + a[8] * a[3] * a[14] + a[12] * a[2] * a[11] - a[12] * a[3] * a[10];
    inv[9] = -a[0] * a[9] * a[15] + a[0] * a[11] * a[13] + a[8] * a[1] * a[15] - a[8] * a[3] * a[13] - a[12] * a[1] * a[11] + a[12] * a[3] * a[9];
    inv[13] = a[0] * a[9] * a[14] - a[0] * a[10] * a[13] - a[8] * a[1] * a[14] + a[8] * a[2] * a[13] + a[12] * a[1] * a[10] - a[12] * a[2] * a[9];
    inv[2] = a[1] * a[6] * a[15] - a[1] * a[7] * a[14] - a[5] * a[2] * a[15] + a[5] * a[3] * a[14] + a[13] * a[2] * a[7] - a[13] * a[3] * a[6];
    inv[6] = -a[0] * a[6] * a[15] + a[0] * a[7] * a[14] + a[4] * a[2] * a[15] - a[4] * a[3] * a[14] - a[12] * a[2] * a[7] + a[12] * a[3] * a[6];
    inv[10] = a[0] * a[5] * a[15] - a[0] * a[7] * a[13] - a[4] * a[1] * a[15] + a[4] * a[3] * a[13] + a[12] * a[1] * a[7] - a[12] * a[3] * a[5];
    inv[14] = -a[0] * a[5] * a[14] + a[0] * a[6] * a[13] + a[4] * a[1] * a[14] - a[4] * a[2] * a[13] - a[12] * a[1] * a[6] + a[12] * a[2] * a[5];
    inv[3] = -a[1] * a[6] * a[11] + a[1] * a[7] * a[10] + a[5] * a[2] * a[11] - a[5] * a[3] * a[10] - a[9] * a[2] * a[7] + a[9] * a[3] * a[6];
    inv[7] = a[0] * a[6] * a[11] - a[0] * a[7] * a[10] - a[4] * a[2] * a[11] + a[4] * a[3] * a[10] + a[8] * a[2] * a[7] - a[8] * a[3] * a[6];
    inv[11] = -a[0] * a[5] * a[11] + a[0] * a[7] * a[9] + a[4] * a[1] * a[11] - a[4] * a[3] * a[9] - a[8] * a[1] * a[7] + a[8] * a[3] * a[5];
    inv[15] = a[0] * a[5] * a[10] - a[0] * a[6] * a[9] - a[4] * a[1] * a[10] + a[4] * a[2] * a[9] + a[8] * a[1] * a[6] - a[8] * a[2] * a[5];
    let det = a[0] * inv[0] + a[1] * inv[4] + a[2] * inv[8] + a[3] * inv[12];
    let d = if det.abs() < 1e-20 { 0.0 } else { 1.0 / det };
    let mut out = [[0f32; 4]; 4];
    for i in 0..16 {
        out[i / 4][i % 4] = inv[i] * d;
    }
    out
}

impl Renderer {
    /// Build the pipelines and upload the atlas for a device whose output format is `format`,
    /// drawing with `samples` (1 or 4) samples per pixel.
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, format: wgpu::TextureFormat, atlas: &Atlas, samples: u32) -> Renderer {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("mcview"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
        let layers = atlas.layers as u32;
        let tile = TILE as u32;
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tiles"),
            size: wgpu::Extent3d { width: tile, height: tile, depth_or_array_layers: layers },
            mip_level_count: MIPS as u32,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (k, data) in atlas.mips.iter().enumerate() {
            let s = (tile >> k).max(1);
            queue.write_texture(
                wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: k as u32, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                data,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * s), rows_per_image: Some(s) },
                wgpu::Extent3d { width: s, height: s, depth_or_array_layers: layers },
            );
        }
        let view = tex.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2Array), ..Default::default() });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("tiles"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            lod_max_clamp: (MIPS - 1) as f32,
            ..Default::default()
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor { label: Some("uniforms"), size: std::mem::size_of::<Uniforms>() as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mcview"),
            entries: &[
                wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::VERTEX_FRAGMENT, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2Array, multisampled: false }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 2, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
                wgpu::BindGroupLayoutEntry { binding: 3, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }, count: None },
            ],
        });
        let (game_tex, game_of) = game_texture(&device, &queue, atlas);
        let bind = Self::bind_group(&device, &bgl, &uniforms, &view, &sampler, &game_tex);
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("mcview"), bind_group_layouts: &[Some(&bgl)], immediate_size: 0 });
        let vlayout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 0, shader_location: 0 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 12, shader_location: 1 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Uint32, offset: 20, shader_location: 2 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Unorm8x4, offset: 24, shader_location: 3 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Unorm8x4, offset: 28, shader_location: 4 },
            ],
        };
        let ms = wgpu::MultisampleState { count: samples, mask: !0, alpha_to_coverage_enabled: false };
        let pipeline = |fs: &str, blend: bool, label: &str| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[Some(vlayout.clone())] },
                // back faces culled in both passes, like the game's chunk render types (translucent
                // included: a glass block shows its three near faces, never its far ones from inside)
                primitive: wgpu::PrimitiveState { cull_mode: Some(wgpu::Face::Back), ..Default::default() },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth24Plus,
                    depth_write_enabled: Some(!blend),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: ms,
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format, blend: if blend { Some(wgpu::BlendState::ALPHA_BLENDING) } else { None }, write_mask: wgpu::ColorWrites::ALL })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let solid = pipeline("fs_solid", false, "solid");
        let blend = pipeline("fs_blend", true, "blend");
        let sky_pipeline = |compare: wgpu::CompareFunction, label: &str| device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_sky"), compilation_options: Default::default(), buffers: &[] },
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: Some(false),
                depth_compare: Some(compare),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: ms,
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_sky"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview_mask: None,
            cache: None,
        });
        // (drawn after the opaque faces: only where the depth is still the far plane's)
        let sky = sky_pipeline(wgpu::CompareFunction::LessEqual, "sky");
        let sky_always = sky_pipeline(wgpu::CompareFunction::Always, "sky (first)");
        Renderer { device, queue, format, samples, solid, blend, sky, sky_always, sky_first: false, uniforms, bind, targets: None, vbuf: None, fixed: None, fixed_visible: None, fixed_visible_cpu: None, fixed_provisional: false, fixed_blend: None, fixed_blend_visible: None, fixed_blend_visible_cpu: None, last_exact_kept: 0, carried_kept: 0, occluders: None, cull: None, tiles: tex, tiles_view: view, bgl, sampler, game_tex, game_of, view_chunks: None, void_below: false, env: None }
    }

    fn bind_group(device: &wgpu::Device, bgl: &wgpu::BindGroupLayout, uniforms: &wgpu::Buffer, tiles: &wgpu::TextureView, sampler: &wgpu::Sampler, game: &wgpu::Texture) -> wgpu::BindGroup {
        let gview = game.create_view(&Default::default());
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mcview"),
            layout: bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniforms.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(tiles) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(sampler) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&gview) },
            ],
        })
    }

    /// Upload the atlas's pixels again (its layers changed: the game's textures arrived), and the
    /// game texture when its game assets are new.
    pub fn upload_atlas(&mut self, atlas: &Atlas) {
        let tile = TILE as u32;
        for (k, data) in atlas.mips.iter().enumerate() {
            let s = (tile >> k).max(1);
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo { texture: &self.tiles, mip_level: k as u32, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                data,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * s), rows_per_image: Some(s) },
                wgpu::Extent3d { width: s, height: s, depth_or_array_layers: atlas.layers as u32 },
            );
        }
        let of = atlas.game.as_ref().map(|g| std::rc::Rc::as_ptr(g) as usize).unwrap_or(0);
        if of != self.game_of {
            let (t, of) = game_texture(&self.device, &self.queue, atlas);
            self.bind = Self::bind_group(&self.device, &self.bgl, &self.uniforms, &self.tiles_view, &self.sampler, &t);
            self.game_tex = t;
            self.game_of = of;
        }
    }

    fn ensure_targets(&mut self, w: u32, h: u32) {
        if let Some((tw, th, _, _)) = &self.targets
            && *tw == w
            && *th == h
        {
            return;
        }
        let mk = |format, label| {
            self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: self.samples,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
        };
        let color = mk(self.format, "msaa color");
        let depth = mk(wgpu::TextureFormat::Depth24Plus, "depth");
        self.targets = Some((w, h, color, depth));
    }

    /// Solid triangles drawn in every frame from now on, before the frame's own, sent to the GPU
    /// once (again only when `key` changes): a mob farm's blocks ([`crate::player::Player::triangles_split_within`]).
    /// None: no such triangles.
    pub fn set_fixed(&mut self, fixed: Option<(u64, &[Vertex])>) {
        match fixed {
            None => {
                self.fixed = None;
                self.fixed_visible = None;
                self.fixed_visible_cpu = None;
                self.fixed_provisional = false;
                self.fixed_blend = None;
                self.fixed_blend_visible = None;
                self.fixed_blend_visible_cpu = None;
                self.occluders = None;
                self.cull = None;
            }
            Some((key, v)) => {
                if self.fixed.as_ref().is_some_and(|(k, _, _)| *k == key) {
                    return;
                }
                let bytes = (std::mem::size_of_val(v) as u64).max(64);
                let buf = self.device.create_buffer(&wgpu::BufferDescriptor { label: Some("fixed vertices"), size: bytes, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
                if !v.is_empty() {
                    self.queue.write_buffer(&buf, 0, bytemuck::cast_slice(v));
                }
                self.fixed = Some((key, buf, v.len() as u32));
            }
        }
    }

    /// The still translucent triangles kept on the GPU with the kept ones (a mob farm's water,
    /// sorted back to front for the camera: `Player::fixed_blend`), drawn first in the translucent
    /// pass, uploaded when their key changes; None: none.
    pub fn set_fixed_blend(&mut self, blend: Option<(u64, &[Vertex])>) {
        match blend {
            None => {
                self.fixed_blend = None;
                self.fixed_blend_visible = None;
                self.fixed_blend_visible_cpu = None;
            }
            Some((key, v)) => {
                if self.fixed_blend.as_ref().is_some_and(|(k, _, _)| *k == key) {
                    return;
                }
                let bytes = (std::mem::size_of_val(v) as u64).max(64);
                let buf = self.device.create_buffer(&wgpu::BufferDescriptor { label: Some("fixed translucent vertices"), size: bytes, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
                if !v.is_empty() {
                    self.queue.write_buffer(&buf, 0, bytemuck::cast_slice(v));
                }
                self.fixed_blend = Some((key, buf, v.len() as u32));
            }
        }
    }

    /// The render distance a camera's frame draws (chunks): the pictures' client's 6, pushed back
    /// for a far camera so the build stays clear, or the browser viewer's own (`view_chunks`).
    pub fn render_chunks(&self, cam: &Camera) -> f32 {
        (((cam.dist * 2.0 + 16.0) / 16.0).ceil()).max(self.view_chunks.unwrap_or(6.0))
    }

    /// Whether the kept vertices `key` have their camera's cull done ([`Renderer::cull_fixed`]).
    pub fn culled(&self, key: u64, cam: &Camera, width: u32, height: u32) -> bool {
        self.fixed_visible.as_ref().is_some_and(|(k, _, _)| *k == (key, *cam, width, height))
    }

    /// Draw only the kept triangles (`fixed`: the vertices [`Renderer::set_fixed`] keeps, by
    /// their key) that `cam` can see at `width x height` ([`crate::occlude::Cull`]: the same
    /// picture), worked out over as many calls as it takes within `out_of_time`, unless they
    /// already are for this camera, size and kept vertices. A frame with another camera (it
    /// moved), size or kept vertices draws all of them meanwhile. The triangles kept and those
    /// tested, when done in this call.
    pub fn cull_fixed(&mut self, fixed: (u64, &[Vertex]), blend: Option<(u64, &[Vertex])>, opaque: &crate::occlude::Opaque, cam: &Camera, width: u32, height: u32, out_of_time: &mut dyn FnMut() -> bool) -> Option<(usize, usize)> {
        let (bk, bv): (u64, &[Vertex]) = blend.unwrap_or((0, &[]));
        let key = (fixed.0, *cam, width, height);
        let bkey = (fixed.0, bk, *cam, width, height);
        let blend_done = blend.is_none() || self.fixed_blend_visible.as_ref().is_some_and(|(k, _, _)| *k == bkey);
        // (a carried cull within a quarter of the last exact one's triangles is kept as it is:
        // the same picture, and the exact cull's tens of ms saved until the camera moves)
        let tight = self.fixed_provisional && self.carried_kept * 4 <= self.last_exact_kept * 5;
        if self.fixed_visible.as_ref().is_some_and(|(k, _, _)| *k == key) && (!self.fixed_provisional || tight) && blend_done {
            return None;
        }
        if self.cull.as_ref().is_none_or(|(k, _)| *k != bkey) {
            let view = crate::occlude::View { vp: cam.view_proj(width.max(1) as f32 / height.max(1) as f32), eye: cam.eye(), chunks: self.render_chunks(cam), width, height };
            // (the occluders' side of the kept vertices, worked out once for them)
            let occluders = self.occluders.as_ref().filter(|(k, _)| *k == fixed.0).map(|(_, o)| o.clone());
            self.cull = Some((bkey, crate::occlude::Cull::new(view, occluders)));
        }
        let (_, job) = self.cull.as_mut().unwrap();
        let (idx, bidx) = job.step_with(fixed.1, bv, opaque, out_of_time)?;
        if let Some(o) = job.occluders() {
            self.occluders = Some((fixed.0, o));
        }
        self.cull = None;
        let index = |label: &str, idx: &[u32]| {
            let buf = self.device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size: ((idx.len() * 4) as u64).max(64), usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
            if !idx.is_empty() {
                self.queue.write_buffer(&buf, 0, bytemuck::cast_slice(idx));
            }
            buf
        };
        self.fixed_visible = Some((key, index("fixed visible", &idx), idx.len() as u32));
        // (the still translucent faces in front of the opaque ones, in their order)
        if blend.is_some() {
            self.fixed_blend_visible = Some((bkey, index("fixed translucent visible", &bidx), bidx.len() as u32));
            self.fixed_blend_visible_cpu = Some(std::rc::Rc::new(bidx));
        }
        self.fixed_provisional = false;
        let kept = idx.len() / 3;
        self.last_exact_kept = kept;
        self.fixed_visible_cpu = Some((key, std::rc::Rc::new(idx)));
        Some((kept, fixed.1.len() / 3))
    }

    /// Carry the cull of the kept vertices `delta.0` over to `delta.1` (an update in place of a mob
    /// farm's blocks: [`crate::remesh::Delta`]) for the same camera and size, at once: its visible
    /// triangles where they are now, every triangle of the blocks drawn again, and every one whose
    /// place on screen meets where the opaque faces changed (with a margin: their occlusion may
    /// have changed there and only there). A superset of the triangles the camera sees: the same
    /// picture, drawn until the exact cull of the new vertices is done ([`Renderer::cull_fixed`]
    /// runs on). The triangles kept and those of the new vertices; None when it does not apply
    /// (no cull for the old, another camera, the new not yet kept, the change not on screen). The
    /// still translucent faces' cull is carried too when they are the same (`blend`: their key and
    /// vertices; Player keeps their key when no translucent face changed), with every one whose
    /// place on screen meets that change.
    pub fn carry_cull(&mut self, delta: (u64, u64, &crate::remesh::Delta), verts: &[Vertex], blend: Option<(u64, &[Vertex])>, cam: &Camera, width: u32, height: u32) -> Option<(usize, usize)> {
        let (old_key, new_key, d) = delta;
        let (k, old_idx) = self.fixed_visible_cpu.as_ref()?;
        if *k != (old_key, *cam, width, height) || self.fixed.as_ref().map(|f| f.0) != Some(new_key) || self.fixed_visible.as_ref().is_some_and(|(k, _, _)| k.0 == new_key) {
            return None;
        }
        let (wf, hf) = (width.max(1) as f32, height.max(1) as f32);
        let vp = cam.view_proj(wf / hf);
        let project = |p: [f32; 3]| -> Option<[f32; 2]> {
            let mut c = [0.0f32; 4];
            for (i, ci) in c.iter_mut().enumerate() {
                *ci = vp[0][i] * p[0] + vp[1][i] * p[1] + vp[2][i] * p[2] + vp[3][i];
            }
            (c[3] > 1e-4).then(|| [(c[0] / c[3] + 1.0) * 0.5 * wf, (1.0 - c[1] / c[3]) * 0.5 * hf])
        };
        // where the opaque faces changed on screen: the tiles of 8 pixels a changed block's box
        // meets, 4 pixels out (the cull's cells and margins); a box off screen hides nothing there
        const TILE: f32 = 8.0;
        let (tw, th) = ((wf / TILE) as usize + 1, (hf / TILE) as usize + 1);
        let mut mask = vec![false; tw * th];
        let tiles = |a: [f32; 2], b: [f32; 2]| -> Option<(usize, usize, usize, usize)> {
            if b[0] < 0.0 || b[1] < 0.0 || a[0] > wf || a[1] > hf {
                return None;
            }
            let x = ((a[0].max(0.0) / TILE) as usize, ((b[0].min(wf) / TILE) as usize).min(tw - 1));
            let y = ((a[1].max(0.0) / TILE) as usize, ((b[1].min(hf) / TILE) as usize).min(th - 1));
            Some((x.0, x.1, y.0, y.1))
        };
        for (lo, hi) in &d.boxes {
            let (mut a, mut b) = ([f32::MAX; 2], [f32::MIN; 2]);
            for c in 0..8 {
                let p = [if c & 1 == 0 { lo[0] } else { hi[0] }, if c & 2 == 0 { lo[1] } else { hi[1] }, if c & 4 == 0 { lo[2] } else { hi[2] }];
                // (a corner behind the eye: no screen box, no carrying)
                let s = project(p)?;
                a = [a[0].min(s[0]), a[1].min(s[1])];
                b = [b[0].max(s[0]), b[1].max(s[1])];
            }
            if let Some((x0, x1, y0, y1)) = tiles([a[0] - 4.0, a[1] - 4.0], [b[0] + 4.0, b[1] + 4.0]) {
                for y in y0..=y1 {
                    mask[y * tw + x0..=y * tw + x1].fill(true);
                }
            }
        }
        let changed = mask.iter().any(|m| *m);
        let n = verts.len() / 3;
        let mut keep = vec![false; n];
        for t in old_idx.chunks_exact(3) {
            if let Some(v) = d.map(t[0]) {
                keep[(v / 3) as usize] = true;
            }
        }
        for &i in &d.redrawn {
            let r = d.new.get(i)?;
            for t in (r[0] / 3)..(r[1] / 3) {
                keep[t as usize] = true;
            }
        }
        // (a triangle whose place on screen meets the change, or one not wholly in front of the
        // eye; one off screen draws nothing)
        let meets = |tri: &[Vertex]| match [project(tri[0].pos), project(tri[1].pos), project(tri[2].pos)] {
            [Some(p), Some(q), Some(r)] => {
                let a = [p[0].min(q[0]).min(r[0]), p[1].min(q[1]).min(r[1])];
                let b = [p[0].max(q[0]).max(r[0]), p[1].max(q[1]).max(r[1])];
                tiles(a, b).is_some_and(|(x0, x1, y0, y1)| (y0..=y1).any(|y| mask[y * tw + x0..=y * tw + x1].iter().any(|m| *m)))
            }
            _ => true,
        };
        if changed {
            for (t, k) in keep.iter_mut().enumerate() {
                if !*k {
                    *k = meets(&verts[t * 3..t * 3 + 3]);
                }
            }
        }
        // the still translucent faces' cull, when they are the same: theirs and those meeting the
        // change, in their order (back to front)
        let old_bkey = (old_key, blend.map(|b| b.0).unwrap_or(0), *cam, width, height);
        let carried_blend = match (blend, self.fixed_blend_visible_cpu.as_ref()) {
            (Some((bk, bv)), Some(bidx)) if self.fixed_blend_visible.as_ref().is_some_and(|(k, _, _)| *k == old_bkey) && self.fixed_blend.as_ref().is_some_and(|f| f.0 == bk) => {
                let mut bkeep = vec![false; bv.len() / 3];
                for t in bidx.chunks_exact(3) {
                    if let Some(k) = bkeep.get_mut((t[0] / 3) as usize) {
                        *k = true;
                    }
                }
                if changed {
                    for (t, k) in bkeep.iter_mut().enumerate() {
                        if !*k {
                            *k = meets(&bv[t * 3..t * 3 + 3]);
                        }
                    }
                }
                let bidx: Vec<u32> = bkeep.iter().enumerate().filter(|(_, k)| **k).flat_map(|(t, _)| [(t * 3) as u32, (t * 3 + 1) as u32, (t * 3 + 2) as u32]).collect();
                Some(((new_key, bk, *cam, width, height), bidx))
            }
            _ => None,
        };
        let upload = |label: &str, idx: &[u32]| {
            let buf = self.device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size: ((idx.len() * 4) as u64).max(64), usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
            if !idx.is_empty() {
                self.queue.write_buffer(&buf, 0, bytemuck::cast_slice(idx));
            }
            buf
        };
        let idx: Vec<u32> = keep.iter().enumerate().filter(|(_, k)| **k).flat_map(|(t, _)| [(t * 3) as u32, (t * 3 + 1) as u32, (t * 3 + 2) as u32]).collect();
        let buf = upload("fixed visible (carried)", &idx);
        let blend_buf = carried_blend.as_ref().map(|(_, bidx)| upload("fixed translucent visible (carried)", bidx));
        let key = (new_key, *cam, width, height);
        self.fixed_visible = Some((key, buf, idx.len() as u32));
        match (carried_blend, blend_buf) {
            (Some((bkey, bidx)), Some(bbuf)) => {
                self.fixed_blend_visible = Some((bkey, bbuf, bidx.len() as u32));
                self.fixed_blend_visible_cpu = Some(std::rc::Rc::new(bidx));
            }
            _ => {
                self.fixed_blend_visible = None;
                self.fixed_blend_visible_cpu = None;
            }
        }
        self.fixed_provisional = true;
        let kept = idx.len() / 3;
        self.carried_kept = kept;
        self.fixed_visible_cpu = Some((key, std::rc::Rc::new(idx)));
        Some((kept, n))
    }

    /// Draw all the kept triangles again (no visible set: [`Renderer::cull_fixed`]).
    pub fn uncull(&mut self) {
        self.fixed_visible = None;
        self.fixed_blend_visible = None;
    }

    /// Draw a frame into `out` (a single-sample view of `self.format`, frame.width x height).
    pub fn draw(&mut self, out: &wgpu::TextureView, frame: &Frame) {
        let (w, h) = (frame.width.max(1), frame.height.max(1));
        self.ensure_targets(w, h);
        let aspect = w as f32 / h as f32;
        let vp = frame.camera.view_proj(aspect);
        let eye = frame.camera.eye();
        let d = frame.camera.dist;
        // the pictures' client: render distance 6 chunks (nothing beyond: the sky shows) with the
        // game's fog (FogRenderer.setupFog: from the render distance minus a tenth of it, 4..64
        // blocks, to the render distance; the overworld's environment fog from 0 to 1024); a
        // farther camera (zoomed out) pushes the render distance back so the build stays clear;
        // the browser viewer's client sees farther (`view_chunks`)
        let chunks = self.render_chunks(&frame.camera);
        let blocks = chunks * 16.0;
        let span = (blocks / 10.0).clamp(4.0, 64.0);
        // the pictures' noon (no day): their sky's colours, the lightmap at noon (the dimension's
        // values: `daylight::Env::noon`'s), no sun drawn
        let noon = crate::daylight::Env::noon();
        let e = self.env.unwrap_or(noon);
        let (sky_top, sky_low) = match &self.env {
            Some(env) => {
                let (f, _, _) = frame.camera.basis();
                let low = crate::daylight::horizon(env, f, chunks);
                ([env.sky_color[0], env.sky_color[1], env.sky_color[2], 1.0], [low[0], low[1], low[2], 1.0])
            }
            None => ([0.55, 0.70, 1.0, 1.0], [0.70, 0.81, 1.0, 1.0]),
        };
        // the sun's and the moon's directions (`SkyRenderer`: turned -90 degrees about y, then by
        // their angle about x: up at noon, east at dawn), the stars' turn with the sun's, and the
        // rain's dimming of them all
        let body = |deg: f32| {
            let a = deg.to_radians();
            [-a.sin(), a.cos(), 0.0]
        };
        let (sun, moon) = (body(e.sun_angle), body(e.moon_angle));
        let rain_dim = 1.0 - e.rain;
        let u = Uniforms {
            vp,
            eye: [eye[0], eye[1], eye[2], 1.0],
            fog: [blocks - span, blocks, chunks, 1024.0],
            sky_top,
            sky_low,
            inv_vp: invert(&vp),
            sky_void: if self.void_below {
                // (dimmed with the day: the horizon's brightness against noon's)
                let luma = |c: [f32; 4]| 0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2];
                let k = match &self.env {
                    Some(_) => (luma(sky_low) / luma([0.70, 0.81, 1.0, 1.0])).min(1.0),
                    None => 1.0,
                };
                [0.47 * k, 0.55 * k, 0.68 * k, 1.0]
            } else {
                [0.0; 4]
            },
            sky_light: [e.sky_light_color[0], e.sky_light_color[1], e.sky_light_color[2], e.sky_light_factor],
            ambient: [e.ambient[0], e.ambient[1], e.ambient[2], 1.4],
            block_tint: [e.block_tint[0], e.block_tint[1], e.block_tint[2], e.brightness],
            sun: [sun[0], sun[1], sun[2], e.star_brightness],
            moon: [moon[0], moon[1], moon[2], if self.env.is_some() { 1.0 } else { 0.0 }],
            sunrise: e.sunrise,
            stars: [e.sun_angle.to_radians().cos(), e.sun_angle.to_radians().sin(), rain_dim, 0.0],
        };
        self.queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&u));
        let n_solid = frame.solid.len() as u32;
        let n_blend = frame.translucent.len() as u32;
        let bytes = ((frame.solid.len() + frame.translucent.len()) * std::mem::size_of::<Vertex>()).max(64) as u64;
        if self.vbuf.as_ref().is_none_or(|(_, cap)| *cap < bytes) {
            let cap = bytes.next_power_of_two();
            self.vbuf = Some((self.device.create_buffer(&wgpu::BufferDescriptor { label: Some("vertices"), size: cap, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false }), cap));
        }
        let vb = &self.vbuf.as_ref().unwrap().0;
        if n_solid > 0 {
            self.queue.write_buffer(vb, 0, bytemuck::cast_slice(frame.solid));
        }
        if n_blend > 0 {
            self.queue.write_buffer(vb, (frame.solid.len() * std::mem::size_of::<Vertex>()) as u64, bytemuck::cast_slice(frame.translucent));
        }
        let (_, _, color, depth) = self.targets.as_ref().unwrap();
        let cview = color.create_view(&Default::default());
        let dview = depth.create_view(&Default::default());
        // (one sample a pixel: straight into `out`, nothing to resolve)
        let single = self.samples <= 1;
        let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("frame"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: if single { out } else { &cview },
                    depth_slice: None,
                    resolve_target: if single { None } else { Some(out) },
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.73, g: 0.83, b: 1.0, a: 1.0 }), store: if single { wgpu::StoreOp::Store } else { wgpu::StoreOp::Discard } },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &dview, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }), stencil_ops: None }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.bind, &[]);
            // the sky first only when asked (`sky_first`: the order of before, for a check); else
            // after the opaque faces, where none is drawn (its depth test: only a sample still at
            // the far plane takes it; the same picture, its shader run on the sky alone)
            if self.sky_first {
                pass.set_pipeline(&self.sky_always);
                pass.draw(0..3, 0..1);
            }
            if let Some((fkey, buf, n)) = self.fixed.as_ref().filter(|f| f.2 > 0) {
                pass.set_pipeline(&self.solid);
                pass.set_vertex_buffer(0, buf.slice(..));
                // (only the triangles this camera can see, once worked out for it: the same
                // picture, a fraction of the triangles for a software GL to set up)
                match self.fixed_visible.as_ref().filter(|(k, _, _)| *k == (*fkey, frame.camera, w, h)) {
                    Some((_, ib, ni)) => {
                        if *ni > 0 {
                            pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                            pass.draw_indexed(0..*ni, 0, 0..1);
                        }
                    }
                    None => pass.draw(0..*n, 0..1),
                }
            }
            pass.set_vertex_buffer(0, vb.slice(..));
            if n_solid > 0 {
                pass.set_pipeline(&self.solid);
                pass.draw(0..n_solid, 0..1);
            }
            if !self.sky_first {
                pass.set_pipeline(&self.sky);
                pass.draw(0..3, 0..1);
            }
            // the still translucent faces kept on the GPU first (sorted back to front; those the
            // camera can see when worked out for it), then the frame's own
            if let Some((bkey, buf, n)) = self.fixed_blend.as_ref().filter(|f| f.2 > 0) {
                pass.set_pipeline(&self.blend);
                pass.set_vertex_buffer(0, buf.slice(..));
                let fkey = self.fixed.as_ref().map(|f| f.0).unwrap_or(0);
                match self.fixed_blend_visible.as_ref().filter(|(k, _, _)| *k == (fkey, *bkey, frame.camera, w, h)) {
                    Some((_, ib, ni)) => {
                        if *ni > 0 {
                            pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                            pass.draw_indexed(0..*ni, 0, 0..1);
                        }
                    }
                    None => pass.draw(0..*n, 0..1),
                }
                pass.set_vertex_buffer(0, vb.slice(..));
            }
            if n_blend > 0 {
                pass.set_pipeline(&self.blend);
                pass.draw(n_solid..n_solid + n_blend, 0..1);
            }
        }
        self.queue.submit(Some(enc.finish()));
    }

    /// Draw one tiny frame offscreen with every pipeline (the sky, the solid and the translucent
    /// pass), so a page's first real frame does not also pay for the GL programs' first use.
    pub fn warm_up(&mut self) {
        let view = self.offscreen(1, 1);
        let v = Vertex { pos: [0.0; 3], uv: [0.0; 2], layer: 0, color: [0; 4], light: crate::mesh::UNLIT };
        let tri = [v, v, v];
        let targets = self.targets.take();
        let camera = Camera { target: [0.0; 3], yaw: 0.5, pitch: 0.5, dist: 10.0, vfov: 0.8 };
        self.draw(&view, &Frame { solid: &tri, translucent: &tri, camera, width: 1, height: 1 });
        // (the page's own size comes back with its first frame)
        self.targets = targets;
    }

    /// The frame targets at the canvas's size, made and cleared once (offscreen, the sky only), so
    /// the first real frame does not make them.
    pub fn warm_up_targets(&mut self, width: u32, height: u32) {
        let (w, h) = (width.max(1), height.max(1));
        let view = self.offscreen(w, h);
        let camera = Camera { target: [0.0; 3], yaw: 0.5, pitch: 0.5, dist: 10.0, vfov: 0.8 };
        self.draw(&view, &Frame { solid: &[], translucent: &[], camera, width: w, height: h });
    }

    fn offscreen(&self, w: u32, h: u32) -> wgpu::TextureView {
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("warm-up"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        tex.create_view(&Default::default())
    }
}
pub fn vertex_buffer(device: &wgpu::Device, v: &[Vertex]) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("v"), contents: bytemuck::cast_slice(v), usage: wgpu::BufferUsages::VERTEX })
}

/// Native, headless: a device on the first adapter (Vulkan; lavapipe on a GPU-less box) and
/// offscreen frames read back as RGBA.
#[cfg(not(target_arch = "wasm32"))]
pub mod headless {
    use super::*;

    pub struct Offscreen {
        pub r: Renderer,
    }

    impl Offscreen {
        pub fn new(atlas: &Atlas) -> Result<Offscreen, String> {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends: wgpu::Backends::VULKAN, ..wgpu::InstanceDescriptor::new_without_display_handle() });
            let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, ..Default::default() }))
                .map_err(|e| format!("no GPU adapter (install mesa-vulkan-drivers for lavapipe): {e}"))?;
            let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("mcview"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
                ..Default::default()
            }))
            .map_err(|e| format!("request_device: {e}"))?;
            Ok(Offscreen { r: Renderer::new(device, queue, wgpu::TextureFormat::Rgba8Unorm, atlas, SAMPLES) })
        }

        /// Draw and read back `width x height` RGBA pixels.
        pub fn render(&mut self, frame: &Frame) -> Result<Vec<u8>, String> {
            let (w, h) = (frame.width, frame.height);
            let tex = self.r.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("out"),
                size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let view = tex.create_view(&Default::default());
            self.r.draw(&view, frame);
            let row = (4 * w).div_ceil(256) * 256;
            let buf = self.r.device.create_buffer(&wgpu::BufferDescriptor { label: Some("readback"), size: (row * h) as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
            let mut enc = self.r.device.create_command_encoder(&Default::default());
            enc.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) } },
                wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            );
            self.r.queue.submit(Some(enc.finish()));
            let slice = buf.slice(..);
            slice.map_async(wgpu::MapMode::Read, |_| {});
            let _ = self.r.device.poll(wgpu::PollType::wait_indefinitely());
            let data = slice.get_mapped_range().map_err(|e| format!("map: {e:?}"))?;
            let mut out = Vec::with_capacity((4 * w * h) as usize);
            for y in 0..h {
                let o = (y * row) as usize;
                out.extend_from_slice(&data[o..o + (4 * w) as usize]);
            }
            drop(data);
            buf.unmap();
            Ok(out)
        }
    }

    /// Write RGBA pixels as a PNG.
    pub fn write_png(path: &std::path::Path, w: u32, h: u32, rgba: &[u8]) -> Result<(), String> {
        let f = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut enc = png::Encoder::new(std::io::BufWriter::new(f), w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut wr = enc.write_header().map_err(|e| e.to_string())?;
        wr.write_image_data(rgba).map_err(|e| e.to_string())?;
        Ok(())
    }
}
