//! 名字位置的统一遍历。
//!
//! 编译器里有两类按名字走遍整棵 AST 的工作：
//!
//! - [`crate::compiler::rename`]：把非 ASCII 名字换成随机 ASCII 别名；
//! - [`crate::modules`]：模块解析时把名字限定到所属模块。
//!
//! 两边必须覆盖完全相同的姓名位置，因此遍历只有这一份实现。回调同时收到
//! 名字的**位置**（声明还是引用）与**类别**（函数、计分变量、查询……），
//! 以及所在函数的参数/局部变量表，调用方据此决定改写方式。字符串字面量、
//! 资源位置与 NBT 键不是标识符，遍历不会触及。

use std::collections::HashSet;

use crate::ast::*;

/// 名字所属的符号类别。
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum NameRole {
    Function,
    /// 函数标签（`#标签`）。
    Tag,
    Score,
    Objective,
    Query,
    Item,
    Storage,
    DataSlot,
    /// JSON 资源（谓词、战利品表、配方、进度资源等）。
    Resource,
    /// 结构化进度声明。
    Advancement,
    /// 进度内的准则名；只在所属进度内可见。
    Criterion,
    Parameter,
    Local,
}

/// 名字出现在声明处还是引用处。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NameSite {
    Declaration,
    Reference,
}

/// 遍历回调的上下文：当前函数的参数与局部变量。
pub struct NameContext<'a> {
    /// 当前函数的参数与局部变量名。
    pub locals: &'a HashSet<String>,
}

impl NameContext<'_> {
    /// 名字是否是当前函数的参数或局部变量（全局计分变量会返回 `false`）。
    pub fn is_local(&self, name: &str) -> bool {
        self.locals.contains(name)
    }
}

impl Program {
    /// 按源码顺序遍历全部用户标识符位置，`visitor` 可以就地读写。
    pub fn for_each_name_mut(
        &mut self,
        visitor: &mut impl FnMut(&NameContext<'_>, NameSite, NameRole, &mut String),
    ) {
        let empty = HashSet::new();
        let top = NameContext { locals: &empty };

        for score in &mut self.scores {
            visitor(
                &top,
                NameSite::Declaration,
                NameRole::Score,
                &mut score.name,
            );
        }
        for objective in &mut self.objectives {
            visitor(
                &top,
                NameSite::Declaration,
                NameRole::Objective,
                &mut objective.name,
            );
            if let Some(display_name) = &mut objective.display_name {
                component_names(display_name, visitor, &top);
            }
            if let Some(NumberFormat::Fixed(component)) = &mut objective.number_format {
                component_names(component, visitor, &top);
            }
        }
        for query in &mut self.queries {
            visitor(
                &top,
                NameSite::Declaration,
                NameRole::Query,
                &mut query.name,
            );
        }
        for item in &mut self.item_stacks {
            visitor(&top, NameSite::Declaration, NameRole::Item, &mut item.name);
        }
        for storage in &mut self.storages {
            visitor(
                &top,
                NameSite::Declaration,
                NameRole::Storage,
                &mut storage.name,
            );
        }
        for slot in &mut self.data_slots {
            visitor(
                &top,
                NameSite::Declaration,
                NameRole::DataSlot,
                &mut slot.name,
            );
        }
        for resource in &mut self.resources {
            visitor(
                &top,
                NameSite::Declaration,
                NameRole::Resource,
                &mut resource.name,
            );
        }
        for advancement in &mut self.advancements {
            visitor(
                &top,
                NameSite::Declaration,
                NameRole::Advancement,
                &mut advancement.name,
            );
            if let Some(parent) = &mut advancement.parent {
                reference_names(parent, NameRole::Advancement, visitor, &top);
            }
            for criterion in &mut advancement.criteria {
                visitor(
                    &top,
                    NameSite::Declaration,
                    NameRole::Criterion,
                    &mut criterion.name,
                );
            }
            if let Some(reward) = &mut advancement.reward {
                if let Some(function) = &mut reward.function {
                    reference_names(function, NameRole::Function, visitor, &top);
                }
                for loot in &mut reward.loot {
                    reference_names(loot, NameRole::Resource, visitor, &top);
                }
                for recipe in &mut reward.recipes {
                    reference_names(recipe, NameRole::Resource, visitor, &top);
                }
            }
            if let Some(display) = &mut advancement.display {
                visitor(&top, NameSite::Reference, NameRole::Item, &mut display.icon);
            }
        }
        for tag in &mut self.function_tags {
            visitor(&top, NameSite::Declaration, NameRole::Tag, &mut tag.name);
            for entry in &mut tag.values {
                match entry {
                    FunctionTagEntry::Function(name, _) => {
                        visitor(&top, NameSite::Reference, NameRole::Function, name);
                    }
                    FunctionTagEntry::Tag(name, _) => {
                        visitor(&top, NameSite::Reference, NameRole::Tag, name);
                    }
                    FunctionTagEntry::External(_, _) => {}
                }
            }
        }
        for function in &mut self.functions {
            let Function {
                name,
                parameters,
                body,
                ..
            } = function;
            visitor(&top, NameSite::Declaration, NameRole::Function, name);
            for parameter in parameters.iter_mut() {
                visitor(
                    &top,
                    NameSite::Declaration,
                    NameRole::Parameter,
                    &mut parameter.name,
                );
            }
            let mut locals = collect_local_names_owned(body);
            // 参数也是函数局部名：模块解析不能把参数引用改写成导入的限定名。
            for parameter in parameters.iter() {
                locals.insert(parameter.name.clone());
            }
            let context = NameContext { locals: &locals };
            statement_names(body, visitor, &context);
        }
    }
}

mod expressions;
mod statements;

use expressions::component_names;
use statements::{reference_names, statement_names};

pub use statements::{collect_local_names, collect_local_names_owned};
