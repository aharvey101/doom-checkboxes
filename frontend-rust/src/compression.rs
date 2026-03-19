//! RLE compression and palette handling for infinite chunks
//!
//! Chunk data format:
//! - Palette: [count: u8, r1, g1, b1, r2, g2, b2, ...]
//! - RLE data: [count_high: u8, count_low: u8, value: u8] repeated
//!
//! Each checkbox is 1 byte: 0 = unchecked, 1-255 = palette index (checked)

/// Decode RLE-compressed data to raw checkbox bytes
/// Returns Vec<u8> where each byte is a palette index (0 = unchecked)
pub fn rle_decode(encoded: &[u8], expected_len: usize) -> Vec<u8> {
    let mut result = Vec::with_capacity(expected_len);
    let mut i = 0;

    while i + 2 < encoded.len() && result.len() < expected_len {
        let count = ((encoded[i] as u16) << 8) | (encoded[i + 1] as u16);
        let value = encoded[i + 2];

        for _ in 0..count {
            result.push(value);
            if result.len() >= expected_len {
                break;
            }
        }
        i += 3;
    }

    // Pad with zeros if needed
    result.resize(expected_len, 0);
    result
}

/// Decode palette from bytes
/// Returns Vec of (R, G, B) tuples, index 0 is unused (unchecked)
pub fn palette_decode(data: &[u8]) -> Vec<(u8, u8, u8)> {
    if data.is_empty() {
        return Vec::new();
    }

    let count = data[0] as usize;
    let mut colors = Vec::with_capacity(count);

    let mut i = 1;
    while i + 2 < data.len() && colors.len() < count {
        colors.push((data[i], data[i + 1], data[i + 2]));
        i += 3;
    }

    colors
}

/// Decompress chunk data to RGBA texture data
/// Input: palette bytes + RLE compressed checkbox data
/// Output: RGBA bytes (4 bytes per checkbox) ready for WebGL texture
pub fn decompress_chunk_to_rgba(
    palette_data: &[u8],
    rle_data: &[u8],
    unchecked_color: (u8, u8, u8),
) -> Vec<u8> {
    const CHECKBOXES_PER_CHUNK: usize = 1_000_000;

    let palette = palette_decode(palette_data);
    let checkbox_data = rle_decode(rle_data, CHECKBOXES_PER_CHUNK);

    let mut rgba = Vec::with_capacity(CHECKBOXES_PER_CHUNK * 4);

    for &palette_idx in &checkbox_data {
        if palette_idx == 0 {
            // Unchecked
            rgba.push(unchecked_color.0);
            rgba.push(unchecked_color.1);
            rgba.push(unchecked_color.2);
            rgba.push(0x00); // unchecked flag
        } else if let Some(&(r, g, b)) = palette.get(palette_idx as usize - 1) {
            // Checked with color from palette
            rgba.push(r);
            rgba.push(g);
            rgba.push(b);
            rgba.push(0xFF); // checked flag
        } else {
            // Invalid palette index, treat as unchecked
            rgba.push(unchecked_color.0);
            rgba.push(unchecked_color.1);
            rgba.push(unchecked_color.2);
            rgba.push(0x00);
        }
    }

    rgba
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rle_decode_empty() {
        let decoded = rle_decode(&[], 100);
        assert_eq!(decoded.len(), 100);
        assert!(decoded.iter().all(|&x| x == 0));
    }

    #[test]
    fn test_rle_decode_single_run() {
        // One run of 10 zeros
        let encoded = vec![0, 10, 0]; // count=10, value=0
        let decoded = rle_decode(&encoded, 10);
        assert_eq!(decoded, vec![0; 10]);
    }

    #[test]
    fn test_rle_decode_multiple_runs() {
        // 5 zeros, 3 ones, 2 zeros
        let encoded = vec![
            0, 5, 0, // 5 zeros
            0, 3, 1, // 3 ones
            0, 2, 0, // 2 zeros
        ];
        let decoded = rle_decode(&encoded, 10);
        assert_eq!(decoded, vec![0, 0, 0, 0, 0, 1, 1, 1, 0, 0]);
    }

    #[test]
    fn test_rle_decode_large_count() {
        // Run of 1000 (requires 2 bytes for count)
        let encoded = vec![3, 232, 5]; // count=1000 (0x03E8), value=5
        let decoded = rle_decode(&encoded, 1000);
        assert_eq!(decoded.len(), 1000);
        assert!(decoded.iter().all(|&x| x == 5));
    }

    #[test]
    fn test_palette_decode_empty() {
        let palette = palette_decode(&[]);
        assert!(palette.is_empty());
    }

    #[test]
    fn test_palette_decode_single_color() {
        let data = vec![1, 255, 0, 0]; // 1 color: red
        let palette = palette_decode(&data);
        assert_eq!(palette.len(), 1);
        assert_eq!(palette[0], (255, 0, 0));
    }

    #[test]
    fn test_palette_decode_multiple_colors() {
        let data = vec![
            3, // 3 colors
            255, 0, 0, // red
            0, 255, 0, // green
            0, 0, 255, // blue
        ];
        let palette = palette_decode(&data);
        assert_eq!(palette.len(), 3);
        assert_eq!(palette[0], (255, 0, 0));
        assert_eq!(palette[1], (0, 255, 0));
        assert_eq!(palette[2], (0, 0, 255));
    }

    #[test]
    fn test_decompress_chunk_empty() {
        // Empty palette, all unchecked
        let palette_data = vec![0]; // 0 colors
        let rle_data = vec![0, 10, 0]; // 10 unchecked

        let rgba = decompress_chunk_to_rgba(&palette_data, &rle_data, (44, 62, 80));

        // First checkbox should be unchecked color
        assert_eq!(rgba[0], 44); // R
        assert_eq!(rgba[1], 62); // G
        assert_eq!(rgba[2], 80); // B
        assert_eq!(rgba[3], 0x00); // unchecked flag
    }

    #[test]
    fn test_decompress_chunk_with_checked() {
        // 1 color in palette (red), some checkboxes checked
        let palette_data = vec![1, 255, 0, 0]; // 1 color: red
        let rle_data = vec![
            0, 2, 0, // 2 unchecked
            0, 1, 1, // 1 checked (palette index 1)
            0, 2, 0, // 2 unchecked
        ];

        let rgba = decompress_chunk_to_rgba(&palette_data, &rle_data, (0, 0, 0));

        // Checkbox 0: unchecked
        assert_eq!(&rgba[0..4], &[0, 0, 0, 0x00]);

        // Checkbox 2: checked red
        assert_eq!(&rgba[8..12], &[255, 0, 0, 0xFF]);

        // Checkbox 3: unchecked
        assert_eq!(&rgba[12..16], &[0, 0, 0, 0x00]);
    }
}
