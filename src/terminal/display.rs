use std::io::{self, Stdout, Write};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent},
    terminal::{self, ClearType},
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

    pub fn render(&mut self, fb: &Framebuffer) -> io::Result<()> {
        self.stdout.execute(cursor::MoveTo(0, 0))?;
        self.stdout
            .execute(terminal::Clear(ClearType::All))?;

        let mut buf = String::with_capacity(fb.width * fb.height + fb.height);
        for y in 0..fb.height {
            for x in 0..fb.width {
                buf.push(fb.chars[y * fb.width + x]);
            }
            if y < fb.height - 1 {
                buf.push('\n');
            }
        }

        write!(self.stdout, "{buf}")?;
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
