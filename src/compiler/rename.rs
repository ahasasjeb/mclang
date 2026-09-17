//! 用户标识符的内部化：把非 ASCII 名字换成确定性的随机 ASCII 别名。
//!
//! 语义检查通过后、代码生成之前运行。用户写中文（或其他 Unicode）标识符，
//! 数据包里只出现 ASCII：
//!
//! - 别名由名字的稳定哈希驱动，同一份源码总是得到同一份产物；
//! - 每个别名在整程序内唯一，并避开所有保持原样的 ASCII 标识符；
//! - 名字的引用与声明走同一张映射表，改名后符号关系不变。
//!
//! 诊断仍在 `validate` 阶段用用户写的名字产生，因此错误信息不受影响。

use std::collections::{HashMap, HashSet};

use crate::ast::*;

use super::stable_hash;

/// 别名的字符数：8 个随机小写字母。
const ALIAS_LENGTH: usize = 8;

/// 把整程序中的非 ASCII 标识符替换为随机 ASCII 别名。
pub(super) fn rename_program(program: &mut Program) {
    let aliases = build_aliases(program);
    if aliases.is_empty() {
        return;
    }
    program.for_each_name_mut(&mut |_, _, _, name| {
        if let Some(alias) = aliases.get(name) {
            *name = alias.clone();
        }
    });
}

/// 为需要内部化的名字分配别名，返回 `原名 -> 别名`。
fn build_aliases(program: &mut Program) -> HashMap<String, String> {
    // 保持原样的 ASCII 标识符与内部固定名不允许被别名占用。
    let mut occupied = HashSet::new();
    program.for_each_name_mut(&mut |_, _, _, name| {
        if name.is_ascii() {
            occupied.insert(name.clone());
        }
    });
    occupied.insert("load".to_owned());
    occupied.insert("tick".to_owned());
    occupied.insert("__mcl".to_owned());

    let mut aliases = HashMap::new();
    program.for_each_name_mut(&mut |_, _, _, name| {
        if name.is_ascii() || aliases.contains_key(name.as_str()) {
            return;
        }
        let alias = random_alias(name, &mut occupied);
        aliases.insert(name.clone(), alias);
    });
    aliases
}

/// 从名字的稳定哈希生成随机小写字母别名；撞名时继续取下一批。
fn random_alias(name: &str, occupied: &mut HashSet<String>) -> String {
    let mut state = stable_hash(name);
    loop {
        let mut alias = String::with_capacity(ALIAS_LENGTH);
        for _ in 0..ALIAS_LENGTH {
            state = splitmix64(state);
            alias.push((b'a' + (state % 26) as u8) as char);
        }
        if occupied.insert(alias.clone()) {
            return alias;
        }
    }
}

/// splitmix64：从 64 位种子继续产生高质量的伪随机序列。
fn splitmix64(seed: u64) -> u64 {
    let seed = seed.wrapping_add(0x9e3779b97f4a7c15);
    let mut value = seed;
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    value ^ (value >> 31)
}
