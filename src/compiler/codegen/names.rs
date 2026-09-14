//! 生成名称：计分板假玩家、objective 和参数/局部变量的内部槽位。
//!
//! 用户可见名称与内部槽位通过稳定哈希一一对应，避免与真实玩家名冲突：
//!
//! - `#v_<name>`：全局计分变量；
//! - `#p_<hash>`：函数参数；
//! - `#l_<hash>`：词法局部变量；
//! - `#t<number>`：表达式临时值；
//! - `mcl_<hash>`：每个命名空间唯一的 objective。

use crate::ast::{Statement, StatementKind};

use super::Compiler;

impl Compiler<'_> {
    /// 变量名解析：优先函数参数，其次词法局部变量，最后全局计分变量。
    pub(super) fn variable_holder(&self, owner: &str, name: &str) -> String {
        let is_parameter = self
            .program
            .functions
            .iter()
            .find(|function| function.name == owner)
            .is_some_and(|function| {
                function
                    .parameters
                    .iter()
                    .any(|parameter| parameter.name == name)
            });
        if is_parameter {
            parameter_holder(owner, name)
        } else if self
            .program
            .functions
            .iter()
            .find(|function| function.name == owner)
            .is_some_and(|function| function_has_local(&function.body, name))
        {
            local_holder(owner, name)
        } else {
            score_holder(name)
        }
    }

    /// 分配一个表达式临时计分项，允许被后续表达式覆盖。
    pub(super) fn temporary(&mut self) -> String {
        let temporary = format!("#t{}", self.temporary_counter);
        self.temporary_counter += 1;
        temporary
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

fn function_has_local(statements: &[Statement], name: &str) -> bool {
    statements.iter().any(|statement| match &statement.kind {
        StatementKind::Let { name: local, .. } => local == name,
        StatementKind::If {
            then_body,
            else_body,
            ..
        } => function_has_local(then_body, name) || function_has_local(else_body, name),
        StatementKind::Execute { body, .. }
        | StatementKind::Each { body, .. }
        | StatementKind::InDimension { body, .. }
        | StatementKind::Spawn { body, .. }
        | StatementKind::While { body, .. } => function_has_local(body, name),
        StatementKind::Run(_)
        | StatementKind::Give { .. }
        | StatementKind::SelfAction(_)
        | StatementKind::Message { .. }
        | StatementKind::PlaySound { .. }
        | StatementKind::Call { .. }
        | StatementKind::Schedule { .. }
        | StatementKind::Assign { .. }
        | StatementKind::Return(_) => false,
    })
}

pub(super) fn objective_name(namespace: &str) -> String {
    format!("mcl_{:012x}", stable_hash(namespace) & 0xffffffffffff)
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
