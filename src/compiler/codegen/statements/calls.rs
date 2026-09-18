use crate::ast::*;

use crate::compiler::codegen::emit::entity_query_selector;
use crate::compiler::codegen::names::parameter_holder;
use crate::compiler::codegen::world;
use crate::compiler::codegen::{Compiler, Value};

impl Compiler<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn compile_play_sound(
        &self,
        sound: &str,
        source: &str,
        targets: Option<&str>,
        position: Option<&PositionValue>,
        volume: Option<&str>,
        pitch: Option<&str>,
        min_volume: Option<&str>,
    ) -> String {
        let selector = match targets {
            None => "@s".to_owned(),
            Some(query) => entity_query_selector(self.query(query)),
        };
        let mut command = format!("playsound {sound} {source} {selector}");
        if position.is_some() || volume.is_some() || pitch.is_some() || min_volume.is_some() {
            let position = position
                .map(world::position_value_text)
                .unwrap_or_else(|| "~ ~ ~".to_owned());
            command.push_str(&format!(" {position}"));
        }
        if volume.is_some() || pitch.is_some() || min_volume.is_some() {
            command.push_str(&format!(" {}", volume.unwrap_or("1")));
        }
        if pitch.is_some() || min_volume.is_some() {
            command.push_str(&format!(" {}", pitch.unwrap_or("1")));
        }
        if let Some(min_volume) = min_volume {
            command.push_str(&format!(" {min_volume}"));
        }
        command
    }
    pub(super) fn compile_call(
        &mut self,
        target: &CallTarget,
        arguments: &[Expr],
        owner: &str,
        commands: &mut Vec<String>,
    ) {
        match target {
            CallTarget::Function(function) => {
                self.bind_arguments(function, arguments, owner, commands);
                commands.push(format!("function {}:{function}", self.program.namespace));
            }
            CallTarget::Tag(tag) => {
                commands.push(format!("function #{}:{tag}", self.program.namespace))
            }
        }
    }

    /// 从左到右求值全部实参，再复制到被调用函数的参数计分项。
    pub(in crate::compiler::codegen) fn bind_arguments(
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

    pub(super) fn compile_assignment(
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
}
