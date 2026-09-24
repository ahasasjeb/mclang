//! 数据包代码生成。
//!
//! 入口是 [`Compiler`]：先为每个用户函数调用一次 [`Compiler::compile_block`]，
//! 再合成 load 函数，最后由 [`Compiler::finish`] 落盘为 [`CompiledPack`]。
//! 具体下降逻辑按职责拆分成子模块：
//!
//! - [`statements`]：控制流（each/if/while/call 等）与辅助函数分配；
//! - [`actions`]：`give` 与 `self` 实体操作；
//! - [`expressions`]：表达式与条件求值，以及临时计分项；
//! - [`names`]：假玩家、objective 和稳定哈希命名；
//! - [`emit`]：Minecraft 命令片段与 JSON 文本的格式化。

mod actions;
mod advancement;
mod command_targets;
mod components;
mod core_commands;
mod emit;
mod entity_commands;
mod expressions;
mod macros;
mod names;
mod scoreboard;
mod statements;
mod ui;
mod world;

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use crate::ast::*;
use crate::diagnostic::Diagnostic;

use super::CompiledPack;
use emit::{empty_slot_source, function_tag_json, pack_metadata, tag_json};
use names::{build_holders, objective_name};

/// 表达式求值结果：编译期整数或运行时计分项。
#[derive(Clone)]
enum Value {
    Integer(i32),
    Score(String),
}

/// 一个正在下降的 `for`/`while` 循环。
///
/// `break`/`continue` 通过循环状态计分项在辅助函数之间传递：
/// 0 = 正常运行，1 = continue（跳到下一轮），2 = break（跳出循环）。
pub(super) struct LoopContext {
    pub(super) state: String,
}

/// 单次代码生成的完整状态。
pub(super) struct Compiler<'a> {
    program: &'a Program,
    objective: String,
    /// `<函数名, 变量名> -> 假玩家`，构造时一次算好。
    holders: HashMap<(&'a str, &'a str), String>,
    functions: BTreeMap<String, Vec<String>>,
    helper_counters: HashMap<String, usize>,
    temporary_counter: usize,
    /// 当前嵌套的循环栈；栈顶是最近一层 `for`/`while`。
    loops: Vec<LoopContext>,
    /// 循环状态与循环上限的编号计数器。
    loop_counter: usize,
    /// 是否使用了 `give(..., self.item)` 需要的空槽来源资源。
    uses_empty_slot: bool,
    selector_overrides: HashMap<String, String>,
    selector_objectives: BTreeMap<String, String>,
}

impl<'a> Compiler<'a> {
    pub(super) fn new(program: &'a Program) -> Self {
        Self {
            program,
            objective: objective_name(&program.namespace),
            holders: build_holders(program),
            functions: BTreeMap::new(),
            helper_counters: HashMap::new(),
            temporary_counter: 0,
            loops: Vec::new(),
            loop_counter: 0,
            uses_empty_slot: false,
            selector_overrides: HashMap::new(),
            selector_objectives: BTreeMap::new(),
        }
    }

    /// 下降全部用户函数，并合成负责初始化的 `__mcl/load` 函数。
    pub(super) fn compile_functions(&mut self) {
        for function in &self.program.functions {
            let commands = self.compile_block(&function.body, &function.name);
            self.functions.insert(function.name.clone(), commands);
        }
        self.finish_macros();

        let load_functions = self
            .program
            .functions
            .iter()
            .filter(|function| function.attributes.contains(&Attribute::Load))
            .collect::<Vec<_>>();
        let mut commands = vec![format!(
            "scoreboard objectives add {} dummy",
            self.objective
        )];
        for objective in self.selector_objectives.values() {
            commands.push(format!("scoreboard objectives add {objective} dummy"));
        }
        for objective in &self.program.objectives {
            let runtime = names::user_objective_name(&self.program.namespace, &objective.name);
            let criteria = objective.criteria.as_deref().unwrap_or("dummy");
            commands.push(format!("scoreboard objectives add {runtime} {criteria}"));
            if let Some(display_name) = &objective.display_name {
                commands.push(format!(
                    "scoreboard objectives modify {runtime} displayname {}",
                    self.component_json(display_name)
                ));
            }
            if let Some(render_type) = &objective.render_type {
                commands.push(format!(
                    "scoreboard objectives modify {runtime} rendertype {render_type}"
                ));
            }
            match &objective.number_format {
                Some(NumberFormat::Blank) => commands.push(format!(
                    "scoreboard objectives modify {runtime} numberformat blank"
                )),
                Some(NumberFormat::Styled(style)) => commands.push(format!(
                    "scoreboard objectives modify {runtime} numberformat styled {}",
                    serde_json::from_str::<serde_json::Value>(style)
                        .expect("validated objective style JSON")
                )),
                Some(NumberFormat::Fixed(component)) => commands.push(format!(
                    "scoreboard objectives modify {runtime} numberformat fixed {}",
                    self.component_json(component)
                )),
                None => {}
            }
            if let Some(slot) = &objective.display_slot {
                commands.push(format!("scoreboard objectives setdisplay {slot} {runtime}"));
            }
        }
        for score in &self.program.scores {
            commands.push(format!(
                "execute unless score {} {} = {} {} run scoreboard players set {} {} {}",
                names::score_holder(&score.name),
                self.objective,
                names::score_holder(&score.name),
                self.objective,
                names::score_holder(&score.name),
                self.objective,
                score.initial
            ));
        }
        for storage in &self.program.storages {
            commands.push(format!(
                "execute unless data storage {} {} run data modify storage {} {} set value []",
                storage.storage_id, storage.path, storage.storage_id, storage.path
            ));
        }
        for function in load_functions {
            commands.push(format!(
                "function {}:{}",
                self.program.namespace, function.name
            ));
        }
        self.functions.insert("__mcl/load".to_owned(), commands);
    }

    /// 生成 `pack.mcmeta`、函数文件、资源文件和 load/tick 标签。
    pub(super) fn finish(self, description: &str) -> Result<CompiledPack, Vec<Diagnostic>> {
        let mut files = BTreeMap::new();
        let mut diagnostics = Vec::new();
        files.insert(PathBuf::from("pack.mcmeta"), pack_metadata(description));
        if self.uses_empty_slot {
            files.insert(
                PathBuf::from("data")
                    .join(&self.program.namespace)
                    .join("slot_source")
                    .join("__mcl/empty_slot.json"),
                empty_slot_source().to_owned(),
            );
        }
        for (name, commands) in self.functions {
            let path = PathBuf::from("data")
                .join(&self.program.namespace)
                .join("function")
                .join(format!("{name}.mcfunction"));
            let source_span = self
                .program
                .functions
                .iter()
                .find(|function| function.name == name)
                .map_or_else(
                    || self.program.namespace_span.unwrap_or_default(),
                    |function| function.span,
                );
            for (index, command) in commands.iter().enumerate() {
                if command.encode_utf16().count() > 2_000_000 {
                    diagnostics.push(Diagnostic::new(
                        format!("函数 `{name}` 第 {} 条命令超过 Minecraft 的 2000000 个 UTF-16 字符上限", index + 1),
                        source_span,
                    ));
                }
                if command.contains(['\n', '\r', '\0']) {
                    diagnostics.push(Diagnostic::new(
                        format!(
                            "函数 `{name}` 第 {} 条命令含换行或 NUL，不能写进 .mcfunction",
                            index + 1
                        ),
                        source_span,
                    ));
                }
                if command.trim_end().ends_with('\\') {
                    diagnostics.push(Diagnostic::new(
                        format!(
                            "函数 `{name}` 第 {} 条命令以反斜杠结尾，会触发续行",
                            index + 1
                        ),
                        source_span,
                    ));
                }
            }
            let mut contents = "# Generated by mclang.\n".to_owned();
            if !commands.is_empty() {
                contents.push_str(&commands.join("\n"));
                contents.push('\n');
            }
            files.insert(path, contents);
        }
        for resource in &self.program.resources {
            let value = serde_json::from_str::<serde_json::Value>(&resource.json)
                .expect("semantic validation guarantees valid JSON");
            let contents = serde_json::to_string_pretty(&value)
                .expect("a parsed JSON value can always be serialized")
                + "\n";
            files.insert(
                PathBuf::from("data")
                    .join(&self.program.namespace)
                    .join(&resource.kind)
                    .join(format!("{}.json", resource.name)),
                contents,
            );
        }
        for advancement in &self.program.advancements {
            files.insert(
                PathBuf::from("data")
                    .join(&self.program.namespace)
                    .join("advancement")
                    .join(format!("{}.json", advancement.name)),
                advancement::advancement_json(
                    &self.program.namespace,
                    advancement,
                    &self.program.item_stacks,
                ),
            );
        }
        for tag in &self.program.function_tags {
            let values = tag
                .values
                .iter()
                .map(|entry| match entry {
                    FunctionTagEntry::Function(name, _) => {
                        format!("{}:{name}", self.program.namespace)
                    }
                    FunctionTagEntry::Tag(name, _) => {
                        format!("#{}:{name}", self.program.namespace)
                    }
                    FunctionTagEntry::External(value, _) => value.clone(),
                })
                .collect::<Vec<_>>();
            files.insert(
                PathBuf::from("data")
                    .join(&self.program.namespace)
                    .join("tags")
                    .join("function")
                    .join(format!("{}.json", tag.name)),
                function_tag_json(&values, tag.replace),
            );
        }

        files.insert(
            PathBuf::from("data/minecraft/tags/function/load.json"),
            tag_json(&[format!("{}:__mcl/load", self.program.namespace)]),
        );
        let ticks = self
            .program
            .functions
            .iter()
            .filter(|function| function.attributes.contains(&Attribute::Tick))
            .map(|function| format!("{}:{}", self.program.namespace, function.name))
            .collect::<Vec<_>>();
        if !ticks.is_empty() {
            files.insert(
                PathBuf::from("data/minecraft/tags/function/tick.json"),
                tag_json(&ticks),
            );
        }
        if diagnostics.is_empty() {
            Ok(CompiledPack {
                files,
                binary_files: BTreeMap::new(),
            })
        } else {
            Err(diagnostics)
        }
    }
}
