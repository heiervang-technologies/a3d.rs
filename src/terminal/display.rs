use std::io::{self, Stdout, Write};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent},
    terminal,
    ExecutableCommand,
};

use crate::render::Framebuffer;

pub struct TerminalDisplay {
    stdout: Stdout,
}

impl TerminalDisplay {
    pub fn new() -> io::Result<Self> {
        let mut stdout = io::stdout();
        terminal::enable_raw_mode()?;
        stdout.execute(terminal::EnterAlternateScreen)?;
        stdout.execute(cursor::Hide)?;
        Ok(Self { stdout })
    }

    pub fn size(&self) -> (usize, usize) {
        let (w, h) = terminal::size().unwrap_or((80, 24));
        (w as usize, h as usize)
    }

    pub fn render(&mut self, fb: &Framebuffer, color: bool) -> io::Result<()> {
        // ~20 bytes per colored char worst case (\x1b[38;2;RRR;GGG;BBBmC)
        let cap = if color {
            fb.width * fb.height * 20 + fb.height * 10
        } else {
            fb.width * fb.height + fb.height * 10
        };
        let mut buf = String::with_capacity(cap);
        buf.push_str("\x1b[H");

        if color {
            let mut prev_r: u8 = 0;
            let mut prev_g: u8 = 0;
            let mut prev_b: u8 = 0;
            let mut has_color = false;

            for y in 0..fb.height {
                for x in 0..fb.width {
                    let idx = y * fb.width + x;
                    let ch = fb.chars[idx];

                    if ch == ' ' {
                        if has_color {
                            buf.push_str("\x1b[0m");
                            has_color = false;
                        }
                        buf.push(' ');
                    } else {
                        let lum = fb.luminances[idx];
                        let col = fb.colors[idx];
                        let r = (col[0] * lum * 255.0).clamp(0.0, 255.0) as u8;
                        let g = (col[1] * lum * 255.0).clamp(0.0, 255.0) as u8;
                        let b = (col[2] * lum * 255.0).clamp(0.0, 255.0) as u8;

                        if !has_color || r != prev_r || g != prev_g || b != prev_b {
                            use std::fmt::Write;
                            write!(buf, "\x1b[38;2;{r};{g};{b}m").unwrap();
                            prev_r = r;
                            prev_g = g;
                            prev_b = b;
                            has_color = true;
                        }
                        buf.push(ch);
                    }
                }
                if y < fb.height - 1 {
                    buf.push_str("\r\n");
                }
            }
            if has_color {
                buf.push_str("\x1b[0m");
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
        self.stdout.write_all(buf.as_bytes())?;
        self.stdout.flush()
    }

    pub fn poll_event(&self) -> Option<KeyCode> {
        if event::poll(std::time::Duration::from_millis(0)).unwrap_or(false) {
            if let Ok(Event::Key(KeyEvent { code, .. })) = event::read() {
                return Some(code);
            }
        }
        None
    }
}

impl Drop for TerminalDisplay {
    fn drop(&mut self) {
        let _ = self.stdout.execute(cursor::Show);
        let _ = self.stdout.execute(terminal::LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}
