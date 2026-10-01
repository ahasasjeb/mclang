//! `place feature` accepts a registry id or an inline value decoded by
//! `ResourceOrIdArgument.feature` with `Feature.DIRECT_CODEC`.

use crate::ast::{NbtEntry, NbtValueKind, PlaceFeatureSource, Span};
use crate::diagnostic::Diagnostic;

use super::super::registry::{validate_id, validate_static_id};

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
    validate_static_id(
        "feature_type",
        "地物类型",
        id,
        feature_type.value.span,
        diagnostics,
    );
}
