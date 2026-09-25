//! Reproducible ZIP output for compiled data packs.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path};

use crate::compiler::CompiledPack;

/// Write an uncompressed, standards-compliant ZIP. Stored entries avoid an
/// extra compression dependency and keep output identical across platforms.
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

    let mut entries = BTreeMap::<String, Vec<u8>>::new();
    for (path, contents) in &pack.files {
        entries.insert(entry_name(path)?, contents.as_bytes().to_vec());
    }
    for (path, contents) in &pack.binary_files {
        entries.insert(entry_name(path)?, contents.clone());
    }
    let bytes = encode_stored_zip(&entries)?;
    fs::write(output, bytes).map_err(|error| format!("无法写入 {}：{error}", output.display()))
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

fn encode_stored_zip(entries: &BTreeMap<String, Vec<u8>>) -> Result<Vec<u8>, String> {
    if entries.len() > usize::from(u16::MAX) {
        return Err("ZIP 文件数超过 65535".to_owned());
    }
    let mut archive = Vec::new();
    let mut central = Vec::new();
    for (name, contents) in entries {
        let name = name.as_bytes();
        let name_len = u16::try_from(name.len()).map_err(|_| "ZIP 文件名过长".to_owned())?;
        let size = u32::try_from(contents.len()).map_err(|_| "ZIP 单文件超过 4 GiB".to_owned())?;
        let offset = u32::try_from(archive.len()).map_err(|_| "ZIP 超过 4 GiB".to_owned())?;
        let crc = crc32(contents);

        push_u32(&mut archive, 0x0403_4b50);
        push_u16(&mut archive, 20);
        push_u16(&mut archive, 0x0800); // UTF-8 names
        push_u16(&mut archive, 0); // stored
        push_u16(&mut archive, 0); // 00:00:00
        push_u16(&mut archive, 0x0021); // 1980-01-01
        push_u32(&mut archive, crc);
        push_u32(&mut archive, size);
        push_u32(&mut archive, size);
        push_u16(&mut archive, name_len);
        push_u16(&mut archive, 0);
        archive.extend_from_slice(name);
        archive.extend_from_slice(contents);

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
    }

    let central_offset = u32::try_from(archive.len()).map_err(|_| "ZIP 超过 4 GiB".to_owned())?;
    let central_size = u32::try_from(central.len()).map_err(|_| "ZIP 超过 4 GiB".to_owned())?;
    archive.extend_from_slice(&central);
    push_u32(&mut archive, 0x0605_4b50);
    push_u16(&mut archive, 0);
    push_u16(&mut archive, 0);
    push_u16(&mut archive, entries.len() as u16);
    push_u16(&mut archive, entries.len() as u16);
    push_u32(&mut archive, central_size);
    push_u32(&mut archive, central_offset);
    push_u16(&mut archive, 0);
    Ok(archive)
}

fn push_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & (0_u32.wrapping_sub(crc & 1)));
        }
    }
    !crc
}
