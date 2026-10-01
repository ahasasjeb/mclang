//! 编译期加载的版本快照：注册表 id、枚举、命令与进度触发器字段。
//!
//! 快照由 `cargo xtask generate-version-data` 生成到
//! `data/version/<版本>/`，通过 `include_str!` 嵌入二进制。校验只对
//! `minecraft:` 命名空间的 id 生效；其它命名空间可能来自数据包，
//! 编译器不掌握其内容，返回 `None` 表示“无法判断”。
//! 注册表、枚举、命令和进度触发器数据按类别在首次查询时分别解析。

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use serde::Deserialize;
use serde_json::Value;

/// 当前随附的版本标识。
pub const VERSION: &str = "26.3";

static SNAPSHOT: Snapshot = Snapshot {
    registries: OnceLock::new(),
    enums: OnceLock::new(),
    commands: OnceLock::new(),
    triggers: OnceLock::new(),
};

/// 槽位清单：单槽、范围槽（前缀 + 数量）与多槽集合。
#[derive(Default, Deserialize)]
pub struct Slots {
    single: BTreeSet<String>,
    ranges: BTreeMap<String, usize>,
    multi: BTreeSet<String>,
}

impl Slots {
    pub fn accepts_single(&self, name: &str) -> bool {
        self.single.contains(name)
            || self.ranges.iter().any(|(prefix, count)| {
                name.strip_prefix(prefix).is_some_and(|suffix| {
                    suffix
                        .parse::<usize>()
                        .is_ok_and(|index| index < *count && index.to_string() == suffix)
                })
            })
    }

    /// 某个槽位名是否在本版本的槽位表里。
    pub fn accepts(&self, name: &str) -> bool {
        if self.single.contains(name) || self.multi.contains(name) {
            return true;
        }
        for (prefix, count) in &self.ranges {
            if name == format!("{prefix}*") {
                return true;
            }
            if let Some(suffix) = name.strip_prefix(prefix.as_str())
                && let Ok(index) = suffix.parse::<usize>()
            {
                return index < *count;
            }
        }
        false
    }

    /// 全部槽位名（范围槽展开成具体索引），用于提示与补全。
    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.single.iter().cloned().collect();
        names.extend(self.multi.iter().cloned());
        for (prefix, count) in &self.ranges {
            for index in 0..*count {
                names.push(format!("{prefix}{index}"));
            }
        }
        names.sort();
        names
    }
}

/// 版本快照。
pub struct Snapshot {
    registries: OnceLock<RegistrySnapshot>,
    enums: OnceLock<EnumSnapshot>,
    commands: OnceLock<CommandSnapshot>,
    triggers: OnceLock<TriggerSnapshot>,
}

#[derive(Deserialize)]
struct RegistrySnapshot {
    enchantment_max_levels: BTreeMap<String, u64>,
    registries: BTreeMap<String, BTreeSet<String>>,
    resource_kinds: BTreeSet<String>,
    tag_registries: BTreeSet<String>,
    slots: Slots,
}

#[derive(Deserialize)]
struct EnumSnapshot {
    enums: BTreeMap<String, Vec<String>>,
}

#[derive(Deserialize)]
struct CommandSnapshot {
    commands: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct TriggerSnapshot {
    triggers: BTreeMap<String, BTreeMap<String, String>>,
}

/// 全局快照。
pub fn snapshot() -> &'static Snapshot {
    &SNAPSHOT
}

impl Snapshot {
    pub fn enchantment_max_level(&self, id: &str) -> Option<u64> {
        self.registry_data().enchantment_max_levels.get(id).copied()
    }

    fn registry_data(&self) -> &RegistrySnapshot {
        self.registries.get_or_init(RegistrySnapshot::load)
    }

    fn enum_data(&self) -> &EnumSnapshot {
        self.enums.get_or_init(EnumSnapshot::load)
    }

    fn command_data(&self) -> &CommandSnapshot {
        self.commands.get_or_init(CommandSnapshot::load)
    }

    fn trigger_data(&self) -> &TriggerSnapshot {
        self.triggers.get_or_init(TriggerSnapshot::load)
    }

    /// 进度触发器的条件字段表：字段名 → 粗类型。
    ///
    /// 未知触发器（快照里没有条目）返回 `None`，此时不做条件校验。
    pub fn trigger_fields(&self, trigger: &str) -> Option<&BTreeMap<String, String>> {
        self.trigger_data().triggers.get(trigger)
    }

    /// 触发器条件字段的最近似拼写（编辑距离 ≤ 2）。
    pub fn suggest_trigger_field(&self, trigger: &str, field: &str) -> Option<String> {
        let fields = self.trigger_data().triggers.get(trigger)?;
        closest(field, fields.keys().map(String::as_str))
    }

    /// 注册表里是否存在该 id。
    ///
    /// 返回 `None` 表示快照不覆盖该注册表，或者 id 属于其它命名空间
    /// （可能由数据包提供），此时不做存在性判断。
    pub fn registry_contains(&self, kind: &str, id: &str) -> Option<bool> {
        let registry = self.registry_data().registries.get(kind)?;
        if namespace_of(id) != "minecraft" {
            return None;
        }
        Some(registry.contains(id))
    }

    /// Exact membership for registries whose entries are registered by client code.
    /// Unlike data-pack registries, loot condition types cannot be added by JSON.
    pub fn registry_contains_exact(&self, kind: &str, id: &str) -> Option<bool> {
        self.registry_data()
            .registries
            .get(kind)
            .map(|registry| registry.contains(id))
    }

    /// 注册表是否来自随附源码（用于决定是否给出拼写建议）。
    pub fn knows_registry(&self, kind: &str) -> bool {
        self.registry_data().registries.contains_key(kind)
    }

    /// 最接近的注册表 id（编辑距离 ≤ 2）。
    pub fn suggest_registry_id(&self, kind: &str, id: &str) -> Option<String> {
        let registry = self.registry_data().registries.get(kind)?;
        if namespace_of(id) != "minecraft" {
            return None;
        }
        closest(id, registry.iter().map(String::as_str))
    }

    /// 枚举取值，例如 `gamemode`、`display_slot`。
    pub fn enum_values(&self, name: &str) -> Option<&[String]> {
        self.enum_data().enums.get(name).map(Vec::as_slice)
    }

    /// 枚举取值是否存在。
    pub fn enum_contains(&self, name: &str, value: &str) -> bool {
        self.enum_data()
            .enums
            .get(name)
            .is_some_and(|values| values.iter().any(|candidate| candidate == value))
    }

    /// 最近似的枚举取值。
    pub fn suggest_enum(&self, name: &str, value: &str) -> Option<String> {
        let values = self.enum_data().enums.get(name)?;
        closest(value, values.iter().map(String::as_str))
    }

    /// `resource` 声明是否支持该类型。
    pub fn resource_kind_supported(&self, kind: &str) -> bool {
        let registries = self.registry_data();
        registries.resource_kinds.contains(kind)
            || kind.strip_prefix("tags/").is_some_and(|registry| {
                registry == "function" || registries.tag_registries.contains(registry)
            })
    }

    /// 槽位表。
    pub fn slots(&self) -> &Slots {
        &self.registry_data().slots
    }

    /// 根命令节点（26.3 Brigadier 树），未知时 `None`。
    pub fn root_command(&self, name: &str) -> Option<&Value> {
        self.command_data().commands.get(name)
    }

    /// 根命令是否存在。
    pub fn has_root_command(&self, name: &str) -> bool {
        self.command_data().commands.contains_key(name)
    }

    /// 根命令声明的 `requires` 权限等级（0–4）；未声明时为 `None`。
    pub fn root_command_level(&self, name: &str) -> Option<u8> {
        self.root_command(name)?
            .get("level")
            .and_then(Value::as_u64)
            .map(|level| level as u8)
    }

    /// 全部根命令名。
    pub fn command_names(&self) -> impl Iterator<Item = &str> {
        self.command_data().commands.keys().map(String::as_str)
    }
}

impl RegistrySnapshot {
    fn load() -> Self {
        serde_json::from_str(include_str!("../../data/version/26.3/registries.json"))
            .expect("registries.json 必须符合注册表快照结构")
    }
}

impl EnumSnapshot {
    fn load() -> Self {
        serde_json::from_str(include_str!("../../data/version/26.3/enums.json"))
            .expect("enums.json 必须符合枚举快照结构")
    }
}

impl CommandSnapshot {
    fn load() -> Self {
        serde_json::from_str(include_str!("../../data/version/26.3/commands.json"))
            .expect("commands.json 必须符合命令快照结构")
    }
}

impl TriggerSnapshot {
    fn load() -> Self {
        serde_json::from_str(include_str!(
            "../../data/version/26.3/advancement_triggers.json"
        ))
        .expect("advancement_triggers.json 必须符合触发器快照结构")
    }
}
fn namespace_of(id: &str) -> &str {
    id.split_once(':')
        .map(|(namespace, _)| namespace)
        .unwrap_or("minecraft")
}

/// Maximum accepted edit distance for spelling suggestions.
const MAX_SUGGESTION_DISTANCE: usize = 2;

/// 在候选集合中找编辑距离 ≤ 2 的最近者。
pub(crate) fn closest<'a>(
    value: &str,
    candidates: impl Iterator<Item = &'a str>,
) -> Option<String> {
    let value: Vec<char> = value.chars().collect();
    let mut best: Option<(usize, &str)> = None;
    let mut characters = Vec::new();
    let mut previous = Vec::new();
    let mut current = Vec::new();
    for candidate in candidates {
        if value.len().abs_diff(candidate.chars().count()) > MAX_SUGGESTION_DISTANCE {
            continue;
        }
        characters.clear();
        characters.extend(candidate.chars());
        let distance = edit_distance(&value, &characters, &mut previous, &mut current);
        if distance <= MAX_SUGGESTION_DISTANCE
            && best.as_ref().is_none_or(|(best, _)| distance < *best)
        {
            best = Some((distance, candidate));
            if distance == 0 {
                break;
            }
        }
    }
    best.map(|(_, candidate)| candidate.to_string())
}

/// Only visit cells within the accepted distance of the diagonal. Row and column
/// numbers both count consumed characters, including the empty prefix at zero.
fn edit_distance(
    left: &[char],
    right: &[char],
    previous: &mut Vec<usize>,
    current: &mut Vec<usize>,
) -> usize {
    let limit = MAX_SUGGESTION_DISTANCE + 1;
    previous.clear();
    previous.extend((0..=right.len()).map(|column| column.min(limit)));
    current.resize(right.len() + 1, limit);
    for row in 1..=left.len() {
        let first = row.saturating_sub(MAX_SUGGESTION_DISTANCE).max(1);
        let last = (row + MAX_SUGGESTION_DISTANCE).min(right.len());
        current[0] = row.min(limit);
        if first > 1 {
            current[first - 1] = limit;
        }
        for column in first..=last {
            let cost = usize::from(left[row - 1] != right[column - 1]);
            current[column] = (previous[column - 1] + cost)
                .min(previous[column] + 1)
                .min(current[column - 1] + 1)
                .min(limit);
        }
        if last < right.len() {
            current[last + 1] = limit;
        }
        std::mem::swap(previous, current);
    }
    previous[right.len()]
}

#[cfg(test)]
mod schema_tests {
    use super::*;

    #[test]
    fn missing_and_malformed_snapshot_fields_are_rejected() {
        assert!(serde_json::from_str::<RegistrySnapshot>("{}").is_err());
        assert!(serde_json::from_str::<EnumSnapshot>(r#"{"enums":{"gamemode":[1]}}"#).is_err());
        assert!(serde_json::from_str::<CommandSnapshot>(r#"{"commands":[]}"#).is_err());
        assert!(
            serde_json::from_str::<TriggerSnapshot>(r#"{"triggers":{"tick":{"player":0}}}"#)
                .is_err()
        );
        assert!(
            serde_json::from_str::<Slots>(r#"{"single":[],"multi":[],"ranges":{"hotbar.":-1}}"#)
                .is_err()
        );
        // Metadata may evolve independently; all semantic fields remain mandatory.
        assert!(serde_json::from_str::<EnumSnapshot>(r#"{"source":"test","enums":{}}"#).is_ok());
    }
}
