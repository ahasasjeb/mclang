//! 控制流下降：语句分派、条件分支、循环、调用和辅助函数分配。
//!
//! `compile_block` 只做分发；需要独立执行上下文的结构块
//! （`if`/`while`/`each`/`spawn`/`in_dimension`/`execute`）通过分配辅助函数实现。
//! `give` 与 `self` 实体操作在 [`super::actions`]。

use crate::ast::*;

use super::Compiler;
use super::Value;
use super::emit::{entity_query_as_clause, entity_query_clause, entity_query_selector, nbt_text};
use super::names::{parameter_holder, user_objective_name};
use super::world;

impl Compiler<'_> {
    /// 下降一个语句块。辅助函数命名计数器按所属函数（`owner`）独立编号。
    ///
    /// 位于循环体内时，任何可能设置 break/continue 状态的语句之后都会插入一条
    /// 状态检查：`execute unless score <state> matches 0 run return 0`。它让
    /// `if` 辅助函数里设置的循环状态能立即终止当前循环体。
    pub(super) fn compile_block(&mut self, statements: &[Statement], owner: &str) -> Vec<String> {
        let mut commands = Vec::new();
        for statement in statements {
            self.compile_statement(statement, owner, &mut commands);
            if let Some(state) = self.loops.last().map(|context| context.state.clone())
                && contains_flow_jump(statement)
            {
                commands.push(format!(
                    "execute unless score {state} {} matches 0 run return 0",
                    self.objective
                ));
            }
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
            StatementKind::For {
                variable,
                start,
                end,
                body,
                ..
            } => {
                self.compile_for(variable, start, end, body, owner, commands);
            }
            StatementKind::Break => {
                let state = self.loop_state();
                commands.push(format!(
                    "scoreboard players set {state} {} 2",
                    self.objective
                ));
            }
            StatementKind::Continue => {
                let state = self.loop_state();
                commands.push(format!(
                    "scoreboard players set {state} {} 1",
                    self.objective
                ));
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

    /// 结构化 `execute`：修饰符拼进真实 execute 链，条件在同一上下文里
    /// 求值为 0/1 标志后再进入块体，store 包裹块内最后一条命令。
    fn compile_execute(
        &mut self,
        clauses: &ExecuteClauses,
        body: &[Statement],
        owner: &str,
        commands: &mut Vec<String>,
    ) {
        let clauses = match clauses {
            ExecuteClauses::Raw(clauses) => {
                let helper = self.compile_helper(body, owner);
                commands.push(format!(
                    "execute {clauses} run function {}:{helper}",
                    self.program.namespace
                ));
                return;
            }
            ExecuteClauses::Structured(clauses) => clauses,
        };

        let mut modifiers = Vec::new();
        let mut conditions: Vec<(&Condition, bool)> = Vec::new();
        let mut plan = StorePlan::default();
        for clause in clauses {
            match &clause.kind {
                ExecuteClauseKind::As { query, .. } => {
                    modifiers.push(entity_query_as_clause(self.query(query)));
                }
                ExecuteClauseKind::At { query, .. } => {
                    modifiers.push(format!("at {}", entity_query_selector(self.query(query))));
                }
                ExecuteClauseKind::Positioned(position) => modifiers.push(format!(
                    "positioned {}",
                    world::position_value_text(position)
                )),
                ExecuteClauseKind::Rotated(rotation) => {
                    modifiers.push(format!("rotated {}", world::rotation_text(rotation)));
                }
                ExecuteClauseKind::FacingPosition(position) => {
                    modifiers.push(format!("facing {}", world::position_value_text(position)))
                }
                ExecuteClauseKind::FacingEntity { query, anchor, .. } => modifiers.push(format!(
                    "facing entity {} {}",
                    entity_query_selector(self.query(query)),
                    anchor.as_str()
                )),
                ExecuteClauseKind::Align { axes, .. } => modifiers.push(format!("align {axes}")),
                ExecuteClauseKind::Anchored(anchor) => {
                    modifiers.push(format!("anchored {}", anchor.as_str()));
                }
                ExecuteClauseKind::In { dimension, .. } => {
                    modifiers.push(format!("in {dimension}"));
                }
                ExecuteClauseKind::On(relation) => {
                    modifiers.push(format!("on {}", relation.as_str()));
                }
                ExecuteClauseKind::Summon { entity_type, .. } => {
                    modifiers.push(format!("summon {entity_type}"));
                }
                ExecuteClauseKind::If(condition) => conditions.push((condition, false)),
                ExecuteClauseKind::Unless(condition) => conditions.push((condition, true)),
                ExecuteClauseKind::StoreResult(target) => {
                    self.plan_store(&mut plan, "result", target);
                }
                ExecuteClauseKind::StoreSuccess(target) => {
                    self.plan_store(&mut plan, "success", target);
                }
                ExecuteClauseKind::StoreData(data) => self.plan_store_data(&mut plan, data),
            }
        }

        // 块体：store 把 `execute store ... run` 套在块内最后一条命令上，
        // 于是捕获的是该命令在修饰符上下文里的结果。
        let body_helper = if plan.clauses.is_empty() {
            self.compile_helper(body, owner)
        } else {
            let mut body_commands = self.compile_block(body, owner);
            let last = body_commands
                .pop()
                .expect("semantic validation guarantees the stored block is not empty");
            body_commands.extend(plan.presets);
            body_commands.push(format!("execute {} run {last}", plan.clauses.join(" ")));
            body_commands.extend(plan.followups);
            let helper = self.next_helper_path(owner);
            self.functions.insert(helper.clone(), body_commands);
            helper
        };
        let namespace = self.program.namespace.clone();
        let prefix = if modifiers.is_empty() {
            String::new()
        } else {
            format!(" {}", modifiers.join(" "))
        };

        if conditions.is_empty() {
            commands.push(format!(
                "execute{prefix} run function {namespace}:{body_helper}"
            ));
            return;
        }

        // 条件在修饰符建立的上下文里求值，全部成立才进入块体。
        let mut entry_commands = Vec::new();
        let mut gates = Vec::new();
        for (condition, negated) in conditions {
            let flag = self.compile_condition(condition, owner, &mut entry_commands);
            gates.push(format!(
                "{} score {flag} {} matches 1",
                if negated { "unless" } else { "if" },
                self.objective
            ));
        }
        entry_commands.push(format!(
            "execute {} run function {namespace}:{body_helper}",
            gates.join(" ")
        ));
        let entry_helper = self.next_helper_path(owner);
        self.functions.insert(entry_helper.clone(), entry_commands);
        commands.push(format!(
            "execute{prefix} run function {namespace}:{entry_helper}"
        ));
    }

    /// `store result|success` 子句：计分板直接拼进 store 链；投掷者目标先落到
    /// 临时计分项，再用一条 `on origin` 命令复制；Boss 栏直接拼进链。
    fn plan_store(&mut self, plan: &mut StorePlan, operation: &str, target: &ExecuteStoreTarget) {
        match target {
            ExecuteStoreTarget::Score(score) => match &score.holder {
                Holder::Origin => {
                    let fired = self.store_fired(plan);
                    let objective = self.objective.clone();
                    let temp = self.temporary();
                    plan.clauses
                        .push(format!("store {operation} score {temp} {objective}"));
                    let user = user_objective_name(&self.program.namespace, &score.objective);
                    plan.followups.push(format!(
                        "execute if score {fired} {objective} matches 0..1 on origin run scoreboard players operation @s {user} = {temp} {objective}"
                    ));
                }
                _ => plan.clauses.push(self.store_score_clause(operation, score)),
            },
            ExecuteStoreTarget::BossBar { id, field, .. } => plan
                .clauses
                .push(format!("store {operation} bossbar {id} {}", field.as_str())),
        }
    }

    /// `store.data` 子句：投掷者来源与计分板同理，先捕获到临时项再复制。
    fn plan_store_data(&mut self, plan: &mut StorePlan, data: &ExecuteStoreData) {
        if let NbtComponentSource::Entity(Holder::Origin) = &data.source {
            let fired = self.store_fired(plan);
            let objective = self.objective.clone();
            let temp = self.temporary();
            plan.clauses.push(format!(
                "store {} score {temp} {objective}",
                data.mode.as_str()
            ));
            plan.followups.push(format!(
                "execute if score {fired} {objective} matches 0..1 on origin store result entity @s {} {} {} run scoreboard players get {temp} {objective}",
                data.path,
                data.kind.as_str(),
                data.scale.as_deref().unwrap_or("1")
            ));
            return;
        }
        plan.clauses.push(self.store_data_clause(data));
    }

    /// origin 目标共享的「回调已触发」标志：预置 -1，回调触发后是 0/1。
    ///
    /// 没有它就无法区分「命令成功但结果为 0」与「命令根本没执行」，
    /// 后者按原版语义不应该写入目标。
    fn store_fired(&mut self, plan: &mut StorePlan) -> String {
        if let Some(fired) = &plan.fired {
            return fired.clone();
        }
        let objective = self.objective.clone();
        let fired = self.temporary();
        plan.presets
            .push(format!("scoreboard players set {fired} {objective} -1"));
        plan.clauses
            .push(format!("store success score {fired} {objective}"));
        plan.fired = Some(fired.clone());
        fired
    }

    /// `store result|success score <持有者> <目标>` 子句文本。
    fn store_score_clause(&self, operation: &str, target: &ScoreTarget) -> String {
        let objective = user_objective_name(&self.program.namespace, &target.objective);
        let holder = match &target.holder {
            Holder::SelfEntity => "@s".to_owned(),
            Holder::Query(name, _) => entity_query_selector(self.query(name)),
            Holder::Origin => unreachable!("origin 走 plan_store 的临时项路径"),
        };
        format!("store {operation} score {holder} {objective}")
    }

    /// `store result|success <来源> <路径> <类型> <缩放>` 子句文本。
    fn store_data_clause(&self, data: &ExecuteStoreData) -> String {
        format!(
            "store {} {} {} {} {}",
            data.mode.as_str(),
            self.nbt_source_text(&data.source),
            data.path,
            data.kind.as_str(),
            data.scale.as_deref().unwrap_or("1")
        )
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
    fn compile_while(
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
    fn compile_for(
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
    fn push_loop(&mut self) -> String {
        let state = self.next_loop_holder("state");
        self.loops.push(super::LoopContext {
            state: state.clone(),
        });
        state
    }

    fn pop_loop(&mut self) {
        self.loops.pop();
    }

    fn next_loop_holder(&mut self, kind: &str) -> String {
        let index = self.loop_counter;
        self.loop_counter += 1;
        format!("#loop_{kind}_{index}")
    }

    /// 当前循环状态计分项；语义检查保证 `break`/`continue` 只在循环体内。
    fn loop_state(&self) -> String {
        self.loops
            .last()
            .expect("语义检查保证 break/continue 只出现在循环体内")
            .state
            .clone()
    }

    fn break_check(&self, state: &str) -> String {
        format!(
            "execute if score {state} {} matches 2 run return 0",
            self.objective
        )
    }

    fn continue_reset(&self, state: &str) -> String {
        format!(
            "execute if score {state} {} matches 1 run scoreboard players set {state} {} 0",
            self.objective, self.objective
        )
    }

    /// `execute if score <循环变量> <目标> matches ..<上限-1>` 或 `... < <上限>`。
    fn range_check(&self, variable: &str, limit: &LoopLimit) -> String {
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

    /// 把表达式求值结果写入目标计分项。
    pub(super) fn store_value(&self, target: &str, value: Value, commands: &mut Vec<String>) {
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

/// `for` 循环的上限：常量直接用 `matches ..N` 比较，否则用计分项。
enum LoopLimit {
    Constant(i32),
    Holder(String),
}

/// 编译期能确定的整数表达式的值。
fn constant_integer(expression: &Expr) -> Option<i32> {
    match &expression.kind {
        ExprKind::Integer(value) => Some(*value),
        ExprKind::Negate(value) => constant_integer(value)?.checked_neg(),
        _ => None,
    }
}

/// 编译期能确定真假的布尔条件；三值逻辑，`None` 表示要在运行期求值。
fn constant_condition(condition: &Condition) -> Option<bool> {
    match condition {
        Condition::Compare {
            left,
            comparison,
            right,
        } => {
            let left = constant_integer(left)?;
            let right = constant_integer(right)?;
            Some(match comparison {
                Comparison::Equal => left == right,
                Comparison::NotEqual => left != right,
                Comparison::Less => left < right,
                Comparison::LessEqual => left <= right,
                Comparison::Greater => left > right,
                Comparison::GreaterEqual => left >= right,
            })
        }
        Condition::Not(inner) => Some(!constant_condition(inner)?),
        Condition::And(left, right) => {
            match (constant_condition(left), constant_condition(right)) {
                (Some(false), _) | (_, Some(false)) => Some(false),
                (Some(true), Some(true)) => Some(true),
                _ => None,
            }
        }
        Condition::Or(left, right) => match (constant_condition(left), constant_condition(right)) {
            (Some(true), _) | (_, Some(true)) => Some(true),
            (Some(false), Some(false)) => Some(false),
            _ => None,
        },
        _ => None,
    }
}

/// 语句（含嵌套块）里是否可能出现 `break`/`continue`。
///
/// 保守判断：`if`/`execute`/内层循环里出现跳转时也返回真。多插入的状态检查
/// 在状态为 0 时是空操作，因此不会改变行为，只多一条命令。
fn contains_flow_jump(statement: &Statement) -> bool {
    match &statement.kind {
        StatementKind::Break | StatementKind::Continue => true,
        StatementKind::If {
            then_body,
            else_body,
            ..
        } => then_body.iter().any(contains_flow_jump) || else_body.iter().any(contains_flow_jump),
        StatementKind::Execute { body, .. }
        | StatementKind::While { body, .. }
        | StatementKind::For { body, .. } => body.iter().any(contains_flow_jump),
        _ => false,
    }
}

/// 结构化 `execute` 的 store 下降计划。
///
/// 计分板与 Boss 栏目标可以直接拼进 `execute store ...` 链；投掷者目标需要
/// 临时计分项：预置、捕获、复制三段分开，复制用 `on origin` 在块体上下文里
/// 找到当前实体的投掷者。
#[derive(Default)]
struct StorePlan {
    /// 拼进 `execute <子句> run <最后一条命令>` 的 store 子句。
    clauses: Vec<String>,
    /// 捕获前写入辅助函数的预置命令。
    presets: Vec<String>,
    /// 捕获后写入辅助函数的复制命令。
    followups: Vec<String>,
    /// origin 目标共享的「回调已触发」标志。
    fired: Option<String>,
}
