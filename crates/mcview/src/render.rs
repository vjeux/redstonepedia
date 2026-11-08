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

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) world: vec3<f32>,
    @location(3) @interpolate(flat) layer: u32,
};

@vertex
fn vs(@location(0) pos: vec3<f32>, @location(1) uv: vec2<f32>, @location(2) layer: u32, @location(3) color: vec4<f32>) -> VOut {
    var o: VOut;
    o.pos = u.vp * vec4<f32>(pos, 1.0);
    o.uv = uv;
    o.color = color;
    o.world = pos;
    o.layer = layer;
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
    // dying mob's red overlay the same way (layer 65534: `OverlayTexture`'s red)
    let flash = select(vec3<f32>(1.0, 1.0, 1.0), vec3<f32>(1.0, 0.0, 0.0), i.layer == 65534u);
    let lit = mix(t.rgb * i.color.rgb, flash, 1.0 - i.color.a);
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
    let c = mix(u.sky_low.rgb, u.sky_top.rgb, t);
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
    uniforms: wgpu::Buffer,
    bind: wgpu::BindGroup,
    /// Multisampled color + depth targets for the current size.
    targets: Option<(u32, u32, wgpu::Texture, wgpu::Texture)>,
    vbuf: Option<(wgpu::Buffer, u64)>,
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
        let sky = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sky"),
            layout: Some(&layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_sky"), compilation_options: Default::default(), buffers: &[] },
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
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
        Renderer { device, queue, format, samples, solid, blend, sky, uniforms, bind, targets: None, vbuf: None, tiles: tex, tiles_view: view, bgl, sampler, game_tex, game_of, view_chunks: None, void_below: false }
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
        let chunks = (((d * 2.0 + 16.0) / 16.0).ceil()).max(self.view_chunks.unwrap_or(6.0));
        let blocks = chunks * 16.0;
        let span = (blocks / 10.0).clamp(4.0, 64.0);
        let u = Uniforms {
            vp,
            eye: [eye[0], eye[1], eye[2], 1.0],
            fog: [blocks - span, blocks, chunks, 1024.0],
            sky_top: [0.55, 0.70, 1.0, 1.0],
            sky_low: [0.70, 0.81, 1.0, 1.0],
            inv_vp: invert(&vp),
            sky_void: if self.void_below { [0.47, 0.55, 0.68, 1.0] } else { [0.0; 4] },
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
        let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("frame"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &cview,
                    depth_slice: None,
                    resolve_target: Some(out),
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.73, g: 0.83, b: 1.0, a: 1.0 }), store: wgpu::StoreOp::Discard },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &dview, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }), stencil_ops: None }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.bind, &[]);
            pass.set_pipeline(&self.sky);
            pass.draw(0..3, 0..1);
            pass.set_vertex_buffer(0, vb.slice(..));
            if n_solid > 0 {
                pass.set_pipeline(&self.solid);
                pass.draw(0..n_solid, 0..1);
            }
            if n_blend > 0 {
                pass.set_pipeline(&self.blend);
                pass.draw(n_solid..n_solid + n_blend, 0..1);
            }
        }
        self.queue.submit(Some(enc.finish()));
    }
}

/// A buffer of vertices for tests and tools.
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
