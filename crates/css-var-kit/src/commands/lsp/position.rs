pub fn utf16_to_byte_offset(line: &str, utf16_col: u32) -> usize {
    let mut utf16_count = 0u32;
    for (byte_idx, ch) in line.char_indices() {
        if utf16_count >= utf16_col {
            return byte_idx;
        }
        utf16_count += ch.len_utf16() as u32;
    }
    line.len()
}

pub fn byte_offset_to_utf16(line: &str, byte_offset: usize) -> u32 {
    line[..byte_offset.min(line.len())]
        .chars()
        .map(|c| c.len_utf16() as u32)
        .sum()
}

pub fn byte_col_to_utf16_in_source(source: &str, line: u32, byte_col: u32) -> u32 {
    source
        .lines()
        .nth(line as usize)
        .map(|line_str| byte_offset_to_utf16(line_str, byte_col as usize))
        .unwrap_or(0)
}
