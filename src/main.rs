mod frame_timer;
use std::iter;
use std::sync::Arc;
use std::time::Instant;
use sdl2::{Sdl, VideoSubsystem};
use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::{Keycode, Mod};
use sdl2::mouse::{Cursor, MouseButton, SystemCursor};
use sdl2::video::Window;
use wgpu::{Backend, Device, Queue, Surface, SurfaceConfiguration};
use core::default::Default;
use egui::{Context, FontDefinitions, FullOutput, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, Rgba};
use egui::mutex::RwLock;
use egui_wgpu::renderer;
use egui_wgpu::renderer::RenderPass;
use egui_sdl2_event::EguiSDL2State;
use crate::frame_timer::FrameTimer;

const INITIAL_WIDTH: u32 = 800;
const INITIAL_HEIGHT: u32 = 600;

struct WGPUSDL2 {
    sdl_window: Window,
    surface: Surface,
    device: Device,
    queue: Queue,
    sdl_context: Sdl,
    sdl_video_subsystem: VideoSubsystem,
    surface_config: SurfaceConfiguration,
}

fn init_sdl(width: u32, height: u32) -> WGPUSDL2 {
    let sdl_context = sdl2::init().expect("Cannot initialize SDL2!");
    let video_subsystem = sdl_context.video().expect("Cannot get SDL2 context!");
    let window = video_subsystem
        .window("egui-sdl2-event-example", width, height)
        .position_centered()
        .resizable()
        .build()
        .map_err(|e| e.to_string()).expect("Cannot create SDL2 window!");

    let instance = wgpu::Instance::new(wgpu::Backends::PRIMARY);
    #[allow(unsafe_code)]
        let surface = unsafe { instance.create_surface(&window) };
    let adapter_opt = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: Some(&surface),
    }));
    let adapter = match adapter_opt {
        Some(a) => { a },
        None => panic!("Failed to find wgpu adapter!") ,
    };

    let (device, queue) = match pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            limits: wgpu::Limits::default(),
            label: Some("device"),
            features: wgpu::Features::empty(),
        },
        None,
    )) {
        Ok(a) => a,
        Err(e) => panic!("{}", e.to_string()),
    };

    let mut config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface.get_preferred_format(&adapter).unwrap(),
        width,
        height,
        present_mode: wgpu::PresentMode::Mailbox,
    };
    surface.configure(&device, &config);

    WGPUSDL2 {
        sdl_context: sdl_context,
        sdl_video_subsystem: video_subsystem,
        sdl_window: window,
        surface: surface,
        surface_config: config,
        device: device,
        queue: queue
    }
}

fn paint_and_update_textures(
    device: &Device,
    queue: &Queue,
    surface: &Surface,
    surface_config: &SurfaceConfiguration,
    egui_rpass: Arc<RwLock<RenderPass>>,
    pixels_per_point: f32,
    clear_color: egui::Rgba,
    clipped_primitives: &[egui::ClippedPrimitive],
    textures_delta: &egui::TexturesDelta,
) {
    let output_frame = match surface.get_current_texture() {
        Ok(frame) => frame,
        Err(wgpu::SurfaceError::Outdated) => {
            return;
        }
        Err(e) => {
            return;
        }
    };
    let output_view = output_frame
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = device.create_command_encoder(
        &wgpu::CommandEncoderDescriptor {
            label: Some("encoder"),
        });

    // Upload all resources for the GPU.
    let screen_descriptor = renderer::ScreenDescriptor {
        size_in_pixels: [surface_config.width, surface_config.height],
        pixels_per_point,
    };

    {
        let mut rpass = egui_rpass.write();
        for (id, image_delta) in &textures_delta.set {
            rpass.update_texture(&device, &queue, *id, image_delta);
        }

        rpass.update_buffers(
            &device,
            &queue,
            clipped_primitives,
            &screen_descriptor,
        );
    }

    // Record all render passes.
    egui_rpass.read().execute(
        &mut encoder,
        &output_view,
        clipped_primitives,
        &screen_descriptor,
        Some(wgpu::Color {
            r: clear_color.r() as f64,
            g: clear_color.g() as f64,
            b: clear_color.b() as f64,
            a: clear_color.a() as f64,
        }),
    );

    {
        let mut rpass = egui_rpass.write();
        for id in &textures_delta.free {
            rpass.free_texture(id);
        }
    }

    // Submit the commands.
    queue.submit(std::iter::once(encoder.finish()));

    // Redraw egui
    output_frame.present();
}


fn main() {
    let mut sys = init_sdl(INITIAL_WIDTH, INITIAL_HEIGHT);
    let mut event_pump = sys.sdl_context.event_pump().expect("Cannot create SDL2 event pump");

    let mut egui_ctx = egui::Context::default();
    let mut egui_rpass = Arc::new(RwLock::new(RenderPass::new(&sys.device, sys.surface_config.format, 1)));

    let mut frame_timer = FrameTimer::new();

    let ddpi = sys.sdl_window.subsystem().display_dpi(0).unwrap().0;
    let mut egui_sdl2_state = EguiSDL2State::new(INITIAL_WIDTH, INITIAL_HEIGHT, 1.0);

    let mut running_time: f64 = 0.0;
    let mut checkbox1_checked = false;
    'running: loop {
        frame_timer.time_start();
        let delta = frame_timer.delta();
        running_time += delta as f64;

        egui_sdl2_state.update_time(Some(running_time), delta);

        for event in event_pump.poll_iter() {
            match &event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => {
                    break 'running;
                }
                Event::Window {
                    window_id,
                    win_event: WindowEvent::SizeChanged(width, height) | WindowEvent::Resized(width, height),
                    ..
                } => {
                    if window_id.clone() == sys.sdl_window.id() {
                        let config = &mut sys.surface_config;
                        config.width = *width as u32;
                        config.height = *height as u32;
                        sys.surface.configure(&sys.device, &config);
                    }
                }
                _ => {}
            }
            egui_sdl2_state.sdl2_input_to_egui(&sys.sdl_window, &event)
        }

        let full_output = egui_ctx.run(egui_sdl2_state.raw_input.take(), |ctx| {
            egui::Window::new("Settings").resizable(true).vscroll(true).show(&ctx, |ui| {
                ui.label("Welcome!");
                ui.label("Welcome!");
                ui.label("Welcome!");
                ui.label("Welcome!");
                ui.label("Welcome!");
                ui.label("Welcome!");
                ui.label("Welcome!");
                ui.label("Welcome!");
                ui.label("Welcome!");
                ui.label("Welcome!");
                ui.label("Welcome!");
                ui.label("Welcome!");
                ui.label("Welcome!");
                ui.label("Welcome!");
                ui.label("Welcome!");
                ui.label("Welcomeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee!");

                if ui.button("Press me").clicked() {
                    println!("you pressed me!")
                }
                ui.checkbox(&mut checkbox1_checked, "checkbox1");
                ui.end_row();
            });
        });

        egui_sdl2_state.process_output(&sys.sdl_window, &full_output.platform_output);
        let tris = egui_ctx.tessellate(full_output.shapes);
        if (full_output.needs_repaint) {
            paint_and_update_textures(&sys.device,
                                      &sys.queue,
                                      &sys.surface,
                                      &sys.surface_config,
                                      egui_rpass.clone(),
                                      egui_sdl2_state.dpi_scaling,
                                      Rgba::from_rgb(0.0, 0.0, 0.0),
                                      &tris,
                                      &full_output.textures_delta)
        }
        frame_timer.time_stop()
    }
}
