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

pub fn offset_to_position(source: &str, offset: usize) -> (u32, u32) {
    source[..offset]
        .bytes()
        .fold((0u32, 0u32), |(line, col), byte| {
            if byte == b'\n' {
                (line + 1, 0)
            } else {
                (line, col + 1)
            }
        })
}

pub fn position_to_byte_offset(source: &str, pos: &lsp_types::Position) -> Option<usize> {
    let line_start: usize = source
        .split_inclusive('\n')
        .take(pos.line as usize)
        .map(str::len)
        .sum();
    let line_str = source.lines().nth(pos.line as usize)?;
    Some(line_start + utf16_to_byte_offset(line_str, pos.character))
}

pub fn byte_range_to_lsp_range(
    source: &str,
    byte_range: std::ops::Range<usize>,
) -> lsp_types::Range {
    let (start_line, start_col) = offset_to_position(source, byte_range.start);
    let (end_line, end_col) = offset_to_position(source, byte_range.end);
    lsp_types::Range {
        start: lsp_types::Position {
            line: start_line,
            character: byte_col_to_utf16_in_source(source, start_line, start_col),
        },
        end: lsp_types::Position {
            line: end_line,
            character: byte_col_to_utf16_in_source(source, end_line, end_col),
        },
    }
}
