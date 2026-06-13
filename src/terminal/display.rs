use std::io::{self, Stdout, Write};

use crossterm::{
    ExecutableCommand, cursor,
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, MouseEvent,
        MouseEventKind,
    },
    terminal,
};

use crate::render::Framebuffer;

/// Input events that the render loop cares about.
pub enum InputEvent {
    Key(KeyCode),
    ScrollUp,
    ScrollDown,
}

pub struct TerminalDisplay {
    stdout: Stdout,
}

impl TerminalDisplay {
    pub fn new() -> io::Result<Self> {
        let mut stdout = io::stdout();
        terminal::enable_raw_mode()?;
        stdout.execute(terminal::EnterAlternateScreen)?;
        stdout.execute(cursor::Hide)?;
        stdout.execute(EnableMouseCapture)?;
        Ok(Self { stdout })
    }

    pub fn size(&self) -> (usize, usize) {
        let (w, h) = terminal::size().unwrap_or((80, 24));
        (w as usize, h as usize)
    }

    pub fn render(
        &mut self,
        fb: &Framebuffer,
        color: bool,
        bg: Option<[f32; 3]>,
        overlay: Option<&str>,
    ) -> io::Result<()> {
        use std::fmt::Write;
        // ~40 bytes per char worst case (fg + bg escape sequences)
        let cap = if color {
            fb.width * fb.height * 40 + fb.height * 10
        } else {
            fb.width * fb.height + fb.height * 10
        };
        let mut buf = String::with_capacity(cap);
        buf.push_str("\x1b[H");

        // Set background color once if specified
        if let Some(bg) = bg {
            let r = (bg[0] * 255.0) as u8;
            let g = (bg[1] * 255.0) as u8;
            let b = (bg[2] * 255.0) as u8;
            write!(buf, "\x1b[48;2;{r};{g};{b}m").unwrap();
        }

        if color {
            let mut prev_r: u8 = 0;
            let mut prev_g: u8 = 0;
            let mut prev_b: u8 = 0;
            let mut has_fg = false;

            for y in 0..fb.height {
                for x in 0..fb.width {
                    let idx = y * fb.width + x;
                    let ch = fb.chars[idx];

                    if ch == ' ' {
                        if has_fg {
                            buf.push_str("\x1b[39m");
                            has_fg = false;
                        }
                        buf.push(' ');
                    } else {
                        let lum = fb.luminances[idx];
                        let col = fb.colors[idx];
                        let r = (col[0] * lum * 255.0).clamp(0.0, 255.0) as u8;
                        let g = (col[1] * lum * 255.0).clamp(0.0, 255.0) as u8;
                        let b = (col[2] * lum * 255.0).clamp(0.0, 255.0) as u8;

                        if !has_fg || r != prev_r || g != prev_g || b != prev_b {
                            write!(buf, "\x1b[38;2;{r};{g};{b}m").unwrap();
                            prev_r = r;
                            prev_g = g;
                            prev_b = b;
                            has_fg = true;
                        }
                        buf.push(ch);
                    }
                }
                if y < fb.height - 1 {
                    buf.push_str("\r\n");
                }
            }
            if has_fg {
                buf.push_str("\x1b[39m");
            }
        } else {
            for y in 0..fb.height {
                for x in 0..fb.width {
                    buf.push(fb.chars[y * fb.width + x]);
                }
                if y < fb.height - 1 {
                    buf.push_str("\r\n");
                }
            }
        }
        // Overlay text in top-right corner (e.g. FPS counter)
        if let Some(text) = overlay {
            let (term_w, _) = terminal::size().unwrap_or((fb.width as u16, 0));
            let col = (term_w as usize).saturating_sub(text.len()) + 1;
            write!(buf, "\x1b[0m\x1b[1;{col}H{text}").unwrap();
        }

        self.stdout.write_all(buf.as_bytes())?;
        self.stdout.flush()
    }

    /// Drain all pending events, returning all actionable ones.
    /// This prevents input lag from queued mouse-move events
    /// and ensures no key presses (like 'q') are lost.
    pub fn poll_events(&self) -> Vec<InputEvent> {
        let mut events = Vec::new();
        // Cap iterations to avoid hanging if poll/read desync (crossterm edge case)
        for _ in 0..256 {
            if !event::poll(std::time::Duration::from_millis(0)).unwrap_or(false) {
                break;
            }
            match event::read() {
                Ok(Event::Key(KeyEvent { code, .. })) => events.push(InputEvent::Key(code)),
                Ok(Event::Mouse(MouseEvent { kind, .. })) => match kind {
                    MouseEventKind::ScrollUp => events.push(InputEvent::ScrollUp),
                    MouseEventKind::ScrollDown => events.push(InputEvent::ScrollDown),
                    _ => {}
                },
                _ => {}
            }
        }
        events
    }
}

impl Drop for TerminalDisplay {
    fn drop(&mut self) {
        let _ = self.stdout.execute(DisableMouseCapture);
        let _ = self.stdout.execute(cursor::Show);
        let _ = self.stdout.execute(terminal::LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}
