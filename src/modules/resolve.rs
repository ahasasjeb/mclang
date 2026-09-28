use super::*;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use crate::ast::*;
use crate::compiler::{valid_user_name, windows_reserved_name};
use crate::diagnostic::Diagnostic;
use crate::name_walk::{NameRole, NameSite};
use crate::parser::keywords::reserved_word;
use crate::version::snapshot::closest;

/// 模块解析结果：合并后的整程序。
pub(crate) fn resolve(
    programs: Vec<(usize, Program)>,
    sources: &[SourcePath<'_>],
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
        .and_then(|path| path.parent())
        .map(Path::to_path_buf);
    let mut module_paths: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    for (index, program) in &by_source {
        if *index == root {
            module_paths.insert(*index, Vec::new());
            continue;
        }
        let (Some(directory), Some(path)) = (root_directory.as_deref(), sources.get(*index)) else {
            continue;
        };
        let Some(segments) = module_segments(path, directory) else {
            continue;
        };
        if segments.first().is_some_and(|segment| segment == "std")
            && !crate::stdlib::is_virtual_path(path, directory)
        {
            diagnostics.push(Diagnostic::new(
                "项目不能定义 `std` 模块：这个路径保留给内置标准库",
                first_span(program),
            ));
        }
        let builtin = crate::stdlib::is_virtual_path(path, directory);
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
    //
    // 声明表按模块独立保存，作用域的键直接来自这些 `String`：改写阶段按 `&str`
    // 借用查询，不再为每个引用分配元组键，也不用复制整张作用域。
    let mut segments_by_module: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    let mut declarations_by_module: BTreeMap<usize, Vec<Declaration>> = BTreeMap::new();
    let mut exports: ModuleExports = BTreeMap::new();
    let mut exports_by_name: ExportsByName = BTreeMap::new();
    let mut scopes: BTreeMap<usize, Scope> = BTreeMap::new();
    for index in &reachable {
        let segments = module_paths.get(index).cloned().unwrap_or_default();
        let declarations: Vec<Declaration> = collect_declarations(&by_source[index])
            .into_iter()
            .map(|(role, name, exported, _)| Declaration {
                role,
                name,
                exported,
            })
            .collect();
        let mut module_exports = Vec::new();
        let mut module_exports_by_name: HashMap<String, Vec<(NameRole, String)>> = HashMap::new();
        let mut scope = Scope::default();
        for declaration in &declarations {
            let qualified = qualify(declaration.role, &segments, &declaration.name);
            if declaration.exported {
                module_exports.push((
                    declaration.role,
                    declaration.name.clone(),
                    qualified.clone(),
                ));
                module_exports_by_name
                    .entry(declaration.name.clone())
                    .or_default()
                    .push((declaration.role, qualified.clone()));
            }
            scope.insert(declaration.role, declaration.name.clone(), qualified);
        }
        declarations_by_module.insert(*index, declarations);
        exports.insert(*index, module_exports);
        exports_by_name.insert(*index, module_exports_by_name);
        scopes.insert(*index, scope);
        segments_by_module.insert(*index, segments);
    }

    // 6. 处理导入：把公开声明映射进导入方的作用域。
    //
    // 放在独立函数里，`by_source` 的只读借用在返回时就结束，第 7 步才能取出模块
    // 做可变改写。
    let scopes = bind_imports(
        &ImportContext {
            by_source: &by_source,
            reachable: &reachable,
            by_module_path: &by_module_path,
            declarations_by_module: &declarations_by_module,
            exports: &exports,
            exports_by_name: &exports_by_name,
        },
        scopes,
        &mut diagnostics,
    );

    // 7. 限定名重写：声明直接改写，引用查作用域，查不到时保持原样，
    //    由语义检查给出「未声明/未导入」的针对性诊断。
    if diagnostics.is_empty() {
        for index in &reachable {
            let scope = scopes.get(index).expect("作用域已为所有可达模块建立");
            let segments = segments_by_module
                .get(index)
                .map(Vec::as_slice)
                .unwrap_or_default();
            let program = by_source.get_mut(index).expect("模块一定来自已解析的程序");
            program.for_each_name_mut(&mut |context, site, role, name| {
                if site == NameSite::Declaration {
                    match role {
                        NameRole::Local | NameRole::Parameter | NameRole::Criterion => {}
                        _ => *name = qualify(role, segments, name),
                    }
                    return;
                }
                match role {
                    NameRole::Local | NameRole::Parameter | NameRole::Criterion => return,
                    NameRole::Score if context.is_local(name) => return,
                    _ => {}
                }
                if let Some(qualified) = scope.get(role, name.as_str()) {
                    name.clear();
                    name.push_str(qualified);
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

/// 选择性导入按原名查公开声明；同名可以对应多个 NameRole。
type ExportsByName = BTreeMap<usize, HashMap<String, Vec<(NameRole, String)>>>;

/// 一个模块的公开声明集合。
type ModuleExports = BTreeMap<usize, Vec<(NameRole, String, String)>>;

/// 导入处理所需的只读上下文。
struct ImportContext<'a> {
    by_source: &'a BTreeMap<usize, Program>,
    reachable: &'a [usize],
    by_module_path: &'a HashMap<String, usize>,
    declarations_by_module: &'a BTreeMap<usize, Vec<Declaration>>,
    exports: &'a ModuleExports,
    exports_by_name: &'a ExportsByName,
}

/// 把每个可达模块的 `import` 绑定进它自己的作用域，返回更新后的作用域表。
///
/// 放在独立函数里，`by_source` 的只读借用在返回时就结束，第 7 步才能取出模块
/// 做可变改写。
fn bind_imports(
    context: &ImportContext<'_>,
    mut scopes: BTreeMap<usize, Scope>,
    diagnostics: &mut Vec<Diagnostic>,
) -> BTreeMap<usize, Scope> {
    let ImportContext {
        by_source,
        reachable,
        by_module_path,
        declarations_by_module,
        exports,
        exports_by_name,
    } = context;
    let locals: HashMap<usize, HashSet<(NameRole, &str)>> = declarations_by_module
        .iter()
        .map(|(index, declarations)| {
            (
                *index,
                declarations
                    .iter()
                    .map(|declaration| (declaration.role, declaration.name.as_str()))
                    .collect(),
            )
        })
        .collect();
    for index in *reachable {
        let scope = scopes.get_mut(index).expect("作用域已为所有可达模块建立");
        let local = &locals[index];
        for import in &by_source[index].imports {
            let Some(target) = by_module_path.get(&import.path.join("/")) else {
                continue;
            };
            match &import.items {
                None => {
                    for (role, name, qualified) in &exports[target] {
                        bind_import(
                            scope,
                            diagnostics,
                            local,
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
                        let Some(matches) = exports_by_name[target].get(item.name.as_str()) else {
                            let target_declarations = &declarations_by_module[target];
                            let declared = target_declarations
                                .iter()
                                .any(|declaration| declaration.name == item.name);
                            if declared {
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
                                    target_declarations
                                        .iter()
                                        .map(|declaration| declaration.name.as_str()),
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
                                scope,
                                diagnostics,
                                local,
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
    }
    scopes
}

/// 一个模块的顶层声明，作用域与导入检查共用。
struct Declaration {
    role: NameRole,
    name: String,
    exported: bool,
}

/// 可见名字：按 `NameRole` 分组，组内按本地名查询。
///
/// 原来是 `HashMap<(NameRole, String), String>`，每次引用都要为元组键临时分配一个
/// `String`。按角色分组后每组是 `HashMap<String, String>`，`get` 直接借用 `&str`
/// 查询（`String: Borrow<str>`），不必构造任何临时键。
#[derive(Default)]
struct Scope {
    groups: [HashMap<String, String>; NameRole::ALL.len()],
}

impl Scope {
    fn insert(&mut self, role: NameRole, name: String, qualified: String) {
        self.groups[role.index()].insert(name, qualified);
    }

    fn get(&self, role: NameRole, name: &str) -> Option<&str> {
        self.groups[role.index()].get(name).map(String::as_str)
    }
}

/// 导入语句的来源信息，用于诊断。
#[derive(Clone, Copy)]
struct ImportOrigin<'a> {
    path: &'a [String],
    span: Span,
}

/// 把导入的名字绑定进作用域：局部声明优先，导入之间冲突报错。
fn bind_import(
    scope: &mut Scope,
    diagnostics: &mut Vec<Diagnostic>,
    locals: &HashSet<(NameRole, &str)>,
    role: NameRole,
    name: &str,
    qualified: &str,
    origin: ImportOrigin<'_>,
) {
    if locals.contains(&(role, name)) {
        // 本模块自己的声明优先，导入的名字被遮蔽。
        return;
    }
    match scope.get(role, name) {
        Some(existing) if existing != qualified => diagnostics.push(Diagnostic::new(
            format!(
                "导入的名字 `{name}` 已经指向 `{existing}`，与模块 `{}` 的 `{qualified}` 冲突；\
                 可以改名或使用选择性导入",
                origin.path.join("::")
            ),
            origin.span,
        )),
        Some(_) => {}
        None => scope.insert(role, name.to_owned(), qualified.to_owned()),
    }
}
