use super::ascii::luminance_to_char;

/// CPU-side framebuffer holding depth and ASCII characters.
///
/// All grids are row-major and `width * height` long; index a cell as
/// `y * width + x`.
pub struct Framebuffer {
    /// Width in character cells.
    pub width: usize,
    /// Height in character cells.
    pub height: usize,
    /// Per-cell depth (z-buffer); nearer is smaller, cleared to `+∞`.
    pub depth: Vec<f32>,
    /// Per-cell ASCII character, cleared to a space.
    pub chars: Vec<char>,
    /// Per-cell linear RGB color in `[0, 1]`.
    pub colors: Vec<[f32; 3]>,
    /// Per-cell luminance in `[0, 1]` (used for color-mode shading).
    pub luminances: Vec<f32>,
}

impl Framebuffer {
    /// Create a `width`×`height` framebuffer cleared to empty (depth `+∞`, spaces).
    pub fn new(width: usize, height: usize) -> Self {
        let size = width * height;
        Self {
            width,
            height,
            depth: vec![f32::INFINITY; size],
            chars: vec![' '; size],
            colors: vec![[0.0; 3]; size],
            luminances: vec![0.0; size],
        }
    }

    /// Reset every cell to empty (depth `+∞`, space, black, zero luminance).
    pub fn clear(&mut self) {
        self.depth.fill(f32::INFINITY);
        self.chars.fill(' ');
        self.colors.iter_mut().for_each(|c| *c = [0.0; 3]);
        self.luminances.fill(0.0);
    }

    /// Depth-test and write a fragment: updates cell `(x, y)` only if `z` is
    /// nearer than the stored depth. Out-of-bounds coordinates are ignored.
    pub fn set_pixel(&mut self, x: usize, y: usize, z: f32, luminance: f32, color: [f32; 3]) {
        if x >= self.width || y >= self.height {
            return;
        }
        let idx = y * self.width + x;
        if z < self.depth[idx] {
            self.depth[idx] = z;
            self.chars[idx] = luminance_to_char(luminance);
            self.colors[idx] = color;
            self.luminances[idx] = luminance;
        }
    }

    /// Resize to `width`×`height` and clear. Call when the terminal size changes.
    pub fn resize(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
        let size = width * height;
        self.depth.resize(size, f32::INFINITY);
        self.chars.resize(size, ' ');
        self.colors.resize(size, [0.0; 3]);
        self.luminances.resize(size, 0.0);
        self.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_initializes_empty() {
        let fb = Framebuffer::new(4, 3);
        assert_eq!((fb.width, fb.height), (4, 3));
        assert_eq!(fb.chars.len(), 12);
        assert!(fb.chars.iter().all(|&c| c == ' '));
        assert!(fb.depth.iter().all(|&d| d == f32::INFINITY));
    }

    #[test]
    fn set_pixel_keeps_nearest() {
        let mut fb = Framebuffer::new(2, 2);
        fb.set_pixel(0, 0, 0.9, 1.0, [1.0, 1.0, 1.0]);
        // A nearer fragment (smaller z) overwrites.
        fb.set_pixel(0, 0, 0.1, 1.0, [1.0, 1.0, 1.0]);
        assert_eq!(fb.depth[0], 0.1);
        // A farther fragment (larger z) is rejected.
        fb.set_pixel(0, 0, 0.5, 1.0, [1.0, 1.0, 1.0]);
        assert_eq!(fb.depth[0], 0.1);
    }

    #[test]
    fn set_pixel_out_of_bounds_is_noop() {
        let mut fb = Framebuffer::new(2, 2);
        fb.set_pixel(5, 5, 0.1, 1.0, [1.0, 1.0, 1.0]);
        assert!(fb.depth.iter().all(|&d| d == f32::INFINITY));
    }

    #[test]
    fn clear_resets() {
        let mut fb = Framebuffer::new(2, 2);
        fb.set_pixel(0, 0, 0.1, 1.0, [1.0, 1.0, 1.0]);
        fb.clear();
        assert!(fb.depth.iter().all(|&d| d == f32::INFINITY));
        assert!(fb.chars.iter().all(|&c| c == ' '));
    }

    #[test]
    fn resize_changes_dimensions() {
        let mut fb = Framebuffer::new(2, 2);
        fb.resize(5, 4);
        assert_eq!((fb.width, fb.height), (5, 4));
        assert_eq!(fb.chars.len(), 20);
    }
}
