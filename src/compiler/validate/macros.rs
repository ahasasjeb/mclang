use crate::{ast::*, diagnostic::Diagnostic};
use super::{statements::ValidationContext, rules::validate_identifier};

pub(super) fn validate_macro_declaration(function: &Function, diagnostics: &mut Vec<Diagnostic>) {
    let Some(signature) = &function.macro_signature else { return; };
    if function.returns_score || function.attributes.iter().any(|a| matches!(a, Attribute::Load | Attribute::Tick)) {
        diagnostics.push(Diagnostic::new("宏函数不能声明 score 返回值，也不能作为 @load/@tick 入口", function.span));
    }
    let mut names = std::collections::HashSet::new();
    for parameter in &signature.parameters {
        validate_identifier("宏参数", &parameter.name, parameter.span, diagnostics);
        if !names.insert(parameter.name.as_str()) { diagnostics.push(Diagnostic::new(format!("重复宏参数 `{}`", parameter.name), parameter.span)); }
        if !signature.uses.iter().any(|(name, _)| name == &parameter.name) { diagnostics.push(Diagnostic::new(format!("宏参数 `{}` 未在函数中使用", parameter.name), parameter.span)); }
    }
    for (name, span) in &signature.uses {
        if !names.contains(name.as_str()) { diagnostics.push(Diagnostic::new(format!("宏占位符 `$({name})` 没有对应参数声明"), *span)); }
    }
}

pub(super) fn validate_macro_call(target: &CallTarget, arguments: &MacroArguments, span: Span, ctx: ValidationContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
    if let MacroArguments::With { source, path } = arguments {
        super::components::validate_nbt_source(source, span, ctx, diagnostics);
        if let NbtComponentSource::Entity(holder) = source { super::entity_commands::entity_target(holder, true, false, span, ctx, diagnostics); }
        if let Some((path, span)) = path && !super::components::valid_nbt_component_path(path) { diagnostics.push(Diagnostic::new(format!("无效宏参数 NBT 路径 `{path}`"), *span)); }
    }
    match target {
        CallTarget::External(id) => validate_external_function(id, span, diagnostics),
        CallTarget::Function(name) => validate_local_macro_call(name, arguments, span, ctx, diagnostics),
        CallTarget::Tag(name) => {
            if !ctx.symbols.function_tags.contains_key(name.as_str()) { diagnostics.push(Diagnostic::new(format!("找不到函数标签 `#{name}`"), span)); }
            for function in super::tags::reachable_functions(name, ctx.symbols.function_tags) { validate_local_macro_call(function, arguments, span, ctx, diagnostics); }
        }
    }
}

fn validate_local_macro_call(name: &str, arguments: &MacroArguments, span: Span, ctx: ValidationContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
    let Some(function) = ctx.symbols.function_declarations.get(name) else { diagnostics.push(Diagnostic::new(format!("找不到函数 `{name}`"), span)); return; };
    if let Some(signature) = ctx.symbols.functions.get(name) { super::expressions::validate_call_context(name, *signature, span, ctx, diagnostics); }
    let Some(signature) = &function.macro_signature else {
        if !function.parameters.is_empty() { diagnostics.push(Diagnostic::new(format!("普通计分参数函数 `{name}` 不能接收 NBT 宏参数"), span)); }
        return;
    };
    let MacroArguments::Literal(NbtValue { kind: NbtValueKind::Compound(entries), .. }) = arguments else { return; };
    for parameter in &signature.parameters {
        let Some(entry) = entries.iter().find(|e| e.key == parameter.name) else { diagnostics.push(Diagnostic::new(format!("宏函数 `{name}` 缺少参数 `{}`", parameter.name), span)); continue; };
        if !macro_value_matches(parameter.kind, &entry.value) { diagnostics.push(Diagnostic::new(format!("宏参数 `{}` 的值不符合 {} 类型；text 不能含引号、反斜杠、控制字符或宏占位符", parameter.name, macro_type_name(parameter.kind)), entry.value.span)); }
    }
    for (key, kind) in &signature.coordinates {
        if let Some(entry) = entries.iter().find(|e| &e.key == key) { validate_coordinate_argument(&entry.value, *kind, diagnostics); }
    }
}

fn validate_coordinate_argument(value: &NbtValue, kind: MacroCoordinateKind, diagnostics: &mut Vec<Diagnostic>) {
    let number = match value.kind { NbtValueKind::Byte(v) => v as f64, NbtValueKind::Short(v) => v as f64, NbtValueKind::Int(v) => v as f64, NbtValueKind::Long(v) => v as f64, NbtValueKind::Float(v) => v as f64, NbtValueKind::Double(v) => v, _ => return };
    let (min, max) = match kind {
        MacroCoordinateKind::Horizontal => (super::world::HORIZONTAL_MIN as f64, super::world::HORIZONTAL_MAX as f64),
        MacroCoordinateKind::Vertical => (super::world::VERTICAL_MIN as f64, super::world::VERTICAL_MAX as f64),
        MacroCoordinateKind::Angle => (-(f32::MAX as f64), f32::MAX as f64),
    };
    if !(min..=max).contains(&number) { diagnostics.push(Diagnostic::new(format!("宏坐标参数 {number} 超出范围（{min} 到 {max}）"), value.span)); }
}

fn macro_type_name(kind: MacroType) -> &'static str { match kind { MacroType::Integer => "integer", MacroType::Decimal => "decimal", MacroType::Text => "text", MacroType::Resource => "resource", MacroType::Nbt => "nbt" } }

fn macro_value_matches(kind: MacroType, value: &NbtValue) -> bool {
    match (kind, &value.kind) {
        (MacroType::Integer, NbtValueKind::Byte(_) | NbtValueKind::Short(_) | NbtValueKind::Int(_)) => true,
        (MacroType::Decimal, NbtValueKind::Byte(_) | NbtValueKind::Short(_) | NbtValueKind::Int(_) | NbtValueKind::Long(_) | NbtValueKind::Float(_) | NbtValueKind::Double(_)) => true,
        (MacroType::Text, NbtValueKind::String(text)) => !text.contains(['"', '\'', '\\']) && !text.contains("$(") && !text.chars().any(char::is_control),
        (MacroType::Resource, NbtValueKind::String(text)) => super::rules::valid_resource_location(text),
        (MacroType::Nbt, _) => true,
        _ => false,
    }
}

pub(in crate::compiler::validate) fn validate_external_function(id: &str, span: Span, diagnostics: &mut Vec<Diagnostic>) {
    super::entity_commands::resource_id(id.strip_prefix('#').unwrap_or(id), span, diagnostics);
}
