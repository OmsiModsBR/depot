//! The launcher's window: winit events into the widget kit's input, a surface drawn each
//! frame. Nothing here knows a page; `Shell` draws them.

use crate::look::scale_for;
use crate::offscreen::{adapter_device, draw_into, load_logo};
use crate::Shell;
use anyhow::Result;
use glam::Vec2;
use omsi_ui::widgets::{Key, Modifiers, Ui};
use omsi_ui::Gpu;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

/// How long the window keeps drawing after an input event: hover and press animations settle
/// within this. An idle launcher draws nothing and sleeps until the next event.
const AWAKE: Duration = Duration::from_millis(600);

struct Surface {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
    gpu: Gpu,
}

struct App {
    instance: wgpu::Instance,
    shell: Shell,
    ui: Ui,
    modifiers: Modifiers,
    state: Option<Surface>,
    last: Instant,
    /// Drawing goes on until then; every input event pushes it forward (see `AWAKE`).
    awake_until: Instant,
    error: Option<anyhow::Error>,
}

/// Open the launcher window on `shell` and run until it is closed.
pub fn run(shell: Shell) -> Result<()> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App {
        instance: wgpu::Instance::default(),
        shell,
        ui: Ui::new(),
        modifiers: Modifiers::default(),
        state: None,
        last: Instant::now(),
        awake_until: Instant::now(),
        error: None,
    };
    event_loop.run_app(&mut app)?;
    match app.error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

impl App {
    fn open(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("Project Depot")
                    .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0))
                    .with_min_inner_size(winit::dpi::LogicalSize::new(960.0, 540.0)),
            )?,
        );
        let surface = self.instance.create_surface(window.clone())?;
        let (adapter, device, queue) = adapter_device(&self.instance, Some(&surface))?;
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);
        let mut gpu = Gpu::new(&device, format, 4, self.ui.atlas.size);
        self.shell.logo = load_logo(&mut gpu, &device, &queue);
        self.state = Some(Surface {
            window,
            surface,
            config,
            device,
            queue,
            gpu,
        });
        Ok(())
    }

    fn redraw(&mut self) {
        let Some(s) = self.state.as_mut() else { return };
        let now = Instant::now();
        let dt = now.duration_since(self.last).as_secs_f32();
        self.last = now;
        let frame = match s.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f)
            | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                s.surface.configure(&s.device, &s.config);
                return;
            }
            _ => return,
        };
        let view = frame.texture.create_view(&Default::default());
        let size = (s.config.width, s.config.height);
        draw_into(
            &mut self.shell,
            &mut self.ui,
            &mut s.gpu,
            &s.device,
            &s.queue,
            &view,
            size,
            dt,
        );
        s.window.set_cursor(self.ui.cursor);
        s.window.pre_present_notify();
        frame.present();
    }

    /// The window's pixel position as a point of the design.
    fn point(&self, x: f64, y: f64) -> Vec2 {
        let size = self
            .state
            .as_ref()
            .map(|s| (s.config.width, s.config.height))
            .unwrap_or((1, 1));
        Vec2::new(x as f32, y as f32) / scale_for(size.0, size.1)
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_none() {
            if let Err(e) = self.open(event_loop) {
                self.error = Some(e);
                event_loop.exit();
                return;
            }
        }
        self.awake_until = Instant::now() + AWAKE;
        if let Some(s) = self.state.as_ref() {
            s.window.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        if !matches!(event, WindowEvent::RedrawRequested) {
            self.awake_until = Instant::now() + AWAKE;
            if let Some(s) = self.state.as_ref() {
                s.window.request_redraw();
            }
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(s) = self.state.as_mut() {
                    s.config.width = size.width.max(1);
                    s.config.height = size.height.max(1);
                    s.surface.configure(&s.device, &s.config);
                }
            }
            WindowEvent::ModifiersChanged(m) => {
                self.modifiers.told(m.state());
                self.modifiers.apply(&mut self.ui.input);
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.ui.input.mouse = self.point(position.x, position.y);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let down = state == ElementState::Pressed;
                if button == MouseButton::Left {
                    if down {
                        self.ui.input.pressed = true;
                    } else {
                        self.ui.input.released = true;
                    }
                    self.ui.input.down = down;
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.ui.input.wheel += match delta {
                    MouseScrollDelta::LineDelta(x, y) => Vec2::new(x, y),
                    MouseScrollDelta::PixelDelta(p) => Vec2::new(p.x as f32, p.y as f32) / 40.0,
                };
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }
                let cmd = self.modifiers.command();
                if let PhysicalKey::Code(code) = event.physical_key {
                    let key = match code {
                        KeyCode::Escape => Some(Key::Escape),
                        KeyCode::Enter | KeyCode::NumpadEnter => Some(Key::Enter),
                        KeyCode::Tab => Some(Key::Tab),
                        KeyCode::Backspace => Some(Key::Backspace),
                        KeyCode::Delete => Some(Key::Delete),
                        KeyCode::ArrowLeft => Some(Key::Left),
                        KeyCode::ArrowRight => Some(Key::Right),
                        KeyCode::ArrowUp => Some(Key::Up),
                        KeyCode::ArrowDown => Some(Key::Down),
                        KeyCode::Home => Some(Key::Home),
                        KeyCode::End => Some(Key::End),
                        KeyCode::KeyA if cmd => Some(Key::SelectAll),
                        _ => None,
                    };
                    self.ui.input.keys.extend(key);
                }
                if !cmd {
                    if let Some(t) = event.text.as_ref() {
                        self.ui.input.text.push_str(t);
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                self.redraw();
                // animations (hover, selection) settle over a few frames after an input event,
                // and a page still reading its catalog needs a frame when the thread finishes
                if Instant::now() < self.awake_until || self.shell.busy() {
                    if let Some(s) = self.state.as_ref() {
                        s.window.request_redraw();
                    }
                }
            }
            _ => {}
        }
    }
}
