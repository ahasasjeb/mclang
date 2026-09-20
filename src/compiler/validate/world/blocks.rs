use crate::ast::{BlockStateValue, ColumnPosition, ForceLoadOperation, Span};
use crate::diagnostic::Diagnostic;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use super::super::registry::validate_id;
use super::super::rules::valid_resource_location;
use super::positions::validate_column_position;

/// 方块状态或方块谓词：`#` 标签只在过滤器里合法，属性名与值限制为 SNBT 安全字符。
pub(crate) fn validate_block_state(
    block: &BlockStateValue,
    allow_tag: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(tag) = block.id.strip_prefix('#') {
        if !allow_tag {
            diagnostics.push(Diagnostic::new(
                format!("`{}` 是方块标签，这里需要具体的方块资源位置", block.id),
                block.span,
            ));
        }
        if !valid_resource_location(tag) {
            diagnostics.push(Diagnostic::new(
                format!("`{}` 不是有效的方块标签资源位置", block.id),
                block.span,
            ));
        }
        if !block.properties.is_empty() {
            diagnostics.push(Diagnostic::new("方块标签不能声明方块属性", block.span));
        }
    } else {
        validate_id("block", "方块", &block.id, block.span, diagnostics);
    }
    let known = block_properties()
        .get(&block.id)
        .filter(|properties| !properties.is_empty());
    for property in &block.properties {
        if !valid_block_property_name(&property.name) {
            diagnostics.push(Diagnostic::new(
                format!(
                    "方块属性名 `{}` 只能包含小写字母、数字和下划线",
                    property.name
                ),
                property.span,
            ));
        }
        if !valid_block_property_value(&property.value) {
            diagnostics.push(Diagnostic::new(
                format!("方块属性值 `{}` 含有不允许的字符", property.value),
                property.span,
            ));
        }
        if let Some(known) = known {
            if let Some(values) = known.get(&property.name) {
                if !values.contains(&property.value) {
                    diagnostics.push(Diagnostic::new(
                        format!(
                            "方块 `{}` 的属性 `{}` 不接受 `{}`；可用 {}",
                            block.id,
                            property.name,
                            property.value,
                            values
                                .iter()
                                .map(|value| format!("`{value}`"))
                                .collect::<Vec<_>>()
                                .join("、")
                        ),
                        property.span,
                    ));
                }
            } else if property.name == "waterlogged" {
                if !matches!(property.value.as_str(), "true" | "false") {
                    diagnostics.push(Diagnostic::new(
                        format!("方块 `{}` 的 waterlogged 只能是 true 或 false", block.id),
                        property.span,
                    ));
                }
            } else {
                diagnostics.push(Diagnostic::new(
                    format!("方块 `{}` 没有已知属性 `{}`", block.id, property.name),
                    property.span,
                ));
            }
        }
    }
}

type BlockProperties = BTreeMap<String, BTreeMap<String, BTreeSet<String>>>;
static BLOCK_PROPERTIES: OnceLock<BlockProperties> = OnceLock::new();

fn block_properties() -> &'static BlockProperties {
    BLOCK_PROPERTIES.get_or_init(|| {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/version/26.3-rc-2/block_states.json"
        ));
        let document: serde_json::Value =
            serde_json::from_str(source).expect("方块状态快照必须是 JSON");
        serde_json::from_value(document["blocks"].clone()).expect("方块状态快照的 blocks 类型无效")
    })
}

fn valid_block_property_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|character| matches!(character, 'a'..='z' | '0'..='9' | '_'))
}

fn valid_block_property_value(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|character| matches!(character, 'a'..='z' | '0'..='9' | '_' | '.' | '+' | '-'))
}

pub(super) fn validate_forceload(
    operation: &ForceLoadOperation,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match operation {
        ForceLoadOperation::Add { from, to } | ForceLoadOperation::Remove { from, to } => {
            validate_column_position(from, diagnostics);
            if let Some(to) = to {
                validate_column_position(to, diagnostics);
            }
            if let Some(count) = forceload_chunk_count(from, to.as_ref())
                && count > 256
            {
                diagnostics.push(Diagnostic::new(
                    format!("forceload 一次最多影响 256 个区块，当前范围包含 {count} 个"),
                    span,
                ));
            }
        }
        ForceLoadOperation::RemoveAll => {}
        ForceLoadOperation::Query { pos } => {
            if let Some(pos) = pos {
                validate_column_position(pos, diagnostics);
            }
        }
    }
}

/// `add`/`remove` 的区块数量：只有两个端点都是绝对坐标时才能静态计算。
fn forceload_chunk_count(from: &ColumnPosition, to: Option<&ColumnPosition>) -> Option<u64> {
    let x0 = from.x.absolute_integer()?;
    let z0 = from.z.absolute_integer()?;
    let (x1, z1) = match to {
        Some(to) => (to.x.absolute_integer()?, to.z.absolute_integer()?),
        None => (x0, z0),
    };
    let (min_x, max_x) = (x0.min(x1), x0.max(x1));
    let (min_z, max_z) = (z0.min(z1), z0.max(z1));
    let chunks_x = (max_x.div_euclid(16) - min_x.div_euclid(16) + 1) as u64;
    let chunks_z = (max_z.div_euclid(16) - min_z.div_euclid(16) + 1) as u64;
    Some(chunks_x * chunks_z)
}
