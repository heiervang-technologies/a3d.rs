use super::ascii::luminance_to_char;

/// CPU-side framebuffer holding depth and ASCII characters.
pub struct Framebuffer {
    pub width: usize,
    pub height: usize,
    pub depth: Vec<f32>,
    pub chars: Vec<char>,
    pub colors: Vec<[f32; 3]>,
}

impl Framebuffer {
    pub fn new(width: usize, height: usize) -> Self {
        let size = width * height;
        Self {
            width,
            height,
            depth: vec![f32::INFINITY; size],
            chars: vec![' '; size],
            colors: vec![[0.0; 3]; size],
        }
    }

    pub fn clear(&mut self) {
        self.depth.fill(f32::INFINITY);
        self.chars.fill(' ');
        self.colors.iter_mut().for_each(|c| *c = [0.0; 3]);
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, z: f32, luminance: f32, color: [f32; 3]) {
        if x >= self.width || y >= self.height {
            return;
        }
        let idx = y * self.width + x;
        if z < self.depth[idx] {
            self.depth[idx] = z;
            self.chars[idx] = luminance_to_char(luminance);
            self.colors[idx] = color;
        }
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
        let size = width * height;
        self.depth.resize(size, f32::INFINITY);
        self.chars.resize(size, ' ');
        self.colors.resize(size, [0.0; 3]);
        self.clear();
    }
}
