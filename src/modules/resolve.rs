use super::*;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use crate::analysis::SourceFile;
use crate::ast::*;
use crate::compiler::{valid_user_name, windows_reserved_name};
use crate::diagnostic::Diagnostic;
use crate::name_walk::{NameRole, NameSite};
use crate::parser::keywords::reserved_word;
use crate::version::snapshot::closest;

/// 模块解析结果：合并后的整程序。
pub(crate) fn resolve(
    programs: Vec<(usize, Program)>,
    sources: &[SourceFile],
    root: usize,
) -> Result<Program, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut by_source: BTreeMap<usize, Program> = programs.into_iter().collect();

    let Some(root_program) = by_source.get(&root) else {
        diagnostics.push(Diagnostic::new(
            "项目没有入口模块：缺少可解析的 main.mcl",
            Span::default(),
        ));
        return Err(diagnostics);
    };

    // 1. 命名空间：入口声明，其他模块继承或核对。
    let namespace = root_program.namespace.clone();
    if namespace.is_empty() {
        diagnostics.push(Diagnostic::new(
            "入口模块必须声明 `namespace <名称>;`",
            root_program.namespace_span.unwrap_or_default(),
        ));
        return Err(diagnostics);
    }
    for (index, program) in &by_source {
        if *index != root && !program.namespace.is_empty() && program.namespace != namespace {
            diagnostics.push(Diagnostic::new(
                format!(
                    "模块命名空间必须与入口一致：预期 `{namespace}`，实际为 `{}`；\
                     模块也可以省略 namespace 声明来继承入口",
                    program.namespace
                ),
                program.namespace_span.unwrap_or_default(),
            ));
        }
    }

    // 2. 模块路径表：文件路径 -> 模块路径。
    let root_directory = sources
        .get(root)
        .and_then(|source| source.path.parent())
        .map(Path::to_path_buf);
    let mut module_paths: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    for (index, program) in &by_source {
        if *index == root {
            module_paths.insert(*index, Vec::new());
            continue;
        }
        let (Some(directory), Some(source)) = (root_directory.as_deref(), sources.get(*index))
        else {
            continue;
        };
        let Some(segments) = module_segments(&source.path, directory) else {
            continue;
        };
        if segments.first().is_some_and(|segment| segment == "std")
            && !crate::stdlib::is_virtual_path(&source.path, directory)
        {
            diagnostics.push(Diagnostic::new(
                "项目不能定义 `std` 模块：这个路径保留给内置标准库",
                first_span(program),
            ));
        }
        let builtin = crate::stdlib::is_virtual_path(&source.path, directory);
        for segment in segments.iter().filter(|_| !builtin) {
            let span = first_span(program);
            if segment.starts_with("__mcl") {
                diagnostics.push(Diagnostic::new(
                    format!("模块路径分段 `{segment}` 不能以 `__mcl` 开头：这是编译器的内部前缀"),
                    span,
                ));
            } else if windows_reserved_name(segment) {
                diagnostics.push(Diagnostic::new(
                    format!("模块路径分段 `{segment}` 与 Windows 保留设备名冲突"),
                    span,
                ));
            } else if !valid_user_name(segment) || reserved_word(segment) {
                diagnostics.push(Diagnostic::new(
                    format!(
                        "模块路径分段 `{segment}` 必须是合法标识符（小写 ASCII 字母、数字、\
                         下划线或中文），且不能是关键词"
                    ),
                    span,
                ));
            }
        }
        module_paths.insert(*index, segments);
    }

    let mut by_module_path: HashMap<String, usize> = HashMap::new();
    for (index, segments) in &module_paths {
        let key = segments.join("/");
        if let Some(previous) = by_module_path.insert(key.clone(), *index)
            && previous != *index
        {
            diagnostics.push(Diagnostic::new(
                format!(
                    "模块 `{key}` 同时对应 `{key}.mcl` 与 `{key}/mod.mcl` 两个文件，请只保留一个",
                ),
                first_span(&by_source[&previous]),
            ));
        }
    }

    // 入口模块的声明保持原名（产物与单文件项目一致），但它也能以 `main`
    // 被其他模块导入，这样入口与子模块可以互相引用。
    match by_module_path.get("main") {
        Some(existing) if *existing != root => diagnostics.push(Diagnostic::new(
            "模块路径 `main` 已被入口模块占用（入口可以用 `import main;` 被导入）；\
             请给 `main/mod.mcl` 换一个目录名",
            first_span(&by_source[existing]),
        )),
        _ => {
            by_module_path.insert("main".to_owned(), root);
        }
    }

    // 3. 依赖图：解析 import 并报告缺失模块。
    let mut edges: BTreeMap<usize, Vec<(usize, Span)>> = BTreeMap::new();
    for (index, program) in &by_source {
        let mut targets = Vec::new();
        for import in &program.imports {
            let key = import.path.join("/");
            match by_module_path.get(&key) {
                Some(target) => targets.push((*target, import.path_span)),
                None => {
                    let location = import.path.join("/");
                    let message = if import.path.first().is_some_and(|part| part == "std") {
                        format!(
                            "找不到内置模块 `{key}`：可用模块为 {}",
                            crate::stdlib::MODULES
                                .iter()
                                .map(|name| format!("std::{name}"))
                                .collect::<Vec<_>>()
                                .join("、")
                        )
                    } else {
                        format!(
                            "找不到模块 `{key}`：预期文件 `{location}.mcl` 或 `{location}/mod.mcl`\
                             （相对项目根目录）"
                        )
                    };
                    diagnostics.push(Diagnostic::new(message, import.path_span));
                }
            }
        }
        targets.sort_by_key(|(target, _)| *target);
        targets.dedup_by_key(|(target, _)| *target);
        edges.insert(*index, targets);
    }

    // 4. 只保留入口可达的模块。导入环是允许的：解析先建好全部模块的作用域，
    //    再做限定名重写，因此互相引用不需要任何顺序假设。
    let reachable = reachable_modules(root, &edges);

    // 5. 每个模块的声明表与作用域。
    let mut modules: BTreeMap<usize, ModuleState> = BTreeMap::new();
    for index in &reachable {
        let program = &by_source[index];
        let segments = module_paths.get(index).cloned().unwrap_or_default();
        let declarations = collect_declarations(program);
        let declaration_names = declarations
            .iter()
            .map(|(_, name, _, _)| name.clone())
            .collect();
        let mut exports = Vec::new();
        let mut exports_by_name: HashMap<String, Vec<(NameRole, String)>> = HashMap::new();
        let mut scope = HashMap::new();
        for (role, name, exported, _) in &declarations {
            let qualified = qualify(*role, &segments, name);
            if *exported {
                exports.push((*role, name.clone(), qualified.clone()));
                exports_by_name
                    .entry(name.clone())
                    .or_default()
                    .push((*role, qualified.clone()));
            }
            scope.insert((*role, name.clone()), qualified);
        }
        modules.insert(
            *index,
            ModuleState {
                segments,
                declarations,
                declaration_names,
                exports,
                exports_by_name,
                scope,
            },
        );
    }

    // 6. 处理导入：把公开声明映射进导入方的作用域。
    let local_names: HashMap<usize, HashSet<(NameRole, String)>> = modules
        .iter()
        .map(|(index, module)| {
            (
                *index,
                module
                    .declarations
                    .iter()
                    .map(|(role, name, _, _)| (*role, name.clone()))
                    .collect(),
            )
        })
        .collect();
    for index in &reachable {
        let program = &by_source[index];
        let module = &modules[index];
        let mut scope = module.scope.clone();
        let locals = &local_names[index];
        for import in &program.imports {
            let Some(target) = by_module_path.get(&import.path.join("/")) else {
                continue;
            };
            let target_module = &modules[target];
            match &import.items {
                None => {
                    for (role, name, qualified) in &target_module.exports {
                        bind_import(
                            &mut scope,
                            &mut diagnostics,
                            locals,
                            *role,
                            name,
                            qualified,
                            ImportOrigin {
                                path: &import.path,
                                span: import.path_span,
                            },
                        );
                    }
                }
                Some(items) => {
                    for item in items {
                        let effective = item.alias.as_deref().unwrap_or(&item.name);
                        let Some(matches) = target_module.exports_by_name.get(&item.name) else {
                            if target_module.declaration_names.contains(&item.name) {
                                diagnostics.push(Diagnostic::new(
                                    format!(
                                        "`{}` 在模块 `{}` 中存在但没有 export，其他模块不能导入",
                                        item.name,
                                        import.path.join("/")
                                    ),
                                    item.name_span,
                                ));
                            } else {
                                let suggestion = closest(
                                    &item.name,
                                    target_module
                                        .declarations
                                        .iter()
                                        .map(|(_, name, _, _)| name.as_str()),
                                )
                                .map(|candidate| format!("；最接近的名字是 `{candidate}`"))
                                .unwrap_or_default();
                                diagnostics.push(Diagnostic::new(
                                    format!(
                                        "模块 `{}` 中没有 `{}`{suggestion}",
                                        import.path.join("/"),
                                        item.name
                                    ),
                                    item.name_span,
                                ));
                            }
                            continue;
                        };
                        for (role, qualified) in matches {
                            bind_import(
                                &mut scope,
                                &mut diagnostics,
                                locals,
                                *role,
                                effective,
                                qualified,
                                ImportOrigin {
                                    path: &import.path,
                                    span: import.path_span,
                                },
                            );
                        }
                    }
                }
            }
        }
        modules.get_mut(index).unwrap().scope = scope;
    }

    // 7. 限定名重写：声明直接改写，引用查作用域，查不到时保持原样，
    //    由语义检查给出「未声明/未导入」的针对性诊断。
    if diagnostics.is_empty() {
        for index in &reachable {
            let segments = modules[index].segments.clone();
            let scope = modules[index].scope.clone();
            let program = by_source.get_mut(index).expect("模块一定来自已解析的程序");
            program.for_each_name_mut(&mut |context, site, role, name| {
                if site == NameSite::Declaration {
                    match role {
                        NameRole::Local | NameRole::Parameter | NameRole::Criterion => {}
                        _ => *name = qualify(role, &segments, name),
                    }
                    return;
                }
                match role {
                    NameRole::Local | NameRole::Parameter | NameRole::Criterion => return,
                    NameRole::Score if context.is_local(name) => return,
                    _ => {}
                }
                if let Some(qualified) = scope.get(&(role, name.clone())) {
                    *name = qualified.clone();
                }
            });
        }
    } else {
        return Err(diagnostics);
    }

    // 8. 合并：入口在前，其余模块按源文件顺序拼接。
    let mut merged = by_source.remove(&root).expect("入口一定存在");
    merged.imports.clear();
    for index in reachable.iter().filter(|index| **index != root) {
        let mut module = by_source
            .remove(index)
            .expect("可达模块一定来自已解析的程序");
        merged.scores.append(&mut module.scores);
        merged.objectives.append(&mut module.objectives);
        merged.queries.append(&mut module.queries);
        merged.item_stacks.append(&mut module.item_stacks);
        merged.storages.append(&mut module.storages);
        merged.data_slots.append(&mut module.data_slots);
        merged.resources.append(&mut module.resources);
        merged.advancements.append(&mut module.advancements);
        merged.function_tags.append(&mut module.function_tags);
        merged.functions.append(&mut module.functions);
    }
    Ok(merged)
}

/// 一个模块的声明、公开接口与已解析的作用域。
struct ModuleState {
    segments: Vec<String>,
    declarations: Vec<(NameRole, String, bool, Span)>,
    declaration_names: HashSet<String>,
    /// 公开声明：`(类别, 原名, 限定名)`。
    exports: Vec<(NameRole, String, String)>,
    /// 选择性导入按原名查公开声明；同名可以对应多个 NameRole。
    exports_by_name: HashMap<String, Vec<(NameRole, String)>>,
    /// 可见名字：`(类别, 本地名) -> 限定名`。
    scope: HashMap<(NameRole, String), String>,
}

/// 导入语句的来源信息，用于诊断。
#[derive(Clone, Copy)]
struct ImportOrigin<'a> {
    path: &'a [String],
    span: Span,
}

/// 把导入的名字绑定进作用域：局部声明优先，导入之间冲突报错。
fn bind_import(
    scope: &mut HashMap<(NameRole, String), String>,
    diagnostics: &mut Vec<Diagnostic>,
    locals: &HashSet<(NameRole, String)>,
    role: NameRole,
    name: &str,
    qualified: &str,
    origin: ImportOrigin<'_>,
) {
    let key = (role, name.to_owned());
    if locals.contains(&key) {
        // 本模块自己的声明优先，导入的名字被遮蔽。
        return;
    }
    match scope.get(&key) {
        Some(existing) if existing != qualified => diagnostics.push(Diagnostic::new(
            format!(
                "导入的名字 `{name}` 已经指向 `{existing}`，与模块 `{}` 的 `{qualified}` 冲突；\
                 可以改名或使用选择性导入",
                origin.path.join("::")
            ),
            origin.span,
        )),
        Some(_) => {}
        None => {
            scope.insert(key, qualified.to_owned());
        }
    }
}
