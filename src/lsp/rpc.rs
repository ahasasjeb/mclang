//! LSP 的 stdio 传输：按 `Content-Length` 头读写 JSON-RPC 消息。
//!
//! 协议只要求 `Content-Length`，其余头（如 `Content-Type`）忽略；解析头时兼容
//! `\r\n` 与 `\n` 两种行尾，便于用普通文本管道调试。

use std::io::{self, BufRead, ErrorKind, Write};

use serde_json::Value;

/// 读取一条消息；流结束时返回 `None`。
pub fn read_message(reader: &mut impl BufRead) -> io::Result<Option<Value>> {
    let mut length = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        let header = line.trim_end_matches(['\r', '\n']);
        if header.is_empty() {
            break;
        }
        if let Some(value) = header.strip_prefix("Content-Length:") {
            length = Some(value.trim().parse::<usize>().map_err(|error| {
                io::Error::new(
                    ErrorKind::InvalidData,
                    format!("无效的 Content-Length：{error}"),
                )
            })?);
        }
    }
    let length = length
        .ok_or_else(|| io::Error::new(ErrorKind::InvalidData, "消息缺少 Content-Length 头"))?;
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body)?;
    let message = serde_json::from_slice(&body).map_err(|error| {
        io::Error::new(
            ErrorKind::InvalidData,
            format!("消息不是合法 JSON：{error}"),
        )
    })?;
    Ok(Some(message))
}

/// 写入一条消息并立即刷新。
pub fn write_message(writer: &mut impl Write, message: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(message).map_err(|error| {
        io::Error::new(ErrorKind::InvalidData, format!("无法序列化消息：{error}"))
    })?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    writer.flush()
}
