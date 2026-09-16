//! 实体操作下降：`give` 语句、`self.item` 原样给予、`effect`/`xp`/`clear`
//! 玩家与实体命令和全部 `self` 方法。
//!
//! 这些方法都只作用于 `@s`，因此集中在单独模块；控制流与辅助函数分配留在
//! [`super::statements`]。

use crate::ast::{
    DataSlotDecl, DataSlotKind, EffectDuration, Expr, GiveItem, GiveTarget, ScoreHolder,
    ScoreTarget, SelfAction, XpKind, XpOperation,
};

use super::Compiler;
use super::Value;
use super::emit::{entity_query_clause, entity_query_selector, item_stack_argument};
use super::names::user_objective_name;

impl Compiler<'_> {
    pub(super) fn compile_give(
        &mut self,
        target: &GiveTarget,
        item: &GiveItem,
        count: Option<u32>,
        owner: &str,
        commands: &mut Vec<String>,
    ) {
        match item {
            GiveItem::Definition(name) => {
                let item = self
                    .program
                    .item_stacks
                    .iter()
                    .find(|candidate| candidate.name == *name)
                    .expect("semantic validation guarantees the item definition exists");
                let argument = item_stack_argument(item);
                let count = count.unwrap_or(item.count);
                match target {
                    GiveTarget::Query(name) => {
                        let query = self
                            .program
                            .queries
                            .iter()
                            .find(|candidate| candidate.name == *name)
                            .expect("semantic validation guarantees the entity query exists");
                        commands.push(format!(
                            "execute {} run give @s {argument} {count}",
                            entity_query_clause(query),
                        ));
                    }
                    GiveTarget::Origin => commands.push(format!(
                        "execute on origin if entity @s[type=minecraft:player] run give @s {argument} {count}"
                    )),
                }
            }
            GiveItem::SelfItem => self.compile_self_item_give(target, owner, commands),
        }
    }

    /// `give(<目标>, self.item)`：把当前实体的物品堆原样交给玩家。
    ///
    /// 编译产物在 `data/<命名空间>/slot_source/__mcl/empty_slot.json` 声明
    /// “玩家背包中的空槽”，命令把源实体 slot 0（掉落物的 `Item`）复制进
    /// 第一个空槽。复制成功才清空源槽，掉落物随之下一次 tick 自行消失；
    /// 目标背包已满时不修改源实体。多个目标玩家时，只有第一个有空位的
    /// 玩家会收到物品。
    fn compile_self_item_give(
        &mut self,
        target: &GiveTarget,
        owner: &str,
        commands: &mut Vec<String>,
    ) {
        self.uses_empty_slot = true;
        let namespace = self.program.namespace.clone();
        let objective = self.objective.clone();
        let slots = format!("{namespace}:__mcl/empty_slot");
        let tag = format!("{objective}_give");
        let flag = self.temporary();
        let replace =
            format!("item replace entity @s {slots} from entity @e[tag={tag},limit=1] contents");
        commands.push(format!("tag @s add {tag}"));
        commands.push(format!("scoreboard players set {flag} {objective} 0"));
        match target {
            GiveTarget::Origin => commands.push(format!(
                "execute on origin if entity @s[type=minecraft:player] store success score {flag} {objective} run {replace}"
            )),
            GiveTarget::Query(name) => {
                let query = self
                    .program
                    .queries
                    .iter()
                    .find(|candidate| candidate.name == *name)
                    .expect("semantic validation guarantees the entity query exists");
                let helper = self.next_helper_path(owner);
                let helper_commands = vec![format!(
                    "execute if score {flag} {objective} matches 0 store success score {flag} {objective} run {replace}"
                )];
                self.functions.insert(helper.clone(), helper_commands);
                commands.push(format!(
                    "execute {} run function {namespace}:{helper}",
                    entity_query_clause(query),
                ));
            }
        }
        commands.push(format!(
            "execute if score {flag} {objective} matches 1 run item replace entity @s contents with minecraft:air"
        ));
        commands.push(format!("tag @s remove {tag}"));
    }

    pub(super) fn compile_self_action(&mut self, action: &SelfAction) -> Vec<String> {
        match action {
            SelfAction::AddTag(tag) => vec![format!("tag @s add {tag}")],
            SelfAction::RemoveTag(tag) => vec![format!("tag @s remove {tag}")],
            SelfAction::SetInvulnerable(value) => vec![format!(
                "data merge entity @s {{Invulnerable:{}b}}",
                if *value { 1 } else { 0 }
            )],
            SelfAction::SetNoGravity(value) => vec![format!(
                "data merge entity @s {{NoGravity:{}b}}",
                if *value { 1 } else { 0 }
            )],
            SelfAction::SaveItems(name) => {
                let storage = self.storage(name);
                vec![format!(
                    "data modify storage {} {} set from entity @s Items",
                    storage.storage_id, storage.path
                )]
            }
            SelfAction::RestoreItems(name) => {
                let storage = self.storage(name);
                vec![format!(
                    "data modify entity @s Items set from storage {} {}",
                    storage.storage_id, storage.path
                )]
            }
            SelfAction::RemovePreservingItems(name) => {
                let storage = self.storage(name);
                vec![
                    format!(
                        "data modify storage {} {} set from entity @s Items",
                        storage.storage_id, storage.path
                    ),
                    "data modify entity @s Items set value []".to_owned(),
                    "kill @s".to_owned(),
                ]
            }
            SelfAction::RemovePreservingSlot { slot, query, .. } => {
                // 与 remove_preserving_items 相同的安全性：只有物品确实写进数据槽
                // 才清空容器；容器一空就移除实体，因此 kill 不会再掉落任何物品。
                // 写入失败（目标查询没有命中）时容器保持原样，留待重试。
                let path = self.data_slot_path(slot);
                let selector = entity_query_selector(self.query(query));
                let flag = self.temporary();
                vec![
                    format!("scoreboard players set {flag} {} 0", self.objective),
                    format!(
                        "execute if data entity @s Items[0] store success score {flag} {} run data modify entity {selector} {path} append from entity @s Items",
                        self.objective
                    ),
                    format!(
                        "execute if score {flag} {} matches 1 run data modify entity @s Items set value []",
                        self.objective
                    ),
                    "execute unless data entity @s Items[0] run kill @s".to_owned(),
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
            SelfAction::DataStore { slot, query, .. } => {
                let path = self.data_slot_path(slot);
                let selector = entity_query_selector(self.query(query));
                // 追加而不是覆盖：目标槽里可能已经有别的容器寄存的物品。
                // 源容器为空时 `append` 会报「没有变化」，因此先检查第一个槽位。
                vec![format!(
                    "execute if data entity @s Items[0] run data modify entity {selector} {path} append from entity @s Items"
                )]
            }
            SelfAction::DataLoad { slot, query, .. } => {
                let path = self.data_slot_path(slot);
                let selector = entity_query_selector(self.query(query));
                vec![
                    format!(
                        "execute if data entity {selector} {path} run data modify entity @s Items set from entity {selector} {path}"
                    ),
                    format!(
                        "execute if data entity {selector} {path} run data remove entity {selector} {path}"
                    ),
                ]
            }
            SelfAction::DataClear { slot, .. } => {
                let path = self.data_slot_path(slot);
                vec![format!(
                    "execute if data entity @s {path} run data remove entity @s {path}"
                )]
            }
            SelfAction::Remove => vec!["kill @s".to_owned()],
        }
    }

    /// 数据槽在实体 NBT 中的绝对路径。
    ///
    /// 物品数据槽落在物品堆的 `minecraft:custom_data` 组件里；实体数据槽落在
    /// 26.3 `Entity` 的通用 `data` 字段里。两者都能容纳任意 NBT 并随世界保存。
    fn data_slot_path(&self, name: &str) -> String {
        let slot = self.data_slot(name);
        match slot.kind {
            DataSlotKind::EntityData => format!("data.{}", slot.key),
            DataSlotKind::ItemData => {
                format!("Item.components.\"minecraft:custom_data\".{}", slot.key)
            }
        }
    }

    /// `scoreboard.set(持有者, 目标, 值)`：把表达式写入用户计分板。
    pub(super) fn compile_score_set(
        &mut self,
        target: &ScoreTarget,
        value: &Expr,
        owner: &str,
        commands: &mut Vec<String>,
    ) {
        let objective = user_objective_name(&self.program.namespace, &target.objective);
        let prefix = self.score_holder_prefix(&target.holder);
        match self.compile_expr(value, owner, commands) {
            Value::Integer(value) => commands.push(format!(
                "{prefix}scoreboard players set @s {objective} {value}"
            )),
            Value::Score(source) => commands.push(format!(
                "{prefix}scoreboard players operation @s {objective} = {source} {}",
                self.objective
            )),
        }
    }

    /// `scoreboard.reset(持有者, 目标)`：删除计分项。
    pub(super) fn compile_score_reset(&self, target: &ScoreTarget, commands: &mut Vec<String>) {
        let objective = user_objective_name(&self.program.namespace, &target.objective);
        let prefix = self.score_holder_prefix(&target.holder);
        commands.push(format!("{prefix}scoreboard players reset @s {objective}"));
    }

    /// 计分操作的上下文前缀：`@s` 是当前实体、投掷者还是查询命中的实体。
    pub(super) fn score_holder_prefix(&self, holder: &ScoreHolder) -> String {
        match holder {
            ScoreHolder::SelfEntity => String::new(),
            ScoreHolder::Origin => "execute on origin run ".to_owned(),
            ScoreHolder::Query(name, _) => {
                format!("execute {} run ", entity_query_clause(self.query(name)))
            }
        }
    }

    /// `effect.give`/`effect.give_infinite`：持续时间为整秒或 `infinite`。
    ///
    /// 等级为 0 且不隐藏粒子时省略后两个可选参数，生成与手写命令一致的最短形状；
    /// 需要隐藏粒子时必须补上等级占位。
    pub(super) fn compile_effect_give(
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

    pub(super) fn compile_effect_clear(
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

    pub(super) fn compile_xp_change(
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

    pub(super) fn compile_clear_inventory(
        &mut self,
        target: &str,
        item: Option<&str>,
        max_count: Option<u32>,
        commands: &mut Vec<String>,
    ) {
        let query = self.query(target);
        let mut command = "clear @s".to_owned();
        if let Some(item) = item {
            command.push_str(&format!(" {item}"));
            if let Some(max_count) = max_count {
                command.push_str(&format!(" {max_count}"));
            }
        }
        commands.push(format!(
            "execute {} run {command}",
            entity_query_clause(query)
        ));
    }

    fn query(&self, name: &str) -> &crate::ast::EntityQueryDecl {
        self.program
            .queries
            .iter()
            .find(|candidate| candidate.name == name)
            .expect("semantic validation guarantees the entity query exists")
    }

    fn storage(&self, name: &str) -> &crate::ast::StorageDecl {
        self.program
            .storages
            .iter()
            .find(|candidate| candidate.name == name)
            .expect("semantic validation guarantees the item storage exists")
    }

    fn data_slot(&self, name: &str) -> &DataSlotDecl {
        self.program
            .data_slots
            .iter()
            .find(|candidate| candidate.name == name)
            .expect("semantic validation guarantees the data slot exists")
    }
}
