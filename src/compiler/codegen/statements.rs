//! 控制流下降：语句分派、条件分支、循环、调用和辅助函数分配。
//!
//! `compile_block` 只做分发；需要独立执行上下文的结构块
//! （`if`/`while`/`each`/`spawn`/`in_dimension`/`execute`）通过分配辅助函数实现。
//! `give` 与 `self` 实体操作在 [`super::actions`]。

use crate::ast::*;

use super::Compiler;
use super::Value;
use super::emit::{entity_query_clause, entity_query_selector, nbt_text};
use super::names::{parameter_holder, user_objective_name};
use super::world;

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
            StatementKind::Spawn {
                entity_type,
                position,
                body,
            } => {
                self.compile_spawn(entity_type, position.as_ref(), body, owner, commands);
            }
            StatementKind::Give {
                target,
                item,
                count,
                ..
            } => self.compile_give(target, item, *count, owner, commands),
            StatementKind::EffectGive {
                target,
                effect,
                duration,
                amplifier,
                hide_particles,
            } => self.compile_effect_give(
                target,
                effect,
                *duration,
                *amplifier,
                *hide_particles,
                commands,
            ),
            StatementKind::EffectClear { target, effect } => {
                self.compile_effect_clear(target, effect.as_deref(), commands);
            }
            StatementKind::XpChange {
                target,
                kind,
                operation,
                amount,
            } => self.compile_xp_change(target, *kind, *operation, *amount, commands),
            StatementKind::StopwatchAction { operation, id } => {
                let operation = match operation {
                    StopwatchOperation::Create => "create",
                    StopwatchOperation::Restart => "restart",
                    StopwatchOperation::Remove => "remove",
                };
                commands.push(format!("stopwatch {operation} {id}"));
            }
            StatementKind::ClearInventory {
                target,
                item,
                max_count,
            } => self.compile_clear_inventory(target, item.as_deref(), *max_count, commands),
            StatementKind::SetBlock {
                pos,
                block,
                mode,
                nbt,
            } => {
                commands.push(world::set_block_command(pos, block, *mode, nbt.as_ref()));
            }
            StatementKind::Fill {
                from,
                to,
                block,
                mode,
                filter,
                nbt,
            } => commands.push(world::fill_command(
                from,
                to,
                block,
                *mode,
                filter.as_ref(),
                nbt.as_ref(),
            )),
            StatementKind::FillBiome {
                from,
                to,
                biome,
                filter,
            } => commands.push(world::fill_biome_command(
                from,
                to,
                biome,
                filter.as_deref(),
            )),
            StatementKind::Clone {
                begin,
                end,
                destination,
                from_dimension,
                to_dimension,
                filter,
                mode,
                strict,
            } => commands.push(world::clone_command(&world::CloneOptions {
                begin,
                end,
                destination,
                from_dimension: from_dimension.as_deref(),
                to_dimension: to_dimension.as_deref(),
                filter,
                mode: *mode,
                strict: *strict,
            })),
            StatementKind::PlaceFeature { feature, pos } => {
                commands.push(world::place_feature_command(feature, pos.as_ref()));
            }
            StatementKind::PlaceJigsaw {
                pool,
                target,
                max_depth,
                pos,
            } => commands.push(world::place_jigsaw_command(
                pool,
                target,
                *max_depth,
                pos.as_ref(),
            )),
            StatementKind::PlaceStructure { structure, pos } => {
                commands.push(world::place_structure_command(structure, pos.as_ref()));
            }
            StatementKind::PlaceTemplate {
                template,
                pos,
                rotation,
                mirror,
                integrity,
                seed,
                strict,
            } => commands.push(world::place_template_command(
                &world::PlaceTemplateOptions {
                    template,
                    pos,
                    rotation: *rotation,
                    mirror: *mirror,
                    integrity: integrity.as_deref(),
                    seed: *seed,
                    strict: *strict,
                },
            )),
            StatementKind::ForceLoad(operation) => {
                commands.push(world::forceload_command(operation));
            }
            StatementKind::TimeAction { operation, clock } => {
                commands.push(world::time_command(operation, clock.as_deref()));
            }
            StatementKind::Weather { kind, duration } => {
                commands.push(world::weather_command(*kind, duration.as_deref()));
            }
            StatementKind::GameRuleSet { name, value } => {
                commands.push(world::gamerule_command(name, *value));
            }
            StatementKind::WorldBorder(operation) => {
                commands.push(world::worldborder_command(operation));
            }
            StatementKind::Locate { kind, target } => {
                commands.push(world::locate_command(kind.as_str(), target));
            }
            StatementKind::SelfAction(action) => commands.extend(self.compile_self_action(action)),
            StatementKind::Message { target, component } => {
                commands.push(self.compile_message(target, component));
            }
            StatementKind::PlaySound {
                sound,
                source,
                targets,
                position,
                volume,
                pitch,
                min_volume,
            } => {
                commands.push(self.compile_play_sound(
                    sound,
                    source,
                    targets.as_deref(),
                    position.as_ref(),
                    volume.as_deref(),
                    pitch.as_deref(),
                    min_volume.as_deref(),
                ));
            }
            StatementKind::Call { target, arguments } => {
                self.compile_call(target, arguments, owner, commands)
            }
            StatementKind::Schedule {
                target,
                delay,
                mode,
            } => self.compile_schedule(target, delay, *mode, commands),
            StatementKind::ScheduleClear { function } => {
                commands.push(format!(
                    "schedule clear {}:{function}",
                    self.program.namespace
                ));
            }
            StatementKind::Assign {
                target,
                operation,
                value,
            } => self.compile_assignment(target, *operation, value, owner, commands),
            StatementKind::ScoreSet { target, value } => {
                self.compile_score_set(target, value, owner, commands);
            }
            StatementKind::ScoreReset { target } => {
                self.compile_score_reset(target, commands);
            }
            StatementKind::ScoreboardEnable { target } => {
                self.compile_scoreboard_enable(target, commands);
            }
            StatementKind::ScoreboardOperation {
                result,
                operation,
                source,
            } => {
                self.compile_scoreboard_operation(result, *operation, source, commands);
            }
            StatementKind::ScoreboardDisplay {
                slot, objective, ..
            } => {
                let objective = objective
                    .as_ref()
                    .map(|(name, _)| user_objective_name(&self.program.namespace, name));
                match objective {
                    Some(objective) => commands.push(format!(
                        "scoreboard objectives setdisplay {slot} {objective}"
                    )),
                    None => commands.push(format!("scoreboard objectives setdisplay {slot}")),
                }
            }
            StatementKind::Teleport {
                targets,
                destination,
                rotation,
            } => {
                self.compile_teleport(targets, destination, rotation.as_ref(), commands);
            }
            StatementKind::ItemAction {
                method,
                target,
                slots,
                slots_span: _,
                action,
            } => {
                self.compile_item_action(*method, target, slots, action, commands);
            }
            StatementKind::NbtMerge { nbt } => {
                commands.push(format!("data merge entity @s {}", nbt_text(nbt)));
            }
            StatementKind::DataMerge { target, nbt } => {
                commands.push(format!(
                    "data merge {} {}",
                    self.nbt_source_text(target),
                    nbt_text(nbt)
                ));
            }
            StatementKind::DataRemove {
                target,
                path,
                path_span: _,
            } => {
                commands.push(format!(
                    "data remove {} {path}",
                    self.nbt_source_text(target)
                ));
            }
            StatementKind::DataModify {
                target,
                path,
                path_span: _,
                operation,
            } => {
                let kind = match operation.kind {
                    DataOperationKind::Insert => "insert",
                    DataOperationKind::Prepend => "prepend",
                    DataOperationKind::Append => "append",
                    DataOperationKind::Set => "set",
                    DataOperationKind::Merge => "merge",
                };
                let mut command =
                    format!("data modify {} {path} {kind}", self.nbt_source_text(target));
                if let Some(index) = operation.index {
                    command.push_str(&format!(" {index}"));
                }
                match &operation.source {
                    DataSource::From {
                        target,
                        path,
                        path_span: _,
                    } => {
                        command.push_str(&format!(" from {} {path}", self.nbt_source_text(target)))
                    }
                    DataSource::Value(nbt) => {
                        command.push_str(&format!(" value {}", nbt_text(nbt)));
                    }
                    DataSource::String {
                        target,
                        path,
                        path_span: _,
                        start,
                        end,
                    } => {
                        command
                            .push_str(&format!(" string {} {path}", self.nbt_source_text(target)));
                        if let Some(start) = start {
                            command.push_str(&format!(" {start}"));
                        }
                        if let Some(end) = end {
                            command.push_str(&format!(" {end}"));
                        }
                    }
                    DataSource::Compute {
                        source,
                        kind,
                        provider,
                        provider_span: _,
                        scale,
                    } => {
                        command.push_str(&format!(
                            " compute {} {} {provider}",
                            self.compute_source_text(source),
                            match kind {
                                ComputeKind::Float => "float",
                                ComputeKind::Integer => "integer",
                            }
                        ));
                        if let Some(scale) = scale {
                            command.push_str(&format!(" {scale}"));
                        }
                    }
                }
                commands.push(command);
            }
            StatementKind::AdvancementAction {
                operation,
                scope,
                targets,
                advancement,
                criterion,
                ..
            } => self.compile_advancement_action(
                *operation,
                *scope,
                targets,
                advancement.as_ref(),
                criterion.as_deref(),
                commands,
            ),
            StatementKind::Let { name, value, .. } => {
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
            StatementKind::Return(kind) => self.compile_return(kind, owner, commands),
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

    /// `sound.self`/`sound.play`：按原版顺序补齐可选参数。
    #[allow(clippy::too_many_arguments)]
    fn compile_play_sound(
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

    fn compile_spawn(
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

    fn compile_schedule(
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

    fn compile_return(&mut self, kind: &ReturnKind, owner: &str, commands: &mut Vec<String>) {
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

    fn compile_call(
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

    pub(super) fn next_helper_path(&mut self, owner: &str) -> String {
        let counter = self.helper_counters.entry(owner.to_owned()).or_default();
        let path = format!("__mcl/{owner}/{}", *counter);
        *counter += 1;
        path
    }
}
