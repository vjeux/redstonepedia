//! A wgpu device on a canvas (WebGL2): the shared thumbnail canvas or a live viewer's own.

use mcview::atlas::Atlas;
use mcview::render::{Frame, Renderer};
use web_sys::HtmlCanvasElement;

/// The browser viewer's render distance in chunks (`Renderer::view_chunks`; the ground reaches
/// past it: `scene::GROUND_REACH`).
const VIEW_CHUNKS: f32 = 24.0;

pub struct Gpu {
    pub canvas: HtmlCanvasElement,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    pub r: Renderer,
}

impl Gpu {
    pub async fn new(canvas: HtmlCanvasElement, atlas: &Atlas) -> Result<Gpu, String> {
        let clock = || crate::window().performance().map(|p| p.now()).unwrap_or(0.0);
        // each step in a task of its own (the context, the adapter, the device, the renderer; the
        // coordinator 2026-10-22: no task over ~50-100 ms). In a page's first second the
        // browser's own GPU set-up lands in the context or the adapter step (a fresh headless
        // Chromium: 50-110 ms; a context made after it takes 5 ms), the renderer's shaders and
        // textures take ~50 ms in a software GL
        let t0 = clock();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends: wgpu::Backends::GL, ..wgpu::InstanceDescriptor::new_without_display_handle() });
        let surface = instance.create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone())).map_err(|e| format!("WebGL2 surface: {e}"))?;
        let ta = clock();
        crate::next_task().await;
        let tb = clock();
        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions { compatible_surface: Some(&surface), ..Default::default() }).await.map_err(|e| format!("no WebGL2 adapter: {e}"))?;
        let tc = clock();
        crate::next_task().await;
        let td = clock();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor { label: Some("mcview"), required_limits: wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()), ..Default::default() })
            .await
            .map_err(|e| format!("WebGL2 device: {e}"))?;
        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).or(caps.formats.first().copied()).ok_or("the canvas offers no format")?;
        let samples = if adapter.get_texture_format_features(format).flags.sample_count_supported(4) { 4 } else { 1 };
        let mut config = surface.get_default_config(&adapter, canvas.width().max(1), canvas.height().max(1)).ok_or("canvas not supported")?;
        config.format = format;
        config.alpha_mode = caps.alpha_modes.first().copied().unwrap_or(wgpu::CompositeAlphaMode::Auto);
        surface.configure(&device, &config);
        let t1 = clock();
        crate::next_task().await;
        let t2 = clock();
        let mut r = Renderer::new(device, queue, format, atlas, samples);
        let t3 = clock();
        // the programs' first use and the frame targets at the canvas's size, each in a task of
        // its own (not in the page's first frame)
        crate::next_task().await;
        let t4 = clock();
        r.warm_up();
        let t5 = clock();
        crate::next_task().await;
        let t6 = clock();
        r.warm_up_targets(canvas.width(), canvas.height());
        if crate::debug() {
            crate::log(&format!("a WebGL context: the context {:.0} ms, the adapter {:.0} ms, the device {:.0} ms, the renderer {:.0} ms, its programs {:.0} ms, its targets {:.0} ms", ta - t0, tc - tb, t1 - td, t3 - t2, t5 - t4, clock() - t6));
        }
        // a player's client with a long render distance: the ground's fogged end at the horizon
        r.view_chunks = Some(VIEW_CHUNKS);
        r.void_below = true;
        // a failed GPU command is reported (else it only leaves the canvas black) and
        // shown on the page: the console log alone is invisible on a phone
        r.device.on_uncaptured_error(std::sync::Arc::new(|e: wgpu::Error| {
            crate::log(&format!("GPU error: {e}"));
            crate::texture_note(&format!("GPU error: {e}"));
        }));
        Ok(Gpu { canvas, surface, config, r })
    }

    /// Draw a frame on the canvas (resized to the frame first).
    pub fn draw(&mut self, frame: &Frame) -> Result<(), String> {
        if self.config.width != frame.width || self.config.height != frame.height {
            self.config.width = frame.width.max(1);
            self.config.height = frame.height.max(1);
            self.surface.configure(&self.r.device, &self.config);
        }
        let tex = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            _ => {
                self.surface.configure(&self.r.device, &self.config);
                return Err("surface lost; reconfigured".into());
            }
        };
        let view = tex.texture.create_view(&Default::default());
        self.r.draw(&view, frame);
        self.r.queue.present(tex);
        Ok(())
    }
}
