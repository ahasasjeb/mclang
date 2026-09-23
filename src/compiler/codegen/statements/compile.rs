use crate::ast::*;

use crate::compiler::codegen::Compiler;
use crate::compiler::codegen::emit::nbt_text;
use crate::compiler::codegen::names::user_objective_name;
use crate::compiler::codegen::world;

use super::helpers::contains_flow_jump;

impl Compiler<'_> {
    /// 下降一个语句块。辅助函数命名计数器按所属函数（`owner`）独立编号。
    ///
    /// 位于循环体内时，任何可能设置 break/continue 状态的语句之后都会插入一条
    /// 状态检查：`execute unless score <state> matches 0 run return 0`。它让
    /// `if` 辅助函数里设置的循环状态能立即终止当前循环体。
    pub(in crate::compiler::codegen) fn compile_block(
        &mut self,
        statements: &[Statement],
        owner: &str,
    ) -> Vec<String> {
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

    pub(super) fn compile_statement(
        &mut self,
        statement: &Statement,
        owner: &str,
        commands: &mut Vec<String>,
    ) {
        match &statement.kind {
            StatementKind::MacroCall { target, arguments } => {
                commands.push(self.macro_command(target, arguments, owner))
            }
            StatementKind::CoreCommand(command) => commands.push(self.core_command(command, owner)),
            StatementKind::EntityCommand(command) => {
                commands.push(self.entity_command(command, owner))
            }
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
            } => self.compile_clear_inventory(target, item.as_ref(), *max_count, commands),
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
            StatementKind::UiCommand(command) => {
                commands.push(self.ui_command(command, owner));
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
            StatementKind::ScheduleClear { target } => {
                commands.push(format!(
                    "schedule clear {}",
                    self.function_target_text(target)
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
            StatementKind::ScoreboardCommand(command) => {
                commands.push(self.scoreboard_command(command, owner));
            }
            StatementKind::Teleport {
                targets,
                destination,
                rotation,
            } => {
                self.compile_teleport(targets, destination, rotation.as_ref(), owner, commands);
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
                let holders = nbt_source_holders(target);
                let command = self.capture_command_targets(&holders, owner, |compiler| {
                    format!(
                        "data merge {} {}",
                        compiler.nbt_source_text(target),
                        nbt_text(nbt)
                    )
                });
                commands.push(command);
            }
            StatementKind::DataRemove {
                target,
                path,
                path_span: _,
            } => {
                let holders = nbt_source_holders(target);
                let command = self.capture_command_targets(&holders, owner, |compiler| {
                    format!("data remove {} {path}", compiler.nbt_source_text(target))
                });
                commands.push(command);
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
                let mut holders = nbt_source_holders(target);
                holders.extend(data_source_holders(&operation.source));
                let command = self.capture_command_targets(&holders, owner, |compiler| {
                    let mut command = format!(
                        "data modify {} {path} {kind}",
                        compiler.nbt_source_text(target)
                    );
                    if let Some(index) = operation.index {
                        command.push_str(&format!(" {index}"));
                    }
                    match &operation.source {
                        DataSource::From {
                            target,
                            path,
                            path_span: _,
                        } => command.push_str(&format!(
                            " from {} {path}",
                            compiler.nbt_source_text(target)
                        )),
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
                            command.push_str(&format!(
                                " string {} {path}",
                                compiler.nbt_source_text(target)
                            ));
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
                                compiler.compute_source_text(source),
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
                    command
                });
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
}

fn nbt_source_holders(source: &NbtComponentSource) -> Vec<&Holder> {
    match source {
        NbtComponentSource::Entity(holder) => vec![holder],
        NbtComponentSource::Block(_) | NbtComponentSource::Storage(_, _) => Vec::new(),
    }
}

fn data_source_holders(source: &DataSource) -> Vec<&Holder> {
    match source {
        DataSource::From { target, .. } | DataSource::String { target, .. } => {
            nbt_source_holders(target)
        }
        DataSource::Compute {
            source: ComputeSource::Entity(holder),
            ..
        } => vec![holder],
        DataSource::Value(_)
        | DataSource::Compute {
            source: ComputeSource::Default | ComputeSource::Block(_),
            ..
        } => Vec::new(),
    }
}
