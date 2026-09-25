use crate::ast::*;

use super::helpers::constant_condition;
use crate::compiler::codegen::emit::{entity_query_as_clause, entity_query_selector};
use crate::compiler::codegen::names::user_objective_name;
use crate::compiler::codegen::world;
use crate::compiler::codegen::{Compiler, Value};

impl Compiler<'_> {
    pub(super) fn compile_execute(
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
        let mut may_fork = false;
        let mut conditions: Vec<(&Condition, bool)> = Vec::new();
        let mut plan = StorePlan::default();
        for clause in clauses {
            match &clause.kind {
                ExecuteClauseKind::As { query, .. } => {
                    may_fork |= self.query(query).limit != Some(1);
                    modifiers.push(entity_query_as_clause(self.query(query)));
                }
                ExecuteClauseKind::At { query, .. } => {
                    may_fork |= self.query(query).limit != Some(1);
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
                    may_fork |= *relation == EntityRelation::Passengers;
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

        let needs_per_source_condition_boundary = may_fork && !conditions.is_empty();
        let body_command = self.compile_execute_body(body, owner, plan);
        let mut continuation = self.compile_execute_conditions(conditions, body_command, owner);
        if needs_per_source_condition_boundary {
            // Native execute prepares conditions for every fork before running
            // any body. Evaluate conditions and the body per source instead.
            let helper = self.next_helper_path(owner);
            self.functions.insert(helper.clone(), vec![continuation]);
            continuation = format!("function {}:{helper}", self.program.namespace);
        }
        if modifiers.is_empty() {
            commands.push(continuation);
        } else {
            commands.push(format!(
                "execute {} run {continuation}",
                modifiers.join(" ")
            ));
        }
    }

    fn compile_execute_body(&mut self, body: &[Statement], owner: &str, plan: StorePlan) -> String {
        // 块体：store 把 `execute store ... run` 套在块内最后一条命令上，
        // 于是捕获的是该命令在修饰符上下文里的结果。
        if plan.clauses.is_empty() && self.preserve_command_result {
            let helper = self.compile_helper(body, owner);
            format!("function {}:{helper}", self.program.namespace)
        } else if plan.clauses.is_empty() {
            self.compile_small_block(body, owner)
        } else {
            let (last, prefix) = body.split_last().expect("validated nonempty stored block");
            let mut body_commands = self.compile_block(prefix, owner);
            let previous = std::mem::replace(&mut self.preserve_command_result, true);
            body_commands.extend(self.compile_block(std::slice::from_ref(last), owner));
            self.preserve_command_result = previous;
            let can_inline_store = !self.preserve_command_result
                && body.len() == 1
                && !matches!(
                    body[0].kind,
                    StatementKind::Return(_) | StatementKind::Run(_)
                )
                && body_commands.len() == 1
                && !body_commands[0].starts_with("return ")
                && !body_commands[0].contains(" run return ")
                && plan.presets.is_empty()
                && plan.followups.is_empty();
            if can_inline_store {
                let command = body_commands.pop().expect("single stored command");
                format!("execute {} run {command}", plan.clauses.join(" "))
            } else {
                let last = body_commands
                    .pop()
                    .expect("semantic validation guarantees the stored block is not empty");
                body_commands.extend(plan.presets);
                body_commands.push(format!("execute {} run {last}", plan.clauses.join(" ")));
                body_commands.extend(plan.followups);
                let helper = self.next_helper_path(owner);
                self.functions.insert(helper.clone(), body_commands);
                format!("function {}:{helper}", self.program.namespace)
            }
        }
    }

    fn compile_execute_conditions(
        &mut self,
        conditions: Vec<(&Condition, bool)>,
        body_command: String,
        owner: &str,
    ) -> String {
        // Build the chain from the end. A dynamic condition gets its own
        // continuation helper; that helper is entered only after all earlier
        // clauses have passed for the current Minecraft command source.
        let mut next = body_command;
        let mut direct = Vec::new();
        for (condition, negated) in conditions.into_iter().rev() {
            if let Some(value) = constant_condition(condition)
                && value != negated
            {
                continue;
            }
            if let Some(clause) = self.direct_condition_clause(condition, negated, owner) {
                direct.push(clause);
                continue;
            }
            let mut entry_commands = Vec::new();
            let flag = self.compile_condition(condition, owner, &mut entry_commands);
            let test = if negated { "unless" } else { "if" };
            direct.reverse();
            let suffix = if direct.is_empty() {
                String::new()
            } else {
                format!(" {}", direct.join(" "))
            };
            direct.clear();
            entry_commands.push(format!(
                "execute {test} score {flag} {} matches 1{suffix} run {next}",
                self.objective
            ));
            let helper = self.next_helper_path(owner);
            self.functions.insert(helper.clone(), entry_commands);
            next = format!("function {}:{helper}", self.program.namespace);
        }
        direct.reverse();
        if direct.is_empty() {
            next
        } else {
            format!("execute {} run {next}", direct.join(" "))
        }
    }

    /// `store result|success` 子句：计分板直接拼进 store 链；投掷者目标先落到
    /// 临时计分项，再用一条 `on origin` 命令复制；Boss 栏直接拼进链。
    pub(super) fn plan_store(
        &mut self,
        plan: &mut StorePlan,
        operation: &str,
        target: &ExecuteStoreTarget,
    ) {
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
    pub(super) fn plan_store_data(&mut self, plan: &mut StorePlan, data: &ExecuteStoreData) {
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
    pub(super) fn store_fired(&mut self, plan: &mut StorePlan) -> String {
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
    pub(super) fn store_score_clause(&self, operation: &str, target: &ScoreTarget) -> String {
        let objective = user_objective_name(&self.program.namespace, &target.objective);
        let holder = match &target.holder {
            Holder::SelfEntity => "@s".to_owned(),
            Holder::Query(name, _) => entity_query_selector(self.query(name)),
            Holder::Origin => unreachable!("origin 走 plan_store 的临时项路径"),
        };
        format!("store {operation} score {holder} {objective}")
    }

    /// `store result|success <来源> <路径> <类型> <缩放>` 子句文本。
    pub(super) fn store_data_clause(&self, data: &ExecuteStoreData) -> String {
        format!(
            "store {} {} {} {} {}",
            data.mode.as_str(),
            self.nbt_source_text(&data.source),
            data.path,
            data.kind.as_str(),
            data.scale.as_deref().unwrap_or("1")
        )
    }
    /// 把表达式求值结果写入目标计分项。
    pub(in crate::compiler::codegen) fn store_value(
        &self,
        target: &str,
        value: Value,
        commands: &mut Vec<String>,
    ) {
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
#[derive(Default)]
pub(super) struct StorePlan {
    /// 拼进 `execute <子句> run <最后一条命令>` 的 store 子句。
    clauses: Vec<String>,
    /// 捕获前写入辅助函数的预置命令。
    presets: Vec<String>,
    /// 捕获后写入辅助函数的复制命令。
    followups: Vec<String>,
    /// origin 目标共享的「回调已触发」标志。
    fired: Option<String>,
}
