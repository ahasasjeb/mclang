//! 编辑器位置与编译器字节范围之间的转换。
//!
//! LSP 默认用 UTF-16 码元计数，而编译器的 [`Span`](crate::Span) 是 UTF-8 字节偏移，
//! 中文源码必须经过这里的换算才能对上。文件 URI 使用最小百分号编码。

use std::path::{Path, PathBuf};

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
pub fn offset_to_position(text: &str, offset: usize) -> (u32, u32) {
    let offset = clamp_boundary(text, offset);
    let mut line = 0u32;
    let mut line_start = 0usize;
    for (index, character) in text.char_indices() {
        if index >= offset {
            break;
        }
        if character == '\n' {
            line += 1;
            line_start = index + 1;
        }
    }
    let character = text[line_start..offset].encode_utf16().count() as u32;
    (line, character)
}

/// LSP 位置转字节偏移；超出文件末尾时落在末尾。
pub fn position_to_offset(text: &str, line: u32, character: u32) -> usize {
    let mut line_start = 0usize;
    for _ in 0..line {
        match text[line_start..].find('\n') {
            Some(index) => line_start += index + 1,
            None => return text.len(),
        }
    }
    let line_end = text[line_start..]
        .find('\n')
        .map_or(text.len(), |index| line_start + index);
    let mut units = 0u32;
    for (index, current) in text[line_start..line_end].char_indices() {
        if units >= character {
            return line_start + index;
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

fn clamp_boundary(text: &str, mut offset: usize) -> usize {
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
    fn converts_chinese_text_to_utf16_columns() {
        let text = "score 计数 = 0;\nquery 玩家 = entity(\"minecraft:player\") {}";
        let second_line = text.find("query").unwrap();
        assert_eq!(offset_to_position(text, second_line), (1, 0));
        // “玩家”在 UTF-16 里各占 1 列，与字符数一致。
        assert_eq!(
            offset_to_position(text, second_line + "query ".len()),
            (1, 6)
        );
        assert_eq!(position_to_offset(text, 1, 6), second_line + "query ".len());
        assert_eq!(position_to_offset(text, 0, 5), 5);
    }

    #[test]
    fn rounds_trip_windows_paths_through_uris() {
        let path = PathBuf::from(r"E:\zaxiang6\mclang\例子\计数.mcl");
        let uri = path_to_uri(&path);
        assert_eq!(
            uri,
            "file:///E:/zaxiang6/mclang/%E4%BE%8B%E5%AD%90/%E8%AE%A1%E6%95%B0.mcl"
        );
        assert_eq!(uri_to_path(&uri), Some(path));
    }

    #[test]
    fn extracts_words_including_chinese() {
        let text = "if 计数 >= 10 {";
        let (word, start, end) = word_at(text, 4).unwrap();
        assert_eq!(word, "计数");
        assert_eq!(&text[start..end], "计数");
        // 光标紧跟在词后（这里是空格前）也要命中前一个词。
        let (word, _, _) = word_at(text, 2).unwrap();
        assert_eq!(word, "if");
        let (word, _, _) = word_at(text, 1).unwrap();
        assert_eq!(word, "if");
        assert!(word_at("if + 1", 3).is_none());
    }
}
