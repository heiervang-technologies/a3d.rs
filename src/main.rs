use std::path::PathBuf;
use std::time::{Duration, Instant};

use clap::Parser;
use crossterm::event::KeyCode;
use glam::Vec3;

use a3d::gpu::{GpuContext, RasterPipeline};
use a3d::model::load_model;
use a3d::render::Framebuffer;
use a3d::terminal::{InputEvent, TerminalDisplay};
use a3d::{render_frame, render_frame_gpu, AZ_SPEED, AL_SPEED};

#[derive(Parser)]
#[command(name = "a3d", about = "GPU-accelerated ASCII 3D renderer")]
struct Args {
    /// Path to 3D model file (OBJ or STL)
    model: PathBuf,

    /// Target frames per second
    #[arg(short, long, default_value_t = 30)]
    fps: u32,

    /// Interactive rotation mode
    #[arg(short, long)]
    interactive: bool,

    /// Initial zoom level
    #[arg(short, long, default_value_t = 1.0)]
    zoom: f32,

    /// Enable ANSI true color output
    #[arg(short, long)]
    color: bool,

    /// Force GPU rendering
    #[arg(long)]
    gpu: bool,

    /// Force CPU rendering
    #[arg(long, conflicts_with = "gpu")]
    cpu: bool,

    /// Foreground color as hex (e.g. ff6600 or #ff6600)
    #[arg(long, value_parser = parse_hex_color)]
    fg: Option<[f32; 3]>,

    /// Background color as hex (e.g. 1a1a2e or #1a1a2e)
    #[arg(long, value_parser = parse_hex_color)]
    bg: Option<[f32; 3]>,
}

fn parse_hex_color(s: &str) -> Result<[f32; 3], String> {
    let s = s.strip_prefix('#').unwrap_or(s);
    if s.len() != 6 {
        return Err("expected 6 hex digits (e.g. ff6600)".to_string());
    }
    let r = u8::from_str_radix(&s[0..2], 16).map_err(|e| e.to_string())?;
    let g = u8::from_str_radix(&s[2..4], 16).map_err(|e| e.to_string())?;
    let b = u8::from_str_radix(&s[4..6], 16).map_err(|e| e.to_string())?;
    Ok([r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0])
}

fn main() {
    env_logger::init();
    let args = Args::parse();

    // Load model
    let mesh = load_model(&args.model);
    log::info!(
        "Loaded {} vertices, {} indices",
        mesh.vertices.len(),
        mesh.indices.len()
    );

    // Initialize GPU (optional)
    let gpu_ctx = if args.cpu {
        log::info!("CPU rendering forced");
        None
    } else {
        let ctx = pollster::block_on(GpuContext::new());
        if ctx.is_none() {
            if args.gpu {
                eprintln!("Error: --gpu requested but no GPU available");
                std::process::exit(1);
            }
            log::info!("No GPU available, falling back to CPU");
        }
        ctx
    };

    // Setup terminal
    let mut display = TerminalDisplay::new().expect("Failed to initialize terminal");
    let (w, h) = display.size();
    let mut fb = Framebuffer::new(w, h);

    // Create GPU pipeline if available
    let mut gpu_pipeline = gpu_ctx.as_ref().map(|ctx| {
        log::info!("Using GPU rendering");
        RasterPipeline::new(ctx, &mesh, w as u32, h as u32)
    });

    let use_gpu = gpu_pipeline.is_some();
    if !use_gpu {
        log::info!("Using CPU rendering");
    }

    let mut azimuth: f32 = 0.0;
    let mut altitude: f32 = 0.0;
    let mut zoom = args.zoom;
    let mut color = args.color;
    let fg_color = args.fg;
    let bg_color = args.bg;

    if fg_color.is_some() || bg_color.is_some() {
        color = true;
    }

    let frame_duration = Duration::from_secs_f64(1.0 / args.fps as f64);
    let start = Instant::now();
    let light_dir = Vec3::new(1.0, -1.0, 0.0).normalize();

    loop {
        let frame_start = Instant::now();
        let t = start.elapsed().as_secs_f32();

        // Handle input
        if let Some(ev) = display.poll_event() {
            match ev {
                InputEvent::Key(KeyCode::Char('q') | KeyCode::Esc) => break,
                InputEvent::Key(KeyCode::Up | KeyCode::Char('k')) => altitude += 0.1,
                InputEvent::Key(KeyCode::Down | KeyCode::Char('j')) => altitude -= 0.1,
                InputEvent::Key(KeyCode::Left | KeyCode::Char('h')) => azimuth += 0.1,
                InputEvent::Key(KeyCode::Right | KeyCode::Char('l')) => azimuth -= 0.1,
                InputEvent::Key(KeyCode::Char('+') | KeyCode::Char('=')) => zoom = (zoom * 1.1).min(10.0),
                InputEvent::Key(KeyCode::Char('-')) => zoom = (zoom * 0.9).max(0.1),
                InputEvent::ScrollUp => zoom = (zoom * 1.1).min(10.0),
                InputEvent::ScrollDown => zoom = (zoom * 0.9).max(0.1),
                InputEvent::Key(KeyCode::Char('c')) => color = !color,
                _ => {}
            }
        }

        // Auto-rotate if not interactive
        if !args.interactive {
            azimuth = AZ_SPEED * t;
            altitude = 0.125 * std::f32::consts::PI * (1.0 - (AL_SPEED * t).sin());
        }

        // Resize if needed
        let (w, h) = display.size();
        if w != fb.width || h != fb.height {
            fb.resize(w, h);
            if let (Some(pipeline), Some(ctx)) = (&mut gpu_pipeline, &gpu_ctx) {
                pipeline.resize(ctx, w as u32, h as u32);
            }
        }

        // Render
        if let (Some(pipeline), Some(ctx)) = (&gpu_pipeline, &gpu_ctx) {
            render_frame_gpu(&mut fb, pipeline, ctx, azimuth, altitude, zoom, light_dir, fg_color);
        } else {
            render_frame(&mut fb, &mesh, azimuth, altitude, zoom, light_dir, fg_color);
        }

        let _ = display.render(&fb, color, bg_color);

        // Frame rate limiting
        let elapsed = frame_start.elapsed();
        if elapsed < frame_duration {
            std::thread::sleep(frame_duration - elapsed);
        }
    }
}
