//! 注册表存在性校验（1.1 版本快照）。
//!
//! 资源位置先做字符集检查，再对照随附的 26.3 注册表快照；只有
//! `minecraft:` 命名空间的 id 才会被判定“不存在”，其它命名空间可能由
//! 数据包提供，编译器不做存在性判断。拼写接近时给出最近候选。

use crate::ast::Span;
use crate::diagnostic::Diagnostic;
use crate::version::snapshot::snapshot;

use super::rules::valid_resource_location;

/// 校验资源位置与注册表存在性。
pub(super) fn validate_id(
    kind: &str,
    label: &str,
    value: &str,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !valid_resource_location(value) {
        diagnostics.push(Diagnostic::new(
            format!("`{value}` 不是有效的{label}资源位置"),
            span,
        ));
        return;
    }
    report_unknown(kind, label, value, span, diagnostics);
}

/// 与 [`validate_id`] 相同，但接受 `#标签` 前缀（标签存在性不做检查）。
pub(super) fn validate_id_or_tag(
    kind: &str,
    label: &str,
    value: &str,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(tag) = value.strip_prefix('#') {
        if !valid_resource_location(tag) {
            diagnostics.push(Diagnostic::new(
                format!("`{value}` 不是有效的{label}资源位置或 `#` 标签"),
                span,
            ));
        }
        return;
    }
    validate_id(kind, label, value, span, diagnostics);
}

/// 校验枚举取值（例如游戏模式、显示槽位）。
pub(super) fn validate_enum(
    name: &str,
    label: &str,
    value: &str,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let snapshot = snapshot();
    if snapshot.enum_contains(name, value) {
        return;
    }
    let mut message = format!("未知{label} `{value}`");
    if let Some(candidate) = snapshot.suggest_enum(name, value) {
        message.push_str(&format!("，是否想写 `{candidate}`？"));
    }
    diagnostics.push(Diagnostic::new(message, span));
}

/// 未知 id 的诊断：快照不覆盖的注册表与其它命名空间直接放行。
fn report_unknown(
    kind: &str,
    label: &str,
    value: &str,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let snapshot = snapshot();
    if snapshot.registry_contains(kind, value) != Some(false) {
        return;
    }
    let mut message = format!("未知{label} `{value}`；该 id 不在 26.3 的注册表中");
    if let Some(candidate) = snapshot.suggest_registry_id(kind, value) {
        message.push_str(&format!("，是否想写 `{candidate}`？"));
    }
    diagnostics.push(Diagnostic::new(message, span));
}
