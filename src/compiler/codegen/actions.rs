//! 实体操作下降：`give` 语句、`self.item` 原样给予和全部 `self` 方法。
//!
//! 这些方法都只作用于 `@s`，因此集中在单独模块；控制流与辅助函数分配留在
//! [`super::statements`]。

use crate::ast::{GiveItem, GiveTarget, SelfAction};

use super::Compiler;
use super::emit::{entity_query_clause, item_stack_argument};

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

    pub(super) fn compile_self_action(&self, action: &SelfAction) -> Vec<String> {
        match action {
            SelfAction::AddTag(tag) => vec![format!("tag @s add {tag}")],
            SelfAction::RemoveTag(tag) => vec![format!("tag @s remove {tag}")],
            SelfAction::SetInvulnerable(value) => vec![format!(
                "data merge entity @s {{Invulnerable:{}b}}",
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
            SelfAction::Remove => vec!["kill @s".to_owned()],
        }
    }

    fn storage(&self, name: &str) -> &crate::ast::StorageDecl {
        self.program
            .storages
            .iter()
            .find(|candidate| candidate.name == name)
            .expect("semantic validation guarantees the item storage exists")
    }
}
