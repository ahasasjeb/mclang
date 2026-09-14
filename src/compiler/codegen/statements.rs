//! 语句下降：把控制流、调用和实体操作展开为数据包函数与命令。
//!
//! `compile_block` 只做分发，每种语句对应一个独立方法；需要独立执行上下文的
//! 结构块（`if`/`while`/`each`/`spawn`/`in_dimension`/`execute`）通过分配辅助
//! 函数实现。

use crate::ast::*;

use super::Compiler;
use super::Value;
use super::emit::{compile_message, entity_query_clause, item_stack_argument};
use super::names::parameter_holder;

impl Compiler<'_> {
    /// 下降一个语句块。辅助函数命名计数器按所属函数（`owner`）独立编号。
    pub(super) fn compile_block(&mut self, statements: &[Statement], owner: &str) -> Vec<String> {
        let mut commands = Vec::new();
        for statement in statements {
            self.compile_statement(statement, owner, &mut commands);
        }
        commands
    }

    fn compile_statement(
        &mut self,
        statement: &Statement,
        owner: &str,
        commands: &mut Vec<String>,
    ) {
        match &statement.kind {
            StatementKind::Run(command) => commands.push(command.clone()),
            StatementKind::Each { query, body } => {
                self.compile_each(query, body, owner, commands);
            }
            StatementKind::InDimension { dimension, body } => {
                self.compile_in_dimension(dimension, body, owner, commands);
            }
            StatementKind::Spawn { entity_type, body } => {
                self.compile_spawn(entity_type, body, owner, commands);
            }
            StatementKind::Give {
                target,
                item,
                count,
                ..
            } => self.compile_give(target, item, *count, commands),
            StatementKind::SelfAction(action) => commands.extend(self.compile_self_action(action)),
            StatementKind::Message {
                target,
                text,
                color,
            } => {
                commands.push(compile_message(target, text, color.as_deref()));
            }
            StatementKind::PlaySound { sound, source } => {
                commands.push(format!("playsound {sound} {source} @s ~ ~ ~ 1 1"));
            }
            StatementKind::Call {
                function,
                arguments,
            } => self.compile_call(function, arguments, owner, commands),
            StatementKind::Schedule {
                function,
                delay,
                mode,
            } => self.compile_schedule(function, delay, *mode, commands),
            StatementKind::Assign {
                target,
                operation,
                value,
            } => self.compile_assignment(target, *operation, value, owner, commands),
            StatementKind::Let { name, value } => {
                self.compile_assignment(name, AssignOp::Set, value, owner, commands);
            }
            StatementKind::If {
                condition,
                then_body,
                else_body,
            } => self.compile_if(condition, then_body, else_body, owner, commands),
            StatementKind::While { condition, body } => {
                self.compile_while(condition, body, owner, commands);
            }
            StatementKind::Execute { clauses, body } => {
                self.compile_execute(clauses, body, owner, commands);
            }
            StatementKind::Return(value) => self.compile_return(value.as_ref(), owner, commands),
        }
    }

    fn compile_each(
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

    fn compile_in_dimension(
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

    fn compile_spawn(
        &mut self,
        entity_type: &str,
        body: &[Statement],
        owner: &str,
        commands: &mut Vec<String>,
    ) {
        let helper = self.compile_helper(body, owner);
        commands.push(format!(
            "execute summon {entity_type} run function {}:{helper}",
            self.program.namespace
        ));
    }

    fn compile_give(
        &self,
        target: &str,
        item: &str,
        count: Option<u32>,
        commands: &mut Vec<String>,
    ) {
        let query = self
            .program
            .queries
            .iter()
            .find(|candidate| candidate.name == target)
            .expect("semantic validation guarantees the entity query exists");
        let item = self
            .program
            .item_stacks
            .iter()
            .find(|candidate| candidate.name == item)
            .expect("semantic validation guarantees the item definition exists");
        commands.push(format!(
            "execute {} run give @s {} {}",
            entity_query_clause(query),
            item_stack_argument(item),
            count.unwrap_or(item.count)
        ));
    }

    fn compile_schedule(
        &self,
        function: &str,
        delay: &str,
        mode: ScheduleMode,
        commands: &mut Vec<String>,
    ) {
        commands.push(format!(
            "schedule function {}:{} {} {}",
            self.program.namespace,
            function,
            delay,
            match mode {
                ScheduleMode::Replace => "replace",
                ScheduleMode::Append => "append",
            }
        ));
    }

    fn compile_execute(
        &mut self,
        clauses: &str,
        body: &[Statement],
        owner: &str,
        commands: &mut Vec<String>,
    ) {
        let helper = self.compile_helper(body, owner);
        commands.push(format!(
            "execute {clauses} run function {}:{helper}",
            self.program.namespace
        ));
    }

    fn compile_return(&mut self, value: Option<&Expr>, owner: &str, commands: &mut Vec<String>) {
        let Some(expression) = value else {
            commands.push("return 0".to_owned());
            return;
        };
        match self.compile_expr(expression, owner, commands) {
            Value::Integer(value) => commands.push(format!("return {value}")),
            Value::Score(score) => commands.push(format!(
                "return run scoreboard players get {score} {}",
                self.objective
            )),
        }
    }

    fn compile_self_action(&self, action: &SelfAction) -> Vec<String> {
        match action {
            SelfAction::AddTag(tag) => vec![format!("tag @s add {tag}")],
            SelfAction::RemoveTag(tag) => vec![format!("tag @s remove {tag}")],
            SelfAction::SetInvulnerable(value) => vec![format!(
                "data merge entity @s {{Invulnerable:{}b}}",
                if *value { 1 } else { 0 }
            )],
            SelfAction::SaveItems(name) => {
                let storage = self
                    .program
                    .storages
                    .iter()
                    .find(|candidate| candidate.name == *name)
                    .expect("semantic validation guarantees the item storage exists");
                vec![format!(
                    "data modify storage {} {} set from entity @s Items",
                    storage.storage_id, storage.path
                )]
            }
            SelfAction::RestoreItems(name) => {
                let storage = self
                    .program
                    .storages
                    .iter()
                    .find(|candidate| candidate.name == *name)
                    .expect("semantic validation guarantees the item storage exists");
                vec![format!(
                    "data modify entity @s Items set from storage {} {}",
                    storage.storage_id, storage.path
                )]
            }
            SelfAction::RemovePreservingItems(name) => {
                let storage = self
                    .program
                    .storages
                    .iter()
                    .find(|candidate| candidate.name == *name)
                    .expect("semantic validation guarantees the item storage exists");
                vec![
                    format!(
                        "data modify storage {} {} set from entity @s Items",
                        storage.storage_id, storage.path
                    ),
                    "data modify entity @s Items set value []".to_owned(),
                    "kill @s".to_owned(),
                ]
            }
            SelfAction::GiveItem { item, count, .. } => {
                let item = self
                    .program
                    .item_stacks
                    .iter()
                    .find(|candidate| candidate.name == *item)
                    .expect("semantic validation guarantees the item definition exists");
                vec![format!(
                    "give @s {} {}",
                    item_stack_argument(item),
                    count.unwrap_or(item.count)
                )]
            }
            SelfAction::ClearItems => {
                vec!["data modify entity @s Items set value []".to_owned()]
            }
            SelfAction::Remove | SelfAction::Consume => vec!["kill @s".to_owned()],
        }
    }

    fn compile_call(
        &mut self,
        function: &str,
        arguments: &[Expr],
        owner: &str,
        commands: &mut Vec<String>,
    ) {
        self.bind_arguments(function, arguments, owner, commands);
        commands.push(format!("function {}:{function}", self.program.namespace));
    }

    /// 从左到右求值全部实参，再复制到被调用函数的参数计分项。
    pub(super) fn bind_arguments(
        &mut self,
        function: &str,
        arguments: &[Expr],
        owner: &str,
        commands: &mut Vec<String>,
    ) {
        let mut values = Vec::new();
        for argument in arguments {
            values.push(self.compile_expr(argument, owner, commands));
        }
        let parameters = self
            .program
            .functions
            .iter()
            .find(|candidate| candidate.name == function)
            .expect("semantic validation guarantees the called function exists")
            .parameters
            .iter()
            .map(|parameter| parameter.name.clone())
            .collect::<Vec<_>>();
        for (parameter, value) in parameters.iter().zip(values) {
            let target = parameter_holder(function, parameter);
            match value {
                Value::Integer(value) => commands.push(format!(
                    "scoreboard players set {target} {} {value}",
                    self.objective
                )),
                Value::Score(source) => commands.push(format!(
                    "scoreboard players operation {target} {} = {source} {}",
                    self.objective, self.objective
                )),
            }
        }
    }

    fn compile_assignment(
        &mut self,
        target: &str,
        operation: AssignOp,
        expression: &Expr,
        owner: &str,
        commands: &mut Vec<String>,
    ) {
        let target = self.variable_holder(owner, target);
        let value = self.compile_expr(expression, owner, commands);
        if operation == AssignOp::Set {
            match value {
                Value::Integer(value) => commands.push(format!(
                    "scoreboard players set {target} {} {value}",
                    self.objective
                )),
                Value::Score(source) => commands.push(format!(
                    "scoreboard players operation {target} {} = {source} {}",
                    self.objective, self.objective
                )),
            }
            return;
        }

        // `add`/`remove` 命令比等价的 `operation +=` 更短，且不受整数溢出习惯影响。
        if matches!(operation, AssignOp::Add | AssignOp::Subtract)
            && let Value::Integer(value) = &value
        {
            let signed = if operation == AssignOp::Add {
                i64::from(*value)
            } else {
                -i64::from(*value)
            };
            if (0..=i64::from(i32::MAX)).contains(&signed) {
                commands.push(format!(
                    "scoreboard players add {target} {} {signed}",
                    self.objective
                ));
                return;
            } else if (-i64::from(i32::MAX)..0).contains(&signed) {
                commands.push(format!(
                    "scoreboard players remove {target} {} {}",
                    self.objective, -signed
                ));
                return;
            }
        }

        let source = self.materialize(value, commands);
        let symbol = match operation {
            AssignOp::Set => unreachable!(),
            AssignOp::Add => "+=",
            AssignOp::Subtract => "-=",
            AssignOp::Multiply => "*=",
            AssignOp::Divide => "/=",
            AssignOp::Modulo => "%=",
        };
        commands.push(format!(
            "scoreboard players operation {target} {} {symbol} {source} {}",
            self.objective, self.objective
        ));
    }

    fn compile_if(
        &mut self,
        condition: &Condition,
        then_body: &[Statement],
        else_body: &[Statement],
        owner: &str,
        commands: &mut Vec<String>,
    ) {
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

    /// `while` 由条件辅助函数和循环体辅助函数互相调度实现，
    /// 运行时上限仍由 Minecraft 的命令链长度决定。
    fn compile_while(
        &mut self,
        condition: &Condition,
        body: &[Statement],
        owner: &str,
        commands: &mut Vec<String>,
    ) {
        let loop_helper = self.next_helper_path(owner);
        let body_helper = self.next_helper_path(owner);

        let mut body_commands = self.compile_block(body, owner);
        body_commands.push(format!("function {}:{loop_helper}", self.program.namespace));
        self.functions.insert(body_helper.clone(), body_commands);

        let mut loop_commands = Vec::new();
        let flag = self.compile_condition(condition, owner, &mut loop_commands);
        loop_commands.push(format!(
            "execute if score {flag} {} matches 1 run function {}:{body_helper}",
            self.objective, self.program.namespace
        ));
        self.functions.insert(loop_helper.clone(), loop_commands);
        commands.push(format!("function {}:{loop_helper}", self.program.namespace));
    }

    /// 把语句块放入新辅助函数，返回该函数的路径。
    fn compile_helper(&mut self, body: &[Statement], owner: &str) -> String {
        let path = self.next_helper_path(owner);
        let commands = self.compile_block(body, owner);
        self.functions.insert(path.clone(), commands);
        path
    }

    fn next_helper_path(&mut self, owner: &str) -> String {
        let counter = self.helper_counters.entry(owner.to_owned()).or_default();
        let path = format!("__mcl/{owner}/{}", *counter);
        *counter += 1;
        path
    }
}
