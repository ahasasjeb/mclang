//! 编辑器位置与编译器字节范围之间的转换。
//!
//! LSP 默认用 UTF-16 码元计数，而编译器的 [`Span`](crate::Span) 是 UTF-8 字节偏移，
//! 中文源码必须经过这里的换算才能对上。文件 URI 使用最小百分号编码。

use std::path::{Path, PathBuf};

use crate::lines::LineIndex;

/// 文件路径转 `file://` URI，非保留字节按 UTF-8 百分号编码。
pub fn path_to_uri(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    let mut uri = String::from("file://");
    if !text.starts_with('/') {
        uri.push('/');
    }
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' | b':' => {
                uri.push(byte as char);
            }
            _ => uri.push_str(&format!("%{byte:02X}")),
        }
    }
    uri
}

/// `file://` URI 转文件路径；非 `file` 协议或解析失败时返回 `None`。
pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let rest = rest.split(['?', '#']).next().unwrap_or(rest);
    let decoded = percent_decode(rest);
    let text = decoded.strip_prefix('/').unwrap_or(&decoded);
    // Windows 的 `file:///E:/…` 解码后是 `/E:/…`，需要去掉盘符前的斜杠；
    // Unix 路径本身以 `/` 开头，保持原样。
    let text = if cfg!(windows) && text.len() >= 2 && text.as_bytes()[1] == b':' {
        text
    } else {
        decoded.as_str()
    };
    Some(PathBuf::from(
        text.replace('/', std::path::MAIN_SEPARATOR_STR),
    ))
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or("");
            if let Ok(byte) = u8::from_str_radix(hex, 16) {
                decoded.push(byte);
                index += 3;
                continue;
            }
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

/// 字节偏移转 LSP 位置（行号与 UTF-16 列号都从 0 开始）。
///
pub fn offset_to_position_in(text: &str, offset: usize, index: &LineIndex) -> (u32, u32) {
    index.utf16_position(text, offset)
}

/// LSP 位置转字节偏移；索引由项目复用，超出文件末尾时落在末尾。
pub fn position_to_offset_in(text: &str, line: u32, character: u32, index: &LineIndex) -> usize {
    let line_start = index.line_start(line as usize);
    let line_end = text[line_start..]
        .find('\n')
        .map_or(text.len(), |position| line_start + position);
    let mut units = 0u32;
    for (position, current) in text[line_start..line_end].char_indices() {
        if units >= character {
            return line_start + position;
        }
        units += current.len_utf16() as u32;
    }
    line_end
}

/// 取偏移处的标识符；偏移落在一个词里或紧跟在词后都能命中。
pub fn word_at(text: &str, offset: usize) -> Option<(String, usize, usize)> {
    let offset = clamp_boundary(text, offset);
    let mut start = offset;
    while start > 0 {
        let previous = text[..start].chars().next_back()?;
        if identifier_continue(previous) {
            start -= previous.len_utf8();
        } else {
            break;
        }
    }
    let mut end = offset;
    while end < text.len() {
        let character = text[end..].chars().next()?;
        if identifier_continue(character) {
            end += character.len_utf8();
        } else {
            break;
        }
    }
    if start == end {
        None
    } else {
        Some((text[start..end].to_owned(), start, end))
    }
}

fn identifier_continue(character: char) -> bool {
    character == '_' || character.is_alphanumeric()
}

pub(super) fn clamp_boundary(text: &str, mut offset: usize) -> usize {
    offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_positions_round_trip_and_out_of_range_lines_reach_eof() {
        for text in ["", "变量🚀x\n第二行\n", "变量\r\n🚀末尾"] {
            let index = LineIndex::new(text);
            for offset in text
                .char_indices()
                .map(|(offset, _)| offset)
                .chain([text.len()])
            {
                let (line, character) = offset_to_position_in(text, offset, &index);
                assert_eq!(position_to_offset_in(text, line, character, &index), offset);
            }
            assert_eq!(
                position_to_offset_in(text, u32::MAX, u32::MAX, &index),
                text.len()
            );
        }
        let text = "🚀x";
        let index = LineIndex::new(text);
        assert_eq!(position_to_offset_in(text, 0, 1, &index), "🚀".len());
        assert_eq!(position_to_offset_in(text, 0, u32::MAX, &index), text.len());
    }
}
