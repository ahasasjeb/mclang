//! `place feature` accepts a registry id or an inline value decoded by
//! `ResourceOrIdArgument.feature` with `Feature.DIRECT_CODEC`.

use crate::ast::{NbtEntry, NbtValueKind, PlaceFeatureSource, Span};
use crate::diagnostic::Diagnostic;
use crate::version::snapshot::snapshot;

use super::super::registry::validate_id;
use super::super::rules::valid_resource_location;

pub(super) fn validate_place_feature(
    feature: &PlaceFeatureSource,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match feature {
        PlaceFeatureSource::Registered(id) => {
            // ResourceOrIdArgument accepts an id, but no `#` tag.
            validate_id("worldgen/feature", "地物", id, span, diagnostics);
        }
        PlaceFeatureSource::Inline(value) => {
            let NbtValueKind::Compound(entries) = &value.kind else {
                unreachable!("内联地物由 nbt_compound 解析");
            };
            validate_inline_feature(entries, value.span, diagnostics);
        }
    }
}

fn validate_inline_feature(entries: &[NbtEntry], span: Span, diagnostics: &mut Vec<Diagnostic>) {
    // Feature.DIRECT_CODEC dispatches on `type`. Its MapCodec fields live at
    // the same level; the older configured-feature `config` wrapper is invalid.
    if let Some(config) = entries.iter().find(|entry| entry.key == "config") {
        diagnostics.push(Diagnostic::new(
            "26.3 内联地物没有 `config` 包裹层；把该类型的字段直接写在 `type` 旁边",
            config.key_span,
        ));
    }

    let Some(feature_type) = entries.iter().find(|entry| entry.key == "type") else {
        diagnostics.push(Diagnostic::new(
            "place.feature 内联地物缺少 `type` 字段，例如 type = \"minecraft:no_op\"",
            span,
        ));
        return;
    };
    let NbtValueKind::String(id) = &feature_type.value.kind else {
        diagnostics.push(Diagnostic::new(
            "place.feature 内联地物的 `type` 必须是地物类型资源位置字符串",
            feature_type.value.span,
        ));
        return;
    };
    if !valid_resource_location(id) {
        diagnostics.push(Diagnostic::new(
            format!("`{id}` 不是有效的地物类型资源位置"),
            feature_type.value.span,
        ));
    } else if snapshot().registry_contains("feature_type", id) != Some(true) {
        // FEATURE_TYPE is a code registry, not a data-pack registry. Data
        // packs cannot add feature codecs under another namespace.
        let mut message = format!("未知地物类型 `{id}`；该类型不在 26.3 的 FeatureTypes 注册表中");
        if let Some(candidate) = snapshot().suggest_registry_id("feature_type", id) {
            message.push_str(&format!("，是否想写 `{candidate}`？"));
        }
        diagnostics.push(Diagnostic::new(message, feature_type.value.span));
    }
}
