/// ASCII luminance ramp from darkest to brightest (matches voxcii).
pub const ASCII_RAMP: &[u8] = b".,':;!+*=#$@";

/// Map a luminance value [0.0, 1.0] to an ASCII character.
pub fn luminance_to_char(luminance: f32) -> char {
    let clamped = luminance.clamp(0.0, 1.0);
    // `round_ties_even` matches WGSL `round` (round half to even); the GPU shader
    // rounds the same way, so CPU and GPU pick identical ramp characters at
    // exact half-bucket luminances. This is what keeps the two backends
    // byte-identical (see the `gpu_compare` pixel-for-pixel invariant).
    let idx = (clamped * (ASCII_RAMP.len() - 1) as f32).round_ties_even() as usize;
    let idx = idx.min(ASCII_RAMP.len() - 1);
    ASCII_RAMP[idx] as char
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ramp_has_twelve_levels() {
        assert_eq!(ASCII_RAMP.len(), 12);
    }

    #[test]
    fn endpoints_map_to_ramp_ends() {
        assert_eq!(luminance_to_char(0.0), '.');
        assert_eq!(luminance_to_char(1.0), '@');
    }

    #[test]
    fn out_of_range_is_clamped() {
        assert_eq!(luminance_to_char(-1.0), '.');
        assert_eq!(luminance_to_char(2.0), '@');
    }

    #[test]
    fn midpoint_maps_to_middle_of_ramp() {
        // round(0.5 * 11) == 6 -> ASCII_RAMP[6] == '+'
        assert_eq!(luminance_to_char(0.5), '+');
    }

    #[test]
    fn brightness_is_monotonic() {
        // Increasing luminance never moves backward along the ramp.
        let mut prev = 0usize;
        for i in 0..=10 {
            let c = luminance_to_char(i as f32 / 10.0);
            let pos = ASCII_RAMP.iter().position(|&b| b as char == c).unwrap();
            assert!(pos >= prev, "luminance {i}/10 regressed ramp position");
            prev = pos;
        }
    }
}
