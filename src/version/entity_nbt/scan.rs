use std::collections::BTreeMap;

use super::{EntityTagType, is_identifier};

/// 类的源码信息：父类与写出的全部标签（不含继承）。
#[derive(Default)]
pub(super) struct ClassInfo {
    pub(super) parent: Option<String>,
    pub(super) tags: BTreeMap<String, EntityTagType>,
}

/// 一个类在文件中的位置，用于把标签归到最内层类。
struct ClassRange {
    name: String,
    start: usize,
    end: usize,
}

pub(super) fn scan_classes(text: &str, classes: &mut BTreeMap<String, ClassInfo>) {
    let mut ranges = Vec::new();
    let mut search = 0;
    while let Some(offset) = text[search..].find("class ") {
        let index = search + offset;
        search = index + "class ".len();
        if index > 0 && text[..index].chars().next_back().is_some_and(is_identifier) {
            continue;
        }
        let Some((name, parent, open)) = parse_header(text, index) else {
            continue;
        };
        let Some(close) = matching_brace(text, open) else {
            continue;
        };
        ranges.push(ClassRange {
            name: name.clone(),
            start: open,
            end: close,
        });
        let entry = classes.entry(name).or_default();
        if entry.parent.is_none() {
            entry.parent = parent;
        }
    }

    for hit in scan_tags(text) {
        // 标签归属包含它的最内层类；嵌套类（如 Display.BlockDisplay）自然分开。
        let owner = ranges
            .iter()
            .filter(|range| range.start <= hit.position && hit.position < range.end)
            .min_by_key(|range| range.end - range.start)
            .map(|range| range.name.clone());
        if let Some(owner) = owner {
            classes
                .entry(owner)
                .or_default()
                .tags
                .entry(hit.name)
                .and_modify(|existing| *existing = existing.merge(hit.category))
                .or_insert(hit.category);
        }
    }
}

/// 类声明头：返回（类名、父类、类体的 `{` 位置）。
fn parse_header(text: &str, index: usize) -> Option<(String, Option<String>, usize)> {
    let after_class = index + "class ".len();
    let rest = &text[after_class..];
    let name_length = rest
        .char_indices()
        .take_while(|(_, character)| is_identifier(*character))
        .last()
        .map(|(offset, character)| offset + character.len_utf8())?;
    let name = rest[..name_length].to_owned();

    let header_end = text[after_class..].find('{')? + after_class;
    let header = &text[after_class..header_end];
    let parent = header.find("extends").and_then(|offset| {
        let tail = header[offset + "extends".len()..].trim_start();
        let stop = tail
            .char_indices()
            .take_while(|(_, character)| {
                is_identifier(*character) || *character == '.' || *character == '<'
            })
            .last()
            .map(|(offset, character)| offset + character.len_utf8())?;
        let token = tail[..stop].split('<').next()?.trim();
        let simple = token.rsplit('.').next()?.trim();
        (!simple.is_empty()).then(|| simple.to_owned())
    });
    Some((name, parent, header_end))
}

fn matching_brace(text: &str, open: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut index = open;
    let mut state = ScanState::Normal;
    while index < bytes.len() {
        let character = bytes[index] as char;
        match state {
            ScanState::Normal => match character {
                '/' if bytes.get(index + 1) == Some(&b'/') => state = ScanState::LineComment,
                '/' if bytes.get(index + 1) == Some(&b'*') => state = ScanState::BlockComment,
                '"' => state = ScanState::String,
                '\'' => state = ScanState::Char,
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(index);
                    }
                }
                _ => {}
            },
            ScanState::LineComment => {
                if character == '\n' {
                    state = ScanState::Normal;
                }
            }
            ScanState::BlockComment => {
                if character == '*' && bytes.get(index + 1) == Some(&b'/') {
                    state = ScanState::Normal;
                    index += 1;
                }
            }
            ScanState::String => {
                if character == '\\' {
                    index += 1;
                } else if character == '"' {
                    state = ScanState::Normal;
                }
            }
            ScanState::Char => {
                if character == '\\' {
                    index += 1;
                } else if character == '\'' {
                    state = ScanState::Normal;
                }
            }
        }
        index += 1;
    }
    None
}

#[derive(Clone, Copy)]
enum ScanState {
    Normal,
    LineComment,
    BlockComment,
    String,
    Char,
}

/// 一处标签：名称、期望类型与源码位置。
struct TagHit {
    position: usize,
    name: String,
    category: EntityTagType,
}

/// 直接写入标签的方法：`putX("键"`。
const WRITE_MARKERS: &[(&str, EntityTagType)] = &[
    ("putBoolean(", EntityTagType::Bool),
    ("putByte(", EntityTagType::Number),
    ("putShort(", EntityTagType::Number),
    ("putInt(", EntityTagType::Number),
    ("putLong(", EntityTagType::Number),
    ("putFloat(", EntityTagType::Number),
    ("putDouble(", EntityTagType::Number),
    ("putString(", EntityTagType::String),
    ("putIntArray(", EntityTagType::IntArray),
    ("putLongArray(", EntityTagType::IntArray),
    ("putByteArray(", EntityTagType::IntArray),
];

/// 读取标签的方法：`getX("键"` / `getXOr("键"`。
const READ_MARKERS: &[(&str, EntityTagType)] = &[
    ("getBooleanOr(", EntityTagType::Bool),
    ("getByteOr(", EntityTagType::Number),
    ("getShortOr(", EntityTagType::Number),
    ("getIntOr(", EntityTagType::Number),
    ("getLongOr(", EntityTagType::Number),
    ("getFloatOr(", EntityTagType::Number),
    ("getDoubleOr(", EntityTagType::Number),
    ("getStringOr(", EntityTagType::String),
    ("getString(", EntityTagType::String),
    ("getInt(", EntityTagType::Number),
    ("getLong(", EntityTagType::Number),
    ("getFloat(", EntityTagType::Number),
    ("getDouble(", EntityTagType::Number),
    ("getIntArray(", EntityTagType::IntArray),
    ("getCompound(", EntityTagType::Compound),
    ("getList(", EntityTagType::List),
];

/// 编解码器式读写：`store("键", 编解码器` / `read("键", 编解码器`。
const CODEC_MARKERS: &[&str] = &["store(", "storeNullable(", "read(", "readNullable("];

fn scan_tags(text: &str) -> Vec<TagHit> {
    let mut hits = Vec::new();
    for (marker, category) in WRITE_MARKERS.iter().chain(READ_MARKERS) {
        let pattern = format!(".{marker}");
        let mut search = 0;
        while let Some(offset) = text[search..].find(&pattern) {
            let marker_start = search + offset;
            search = marker_start + pattern.len();
            let key_start = marker_start + pattern.len();
            let Some((name, _)) = parse_string(text, key_start) else {
                continue;
            };
            hits.push(TagHit {
                position: marker_start,
                name,
                category: *category,
            });
        }
    }
    for marker in CODEC_MARKERS {
        let pattern = format!(".{marker}");
        let mut search = 0;
        while let Some(offset) = text[search..].find(&pattern) {
            let marker_start = search + offset;
            search = marker_start + pattern.len();
            let key_start = marker_start + pattern.len();
            let Some((name, key_end)) = parse_string(text, key_start) else {
                continue;
            };
            let Some(comma) = text[key_end..].find(',') else {
                continue;
            };
            let expression_start = key_end + comma + 1;
            let (expression, _) = capture_expression(text, expression_start);
            hits.push(TagHit {
                position: marker_start,
                name,
                category: classify_codec(&expression),
            });
        }
    }
    hits
}

pub(super) fn parse_string(text: &str, start: usize) -> Option<(String, usize)> {
    if !text[start..].starts_with('"') {
        return None;
    }
    let mut value = String::new();
    let index = start + 1;
    for (offset, character) in text[index..].char_indices() {
        match character {
            '"' => return Some((value, index + offset + 1)),
            '\\' => {}
            character => value.push(character),
        }
    }
    None
}

/// 截取一个实参表达式：到同层的 `,`、`)`、`;` 或行尾为止。
fn capture_expression(text: &str, start: usize) -> (String, usize) {
    let mut depth = 0i32;
    let mut index = start;
    let mut result = String::new();
    for character in text[start..].chars() {
        match character {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
            }
            ',' | ';' | '\n' if depth == 0 => break,
            _ => {}
        }
        result.push(character);
        index += character.len_utf8();
    }
    (result, index)
}

fn classify_codec(expression: &str) -> EntityTagType {
    let expression = expression.trim();
    if expression.contains("Vec3.CODEC")
        || expression.contains("Vec2.CODEC")
        || expression.contains("DropChances.CODEC")
        || expression.contains("DoubleStream")
        || expression.contains("FloatStream")
        || expression.contains("Quaternion")
        || expression.contains("Rotations")
    {
        return EntityTagType::NumericList;
    }
    if expression.contains("BlockPos.CODEC")
        || expression.contains("UUIDUtil.CODEC")
        || expression.contains("UUID.CODEC")
        || expression.contains("INT_STREAM")
        || expression.contains("IntStream")
    {
        return EntityTagType::IntArray;
    }
    if expression.contains("ComponentSerialization") {
        return EntityTagType::Component;
    }
    if expression.contains("CustomData.CODEC")
        || expression.contains("CompoundTag")
        || expression.contains("ItemStack")
        || expression.contains("BlockState")
    {
        return EntityTagType::Compound;
    }
    if expression.contains("TAG_LIST_CODEC") {
        return EntityTagType::StringList;
    }
    if expression.contains("Codec.STRING")
        || expression.contains("KEY_CODEC")
        || expression.contains("Identifier.CODEC")
        || expression.contains("ResourceKey")
        || expression.contains("Registry")
        || expression.contains("TagKey")
        || expression.contains("Holder")
    {
        return EntityTagType::String;
    }
    if expression.contains("Codec.BOOL") {
        return EntityTagType::Bool;
    }
    if expression.contains("Codec.BYTE")
        || expression.contains("Codec.SHORT")
        || expression.contains("Codec.INT")
        || expression.contains("Codec.LONG")
        || expression.contains("Codec.FLOAT")
        || expression.contains("Codec.DOUBLE")
    {
        return EntityTagType::Number;
    }
    if expression.contains("listOf") {
        return EntityTagType::List;
    }
    EntityTagType::Any
}
