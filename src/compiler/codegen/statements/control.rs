use crate::ast::*;

use crate::compiler::codegen::emit::entity_query_clause;
use crate::compiler::codegen::world;
use crate::compiler::codegen::{Compiler, Value};

use super::helpers::{LoopLimit, constant_condition, constant_integer};

impl Compiler<'_> {
    pub(super) fn compile_each(
        &mut self,
        query_name: &str,
        body: &[Statement],
        owner: &str,
        commands: &mut Vec<String>,
    ) {
        let helper = self.compile_helper(body, owner);
        let query = self
            .program
            .queries
            .iter()
            .find(|candidate| candidate.name == query_name)
            .expect("semantic validation guarantees the entity query exists");
        commands.push(format!(
            "execute {} run function {}:{helper}",
            entity_query_clause(query),
            self.program.namespace
        ));
    }

    pub(super) fn compile_in_dimension(
        &mut self,
        dimension: &str,
        body: &[Statement],
        owner: &str,
        commands: &mut Vec<String>,
    ) {
        let helper = self.compile_helper(body, owner);
        commands.push(format!(
            "execute in {dimension} run function {}:{helper}",
            self.program.namespace
        ));
    }

    /// `sound.self`/`sound.play`：按原版顺序补齐可选参数。
    pub(super) fn compile_spawn(
        &mut self,
        entity_type: &str,
        position: Option<&PositionValue>,
        body: &[Statement],
        owner: &str,
        commands: &mut Vec<String>,
    ) {
        let helper = self.compile_helper(body, owner);
        let positioned = match position {
            Some(position) => format!(" positioned {}", world::position_value_text(position)),
            None => String::new(),
        };
        commands.push(format!(
            "execute{positioned} summon {entity_type} run function {}:{helper}",
            self.program.namespace
        ));
    }

    pub(super) fn compile_schedule(
        &self,
        target: &CallTarget,
        delay: &str,
        mode: ScheduleMode,
        commands: &mut Vec<String>,
    ) {
        let name = match target {
            CallTarget::Function(function) => format!("{}:{function}", self.program.namespace),
            CallTarget::Tag(tag) => format!("#{}:{tag}", self.program.namespace),
        };
        commands.push(format!(
            "schedule function {name} {delay} {}",
            match mode {
                ScheduleMode::Replace => "replace",
                ScheduleMode::Append => "append",
            }
        ));
    }

    /// 结构化 `execute`：修饰符拼进真实 execute 链，条件在同一上下文里
    pub(super) fn compile_return(
        &mut self,
        kind: &ReturnKind,
        owner: &str,
        commands: &mut Vec<String>,
    ) {
        match kind {
            ReturnKind::Void => commands.push("return 0".to_owned()),
            ReturnKind::Fail => commands.push("return fail".to_owned()),
            ReturnKind::Run(command) => commands.push(format!("return run {command}")),
            ReturnKind::Value(expression) => match self.compile_expr(expression, owner, commands) {
                Value::Integer(value) => commands.push(format!("return {value}")),
                Value::Score(score) => commands.push(format!(
                    "return run scoreboard players get {score} {}",
                    self.objective
                )),
            },
        }
    }
    pub(super) fn compile_if(
        &mut self,
        condition: &Condition,
        then_body: &[Statement],
        else_body: &[Statement],
        owner: &str,
        commands: &mut Vec<String>,
    ) {
        // 常量条件在编译期直接选定分支：不生成标志，也不分配辅助函数。
        match constant_condition(condition) {
            Some(true) => {
                let mut branch = self.compile_block(then_body, owner);
                commands.append(&mut branch);
                return;
            }
            Some(false) => {
                let mut branch = self.compile_block(else_body, owner);
                commands.append(&mut branch);
                return;
            }
            None => {}
        }
        let flag = self.compile_condition(condition, owner, commands);
        let then_helper = self.compile_helper(then_body, owner);
        commands.push(format!(
            "execute if score {flag} {} matches 1 run function {}:{then_helper}",
            self.objective, self.program.namespace
        ));
        if !else_body.is_empty() {
            let else_helper = self.compile_helper(else_body, owner);
            commands.push(format!(
                "execute if score {flag} {} matches 0 run function {}:{else_helper}",
                self.objective, self.program.namespace
            ));
        }
    }

    /// `while` 循环：循环辅助函数在每轮开头求值条件，命中就调用循环体辅助函数，
    /// 再按循环状态处理 `break`/`continue`。条件为常量时省去标志求值；
    /// `while 0` 不生成任何命令。
    pub(super) fn compile_while(
        &mut self,
        condition: &Condition,
        body: &[Statement],
        owner: &str,
        commands: &mut Vec<String>,
    ) {
        let constant = constant_condition(condition);
        if constant == Some(false) {
            return;
        }

        let state = self.push_loop();
        let body_helper = self.compile_helper(body, owner);
        self.pop_loop();

        let loop_helper = self.next_helper_path(owner);
        let mut loop_commands = Vec::new();
        if constant == Some(true) {
            loop_commands.push(format!(
                "execute if score {state} {} matches 0 run function {}:{body_helper}",
                self.objective, self.program.namespace
            ));
            loop_commands.push(self.break_check(&state));
            loop_commands.push(self.continue_reset(&state));
            loop_commands.push(format!("function {}:{loop_helper}", self.program.namespace));
        } else {
            let flag = self.compile_condition(condition, owner, &mut loop_commands);
            loop_commands.push(format!(
                "execute if score {flag} {} matches 1 run function {}:{body_helper}",
                self.objective, self.program.namespace
            ));
            loop_commands.push(self.break_check(&state));
            loop_commands.push(self.continue_reset(&state));
            loop_commands.push(format!(
                "execute if score {flag} {} matches 1 run function {}:{loop_helper}",
                self.objective, self.program.namespace
            ));
        }
        self.functions.insert(loop_helper.clone(), loop_commands);
        commands.push(format!(
            "scoreboard players set {state} {} 0",
            self.objective
        ));
        commands.push(format!("function {}:{loop_helper}", self.program.namespace));
    }

    /// `for <变量> in <起点>..<终点> { ... }`：半开区间循环。
    ///
    /// 起点与终点可以是表达式：终点是常量时直接用 `matches ..N` 比较，否则
    /// materialize 到循环上限计分项。上限不超过起点的常量区间不生成命令。
    pub(super) fn compile_for(
        &mut self,
        variable: &str,
        start: &Expr,
        end: &Expr,
        body: &[Statement],
        owner: &str,
        commands: &mut Vec<String>,
    ) {
        let start_value = constant_integer(start);
        let end_value = constant_integer(end);
        if let (Some(start_value), Some(end_value)) = (start_value, end_value)
            && start_value >= end_value
        {
            return;
        }

        let state = self.push_loop();
        let variable_holder = self.variable_holder(owner, variable);
        let body_helper = self.compile_helper(body, owner);
        self.pop_loop();

        let limit = match end_value {
            Some(end_value) => LoopLimit::Constant(end_value),
            None => {
                let holder = self.next_loop_holder("limit");
                let mut limit_commands = Vec::new();
                let value = self.compile_expr(end, owner, &mut limit_commands);
                self.store_value(&holder, value, &mut limit_commands);
                for command in limit_commands {
                    commands.push(command);
                }
                LoopLimit::Holder(holder)
            }
        };

        commands.push(format!(
            "scoreboard players set {state} {} 0",
            self.objective
        ));
        match start_value {
            Some(start_value) => commands.push(format!(
                "scoreboard players set {variable_holder} {} {start_value}",
                self.objective
            )),
            None => {
                let mut start_commands = Vec::new();
                let value = self.compile_expr(start, owner, &mut start_commands);
                self.store_value(&variable_holder, value, &mut start_commands);
                for command in start_commands {
                    commands.push(command);
                }
            }
        }

        let loop_helper = self.next_helper_path(owner);
        let in_range = self.range_check(&variable_holder, &limit);
        let loop_commands = vec![
            format!(
                "execute if score {state} {} matches 0 {in_range} run function {}:{body_helper}",
                self.objective, self.program.namespace
            ),
            self.break_check(&state),
            self.continue_reset(&state),
            format!(
                "scoreboard players add {variable_holder} {} 1",
                self.objective
            ),
            format!(
                "execute {in_range} run function {}:{loop_helper}",
                self.program.namespace
            ),
        ];
        self.functions.insert(loop_helper.clone(), loop_commands);
        commands.push(format!("function {}:{loop_helper}", self.program.namespace));
    }

    /// 循环状态计分项：0 = 正常，1 = continue，2 = break。
    pub(super) fn push_loop(&mut self) -> String {
        let state = self.next_loop_holder("state");
        self.loops.push(crate::compiler::codegen::LoopContext {
            state: state.clone(),
        });
        state
    }

    pub(super) fn pop_loop(&mut self) {
        self.loops.pop();
    }

    pub(super) fn next_loop_holder(&mut self, kind: &str) -> String {
        let index = self.loop_counter;
        self.loop_counter += 1;
        format!("#loop_{kind}_{index}")
    }

    /// 当前循环状态计分项；语义检查保证 `break`/`continue` 只在循环体内。
    pub(super) fn loop_state(&self) -> String {
        self.loops
            .last()
            .expect("语义检查保证 break/continue 只出现在循环体内")
            .state
            .clone()
    }

    pub(super) fn break_check(&self, state: &str) -> String {
        format!(
            "execute if score {state} {} matches 2 run return 0",
            self.objective
        )
    }

    pub(super) fn continue_reset(&self, state: &str) -> String {
        format!(
            "execute if score {state} {} matches 1 run scoreboard players set {state} {} 0",
            self.objective, self.objective
        )
    }

    /// `execute if score <循环变量> <目标> matches ..<上限-1>` 或 `... < <上限>`。
    pub(super) fn range_check(&self, variable: &str, limit: &LoopLimit) -> String {
        match limit {
            LoopLimit::Constant(end) => format!(
                "if score {variable} {} matches ..{}",
                self.objective,
                i64::from(*end) - 1
            ),
            LoopLimit::Holder(holder) => format!(
                "if score {variable} {} < {holder} {}",
                self.objective, self.objective
            ),
        }
    }
}
