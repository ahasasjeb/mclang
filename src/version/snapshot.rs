//! 编译期加载的版本快照：注册表 id、枚举、命令与进度触发器字段。
//!
//! 快照由 `cargo xtask generate-version-data` 生成到
//! `data/version/<版本>/`，通过 `include_str!` 嵌入二进制。校验只对
//! `minecraft:` 命名空间的 id 生效；其它命名空间可能来自数据包，
//! 编译器不掌握其内容，返回 `None` 表示“无法判断”。
//! 注册表、枚举、命令和进度触发器数据按类别在首次查询时分别解析。

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

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
#[derive(Default)]
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
    fn from_value(value: Option<&Value>) -> Self {
        let Some(object) = value.and_then(Value::as_object) else {
            return Self::default();
        };
        let ranges = object
            .get("ranges")
            .and_then(Value::as_object)
            .map(|ranges| {
                ranges
                    .iter()
                    .filter_map(|(prefix, count)| {
                        count.as_u64().map(|count| (prefix.clone(), count as usize))
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self {
            single: string_set(object.get("single")),
            ranges,
            multi: string_set(object.get("multi")),
        }
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

struct RegistrySnapshot {
    enchantment_max_levels: BTreeMap<String, u64>,
    registries: BTreeMap<String, BTreeSet<String>>,
    resource_kinds: BTreeSet<String>,
    tag_registries: BTreeSet<String>,
    slots: Slots,
}

struct EnumSnapshot {
    enums: BTreeMap<String, Vec<String>>,
}

struct CommandSnapshot {
    commands: BTreeMap<String, Value>,
}

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
        let registries = parse_json(include_str!("../../data/version/26.3/registries.json"));
        Self {
            enchantment_max_levels: serde_json::from_value(
                registries
                    .get("enchantment_max_levels")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({})),
            )
            .expect("valid enchantment level snapshot"),
            registries: object_sets(registries.get("registries")),
            resource_kinds: string_set(registries.get("resource_kinds")),
            tag_registries: string_set(registries.get("tag_registries")),
            slots: Slots::from_value(registries.get("slots")),
        }
    }
}

impl EnumSnapshot {
    fn load() -> Self {
        let enums = parse_json(include_str!("../../data/version/26.3/enums.json"));
        Self {
            enums: object_arrays(enums.get("enums")),
        }
    }
}

impl CommandSnapshot {
    fn load() -> Self {
        let commands = parse_json(include_str!("../../data/version/26.3/commands.json"));
        Self {
            commands: commands
                .get("commands")
                .and_then(Value::as_object)
                .map(|object| {
                    object
                        .iter()
                        .map(|(name, node)| (name.clone(), node.clone()))
                        .collect()
                })
                .unwrap_or_default(),
        }
    }
}

impl TriggerSnapshot {
    fn load() -> Self {
        let triggers = parse_json(include_str!(
            "../../data/version/26.3/advancement_triggers.json"
        ));
        Self {
            triggers: object_maps(triggers.get("triggers")),
        }
    }
}

fn parse_json(text: &str) -> Value {
    serde_json::from_str(text).expect("版本快照必须是合法 JSON")
}

fn object_sets(value: Option<&Value>) -> BTreeMap<String, BTreeSet<String>> {
    value
        .and_then(Value::as_object)
        .map(|object| {
            object
                .iter()
                .map(|(key, value)| (key.clone(), string_set(Some(value))))
                .collect()
        })
        .unwrap_or_default()
}

fn object_arrays(value: Option<&Value>) -> BTreeMap<String, Vec<String>> {
    value
        .and_then(Value::as_object)
        .map(|object| {
            object
                .iter()
                .map(|(key, value)| {
                    let values = value
                        .as_array()
                        .map(|values| {
                            values
                                .iter()
                                .filter_map(|value| value.as_str().map(str::to_string))
                                .collect()
                        })
                        .unwrap_or_default();
                    (key.clone(), values)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn object_maps(value: Option<&Value>) -> BTreeMap<String, BTreeMap<String, String>> {
    value
        .and_then(Value::as_object)
        .map(|object| {
            object
                .iter()
                .filter_map(|(key, value)| {
                    let inner = value.as_object()?;
                    let map = inner
                        .iter()
                        .filter_map(|(key, value)| Some((key.clone(), value.as_str()?.to_owned())))
                        .collect();
                    Some((key.clone(), map))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn string_set(value: Option<&Value>) -> BTreeSet<String> {
    value
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn namespace_of(id: &str) -> &str {
    id.split_once(':')
        .map(|(namespace, _)| namespace)
        .unwrap_or("minecraft")
}

/// 在候选集合中找编辑距离 ≤ 2 的最近者。
pub(crate) fn closest<'a>(
    value: &str,
    candidates: impl Iterator<Item = &'a str>,
) -> Option<String> {
    let mut best: Option<(usize, String)> = None;
    for candidate in candidates {
        let distance = edit_distance(value, candidate);
        if distance <= 2 && best.as_ref().is_none_or(|(best, _)| distance < *best) {
            best = Some((distance, candidate.to_string()));
        }
    }
    best.map(|(_, candidate)| candidate)
}

/// 两段文本的 Levenshtein 距离（长度差异过大时直接返回上限）。
fn edit_distance(left: &str, right: &str) -> usize {
    let left: Vec<char> = left.chars().collect();
    let right: Vec<char> = right.chars().collect();
    if left.len().abs_diff(right.len()) > 4 {
        return usize::MAX;
    }
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    let mut current = vec![0; right.len() + 1];
    for (i, left_character) in left.iter().enumerate() {
        current[0] = i + 1;
        for (j, right_character) in right.iter().enumerate() {
            let cost = usize::from(left_character != right_character);
            current[j + 1] = (previous[j] + cost)
                .min(previous[j + 1] + 1)
                .min(current[j] + 1);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right.len()]
}
