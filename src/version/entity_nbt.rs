//! 实体 NBT 标签表：从 26.3 客户端源码提取，供编译器校验具名键。
//!
//! 生成器 [`generate`] 扫描 `net/minecraft/world/entity/**.java`：
//!
//! 1. 找到每个类的源码范围（含 `Display.BlockDisplay` 这类嵌套类），
//!    以及它的 `extends` 父类；
//! 2. 对每个 `.putX("键"` / `.store("键", 编解码器` / `.read("键", 编解码器`
//!    调用，按所在的最内层类归档，编解码器文本映射到粗类型；
//! 3. 从 `EntityTypeIds.java` 与 `EntityTypes.java` 建立
//!    `minecraft:<id>` → 类的映射；
//! 4. 沿继承链求并集，输出排序、可复现的 JSON。
//!
//! 快照保存在 `data/version/26.3-rc-2/entity_nbt.json`，测试会重新生成并比对，
//! 因此表与随附源码不会脱节。运行时 [`catalog`] 解析该快照供语义检查使用。

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

/// 具名 NBT 标签的期望粗类型；`Any` 表示无法从源码判定。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntityTagType {
    /// `putBoolean` / `getBooleanOr`。
    Bool,
    /// `putByte` / `putShort` / `putInt` / `putLong` / `putFloat` / `putDouble`。
    Number,
    /// `putString` / 字符串编解码器。
    String,
    /// 文本组件：字符串、复合或列表。
    Component,
    /// 元素类型不定的列表。
    List,
    /// 数值列表（`Vec3`、`Vec2`、掉率表等）。
    NumericList,
    /// 字符串列表（`Tags`）。
    StringList,
    /// 复合（自定义数据、物品堆、方块状态等）。
    Compound,
    /// 整数数组（`BlockPos`、UUID 等）。
    IntArray,
    /// 源码无法判定，不做检查。
    Any,
}

impl EntityTagType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::Number => "number",
            Self::String => "string",
            Self::Component => "component",
            Self::List => "list",
            Self::NumericList => "numeric_list",
            Self::StringList => "string_list",
            Self::Compound => "compound",
            Self::IntArray => "int_array",
            Self::Any => "any",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "bool" => Self::Bool,
            "number" => Self::Number,
            "string" => Self::String,
            "component" => Self::Component,
            "list" => Self::List,
            "numeric_list" => Self::NumericList,
            "string_list" => Self::StringList,
            "compound" => Self::Compound,
            "int_array" => Self::IntArray,
            "any" => Self::Any,
            _ => return None,
        })
    }

    /// 人类可读的期望说明，用于诊断。
    pub fn label(self) -> &'static str {
        match self {
            Self::Bool => "布尔值（true/false 或 0b/1b）",
            Self::Number => "数字",
            Self::String => "字符串",
            Self::Component => "文本组件（字符串或复合）",
            Self::List => "列表",
            Self::NumericList => "数字列表",
            Self::StringList => "字符串列表",
            Self::Compound => "复合",
            Self::IntArray => "整数数组（如 [I; 1, 2, 3]）",
            Self::Any => "任意 NBT 值",
        }
    }

    /// 同键在不同类里类型冲突时取更宽松的 `Any`。
    fn merge(self, other: Self) -> Self {
        if self == other { self } else { Self::Any }
    }
}

/// 生成实体 NBT 标签快照 JSON。
pub fn generate(source_root: &Path) -> Result<String, String> {
    let entity_root = source_root
        .join("net")
        .join("minecraft")
        .join("world")
        .join("entity");
    let mut files = Vec::new();
    collect_java_files(&entity_root, &mut files)?;
    files.sort();

    let mut classes: BTreeMap<String, ClassInfo> = BTreeMap::new();
    for file in &files {
        let text = fs::read_to_string(file)
            .map_err(|error| format!("无法读取 {}：{error}", file.display()))?;
        scan_classes(&text, &mut classes);
    }

    let ids = read_entity_ids(&source_root.join("net/minecraft/world/entity/EntityTypeIds.java"))?;
    let registrations = read_entity_registrations(
        &source_root.join("net/minecraft/world/entity/EntityTypes.java"),
    )?;

    let mut tags: BTreeMap<String, EntityTagType> = BTreeMap::new();
    let mut entities: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (identifier, class) in registrations {
        let Some(id) = ids.get(&identifier) else {
            continue;
        };
        let mut resolved = BTreeSet::new();
        inherited_tags(&classes, &class, &mut resolved, &mut HashSet::new());
        for (key, category) in all_class_tags(&classes, &class) {
            tags.entry(key)
                .and_modify(|existing| *existing = existing.merge(category))
                .or_insert(category);
        }
        entities.insert(format!("minecraft:{id}"), resolved);
    }

    let mut root = serde_json::Map::new();
    root.insert(
        "source".to_owned(),
        serde_json::Value::String("minecraft_client_26.3-rc-2".to_owned()),
    );
    let mut tag_object = serde_json::Map::new();
    for (key, category) in &tags {
        tag_object.insert(
            key.clone(),
            serde_json::Value::String(category.as_str().to_owned()),
        );
    }
    root.insert("tags".to_owned(), serde_json::Value::Object(tag_object));
    let mut entity_object = serde_json::Map::new();
    for (id, keys) in &entities {
        entity_object.insert(
            id.clone(),
            serde_json::Value::Array(
                keys.iter()
                    .map(|key| serde_json::Value::String(key.clone()))
                    .collect(),
            ),
        );
    }
    root.insert(
        "entities".to_owned(),
        serde_json::Value::Object(entity_object),
    );
    let value = serde_json::Value::Object(root);
    serde_json::to_string_pretty(&value)
        .map(|mut text| {
            text.push('\n');
            text
        })
        .map_err(|error| format!("无法序列化实体 NBT 表：{error}"))
}

/// 类的源码信息：父类与写出的全部标签（不含继承）。
#[derive(Default)]
struct ClassInfo {
    parent: Option<String>,
    tags: BTreeMap<String, EntityTagType>,
}

/// 一个类在文件中的位置，用于把标签归到最内层类。
struct ClassRange {
    name: String,
    start: usize,
    end: usize,
}

fn scan_classes(text: &str, classes: &mut BTreeMap<String, ClassInfo>) {
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

fn parse_string(text: &str, start: usize) -> Option<(String, usize)> {
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

/// `EntityTypeIds.java`：常量名 → 实体 id。
fn read_entity_ids(path: &Path) -> Result<BTreeMap<String, String>, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("无法读取 {}：{error}", path.display()))?;
    let mut ids = BTreeMap::new();
    let mut search = 0;
    while let Some(offset) = text[search..].find("create(\"") {
        let index = search + offset;
        search = index + "create(\"".len();
        let Some((id, _)) = parse_string(&text, index + "create(".len()) else {
            continue;
        };
        let name = text[..index]
            .trim_end()
            .trim_end_matches('=')
            .trim_end()
            .rsplit(|character: char| !is_identifier(character))
            .next()
            .unwrap_or_default()
            .to_owned();
        if !name.is_empty() {
            ids.insert(name, id);
        }
    }
    Ok(ids)
}

/// `EntityTypes.java`：`EntityType<类> 常量 = register(EntityTypeIds.常量,`。
fn read_entity_registrations(path: &Path) -> Result<Vec<(String, String)>, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("无法读取 {}：{error}", path.display()))?;
    let mut registrations = Vec::new();
    for line in text.lines() {
        let Some(register) = line.find("register(EntityTypeIds.") else {
            continue;
        };
        let Some(generic) = line.find("EntityType<") else {
            continue;
        };
        let type_start = generic + "EntityType<".len();
        let Some(type_end) = line[type_start..].find('>') else {
            continue;
        };
        let class = line[type_start..type_start + type_end]
            .rsplit('.')
            .next()
            .unwrap_or_default()
            .trim()
            .to_owned();
        let id_start = register + "register(EntityTypeIds.".len();
        let id_length = line[id_start..]
            .char_indices()
            .take_while(|(_, character)| is_identifier(*character))
            .last()
            .map(|(offset, character)| offset + character.len_utf8())
            .unwrap_or(0);
        let identifier = line[id_start..id_start + id_length].to_owned();
        if !class.is_empty() && !identifier.is_empty() {
            registrations.push((identifier, class));
        }
    }
    Ok(registrations)
}

/// 类沿继承链能写出的全部键。
fn inherited_tags(
    classes: &BTreeMap<String, ClassInfo>,
    class: &str,
    resolved: &mut BTreeSet<String>,
    visiting: &mut HashSet<String>,
) {
    let Some(info) = classes.get(class) else {
        return;
    };
    if visiting.contains(class) {
        return;
    }
    visiting.insert(class.to_owned());
    for key in info.tags.keys() {
        resolved.insert(key.clone());
    }
    if let Some(parent) = &info.parent {
        inherited_tags(classes, parent, resolved, visiting);
    }
}

fn all_class_tags(
    classes: &BTreeMap<String, ClassInfo>,
    class: &str,
) -> Vec<(String, EntityTagType)> {
    let mut resolved = BTreeSet::new();
    inherited_tags(classes, class, &mut resolved, &mut HashSet::new());
    let mut categories = Vec::new();
    for key in resolved {
        let category = inherited_category(classes, class, &key).unwrap_or(EntityTagType::Any);
        categories.push((key, category));
    }
    categories
}

fn inherited_category(
    classes: &BTreeMap<String, ClassInfo>,
    class: &str,
    key: &str,
) -> Option<EntityTagType> {
    let info = classes.get(class)?;
    if let Some(category) = info.tags.get(key) {
        return Some(*category);
    }
    info.parent
        .as_deref()
        .and_then(|parent| inherited_category(classes, parent, key))
}

fn collect_java_files(root: &Path, files: &mut Vec<std::path::PathBuf>) -> Result<(), String> {
    let entries =
        fs::read_dir(root).map_err(|error| format!("无法读取 {}：{error}", root.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("无法读取目录项：{error}"))?;
        let path = entry.path();
        if path.is_dir() {
            collect_java_files(&path, files)?;
        } else if path
            .extension()
            .is_some_and(|extension| extension == "java")
        {
            files.push(path);
        }
    }
    Ok(())
}

fn is_identifier(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

/// 常用实体 NBT 标签的中文别名（别名 → 规范英文键）。
///
/// 与语言关键词表一样保持中英双向一对一：每个英文键只有一个中文别名，
/// 每个别名只指向一个英文键，测试 `chinese_aliases_resolve_to_real_tags` 保证。
///
/// 别名只在 `nbt { ... }` 语句与 `set_block`/`fill` 方块实体数据的顶层键上生效：
/// 解析期归一化为英文键，产物与英文写法逐字节一致。物品 `custom_data` 的键是
/// 用户数据，不做替换；英文键始终可用，别名只覆盖数据包作者常用的标签
/// （完整标签表见快照）。
pub const CHINESE_ALIASES: &[(&str, &str)] = &[
    // 所有实体通用。
    ("自定义名称", "CustomName"),
    ("名称可见", "CustomNameVisible"),
    ("静音", "Silent"),
    ("发光", "Glowing"),
    ("无敌", "Invulnerable"),
    ("无重力", "NoGravity"),
    ("标签", "Tags"),
    ("自定义数据", "data"),
    ("坐标", "Pos"),
    ("速度", "Motion"),
    ("朝向", "Rotation"),
    ("着火时间", "Fire"),
    ("氧气", "Air"),
    ("在地面", "OnGround"),
    ("传送门冷却", "PortalCooldown"),
    ("冰冻时间", "TicksFrozen"),
    ("下落距离", "fall_distance"),
    ("无敌时间", "invulnerable_time"),
    ("队伍", "Team"),
    ("视觉火焰", "HasVisualFire"),
    // 生物（Mob / LivingEntity）。
    ("无AI", "NoAI"),
    ("生命", "Health"),
    ("持久化", "PersistenceRequired"),
    ("可拾取物品", "CanPickUpLoot"),
    ("左手", "LeftHanded"),
    ("死亡战利品表", "DeathLootTable"),
    ("死亡战利品表种子", "DeathLootTableSeed"),
    ("掉落概率", "drop_chances"),
    ("活动半径", "home_radius"),
    ("活动中心", "home_pos"),
    ("属性", "attributes"),
    ("状态效果", "active_effects"),
    ("装备", "equipment"),
    ("大脑", "Brain"),
    ("滑翔", "FallFlying"),
    ("睡眠中", "Sleeping"),
    ("睡觉位置", "sleeping_pos"),
    ("受伤时间", "HurtTime"),
    ("死亡时间", "DeathTime"),
    ("伤害吸收", "AbsorptionAmount"),
    ("求偶", "InLove"),
    ("年龄", "Age"),
    ("年龄锁定", "AgeLocked"),
    ("强制年龄", "ForcedAge"),
    ("已繁殖", "Bred"),
    ("已驯服", "Tame"),
    ("脾气", "Temper"),
    ("坐下", "Sitting"),
    ("潜行", "Crouching"),
    ("隐身", "Invisible"),
    ("标记", "Marker"),
    ("充能", "powered"),
    ("爆炸半径", "ExplosionRadius"),
    ("爆炸威力", "ExplosionPower"),
    ("已点燃", "ignited"),
    ("幼年", "IsBaby"),
    ("可破坏门", "CanBreakDoors"),
    ("可参与袭击", "CanJoinRaid"),
    ("巡逻中", "Patrolling"),
    ("巡逻队长", "PatrolLeader"),
    ("施法时间", "SpellTicks"),
    ("大小", "Size"),
    ("颜色", "Color"),
    ("变种", "Variant"),
    ("来自桶", "FromBucket"),
    ("湿度", "Moistness"),
    ("抓到鱼", "GotFish"),
    ("有花蜜", "HasNectar"),
    ("已蜇", "HasStung"),
    ("项圈颜色", "CollarColor"),
    ("交易列表", "Offers"),
    ("村民数据", "VillagerData"),
    ("传闻", "Gossips"),
    ("今日补货", "RestocksToday"),
    ("上次补货", "LastRestock"),
    ("饥饿值", "FoodLevel"),
    ("分数", "Score"),
    ("选中槽位", "SelectedItemSlot"),
    ("能力", "abilities"),
    ("经验等级", "XpLevel"),
    ("经验进度", "XpP"),
    ("经验总量", "XpTotal"),
    ("经验种子", "XpSeed"),
    ("上次死亡位置", "LastDeathLocation"),
    ("睡眠计时", "SleepTimer"),
    ("兔子类型", "RabbitType"),
    ("膨胀状态", "PuffState"),
    ("南瓜头", "Pumpkin"),
    ("杀手兔", "Johnny"),
    ("信任", "Trusting"),
    ("鸡骑士", "IsChickenJockey"),
    ("免疫僵尸化", "IsImmuneToZombification"),
    ("尖叫山羊", "IsScreamingGoat"),
    ("有左角", "HasLeftHorn"),
    ("有右角", "HasRightHorn"),
    ("吃草", "EatingHaystack"),
    ("携带箱子", "ChestedHorse"),
    ("驮运强度", "Strength"),
    ("消失延迟", "DespawnDelay"),
    ("声音变种", "sound_variant"),
    // 物品、箭矢与投射物。
    ("拾取延迟", "PickupDelay"),
    ("穿透等级", "PierceLevel"),
    ("暴击", "crit"),
    ("可拾取", "pickup"),
    ("斜射", "ShotAtAngle"),
    // 展示实体与方块实体数据。
    ("方块状态", "BlockState"),
    ("方块实体数据", "TileEntityData"),
    ("文本", "text"),
    ("背景色", "background"),
    ("对齐", "alignment"),
    ("行宽", "line_width"),
    ("文本透明度", "text_opacity"),
    ("亮度", "brightness"),
    ("发光颜色", "glow_color_override"),
    ("变换", "transformation"),
    ("插值时长", "interpolation_duration"),
    ("传送时长", "teleport_duration"),
    ("可视距离", "view_range"),
    ("阴影半径", "shadow_radius"),
    ("阴影强度", "shadow_strength"),
    ("宽度", "width"),
    ("高度", "height"),
    ("展示方块", "block_state"),
    ("展示物品", "item_display"),
    ("烟花物品", "FireworksItem"),
];

/// 中文别名 → 规范英文键。
pub fn chinese_alias(word: &str) -> Option<&'static str> {
    CHINESE_ALIASES
        .iter()
        .find(|(alias, _)| *alias == word)
        .map(|(_, key)| *key)
}

/// 规范英文键 → 首选中文别名；用于诊断里同时给出两种写法。
pub fn alias_of(key: &str) -> Option<&'static str> {
    CHINESE_ALIASES
        .iter()
        .find(|(_, canonical)| *canonical == key)
        .map(|(alias, _)| *alias)
}

/// 运行时标签目录：解析随附快照。
pub struct EntityNbtCatalog {
    tags: BTreeMap<String, EntityTagType>,
    entities: HashMap<String, HashSet<String>>,
}

static CATALOG: OnceLock<EntityNbtCatalog> = OnceLock::new();

/// 进程内共享的实体 NBT 标签目录。
pub fn catalog() -> &'static EntityNbtCatalog {
    CATALOG.get_or_init(|| {
        let text = include_str!("../../data/version/26.3-rc-2/entity_nbt.json");
        EntityNbtCatalog::from_json(text)
            .expect("data/version/26.3-rc-2/entity_nbt.json 必须是有效的实体 NBT 快照")
    })
}

impl EntityNbtCatalog {
    fn from_json(text: &str) -> Result<Self, String> {
        let root: serde_json::Value =
            serde_json::from_str(text).map_err(|error| format!("快照 JSON 无效：{error}"))?;
        let mut tags = BTreeMap::new();
        if let Some(object) = root.get("tags").and_then(serde_json::Value::as_object) {
            for (key, value) in object {
                let Some(category) = value.as_str().and_then(EntityTagType::parse) else {
                    return Err(format!("标签 `{key}` 的类型无法识别"));
                };
                tags.insert(key.clone(), category);
            }
        }
        let mut entities = HashMap::new();
        if let Some(object) = root.get("entities").and_then(serde_json::Value::as_object) {
            for (id, value) in object {
                let keys = value
                    .as_array()
                    .ok_or_else(|| format!("实体 `{id}` 的标签表必须是数组"))?
                    .iter()
                    .filter_map(|key| key.as_str().map(str::to_owned))
                    .collect::<HashSet<_>>();
                let normalized = id.strip_prefix("minecraft:").unwrap_or(id);
                entities.insert(normalized.to_owned(), keys);
            }
        }
        Ok(Self { tags, entities })
    }

    /// 某个实体类型允许的标签；未知类型返回 `None`。
    pub fn entity_tags(&self, entity_type: &str) -> Option<&HashSet<String>> {
        let normalized = entity_type
            .strip_prefix("minecraft:")
            .unwrap_or(entity_type);
        self.entities.get(normalized)
    }

    /// 全部实体标签的并集，供实体类型未知的上下文使用。
    pub fn knows(&self, key: &str) -> bool {
        self.tags.contains_key(key)
    }

    /// 标签的期望类型；未知键返回 `None`。
    pub fn tag_type(&self, key: &str) -> Option<EntityTagType> {
        self.tags.get(key).copied()
    }

    /// 与给定键最接近的候选的展示文本（已带反引号），用于诊断提示。
    ///
    /// 候选同时考虑英文键与可用中文别名：`NoAi` → `` `NoAI` ``，
    /// `无ai` → `` `无AI`（英文 `NoAI`） ``。
    pub fn suggest(&self, key: &str, allowed: Option<&HashSet<String>>) -> Option<String> {
        let allowed = allowed.cloned();
        let accepts = |candidate: &str| {
            allowed
                .as_ref()
                .is_none_or(|allowed| allowed.contains(candidate))
        };
        let mut best: Option<(usize, String)> = None;
        for candidate in self.tags.keys().filter(|candidate| accepts(candidate)) {
            let distance = edit_distance(&fold(key), &fold(candidate));
            if distance > 2 {
                continue;
            }
            let display = format!("`{candidate}`");
            keep_closer(&mut best, distance, display);
        }
        for (alias, canonical) in CHINESE_ALIASES {
            if !accepts(canonical) {
                continue;
            }
            let distance = edit_distance(&fold(key), &fold(alias));
            if distance > 2 {
                continue;
            }
            let display = format!("`{alias}`（英文 `{canonical}`）");
            keep_closer(&mut best, distance, display);
        }
        best.map(|(_, display)| display)
    }
}

/// ASCII 大小写折叠：`NoAi` 与 `NoAI` 视为完全一致，便于候选排序。
fn fold(text: &str) -> String {
    text.chars()
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

/// 保留更接近的候选；距离相同时保留更早出现的（英文键优先）。
fn keep_closer(best: &mut Option<(usize, String)>, distance: usize, display: String) {
    if best.as_ref().is_none_or(|(current, _)| distance < *current) {
        *best = Some((distance, display));
    }
}

/// 简单编辑距离；键很短，直接两行动态规划即可。
fn edit_distance(left: &str, right: &str) -> usize {
    let left = left.chars().collect::<Vec<_>>();
    let right = right.chars().collect::<Vec<_>>();
    let mut previous = (0..=right.len()).collect::<Vec<_>>();
    for (i, left_character) in left.iter().enumerate() {
        let mut current = vec![i + 1];
        for (j, right_character) in right.iter().enumerate() {
            let substitution = previous[j] + usize::from(left_character != right_character);
            let insertion = current[j] + 1;
            let deletion = previous[j + 1] + 1;
            current.push(substitution.min(insertion).min(deletion));
        }
        previous = current;
    }
    previous[right.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source_root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("minecraft_client_26.3-rc-2")
    }

    /// 快照必须与随附源码一致；运行 `cargo run --bin generate-version-data` 重新生成。
    #[test]
    fn committed_snapshot_matches_the_source_tree() {
        if !source_root().exists() {
            return;
        }
        let generated = generate(&source_root()).expect("生成实体 NBT 表失败");
        let committed = include_str!("../../data/version/26.3-rc-2/entity_nbt.json");
        assert_eq!(generated, committed);
    }

    #[test]
    fn catalog_knows_common_mob_tags() {
        let catalog = catalog();
        assert_eq!(catalog.tag_type("NoAI"), Some(EntityTagType::Bool));
        assert!(
            catalog
                .entity_tags("minecraft:zombie")
                .unwrap()
                .contains("NoAI")
        );
        assert!(
            catalog
                .entity_tags("minecraft:zombie")
                .unwrap()
                .contains("IsBaby")
        );
        assert!(
            catalog
                .entity_tags("minecraft:zombie")
                .unwrap()
                .contains("CustomName")
        );
        assert!(
            !catalog
                .entity_tags("minecraft:zombie")
                .unwrap()
                .contains("Strength")
        );
        assert!(
            catalog
                .entity_tags("minecraft:llama")
                .unwrap()
                .contains("Strength")
        );
        assert_eq!(
            catalog.suggest("NoAi", catalog.entity_tags("minecraft:zombie")),
            Some("`NoAI`".to_owned())
        );
    }

    /// 每个中文别名都必须指向快照里真实存在的标签，且中英双向都是一对一。
    #[test]
    fn chinese_aliases_resolve_to_real_tags() {
        let catalog = catalog();
        let mut aliases = HashSet::new();
        let mut canonicals = HashSet::new();
        for (alias, canonical) in CHINESE_ALIASES {
            assert!(!alias.is_empty(), "别名不能为空");
            assert!(
                catalog.knows(canonical),
                "别名 `{alias}` 指向未知标签 `{canonical}`"
            );
            assert!(aliases.insert(*alias), "别名 `{alias}` 重复");
            assert!(
                canonicals.insert(*canonical),
                "标签 `{canonical}` 对应多个中文别名"
            );
        }
    }

    #[test]
    fn suggestions_understand_chinese_aliases() {
        let catalog = catalog();
        assert_eq!(
            catalog.suggest("无ai", catalog.entity_tags("minecraft:zombie")),
            Some("`无AI`（英文 `NoAI`）".to_owned())
        );
        assert_eq!(chinese_alias("静音"), Some("Silent"));
        assert_eq!(chinese_alias("自定义名称"), Some("CustomName"));
        assert_eq!(chinese_alias("不存在"), None);
    }
}
