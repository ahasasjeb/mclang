//! Reproducible ZIP output for compiled data packs.

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, BufWriter, Write};
use std::path::{Component, Path};
use std::sync::OnceLock;

use crate::compiler::CompiledPack;

const WRITE_BUFFER: usize = 64 * 1024;

/// Stream stored entries in path order, borrowing file contents from the pack.
pub(crate) fn write_pack_zip(output: &Path, pack: &CompiledPack) -> Result<(), String> {
    if output.is_dir() {
        return Err(format!("ZIP 输出路径 {} 已是目录", output.display()));
    }
    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|error| format!("无法创建输出目录 {}：{error}", parent.display()))?;
    }

    let mut entries = BTreeMap::new();
    for (path, contents) in &pack.files {
        entries.insert(entry_name(path)?, contents.as_bytes());
    }
    for (path, contents) in &pack.binary_files {
        entries.insert(entry_name(path)?, contents.as_slice());
    }
    // Check all format limits before truncating a previously generated archive.
    let (central_offset, central_size) = validate_layout(&entries)?;
    let file = fs::File::create(output)
        .map_err(|error| format!("无法写入 {}：{error}", output.display()))?;
    let mut writer = BufWriter::with_capacity(WRITE_BUFFER, file);
    write_entries(&mut writer, &entries, central_offset, central_size)
        .and_then(|()| writer.flush())
        .map_err(|error| format!("无法写入 {}：{error}", output.display()))
}

fn validate_layout(entries: &BTreeMap<String, &[u8]>) -> Result<(u32, u32), String> {
    if entries.len() > usize::from(u16::MAX) {
        return Err("ZIP 文件数超过 65535".to_owned());
    }
    let mut offset = 0u64;
    let mut central_size = 0u64;
    for (name, contents) in entries {
        let name_len = u16::try_from(name.len()).map_err(|_| "ZIP 文件名过长".to_owned())?;
        let size = u32::try_from(contents.len()).map_err(|_| "ZIP 单文件超过 4 GiB".to_owned())?;
        offset += 30 + u64::from(name_len) + u64::from(size);
        central_size += 46 + u64::from(name_len);
    }
    if offset + central_size + 22 > u64::from(u32::MAX) {
        return Err("ZIP 超过 4 GiB".to_owned());
    }
    Ok((offset as u32, central_size as u32))
}

fn write_entries(
    writer: &mut impl Write,
    entries: &BTreeMap<String, &[u8]>,
    central_offset: u32,
    central_size: u32,
) -> io::Result<()> {
    let mut central = Vec::with_capacity(central_size as usize + 22);
    let mut header = Vec::new();
    // Track the logical offset: seeking a BufWriter would flush each small entry.
    let mut offset = 0u32;
    for (name, contents) in entries {
        let name = name.as_bytes();
        let name_len = name.len() as u16;
        let size = contents.len() as u32;
        let crc = crc32(contents);

        header.clear();
        push_u32(&mut header, 0x0403_4b50);
        push_u16(&mut header, 20);
        push_u16(&mut header, 0x0800); // UTF-8 names
        push_u16(&mut header, 0); // stored
        push_u16(&mut header, 0); // 00:00:00
        push_u16(&mut header, 0x0021); // 1980-01-01
        push_u32(&mut header, crc);
        push_u32(&mut header, size);
        push_u32(&mut header, size);
        push_u16(&mut header, name_len);
        push_u16(&mut header, 0);
        header.extend_from_slice(name);
        writer.write_all(&header)?;
        writer.write_all(contents)?;

        push_u32(&mut central, 0x0201_4b50);
        push_u16(&mut central, 20);
        push_u16(&mut central, 20);
        push_u16(&mut central, 0x0800);
        push_u16(&mut central, 0);
        push_u16(&mut central, 0);
        push_u16(&mut central, 0x0021);
        push_u32(&mut central, crc);
        push_u32(&mut central, size);
        push_u32(&mut central, size);
        push_u16(&mut central, name_len);
        push_u16(&mut central, 0);
        push_u16(&mut central, 0);
        push_u16(&mut central, 0);
        push_u16(&mut central, 0);
        push_u32(&mut central, 0);
        push_u32(&mut central, offset);
        central.extend_from_slice(name);
        offset += header.len() as u32 + size;
    }
    debug_assert_eq!(offset, central_offset);
    debug_assert_eq!(central.len(), central_size as usize);
    push_u32(&mut central, 0x0605_4b50);
    push_u16(&mut central, 0);
    push_u16(&mut central, 0);
    push_u16(&mut central, entries.len() as u16);
    push_u16(&mut central, entries.len() as u16);
    push_u32(&mut central, central_size);
    push_u32(&mut central, central_offset);
    push_u16(&mut central, 0);
    writer.write_all(&central)
}

fn entry_name(path: &Path) -> Result<String, String> {
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!("拒绝把不安全路径 {} 写入 ZIP", path.display()));
    }
    Ok(path.to_string_lossy().replace('\\', "/"))
}

fn push_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn crc32(bytes: &[u8]) -> u32 {
    let table = crc_table();
    let mut crc = u32::MAX;
    for byte in bytes {
        crc = table[((crc ^ u32::from(*byte)) & 0xff) as usize] ^ (crc >> 8);
    }
    !crc
}

fn crc_table() -> &'static [u32; 256] {
    static TABLE: OnceLock<[u32; 256]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = [0u32; 256];
        for (index, slot) in table.iter_mut().enumerate() {
            let mut crc = index as u32;
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0xedb8_8320 & (0_u32.wrapping_sub(crc & 1)));
            }
            *slot = crc;
        }
        table
    })
}
