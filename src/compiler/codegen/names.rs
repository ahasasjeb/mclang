//! 生成名称：计分板假玩家、objective 和参数/局部变量的内部槽位。
//!
//! 用户可见名称与内部槽位通过稳定哈希一一对应，避免与真实玩家名冲突：
//!
//! - `#v_<name>`：全局计分变量；
//! - `#p_<hash>`：函数参数；
//! - `#l_<hash>`：词法局部变量；
//! - `#t<number>`：表达式临时值；
//! - `mcl_<hash>`：每个命名空间唯一的 objective。
//!
//! 编译器构造时一次性建立 `<函数, 名称> -> 假玩家` 表，代码生成不再反复扫描
//! 函数体判断某个名字是参数、局部变量还是全局变量。

use std::collections::HashMap;

use crate::ast::{Program, Statement, StatementKind};

use super::Compiler;

impl Compiler<'_> {
    /// 变量名解析：优先函数参数，其次词法局部变量，最后全局计分变量。
    pub(super) fn variable_holder(&self, owner: &str, name: &str) -> String {
        self.holders
            .get(&(owner, name))
            .cloned()
            .unwrap_or_else(|| score_holder(name))
    }

    /// 分配一个表达式临时计分项，允许被后续表达式覆盖。
    pub(super) fn temporary(&mut self) -> String {
        let temporary = format!("#t{}", self.temporary_counter);
        self.temporary_counter += 1;
        temporary
    }
}

/// 收集整程序中每个函数的参数与局部变量，建立内部假玩家名称表。
pub(super) fn build_holders(program: &Program) -> HashMap<(&str, &str), String> {
    let mut holders = HashMap::new();
    for function in &program.functions {
        for parameter in &function.parameters {
            holders.insert(
                (function.name.as_str(), parameter.name.as_str()),
                parameter_holder(&function.name, &parameter.name),
            );
        }
        let mut locals = Vec::new();
        collect_local_names(&function.body, &mut locals);
        for name in locals {
            holders.insert(
                (function.name.as_str(), name),
                local_holder(&function.name, name),
            );
        }
    }
    holders
}

pub(super) fn collect_local_names<'a>(statements: &'a [Statement], locals: &mut Vec<&'a str>) {
    for statement in statements {
        match &statement.kind {
            StatementKind::Let { name, .. } => locals.push(name),
            StatementKind::If {
                then_body,
                else_body,
                ..
            } => {
                collect_local_names(then_body, locals);
                collect_local_names(else_body, locals);
            }
            StatementKind::Execute { body, .. }
            | StatementKind::Each { body, .. }
            | StatementKind::InDimension { body, .. }
            | StatementKind::Spawn { body, .. }
            | StatementKind::While { body, .. } => collect_local_names(body, locals),
            StatementKind::Run(_)
            | StatementKind::Give { .. }
            | StatementKind::EffectGive { .. }
            | StatementKind::EffectClear { .. }
            | StatementKind::XpChange { .. }
            | StatementKind::StopwatchAction { .. }
            | StatementKind::ClearInventory { .. }
            | StatementKind::SetBlock { .. }
            | StatementKind::Fill { .. }
            | StatementKind::FillBiome { .. }
            | StatementKind::Clone { .. }
            | StatementKind::PlaceFeature { .. }
            | StatementKind::PlaceJigsaw { .. }
            | StatementKind::PlaceStructure { .. }
            | StatementKind::PlaceTemplate { .. }
            | StatementKind::ForceLoad(_)
            | StatementKind::TimeAction { .. }
            | StatementKind::Weather { .. }
            | StatementKind::GameRuleSet { .. }
            | StatementKind::WorldBorder(_)
            | StatementKind::Locate { .. }
            | StatementKind::SelfAction(_)
            | StatementKind::Message { .. }
            | StatementKind::PlaySound { .. }
            | StatementKind::Call { .. }
            | StatementKind::Schedule { .. }
            | StatementKind::ScheduleClear { .. }
            | StatementKind::Assign { .. }
            | StatementKind::ScoreSet { .. }
            | StatementKind::ScoreReset { .. }
            | StatementKind::Teleport { .. }
            | StatementKind::NbtMerge { .. }
            | StatementKind::AdvancementAction { .. }
            | StatementKind::Return(_) => {}
        }
    }
}

pub(super) fn score_holder(name: &str) -> String {
    format!("#v_{name}")
}

pub(super) fn parameter_holder(function: &str, parameter: &str) -> String {
    format!(
        "#p_{:012x}",
        stable_hash(&format!("{function}:{parameter}")) & 0xffffffffffff
    )
}

pub(super) fn local_holder(function: &str, local: &str) -> String {
    format!(
        "#l_{:012x}",
        stable_hash(&format!("{function}:{local}")) & 0xffffffffffff
    )
}

pub(super) fn objective_name(namespace: &str) -> String {
    format!("mcl_{:012x}", stable_hash(namespace) & 0xffffffffffff)
}

/// 用户计分板目标的运行期名称：`<命名空间>_<名称>`。
///
/// 目标名在所有数据包之间共享，加上命名空间前缀避免与其他包冲突。
pub(super) fn user_objective_name(namespace: &str, name: &str) -> String {
    format!("{namespace}_{name}")
}

/// FNV-1a：跨平台稳定的 64 位哈希，保证生成名称可复现。
fn stable_hash(value: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in value.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
