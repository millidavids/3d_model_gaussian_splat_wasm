//! The GPU side: a wgpu surface/device plus a [`gs::Viewer`] rendering the
//! sample splat. The wgpu 29 call shapes here mirror the viewer crate's own
//! `simple` example (its CI-tested reference for this version), generalised and
//! kept platform-agnostic so native `cargo check` exercises the same code.

use std::sync::Arc;

use glam::uvec2;
use wgpu_3dgs_viewer as gs;
use wgpu_3dgs_viewer::core::{GaussianDisplayMode, GaussianMaxStdDev, GaussianShDegree, Gaussians};
use winit::window::Window;

use crate::camera_control::OrbitCamera;
use crate::scene;

/// Max Gaussian std-dev cutoff; 3σ keeps splats crisp without clipping.
const MAX_STD_DEV: f32 = 3.0;
/// Overall splat size multiplier (1.0 = as authored).
const SPLAT_SIZE: f32 = 1.0;

/// Owns the GPU resources and the splat viewer for one window/canvas.
pub struct Graphics {
    /// Kept so the web build can read the live canvas size each frame.
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,

    camera: gs::Camera,
    viewer: gs::Viewer,
}

impl Graphics {
    /// Initialise wgpu against `window` and build the viewer for the sample splat.
    pub async fn new(window: Arc<Window>) -> Self {
        let (init_width, init_height) = drawable_size(&window);

        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let surface = instance
            .create_surface(window.clone())
            .expect("create surface");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("request a WebGPU adapter");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("gsplat device"),
                required_limits: adapter.limits(),
                ..Default::default()
            })
            .await
            .expect("request a device");

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps.formats[0];
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: init_width,
            height: init_height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            // The viewer renders to a non-sRGB view; let the surface apply sRGB.
            view_formats: vec![surface_format.remove_srgb_suffix()],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let gaussians = scene::sample_splat();
        log::info!("built sample splat with {} gaussians", gaussians.len());

        let camera = gs::Camera::new(0.1..1e4, 60f32.to_radians());
        let mut viewer =
            gs::Viewer::new(&device, config.view_formats[0], &gaussians).expect("create viewer");
        viewer.update_gaussian_transform(
            &queue,
            SPLAT_SIZE,
            GaussianDisplayMode::Splat,
            GaussianShDegree::new(0).expect("sh degree 0 is valid"),
            false,
            GaussianMaxStdDev::new(MAX_STD_DEV).expect("max std dev in range"),
        );

        Self {
            window,
            surface,
            device,
            queue,
            config,
            camera,
            viewer,
        }
    }

    /// Replace the displayed splat (e.g. from a dropped `.ply`/`.spz` file).
    ///
    /// Rebuilds the viewer for `gaussians`. Uses SH degree 3 so real, trained
    /// splats show their view-dependent colour (the synthetic sample is flat).
    pub fn load_gaussians(&mut self, gaussians: &Gaussians) {
        match gs::Viewer::new(&self.device, self.config.view_formats[0], gaussians) {
            Ok(mut viewer) => {
                viewer.update_gaussian_transform(
                    &self.queue,
                    SPLAT_SIZE,
                    GaussianDisplayMode::Splat,
                    GaussianShDegree::new(3).expect("sh degree 3 is valid"),
                    false,
                    GaussianMaxStdDev::new(MAX_STD_DEV).expect("max std dev in range"),
                );
                self.viewer = viewer;
                log::info!("loaded splat with {} gaussians", gaussians.len());
            }
            Err(e) => log::error!("could not build viewer for loaded splat: {e:?}"),
        }
    }

    /// Reconfigure the surface after a canvas/window resize.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    /// Place the camera per `orbit`, then draw one frame.
    pub fn render(&mut self, orbit: &OrbitCamera) {
        // On the web the canvas size isn't reliably reported through winit's
        // resize events, so reconcile the surface to the live size each frame.
        // Without this the viewer projects splats for a stale (often tiny)
        // viewport and they smear across the whole canvas.
        let (width, height) = drawable_size(&self.window);
        if width != self.config.width || height != self.config.height {
            self.resize(width, height);
        }

        orbit.apply_to(&mut self.camera);
        self.viewer.update_camera(
            &self.queue,
            &self.camera,
            uvec2(self.config.width, self.config.height),
        );

        let texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            other => {
                log::warn!("skipping frame, surface texture unavailable: {other:?}");
                return;
            }
        };
        let view = texture.texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("frame view"),
            format: Some(self.config.view_formats[0]),
            ..Default::default()
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame encoder"),
            });
        self.viewer.render(&mut encoder, &view);
        self.queue.submit(std::iter::once(encoder.finish()));

        // Native blocks until the GPU is done; on the web the browser drives the
        // frame, so polling there is unnecessary.
        #[cfg(not(target_arch = "wasm32"))]
        if let Err(e) = self.device.poll(wgpu::PollType::wait_indefinitely()) {
            log::error!("device poll failed: {e:?}");
        }

        texture.present();
    }
}

/// The surface's target size in physical pixels.
///
/// Native: trust winit's window size. Web: derive it from the canvas's CSS size
/// and the device pixel ratio, and set the canvas backing store to match — the
/// authoritative path winit's resize events don't reliably provide.
#[cfg(not(target_arch = "wasm32"))]
fn drawable_size(window: &Window) -> (u32, u32) {
    let size = window.inner_size();
    (size.width.max(1), size.height.max(1))
}

#[cfg(target_arch = "wasm32")]
fn drawable_size(window: &Window) -> (u32, u32) {
    use winit::platform::web::WindowExtWebSys;

    let canvas = window.canvas().expect("winit canvas");
    let dpr = web_sys::window()
        .map(|w| w.device_pixel_ratio())
        .filter(|r| *r > 0.0)
        .unwrap_or(1.0);

    let width = ((canvas.client_width().max(1) as f64) * dpr).round() as u32;
    let height = ((canvas.client_height().max(1) as f64) * dpr).round() as u32;

    if canvas.width() != width {
        canvas.set_width(width);
    }
    if canvas.height() != height {
        canvas.set_height(height);
    }
    (width.max(1), height.max(1))
}
