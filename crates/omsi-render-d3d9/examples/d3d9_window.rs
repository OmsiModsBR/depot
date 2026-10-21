//! Opens a window and draws a checkered quad with the Direct3D 9Ex renderer, to see the
//! window path working on a real machine: `cargo run -p omsi-render-d3d9 --example d3d9_window`.
//! The three columns show the alpha modes: opaque, test (holes) and blend.

#[cfg(not(windows))]
fn main() {
    eprintln!("the Direct3D 9 renderer exists only on Windows");
}

#[cfg(windows)]
fn main() {
    use omsi_render_d3d9::d3d9::D3d9Device;
    use omsi_render_d3d9::{AlphaMode, DrawBatch, FrameDesc, MeshDesc, RenderBackend, TextureDesc};
    use windows::Win32::Foundation::HWND;
    use winit::application::ApplicationHandler;
    use winit::event::WindowEvent;
    use winit::event_loop::{ActiveEventLoop, EventLoop};
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use winit::window::{Window, WindowId};

    struct Scene {
        dev: D3d9Device,
        mesh: omsi_render_d3d9::MeshHandle,
        tex: omsi_render_d3d9::TextureHandle,
    }

    #[derive(Default)]
    struct App {
        window: Option<Window>,
        scene: Option<Scene>,
    }

    fn checker() -> TextureDesc {
        let mut rgba = Vec::with_capacity(16 * 16 * 4);
        for y in 0..16 {
            for x in 0..16 {
                let on = (x / 4 + y / 4) % 2 == 0;
                // Dark squares are half transparent, to show the alpha modes apart.
                rgba.extend_from_slice(if on {
                    &[240, 200, 40, 255]
                } else {
                    &[40, 80, 200, 90]
                });
            }
        }
        TextureDesc {
            width: 16,
            height: 16,
            rgba,
        }
    }

    fn quad() -> MeshDesc {
        let v = |x: f32, y: f32, u: f32, w: f32| [x, y, 0.5, 0., 0., -1., u, w];
        MeshDesc {
            vertices: [
                v(-1., -1., 0., 1.),
                v(1., -1., 1., 1.),
                v(1., 1., 1., 0.),
                v(-1., 1., 0., 0.),
            ]
            .concat(),
            indices: vec![0, 1, 2, 0, 2, 3],
        }
    }

    impl ApplicationHandler for App {
        fn resumed(&mut self, el: &ActiveEventLoop) {
            let attrs = Window::default_attributes().with_title("Depot - Direct3D 9Ex foundation");
            let window = el.create_window(attrs).expect("window");
            let RawWindowHandle::Win32(h) = window.window_handle().expect("handle").as_raw() else {
                unreachable!("Win32 window")
            };
            let size = window.inner_size();
            // SAFETY: the window lives in `self.window` for as long as the device.
            let mut dev = unsafe {
                D3d9Device::open_window(HWND(h.hwnd.get() as _), size.width, size.height)
            }
            .expect("Direct3D 9Ex device");
            println!("adapter: {:?}", dev.caps());
            let mesh = dev.create_mesh(&quad()).unwrap();
            let tex = dev.create_texture(&checker()).unwrap();
            self.scene = Some(Scene { dev, mesh, tex });
            self.window = Some(window);
        }

        fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
            let (Some(window), Some(s)) = (&self.window, &mut self.scene) else {
                return;
            };
            match event {
                WindowEvent::CloseRequested => el.exit(),
                WindowEvent::Resized(size) => {
                    s.dev.resize(size.width, size.height).unwrap();
                    window.request_redraw();
                }
                WindowEvent::RedrawRequested => {
                    let size = window.inner_size();
                    s.dev
                        .begin_frame(&FrameDesc {
                            width: size.width,
                            height: size.height,
                            clear_rgba: [0.1, 0.1, 0.12, 1.0],
                        })
                        .unwrap();
                    for (i, alpha) in [AlphaMode::Opaque, AlphaMode::Test, AlphaMode::Blend]
                        .into_iter()
                        .enumerate()
                    {
                        // One third of the width each, side by side.
                        let x = -2.0 / 3.0 + i as f32 * 2.0 / 3.0;
                        let mvp = [
                            0.3, 0., 0., 0., 0., 0.6, 0., 0., 0., 0., 1., 0., x, 0., 0., 1.,
                        ];
                        s.dev
                            .draw(&DrawBatch {
                                mesh: s.mesh,
                                texture: Some(s.tex),
                                alpha,
                                mvp,
                                tint: [1.; 4],
                            })
                            .unwrap();
                    }
                    s.dev.end_frame().unwrap();
                }
                _ => {}
            }
        }
    }

    let event_loop = EventLoop::new().expect("event loop");
    event_loop.run_app(&mut App::default()).expect("run");
}
