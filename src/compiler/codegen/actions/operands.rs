use super::*;

impl Compiler<'_> {
    /// 运算的来源操作数：`@s` 或显式选择器（`origin` 已在语义阶段拒绝）。
    pub(super) fn score_operand(&self, holder: &Holder) -> String {
        match holder {
            Holder::SelfEntity | Holder::Origin => "@s".to_owned(),
            Holder::Query(name, _) => entity_query_selector(self.query(name)),
        }
    }

    /// `teleport(持有者, 坐标或实体查询[, rotation(朝向)])`：`tp @s` 到坐标或单个实体。
    pub(in crate::compiler::codegen) fn compile_teleport(
        &self,
        targets: &Holder,
        destination: &TeleportDestination,
        rotation: Option<&crate::ast::Facing>,
        commands: &mut Vec<String>,
    ) {
        let prefix = self.score_holder_prefix(targets);
        let destination = match destination {
            TeleportDestination::Position(position) => world::position_value_text(position),
            TeleportDestination::Entity { query, .. } => entity_query_selector(self.query(query)),
        };
        let rotation = rotation
            .map(|rotation| format!(" {}", self.facing_text(rotation)))
            .unwrap_or_default();
        commands.push(format!("{prefix}tp @s {destination}{rotation}"));
    }

    /// 计分操作的上下文前缀：`@s` 是当前实体、投掷者还是查询命中的实体。
    pub(super) fn score_holder_prefix(&self, holder: &Holder) -> String {
        match holder {
            Holder::SelfEntity => String::new(),
            Holder::Origin => "execute on origin run ".to_owned(),
            Holder::Query(name, _) => {
                format!("execute {} run ", entity_query_clause(self.query(name)))
            }
        }
    }

    /// `advancement.grant/revoke(...)`：目标继承持有者前缀，进度引用补命名空间。
    pub(in crate::compiler::codegen) fn compile_advancement_action(
        &self,
        operation: AdvancementOperation,
        scope: AdvancementScope,
        targets: &Holder,
        advancement: Option<&AdvancementReference>,
        criterion: Option<&str>,
        commands: &mut Vec<String>,
    ) {
        let prefix = self.score_holder_prefix(targets);
        let mut command = format!("advancement {} @s {}", operation.as_str(), scope.as_str());
        if let Some(advancement) = advancement {
            command.push(' ');
            command.push_str(&reference_id(&self.program.namespace, advancement));
            if let Some(criterion) = criterion {
                command.push(' ');
                command.push_str(criterion);
            }
        }
        commands.push(format!("{prefix}{command}"));
    }

    /// `effect.give`/`effect.give_infinite`：持续时间为整秒或 `infinite`。
    ///
    /// 等级为 0 且不隐藏粒子时省略后两个可选参数，生成与手写命令一致的最短形状；
    /// 需要隐藏粒子时必须补上等级占位。
    pub(in crate::compiler::codegen) fn compile_effect_give(
        &mut self,
        target: &str,
        effect: &str,
        duration: EffectDuration,
        amplifier: Option<u32>,
        hide_particles: bool,
        commands: &mut Vec<String>,
    ) {
        let query = self.query(target);
        let mut command = format!("effect give @s {effect}");
        match duration {
            EffectDuration::Seconds(seconds) => command.push_str(&format!(" {seconds}")),
            EffectDuration::Infinite => command.push_str(" infinite"),
        }
        let amplifier = amplifier.unwrap_or(0);
        if amplifier > 0 || hide_particles {
            command.push_str(&format!(" {amplifier}"));
            if hide_particles {
                command.push_str(" true");
            }
        }
        commands.push(format!(
            "execute {} run {command}",
            entity_query_clause(query)
        ));
    }

    pub(in crate::compiler::codegen) fn compile_effect_clear(
        &mut self,
        target: &str,
        effect: Option<&str>,
        commands: &mut Vec<String>,
    ) {
        let query = self.query(target);
        let command = match effect {
            Some(effect) => format!("effect clear @s {effect}"),
            None => "effect clear @s".to_owned(),
        };
        commands.push(format!(
            "execute {} run {command}",
            entity_query_clause(query)
        ));
    }

    pub(in crate::compiler::codegen) fn compile_xp_change(
        &mut self,
        target: &str,
        kind: XpKind,
        operation: XpOperation,
        amount: i32,
        commands: &mut Vec<String>,
    ) {
        let query = self.query(target);
        let operation = match operation {
            XpOperation::Add => "add",
            XpOperation::Set => "set",
        };
        commands.push(format!(
            "execute {} run xp {operation} @s {amount} {}",
            entity_query_clause(query),
            kind.as_str()
        ));
    }

    pub(in crate::compiler::codegen) fn compile_clear_inventory(
        &mut self,
        target: &str,
        item: Option<&crate::ast::ItemPredicate>,
        max_count: Option<u32>,
        commands: &mut Vec<String>,
    ) {
        let query = self.query(target);
        let mut command = "clear @s".to_owned();
        if let Some(item) = item {
            command.push_str(&format!(
                " {}",
                super::super::emit::item_predicate_text(item)
            ));
            if let Some(max_count) = max_count {
                command.push_str(&format!(" {max_count}"));
            }
        }
        commands.push(format!(
            "execute {} run {command}",
            entity_query_clause(query)
        ));
    }

    pub(in crate::compiler::codegen) fn query(&self, name: &str) -> &crate::ast::EntityQueryDecl {
        self.program
            .queries
            .iter()
            .find(|candidate| candidate.name == name)
            .expect("semantic validation guarantees the entity query exists")
    }

    pub(super) fn storage(&self, name: &str) -> &crate::ast::StorageDecl {
        self.program
            .storages
            .iter()
            .find(|candidate| candidate.name == name)
            .expect("semantic validation guarantees the item storage exists")
    }

    pub(super) fn data_slot(&self, name: &str) -> &DataSlotDecl {
        self.program
            .data_slots
            .iter()
            .find(|candidate| candidate.name == name)
            .expect("semantic validation guarantees the data slot exists")
    }
}
