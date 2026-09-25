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
//! 快照保存在 `data/version/26.3/entity_nbt.json`，测试会重新生成并比对，
//! 因此表与随附源码不会脱节。运行时 [`catalog`] 解析该快照供语义检查使用。
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

mod aliases;
mod registry;
mod scan;

pub use aliases::{CHINESE_ALIASES, alias_of, chinese_alias};

use registry::{
    all_class_tags, collect_java_files, inherited_tags, read_entity_ids, read_entity_registrations,
};
use scan::{ClassInfo, SharedTags, collect_method_tags, collect_tags, scan_classes};

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
    let shared_tags = read_shared_tags(source_root)?;

    let mut classes: BTreeMap<String, ClassInfo> = BTreeMap::new();
    for file in &files {
        let text = fs::read_to_string(file)
            .map_err(|error| format!("无法读取 {}：{error}", file.display()))?;
        scan_classes(&text, &mut classes, &shared_tags);
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
        serde_json::Value::String("minecraft_client_26.3".to_owned()),
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

/// Resolve the ValueInput/ValueOutput keys written by shared entity helpers.
/// The method names are call sites to look for; the keys still come from 26.3 source.
fn read_shared_tags(source_root: &Path) -> Result<SharedTags, String> {
    const HELPERS: &[(&str, &[&str])] = &[
        (
            "net/minecraft/world/entity/NeutralMob.java",
            &["addPersistentAngerSaveData", "readPersistentAngerSaveData"],
        ),
        (
            "net/minecraft/world/entity/Leashable.java",
            &["readLeashData", "writeLeashData"],
        ),
        (
            "net/minecraft/world/entity/vehicle/ContainerEntity.java",
            &["addChestVehicleSaveData", "readChestVehicleSaveData"],
        ),
        (
            "net/minecraft/world/entity/variant/VariantUtils.java",
            &["writeVariant", "readVariant"],
        ),
        (
            "net/minecraft/world/entity/npc/InventoryCarrier.java",
            &["readInventoryFromTag", "writeInventoryToTag"],
        ),
    ];
    let container_helper = source_root.join("net/minecraft/world/ContainerHelper.java");
    let container_text = fs::read_to_string(&container_helper)
        .map_err(|error| format!("无法读取 {}：{error}", container_helper.display()))?;
    let container_tags = collect_tags(&container_text);
    let mut shared = SharedTags::new();
    for (relative_path, methods) in HELPERS {
        let path = source_root.join(relative_path);
        let text = fs::read_to_string(&path)
            .map_err(|error| format!("无法读取 {}：{error}", path.display()))?;
        let mut tags = collect_tags(&text);
        // ContainerEntity delegates its item list to ContainerHelper.
        if relative_path.ends_with("/ContainerEntity.java") {
            tags.extend(container_tags.clone());
        }
        for method in *methods {
            shared.insert(method, tags.clone());
        }
    }
    let food_data = source_root.join("net/minecraft/world/food/FoodData.java");
    let food_text = fs::read_to_string(&food_data)
        .map_err(|error| format!("无法读取 {}：{error}", food_data.display()))?;
    shared.insert("foodData.addAdditionalSaveData", collect_tags(&food_text));

    let nbt_utils = source_root.join("net/minecraft/nbt/NbtUtils.java");
    let utils_text = fs::read_to_string(&nbt_utils)
        .map_err(|error| format!("无法读取 {}：{error}", nbt_utils.display()))?;
    let version_tags = collect_method_tags(
        &utils_text,
        "public static void addDataVersion(final ValueOutput output",
    )
    .ok_or_else(|| format!("无法提取 {} 的 ValueOutput 数据版本键", nbt_utils.display()))?;
    shared.insert("addCurrentDataVersion", version_tags);
    Ok(shared)
}

fn is_identifier(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
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
        let text = include_str!("../../data/version/26.3/entity_nbt.json");
        EntityNbtCatalog::from_json(text)
            .expect("data/version/26.3/entity_nbt.json 必须是有效的实体 NBT 快照")
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
