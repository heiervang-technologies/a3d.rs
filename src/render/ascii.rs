/// ASCII luminance ramp from darkest to brightest (matches voxcii).
pub const ASCII_RAMP: &[u8] = b".,':;!+*=#$@";

/// Map a luminance value [0.0, 1.0] to an ASCII character.
pub fn luminance_to_char(luminance: f32) -> char {
    let clamped = luminance.clamp(0.0, 1.0);
    let idx = (clamped * (ASCII_RAMP.len() - 1) as f32).round() as usize;
    let idx = idx.min(ASCII_RAMP.len() - 1);
    ASCII_RAMP[idx] as char
}
