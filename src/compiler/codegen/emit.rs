//! 把结构化 AST 片段格式化为 Minecraft 命令参数、SNBT 和 JSON 文本。

use crate::ast::{
    AdvancementReference, EntityQueryDecl, EntityTypeFilter, ItemEnchantment, ItemFilter,
    ItemStackDecl, NbtValue, NbtValueKind,
};

/// 资源引用文本：本命名空间声明补上命名空间前缀，外部字符串原样保留。
pub(super) fn reference_id(namespace: &str, reference: &AdvancementReference) -> String {
    if reference.external {
        reference.name.clone()
    } else {
        format!("{namespace}:{}", reference.name)
    }
}

pub(super) fn entity_query_selector(query: &EntityQueryDecl) -> String {
    let mut selector = Vec::new();
    if let Some(tag) = query.entity_type.strip_prefix('#') {
        selector.push(format!("type=#{tag}"));
    } else {
        selector.push(format!("type={}", query.entity_type));
    }
    for filter in &query.type_filters {
        match filter {
            EntityTypeFilter::Include(value, _) => selector.push(format!("type={value}")),
            EntityTypeFilter::Exclude(value, _) => selector.push(format!("type=!{value}")),
        }
    }
    selector.extend(query.tags.iter().map(|tag| format!("tag={tag}")));
    selector.extend(query.excluded_tags.iter().map(|tag| format!("tag=!{tag}")));
    if let Some(filter) = &query.name_filter {
        selector.push(format!(
            "name={}{}",
            if filter.negated { "!" } else { "" },
            filter.value
        ));
    }
    for score in &query.scores {
        selector.push(format!("scores={{{}={}}}", score.objective, score.range));
    }
    if let Some(filter) = &query.nbt_filter {
        selector.push(format!(
            "nbt={}{}",
            if filter.negated { "!" } else { "" },
            filter.value
        ));
    }
    if let Some(box_filter) = &query.box_filter {
        selector.push(format!(
            "x={},y={},z={},dx={},dy={},dz={}",
            box_filter.x, box_filter.y, box_filter.z, box_filter.dx, box_filter.dy, box_filter.dz
        ));
    }
    if let Some(distance) = distance_option(query) {
        selector.push(format!("distance={distance}"));
    }
    if let Some(level) = &query.level {
        selector.push(format!("level={level}"));
    }
    if let Some(gamemode) = &query.gamemode {
        selector.push(format!("gamemode={gamemode}"));
    }
    if let Some(filter) = &query.team_filter {
        selector.push(format!(
            "team={}{}",
            if filter.negated { "!" } else { "" },
            filter.value
        ));
    }
    if let Some(rotation) = &query.rotation {
        // 选择器里 `x_rotation` 是俯仰、`y_rotation` 是偏航。
        selector.push(format!(
            "x_rotation={},y_rotation={}",
            rotation.pitch, rotation.yaw
        ));
    }
    if let Some(predicate) = &query.predicate {
        selector.push(format!("predicate={predicate}"));
    }
    if let Some(advancements) = &query.advancements {
        selector.push(format!("advancements={advancements}"));
    }
    if let Some(sort) = query.sort {
        selector.push(format!("sort={}", sort.as_str()));
    }
    if let Some(limit) = query.limit {
        selector.push(format!("limit={limit}"));
    }
    format!("@e[{}]", selector.join(","))
}

/// 选择器的 `distance` 选项只能出现一次：`distance` 与 `within` 取交集。
fn distance_option(query: &EntityQueryDecl) -> Option<String> {
    match (&query.distance, query.within) {
        (None, None) => None,
        (Some(distance), None) => Some(distance.clone()),
        (None, Some(within)) => Some(format!("..{within}")),
        (Some(distance), Some(within)) => Some(
            intersect_ranges(distance, &format!("..{within}")).unwrap_or_else(|| distance.clone()),
        ),
    }
}

/// 两个浮点区间的交集：`3..10` 与 `..32` 得到 `3..10`。
fn intersect_ranges(left: &str, right: &str) -> Option<String> {
    let (left_low, left_high) = parse_range(left)?;
    let (right_low, right_high) = parse_range(right)?;
    let low = match (left_low, right_low) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    };
    let high = match (left_high, right_high) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    };
    if let (Some(low), Some(high)) = (low, high)
        && low > high
    {
        return None;
    }
    Some(match (low, high) {
        (Some(low), Some(high)) => format!("{low}..{high}"),
        (Some(low), None) => format!("{low}.."),
        (None, Some(high)) => format!("..{high}"),
        (None, None) => "..".to_owned(),
    })
}

fn parse_range(text: &str) -> Option<(Option<f64>, Option<f64>)> {
    if let Some((low, high)) = text.split_once("..") {
        let low = if low.is_empty() {
            None
        } else {
            Some(low.parse::<f64>().ok()?)
        };
        let high = if high.is_empty() {
            None
        } else {
            Some(high.parse::<f64>().ok()?)
        };
        Some((low, high))
    } else {
        let value = text.parse::<f64>().ok()?;
        Some((Some(value), Some(value)))
    }
}

/// 物品过滤器到谓词文本：`id[组件...]`，`count`/`custom_name` 合并进组件列表。
pub(super) fn item_filter_predicate(item: &ItemFilter) -> String {
    let mut components = Vec::new();
    if let Some(custom_name) = &item.custom_name {
        components.push(format!(
            "minecraft:custom_name={{text:{}}}",
            snbt_string(custom_name)
        ));
    }
    if let Some(count) = item.count {
        components.push(format!("minecraft:count={count}"));
    }
    if components.is_empty() {
        return item.item_id.clone();
    }
    let extra = components.join(",");
    match item.item_id.rfind('[') {
        Some(open) if item.item_id.ends_with(']') => {
            let inner = &item.item_id[open + 1..item.item_id.len() - 1];
            if inner.is_empty() {
                format!("{}[{extra}]", &item.item_id[..open])
            } else {
                format!("{}[{inner},{extra}]", &item.item_id[..open])
            }
        }
        _ => format!("{}[{extra}]", item.item_id),
    }
}

pub(super) fn entity_query_clause(query: &EntityQueryDecl) -> String {
    let mut clause = format!("as {} at @s", entity_query_selector(query));
    if let Some(item) = &query.item {
        clause.push_str(&format!(
            " if items entity @s {} {}",
            item.slot,
            item_filter_predicate(item)
        ));
    }
    clause
}

pub(super) fn item_stack_argument(item: &ItemStackDecl) -> String {
    let mut components = Vec::new();
    if let Some(custom_name) = &item.custom_name {
        components.push(format!(
            "minecraft:custom_name={{text:{}}}",
            snbt_string(custom_name)
        ));
    }
    if let Some(item_name) = &item.item_name {
        components.push(format!(
            "minecraft:item_name={{text:{}}}",
            snbt_string(item_name)
        ));
    }
    if !item.lore.is_empty() {
        let lines = item
            .lore
            .iter()
            .map(|line| format!("{{text:{}}}", snbt_string(line)))
            .collect::<Vec<_>>()
            .join(",");
        components.push(format!("minecraft:lore=[{lines}]"));
    }
    append_enchantments_component(
        &mut components,
        "minecraft:enchantments",
        &item.enchantments,
    );
    append_enchantments_component(
        &mut components,
        "minecraft:stored_enchantments",
        &item.stored_enchantments,
    );
    if let Some(damage) = item.damage {
        components.push(format!("minecraft:damage={damage}"));
    }
    if let Some(max_damage) = item.max_damage {
        components.push(format!("minecraft:max_damage={max_damage}"));
    }
    if let Some(max_stack_size) = item.max_stack_size {
        components.push(format!("minecraft:max_stack_size={max_stack_size}"));
    }
    if let Some(rarity) = item.rarity {
        components.push(format!("minecraft:rarity=\"{}\"", rarity.as_str()));
    }
    if let Some(item_model) = &item.item_model {
        components.push(format!("minecraft:item_model={}", snbt_string(item_model)));
    }
    if let Some(dyed_color) = item.dyed_color {
        components.push(format!("minecraft:dyed_color={dyed_color}"));
    }
    if let Some(glint) = item.enchantment_glint_override {
        components.push(format!(
            "minecraft:enchantment_glint_override={}",
            if glint { "true" } else { "false" }
        ));
    }
    if item.unbreakable {
        components.push("minecraft:unbreakable={}".to_owned());
    }
    if let Some(custom_data) = &item.custom_data {
        components.push(format!("minecraft:custom_data={}", nbt_text(custom_data)));
    }
    if components.is_empty() {
        item.item_id.clone()
    } else {
        format!("{}[{}]", item.item_id, components.join(","))
    }
}

fn append_enchantments_component(
    components: &mut Vec<String>,
    component_id: &str,
    enchantments: &[ItemEnchantment],
) {
    if enchantments.is_empty() {
        return;
    }
    let entries = enchantments
        .iter()
        .map(|enchantment| {
            format!(
                "{}:{}",
                snbt_string(&enchantment.enchantment_id),
                enchantment.level
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    components.push(format!("{component_id}={{{entries}}}"));
}

/// 结构化 NBT 值的 SNBT 文本。
///
/// 输出遵循 26.3 `StringTagVisitor` 的规范化写法：字节/短整数/长整数带 `b`/`s`/`L`
/// 后缀，单精度与双精度带 `f`/`d` 后缀，列表与复合用逗号分隔，整数数组写成
/// `[B;1B,2B]`。字符串与不安全的键加引号并转义。
pub(super) fn nbt_text(value: &NbtValue) -> String {
    match &value.kind {
        NbtValueKind::Byte(value) => format!("{value}b"),
        NbtValueKind::Short(value) => format!("{value}s"),
        NbtValueKind::Int(value) => value.to_string(),
        NbtValueKind::Long(value) => format!("{value}L"),
        NbtValueKind::Float(value) => format!("{value:?}f"),
        NbtValueKind::Double(value) => format!("{value:?}d"),
        NbtValueKind::String(value) => snbt_string(value),
        NbtValueKind::List(values) => {
            let values = values.iter().map(nbt_text).collect::<Vec<_>>().join(",");
            format!("[{values}]")
        }
        NbtValueKind::Compound(entries) => {
            let entries = entries
                .iter()
                .map(|entry| format!("{}:{}", snbt_key(&entry.key), nbt_text(&entry.value)))
                .collect::<Vec<_>>()
                .join(",");
            format!("{{{entries}}}")
        }
        NbtValueKind::ByteArray(values) => {
            let values = values
                .iter()
                .map(|value| format!("{value}B"))
                .collect::<Vec<_>>()
                .join(",");
            format!("[B;{values}]")
        }
        NbtValueKind::IntArray(values) => {
            let values = values
                .iter()
                .map(i32::to_string)
                .collect::<Vec<_>>()
                .join(",");
            format!("[I;{values}]")
        }
        NbtValueKind::LongArray(values) => {
            let values = values
                .iter()
                .map(|value| format!("{value}L"))
                .collect::<Vec<_>>()
                .join(",");
            format!("[L;{values}]")
        }
    }
}

/// NBT 键：安全的键直接输出，其余加引号，与 `CompoundTag.writeString` 一致。
fn snbt_key(key: &str) -> String {
    let simple = !key.is_empty()
        && key
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "._+-".contains(character));
    if simple {
        key.to_owned()
    } else {
        snbt_string(key)
    }
}

fn snbt_string(value: &str) -> String {
    let mut escaped = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character < ' ' => {
                escaped.push_str(&format!("\\u{:04x}", character as u32))
            }
            character => escaped.push(character),
        }
    }
    escaped.push('\"');
    escaped
}

pub(super) fn pack_metadata(description: &str) -> String {
    format!(
        "{{\n  \"pack\": {{\n    \"description\": \"{}\",\n    \"min_format\": [121, 0],\n    \"max_format\": [121, 0]\n  }}\n}}\n",
        json_escape(description)
    )
}

/// `give(..., self.item)` 使用的槽位来源：目标玩家背包里的第一个空槽。
///
/// 26.3 的 `filtered` 槽位来源把 `slot_source` 与物品谓词组合起来；空堆的
/// `count` 是 0，因此 `"count": 0` 只选中空槽。`hotbar.*` 覆盖 0 到 8 号槽，
/// `inventory.*` 覆盖 9 到 35 号槽，合起来正好是玩家的 36 格快捷栏与主背包；
/// 刻意不包含 `container.*` 里的盔甲、副手和合成槽，避免物品被放进装备槽。
pub(super) fn empty_slot_source() -> &'static str {
    r#"{
  "type": "minecraft:filtered",
  "slot_source": {
    "type": "minecraft:group",
    "terms": [
      {
        "type": "minecraft:slot_range",
        "slots": "hotbar.*"
      },
      {
        "type": "minecraft:slot_range",
        "slots": "inventory.*"
      }
    ]
  },
  "item_filter": {
    "count": 0
  }
}
"#
}

pub(super) fn tag_json(values: &[String]) -> String {
    let values = values
        .iter()
        .map(|value| format!("    \"{}\"", json_escape(value)))
        .collect::<Vec<_>>()
        .join(",\n");
    format!("{{\n  \"values\": [\n{values}\n  ]\n}}\n")
}

/// 函数标签文件。`replace` 为假（26.3 默认值）时省略字段，保持输出最小。
pub(super) fn function_tag_json(values: &[String], replace: bool) -> String {
    let values = if values.is_empty() {
        "[]".to_owned()
    } else {
        format!(
            "[\n{}\n  ]",
            values
                .iter()
                .map(|value| format!("    \"{}\"", json_escape(value)))
                .collect::<Vec<_>>()
                .join(",\n")
        )
    };
    if replace {
        format!("{{\n  \"values\": {values},\n  \"replace\": true\n}}\n")
    } else {
        format!("{{\n  \"values\": {values}\n}}\n")
    }
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character < ' ' => {
                escaped.push_str(&format!("\\u{:04x}", character as u32))
            }
            character => escaped.push(character),
        }
    }
    escaped
}
