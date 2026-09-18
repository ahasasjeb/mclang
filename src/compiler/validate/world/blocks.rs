use crate::ast::{BlockStateValue, ColumnPosition, ForceLoadOperation, Span};
use crate::diagnostic::Diagnostic;

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
    }
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
