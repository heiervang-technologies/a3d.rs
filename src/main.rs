#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use clap::Parser;
use crossterm::event::KeyCode;
use glam::Vec3;

use a3d::gpu::{GpuContext, RasterPipeline};
use a3d::model::load_model;
use a3d::render::Framebuffer;
use a3d::terminal::{InputEvent, TerminalDisplay};
use a3d::{AL_SPEED, AZ_SPEED, framebuffer_to_string, render_frame, render_frame_gpu};

#[derive(Parser)]
#[command(name = "a3d", about = "GPU-accelerated ASCII 3D renderer")]
struct Args {
    /// Path to 3D model file (OBJ, STL, or GLB)
    model: PathBuf,

    /// Target frames per second
    #[arg(short, long, default_value_t = 30, value_parser = clap::value_parser!(u32).range(1..))]
    fps: u32,

    /// Interactive rotation mode
    #[arg(short, long)]
    interactive: bool,

    /// Initial zoom level (0.1 to 10)
    #[arg(short, long, default_value_t = 1.0, value_parser = parse_zoom)]
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

    /// Render one plain-text frame at WIDTHxHEIGHT and exit
    #[arg(long, value_parser = parse_frame_size)]
    frame: Option<(usize, usize)>,

    /// Animation time used by --frame, in seconds
    #[arg(long, default_value_t = 0.0, requires = "frame", value_parser = parse_frame_time)]
    time: f32,
}

fn parse_frame_size(value: &str) -> Result<(usize, usize), String> {
    let (width, height) = value
        .split_once('x')
        .ok_or_else(|| "expected WIDTHxHEIGHT (for example 8x10)".to_string())?;
    let width = width
        .parse::<usize>()
        .map_err(|_| "frame width must be a positive integer".to_string())?;
    let height = height
        .parse::<usize>()
        .map_err(|_| "frame height must be a positive integer".to_string())?;
    if width == 0 || height == 0 || width > 256 || height > 256 {
        return Err("frame dimensions must each be from 1 to 256".to_string());
    }
    Ok((width, height))
}

fn parse_frame_time(value: &str) -> Result<f32, String> {
    let time = value
        .parse::<f32>()
        .map_err(|_| "time must be a non-negative number".to_string())?;
    if time.is_finite() && time >= 0.0 {
        Ok(time)
    } else {
        Err("time must be a finite non-negative number".to_string())
    }
}

fn parse_hex_color(s: &str) -> Result<[f32; 3], String> {
    let s = s.strip_prefix('#').unwrap_or(s);
    if s.len() != 6 {
        return Err("expected 6 hex digits (e.g. ff6600)".to_string());
    }
    // Parse the whole value before splitting it. Byte-indexing a UTF-8 string
    // can panic when a six-byte non-ASCII value lands between code points.
    let rgb = u32::from_str_radix(s, 16)
        .map_err(|_| "expected 6 hex digits (e.g. ff6600)".to_string())?;
    let r = ((rgb >> 16) & 0xff) as u8;
    let g = ((rgb >> 8) & 0xff) as u8;
    let b = (rgb & 0xff) as u8;
    Ok([r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0])
}

fn parse_zoom(s: &str) -> Result<f32, String> {
    let zoom = s
        .parse::<f32>()
        .map_err(|_| "expected a number from 0.1 to 10".to_string())?;
    if zoom.is_finite() && (0.1..=10.0).contains(&zoom) {
        Ok(zoom)
    } else {
        Err("expected a finite number from 0.1 to 10".to_string())
    }
}

const BENCHMARK_TARGET: Duration = Duration::from_millis(10);
const BENCHMARK_MIN_FRAMES: u32 = 3;
const BENCHMARK_MAX_FRAMES: u32 = 30;

/// Warm a renderer, then measure enough frames for a useful startup comparison
/// without delaying launch excessively on slow or very large scenes.
fn benchmark_renderer(mut render: impl FnMut(f32)) -> Duration {
    render(0.31);

    let start = Instant::now();
    let mut frames = 0;
    loop {
        render(0.31 + frames as f32 * 0.017);
        frames += 1;
        let elapsed = start.elapsed();
        if frames >= BENCHMARK_MIN_FRAMES
            && (elapsed >= BENCHMARK_TARGET || frames >= BENCHMARK_MAX_FRAMES)
        {
            return Duration::from_secs_f64(elapsed.as_secs_f64() / f64::from(frames));
        }
    }
}

fn prefer_gpu(cpu_frame_time: Duration, gpu_frame_time: Duration) -> bool {
    gpu_frame_time < cpu_frame_time
}

// Explicit --gpu fails cleanly; automatic mode logs once and drops the GPU
// backend. Callers restore the terminal before reporting a fatal error.
fn handle_gpu_error(forced: bool, error: String) -> std::io::Result<()> {
    if forced {
        Err(std::io::Error::other(format!(
            "GPU rendering failed: {error}"
        )))
    } else {
        log::warn!("GPU rendering failed: {error}; using CPU renderer");
        Ok(())
    }
}

fn main() {
    env_logger::init();
    let args = Args::parse();

    // Load model
    let mesh = match load_model(&args.model) {
        Ok(mesh) => mesh,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };
    log::info!(
        "Loaded {} vertices, {} indices",
        mesh.vertices.len(),
        mesh.indices.len()
    );

    if let Some((width, height)) = args.frame {
        let mut framebuffer = Framebuffer::new(width, height);
        let azimuth = AZ_SPEED * args.time;
        let altitude = 0.125 * std::f32::consts::PI * (1.0 - (AL_SPEED * args.time).sin());
        render_frame(
            &mut framebuffer,
            &mesh,
            azimuth,
            altitude,
            args.zoom,
            Vec3::new(1.0, -1.0, 0.0).normalize(),
            args.fg,
        );
        println!("{}", framebuffer_to_string(&framebuffer));
        return;
    }

    // Initialize GPU (optional)
    let mut gpu_ctx = if args.cpu {
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
    let mut display = match TerminalDisplay::new() {
        Ok(display) => display,
        Err(e) => {
            eprintln!("error: failed to initialize terminal: {e}");
            std::process::exit(1);
        }
    };
    let (w, h) = display.size();
    let mut fb = Framebuffer::new(w, h);

    let mut azimuth: f32 = 0.0;
    let mut altitude: f32 = 0.0;
    let mut zoom = args.zoom;
    let mut color = args.color;
    let fg_color = args.fg;
    let bg_color = args.bg;
    let light_dir = Vec3::new(1.0, -1.0, 0.0).normalize();

    if fg_color.is_some() || bg_color.is_some() {
        color = true;
    }

    // Create GPU pipeline if available
    let mut gpu_pipeline = match gpu_ctx
        .as_ref()
        .map(|ctx| RasterPipeline::new(ctx, &mesh, w as u32, h as u32))
        .transpose()
    {
        Ok(pipeline) => pipeline,
        Err(error) => {
            if let Err(error) = handle_gpu_error(args.gpu, error) {
                drop(display);
                eprintln!("error: {error}");
                std::process::exit(1);
            }
            gpu_ctx = None;
            None
        }
    };

    // In automatic mode, compare the actual model and framebuffer after GPU
    // pipeline creation. This captures triangle count, projected coverage,
    // driver behavior, and synchronous readback cost better than a static size
    // heuristic. Explicit --cpu/--gpu always bypass this choice.
    if !args.gpu {
        if let (Some(pipeline), Some(ctx)) = (gpu_pipeline.as_ref(), gpu_ctx.as_ref()) {
            let cpu_frame_time = benchmark_renderer(|angle| {
                render_frame(&mut fb, &mesh, angle, 0.2, zoom, light_dir, fg_color);
                std::hint::black_box(&fb.chars);
            });
            let mut benchmark_error = None;
            let gpu_frame_time = benchmark_renderer(|angle| {
                if benchmark_error.is_none() {
                    benchmark_error = render_frame_gpu(
                        &mut fb, pipeline, ctx, angle, 0.2, zoom, light_dir, fg_color,
                    )
                    .err();
                }
                std::hint::black_box(&fb.chars);
            });
            log::info!(
                "Startup benchmark: CPU {:.3} ms/frame, GPU {:.3} ms/frame",
                cpu_frame_time.as_secs_f64() * 1_000.0,
                gpu_frame_time.as_secs_f64() * 1_000.0,
            );
            let benchmark_failed = benchmark_error.is_some();
            if let Some(error) = benchmark_error {
                // This benchmark runs only in automatic mode.
                log::warn!("GPU benchmark failed: {error}; using CPU renderer");
            }
            if benchmark_failed || !prefer_gpu(cpu_frame_time, gpu_frame_time) {
                gpu_pipeline = None;
                gpu_ctx = None;
            }
        }
    }

    log::info!(
        "Using {} rendering",
        if gpu_pipeline.is_some() { "GPU" } else { "CPU" }
    );

    let frame_duration = Duration::from_secs_f64(1.0 / args.fps as f64);
    let start = Instant::now();
    let mut last_fps_update = Instant::now();
    let mut frame_count = 0u32;
    let mut current_fps = 0.0f32;

    let render_error = loop {
        let frame_start = Instant::now();
        let t = start.elapsed().as_secs_f32();

        // Update FPS counter
        frame_count += 1;
        let fps_elapsed = last_fps_update.elapsed().as_secs_f32();
        if fps_elapsed >= 0.5 {
            current_fps = frame_count as f32 / fps_elapsed;
            frame_count = 0;
            last_fps_update = Instant::now();
        }

        // Handle all pending input events
        let mut should_quit = false;
        for ev in display.poll_events() {
            match ev {
                InputEvent::Quit | InputEvent::Key(KeyCode::Char('q') | KeyCode::Esc) => {
                    should_quit = true
                }
                InputEvent::Key(KeyCode::Up | KeyCode::Char('k')) => altitude += 0.1,
                InputEvent::Key(KeyCode::Down | KeyCode::Char('j')) => altitude -= 0.1,
                InputEvent::Key(KeyCode::Left | KeyCode::Char('h')) => azimuth += 0.1,
                InputEvent::Key(KeyCode::Right | KeyCode::Char('l')) => azimuth -= 0.1,
                InputEvent::Key(KeyCode::Char('+') | KeyCode::Char('=')) => {
                    zoom = (zoom * 1.1).min(10.0)
                }
                InputEvent::Key(KeyCode::Char('-')) => zoom = (zoom * 0.9).max(0.1),
                InputEvent::ScrollUp => zoom = (zoom * 1.1).min(10.0),
                InputEvent::ScrollDown => zoom = (zoom * 0.9).max(0.1),
                InputEvent::Key(KeyCode::Char('c')) => color = !color,
                _ => {}
            }
        }
        if should_quit {
            break None;
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
            let resize_error = match (&mut gpu_pipeline, &gpu_ctx) {
                (Some(pipeline), Some(ctx)) => pipeline.resize(ctx, w as u32, h as u32).err(),
                _ => None,
            };
            if let Some(error) = resize_error {
                if let Err(error) = handle_gpu_error(args.gpu, error) {
                    break Some(error);
                }
                gpu_pipeline = None;
                gpu_ctx = None;
            }
        }

        // Render
        if let (Some(pipeline), Some(ctx)) = (&gpu_pipeline, &gpu_ctx) {
            if let Err(error) = render_frame_gpu(
                &mut fb, pipeline, ctx, azimuth, altitude, zoom, light_dir, fg_color,
            ) {
                if let Err(error) = handle_gpu_error(args.gpu, error) {
                    break Some(error);
                }
                gpu_pipeline = None;
                gpu_ctx = None;
            }
        }
        if gpu_pipeline.is_none() {
            render_frame(&mut fb, &mesh, azimuth, altitude, zoom, light_dir, fg_color);
        }

        let backend_label = if gpu_pipeline.is_some() { "GPU" } else { "CPU" };
        let fps_label = if current_fps > 0.0 {
            Some(format!("{:.0} FPS [{}]", current_fps, backend_label))
        } else {
            None
        };

        if let Err(e) = display.render(&fb, color, bg_color, fps_label.as_deref()) {
            break Some(e);
        }

        // Frame rate limiting
        let elapsed = frame_start.elapsed();
        if elapsed < frame_duration {
            std::thread::sleep(frame_duration - elapsed);
        }
    };

    // Restore the terminal before printing an error to the normal screen.
    drop(display);
    if let Some(e) = render_error {
        eprintln!("error: rendering failed: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_failure_policy_preserves_explicit_backend_choice() {
        assert!(handle_gpu_error(false, "device lost".into()).is_ok());
        let error = handle_gpu_error(true, "device lost".into()).unwrap_err();
        assert!(error.to_string().contains("device lost"));
    }

    #[test]
    fn parses_six_hex_digits() {
        let c = parse_hex_color("ff6600").unwrap();
        assert!((c[0] - 1.0).abs() < 1e-6);
        assert!((c[1] - 102.0 / 255.0).abs() < 1e-6);
        assert!((c[2] - 0.0).abs() < 1e-6);
    }

    #[test]
    fn leading_hash_is_optional() {
        assert_eq!(parse_hex_color("#1a2b3c"), parse_hex_color("1a2b3c"));
    }

    #[test]
    fn endpoints() {
        assert_eq!(parse_hex_color("000000").unwrap(), [0.0, 0.0, 0.0]);
        assert_eq!(parse_hex_color("ffffff").unwrap(), [1.0, 1.0, 1.0]);
    }

    #[test]
    fn rejects_wrong_length() {
        assert!(parse_hex_color("fff").is_err());
        assert!(parse_hex_color("ff66000").is_err());
        assert!(parse_hex_color("").is_err());
    }

    #[test]
    fn rejects_non_hex() {
        assert!(parse_hex_color("gggggg").is_err());
        assert!(parse_hex_color("12345z").is_err());
        assert!(parse_hex_color("aéaaa").is_err());
    }

    #[test]
    fn zoom_is_finite_and_bounded() {
        assert_eq!(parse_zoom("0.1").unwrap(), 0.1);
        assert_eq!(parse_zoom("10").unwrap(), 10.0);
        for invalid in ["0", "-1", "10.1", "NaN", "inf", "nope"] {
            assert!(parse_zoom(invalid).is_err(), "accepted {invalid}");
        }
    }

    #[test]
    fn backend_selection_uses_the_faster_renderer() {
        assert!(prefer_gpu(
            Duration::from_millis(2),
            Duration::from_millis(1)
        ));
        assert!(!prefer_gpu(
            Duration::from_millis(1),
            Duration::from_millis(2)
        ));
        assert!(!prefer_gpu(
            Duration::from_millis(1),
            Duration::from_millis(1)
        ));
    }

    #[test]
    fn frame_size_is_bounded_and_well_formed() {
        assert_eq!(parse_frame_size("8x10").unwrap(), (8, 10));
        assert!(parse_frame_size("0x10").is_err());
        assert!(parse_frame_size("8").is_err());
        assert!(parse_frame_size("999x10").is_err());
    }

    #[test]
    fn frame_time_must_be_finite_and_non_negative() {
        assert_eq!(parse_frame_time("3.5").unwrap(), 3.5);
        assert!(parse_frame_time("-1").is_err());
        assert!(parse_frame_time("NaN").is_err());
    }
}
