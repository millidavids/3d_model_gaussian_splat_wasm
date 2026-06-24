//! The winit event loop: owns the window, drives the render loop, and turns
//! pointer input into orbit-camera motion.
//!
//! wgpu init is async, which the browser cannot block on, so [`Graphics`] is
//! built into a shared slot — synchronously on native, via a spawned future on
//! the web — and the loop simply renders once it appears.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use web_time::Instant;
use wgpu_3dgs_viewer::core::Gaussians;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use crate::camera_control::OrbitCamera;
use crate::graphics::Graphics;
use crate::scene;

/// A one-slot inbox for a splat parsed off the main loop (drag-and-drop, async):
/// the loader fills it, the render loop drains it on the next frame.
pub(crate) type LoadInbox = Rc<RefCell<Option<Gaussians>>>;

/// Build the event loop and run the viewer.
pub fn run() {
    let event_loop = EventLoop::new().expect("create event loop");

    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::EventLoopExtWebSys;
        event_loop.spawn_app(App::new());
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut app = App::new();
        event_loop.run_app(&mut app).expect("run event loop");
    }
}

/// Application state for the winit [`ApplicationHandler`].
struct App {
    window: Option<Arc<Window>>,
    /// Filled once async GPU init finishes (immediately on native).
    graphics: Rc<RefCell<Option<Graphics>>>,
    orbit: OrbitCamera,
    last_frame: Option<Instant>,
    dragging: bool,
    last_cursor: Option<(f64, f64)>,
    /// Splat dropped onto the page, waiting to be swapped in (web only).
    load_inbox: LoadInbox,
}

impl App {
    fn new() -> Self {
        Self {
            window: None,
            graphics: Rc::new(RefCell::new(None)),
            orbit: OrbitCamera::new(),
            last_frame: None,
            dragging: false,
            last_cursor: None,
            load_inbox: Rc::new(RefCell::new(None)),
        }
    }

    /// Swap in a splat that finished loading, framing the camera to fit it.
    fn drain_pending_load(&mut self) {
        let Some(gaussians) = self.load_inbox.borrow_mut().take() else {
            return;
        };
        let (center, radius) = scene::bounds(&gaussians);
        self.orbit.frame(center, radius);
        if let Some(graphics) = self.graphics.borrow_mut().as_mut() {
            graphics.load_gaussians(&gaussians);
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return; // `resumed` can fire more than once; build the window only once.
        }

        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_title("gsplat-wasm viewer"))
                .expect("create window"),
        );

        #[cfg(target_arch = "wasm32")]
        {
            mount_canvas(&window);
            crate::loader::setup_drag_and_drop(self.load_inbox.clone());
        }

        self.window = Some(window.clone());
        event_loop.set_control_flow(ControlFlow::Poll);

        // Build the GPU side; render starts once it lands in the shared slot.
        let slot = self.graphics.clone();
        #[cfg(not(target_arch = "wasm32"))]
        {
            *slot.borrow_mut() = Some(pollster::block_on(Graphics::new(window.clone())));
            window.request_redraw();
        }
        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(async move {
            let graphics = Graphics::new(window.clone()).await;
            *slot.borrow_mut() = Some(graphics);
            window.request_redraw();
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::Resized(size) => {
                if let Some(graphics) = self.graphics.borrow_mut().as_mut() {
                    graphics.resize(size.width, size.height);
                }
            }

            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                self.dragging = state == ElementState::Pressed;
                self.last_cursor = None; // re-baseline so the next move has no jump
            }

            WindowEvent::CursorMoved { position, .. } => {
                if self.dragging {
                    if let Some((lx, ly)) = self.last_cursor {
                        self.orbit
                            .orbit((position.x - lx) as f32, (position.y - ly) as f32);
                    }
                    self.last_cursor = Some((position.x, position.y));
                }
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let scroll = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 100.0,
                };
                self.orbit.zoom(scroll);
            }

            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = self
                    .last_frame
                    .map_or(0.0, |last| (now - last).as_secs_f32());
                self.last_frame = Some(now);

                self.drain_pending_load();

                if !self.dragging {
                    self.orbit.advance(dt);
                }

                if let Some(graphics) = self.graphics.borrow_mut().as_mut() {
                    graphics.render(&self.orbit);
                }

                // Keep the loop alive for the next frame.
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }

            _ => {}
        }
    }
}

/// Attach winit's canvas to the page and size it to the viewport.
#[cfg(target_arch = "wasm32")]
fn mount_canvas(window: &Window) {
    use winit::platform::web::WindowExtWebSys;

    let web_window = web_sys::window().expect("browser window");
    let document = web_window.document().expect("document");
    let canvas = window.canvas().expect("winit canvas");

    let style = canvas.style();
    let _ = style.set_property("display", "block");
    let _ = style.set_property("width", "100%");
    let _ = style.set_property("height", "100%");

    document
        .body()
        .expect("document body")
        .append_child(&canvas)
        .expect("append canvas to body");

    // The canvas fills the viewport via CSS; its pixel size is reconciled to the
    // CSS size each frame in `Graphics::render`, so nothing to size here.
}
