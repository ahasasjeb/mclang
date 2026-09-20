//! 版本数据生成：从仓库内的 Minecraft 源码提取可复现快照。
//!
//! 生成逻辑只做保守的文本提取：识别稳定出现的注册调用与数据包目录，
//! 输出排序后的 JSON，并把全部输入的 FNV-1a 摘要写入 `digest` 字段，
//! 便于 `cargo xtask check-version-data` 校验随附快照与源码一致。
//!
//! 产物位于 `data/version/<版本>/`：
//!
//! - `registries.json`：各注册表的完整 id 集合、标签注册表清单、资源类型清单；
//! - `enums.json`：命令参数使用的枚举值（游戏模式、难度、显示槽位等）；
//! - `entity_nbt.json`：实体 NBT 标签表（由 [`super::entity_nbt`] 生成）。

use std::fs;
use std::path::{Path, PathBuf};

mod block_states;
mod commands;
mod enums;
mod extract;
mod registries;
mod text;
mod triggers;

pub use block_states::generate_block_states;
pub use commands::generate_commands;
pub use enums::generate_enums;
pub use registries::generate_registries;
pub use triggers::generate_triggers;

/// 仓库内 Minecraft 源码目录名。
pub const SOURCE_DIR: &str = "minecraft_client_26.3-rc-2";
/// 快照对应的版本标识。
pub const VERSION: &str = "26.3-rc-2";

/// 一次生成的产物：文件名与内容。
pub struct Output {
    pub name: &'static str,
    pub contents: String,
}

/// 生成全部版本数据，返回按文件名排序的产物。
pub fn generate_all(repo_root: &Path) -> Result<Vec<Output>, String> {
    let source = repo_root.join(SOURCE_DIR);
    if !source.is_dir() {
        return Err(format!("找不到 Minecraft 源码目录 {}", source.display()));
    }
    let mut outputs = vec![
        Output {
            name: "registries.json",
            contents: generate_registries(&source)?,
        },
        Output {
            name: "enums.json",
            contents: generate_enums(&source)?,
        },
        Output {
            name: "commands.json",
            contents: generate_commands(&source)?,
        },
        Output {
            name: "block_states.json",
            contents: generate_block_states(&source)?,
        },
        Output {
            name: "entity_nbt.json",
            contents: super::entity_nbt::generate(&source)?,
        },
        Output {
            name: "advancement_triggers.json",
            contents: generate_triggers(&source)?,
        },
    ];
    outputs.sort_by_key(|output| output.name);
    Ok(outputs)
}

/// `generate-version-data` 子命令：重新生成快照并写盘。
pub fn generate_command(repo_root: &Path) -> Result<Vec<PathBuf>, String> {
    let directory = snapshot_directory(repo_root);
    fs::create_dir_all(&directory)
        .map_err(|error| format!("无法创建 {}：{error}", directory.display()))?;
    let mut written = Vec::new();
    for output in generate_all(repo_root)? {
        let path = directory.join(output.name);
        fs::write(&path, &output.contents)
            .map_err(|error| format!("无法写入 {}：{error}", path.display()))?;
        written.push(path);
    }
    Ok(written)
}

/// `check-version-data` 子命令：比对当前快照与源码，不写盘。
pub fn check_command(repo_root: &Path) -> Result<(), String> {
    let directory = snapshot_directory(repo_root);
    let mut stale = Vec::new();
    for output in generate_all(repo_root)? {
        let path = directory.join(output.name);
        match fs::read_to_string(&path) {
            Ok(existing) if existing == output.contents => {}
            Ok(_) => stale.push(output.name),
            Err(_) => stale.push(output.name),
        }
    }
    if stale.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "以下快照与源码不一致，请运行 `cargo xtask generate-version-data`：{}",
            stale.join("、")
        ))
    }
}

/// 快照目录 `data/version/<版本>`。
pub fn snapshot_directory(repo_root: &Path) -> PathBuf {
    repo_root.join("data/version").join(VERSION)
}
#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::commands::parse_command_file;
    use super::text::{call_args, constant_string_pairs, string_args_in_calls};

    #[test]
    fn call_arguments_handle_nesting_and_strings() {
        let text = r#"register("a", foo("b"), register("c"));"#;
        let args = call_args(text, "register");
        assert!(args.contains(&r#""a", foo("b"), register("c")"#));
        // 嵌套的同名调用也会被找到，提取集合会自然去重。
        assert!(args.contains(&r#""c""#));
        assert_eq!(
            string_args_in_calls(text, "register"),
            BTreeSet::from(["a".to_string(), "c".to_string()])
        );
    }

    #[test]
    fn constant_pairs_read_declaration_lines() {
        let text = "public static final ResourceKey<Potion> WATER = create(\"water\");\n";
        let pairs = constant_string_pairs(text);
        assert_eq!(pairs.get("WATER").map(String::as_str), Some("water"));
    }

    #[test]
    fn parses_command_builder_casts() {
        let text = r#"
    public static void register(final CommandDispatcher<CommandSourceStack> dispatcher) {
        dispatcher.register((LiteralArgumentBuilder) ((LiteralArgumentBuilder) ((LiteralArgumentBuilder) Commands.literal("advancement").requires(Commands.hasPermission(Commands.LEVEL_GAMEMASTERS))).then(Commands.literal("grant").then(Commands.argument("targets", EntityArgument.players())))));
    }
"#;
        let roots = parse_command_file(text);
        assert_eq!(roots.len(), 1, "应当解析出一个根命令：{roots:?}");
        assert_eq!(roots[0].name, "advancement");
        assert_eq!(roots[0].level, Some(2));
        assert_eq!(roots[0].children[0].name, "grant");
        assert_eq!(roots[0].children[0].children[0].name, "targets");
        assert_eq!(
            roots[0].children[0].children[0].argument.as_deref(),
            Some("EntityArgument.players()")
        );
    }
}
