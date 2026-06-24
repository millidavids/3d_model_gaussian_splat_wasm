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
/// SH degree for the flat synthetic sample (no view-dependent colour).
const SAMPLE_SH_DEGREE: u8 = 0;
/// SH degree for loaded splats (use the full SH so trained splats look right).
const LOADED_SH_DEGREE: u8 = 3;

/// Why GPU initialisation failed. These are real runtime conditions (no WebGPU,
/// no suitable device, …), not invariants, so [`Graphics::new`] returns them for
/// the caller to surface — see the error overlay in [`crate::app`].
#[derive(Debug, thiserror::Error)]
pub enum GraphicsError {
    #[error("could not create a rendering surface: {0}")]
    CreateSurface(#[from] wgpu::CreateSurfaceError),
    #[error("no compatible WebGPU adapter — is WebGPU available in this browser?: {0}")]
    RequestAdapter(#[from] wgpu::RequestAdapterError),
    #[error("could not acquire a GPU device: {0}")]
    RequestDevice(#[from] wgpu::RequestDeviceError),
    #[error("could not create the splat viewer: {0}")]
    CreateViewer(#[from] gs::ViewerCreateError),
}

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
    ///
    /// Fallible: a browser without WebGPU (or no suitable device) returns
    /// [`GraphicsError`] so the caller can show a message instead of crashing.
    pub async fn new(window: Arc<Window>) -> Result<Self, GraphicsError> {
        let (init_width, init_height) = drawable_size(&window);

        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let surface = instance.create_surface(window.clone())?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("gsplat device"),
                required_limits: adapter.limits(),
                ..Default::default()
            })
            .await?;

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
        let mut viewer = gs::Viewer::new(&device, config.view_formats[0], &gaussians)?;
        configure_viewer(&mut viewer, &queue, SAMPLE_SH_DEGREE);

        Ok(Self {
            window,
            surface,
            device,
            queue,
            config,
            camera,
            viewer,
        })
    }

    /// Replace the displayed splat (e.g. from a dropped `.ply`/`.spz` file).
    ///
    /// Rebuilds the viewer for `gaussians` (the viewer crate has no in-place
    /// gaussian swap). Returns `false` if the viewer build fails so the caller
    /// can skip re-framing the camera onto a splat that isn't shown.
    #[must_use]
    pub fn load_gaussians(&mut self, gaussians: &Gaussians) -> bool {
        match gs::Viewer::new(&self.device, self.config.view_formats[0], gaussians) {
            Ok(mut viewer) => {
                configure_viewer(&mut viewer, &self.queue, LOADED_SH_DEGREE);
                self.viewer = viewer;
                log::info!("loaded splat with {} gaussians", gaussians.len());
                true
            }
            Err(e) => {
                log::error!("could not build viewer for loaded splat: {e:?}");
                false
            }
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
                // Lost/Outdated/etc.: reconfigure so we recover next frame
                // instead of staying black forever when the size is unchanged
                // (resize() is the only other path that reconfigures).
                log::warn!("surface texture unavailable, reconfiguring: {other:?}");
                self.surface.configure(&self.device, &self.config);
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

/// Apply the shared splat-display settings to a freshly-built viewer. Only the
/// SH degree differs between the flat sample and a loaded splat.
fn configure_viewer(viewer: &mut gs::Viewer, queue: &wgpu::Queue, sh_degree: u8) {
    viewer.update_gaussian_transform(
        queue,
        SPLAT_SIZE,
        GaussianDisplayMode::Splat,
        GaussianShDegree::new(sh_degree).expect("sh degree in 0..=3"),
        false,
        GaussianMaxStdDev::new(MAX_STD_DEV).expect("max std dev in range"),
    );
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
