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

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

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
            name: "entity_nbt.json",
            contents: super::entity_nbt::generate(&source)?,
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

/// 提取 `registries.json`。
pub fn generate_registries(root: &Path) -> Result<String, String> {
    let mut extractor = Extractor::new(root);
    let mut registries: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut slots = Slots::default();

    // 方块与物品：BlockItemIds 同时登记方块与物品，BlockIds 只登记没有
    // 对应物品的方块（水、移动的活塞等），ItemIds 登记其余物品。
    let block_item_ids = extractor.read("net/minecraft/references/BlockItemIds.java")?;
    for id in string_args_in_calls(&block_item_ids, "BlockItemId.create") {
        insert(&mut registries, "block", &id);
        insert(&mut registries, "item", &id);
    }
    for base in string_args_in_calls(&block_item_ids, "createSimpleColored") {
        for color in DYE_COLORS {
            let id = format!("{color}_{base}");
            insert(&mut registries, "block", &id);
            insert(&mut registries, "item", &id);
        }
    }
    for base in string_args_in_calls(&block_item_ids, "createSimpleCopper") {
        for prefix in COPPER_PREFIXES {
            let id = format!("{prefix}{base}");
            insert(&mut registries, "block", &id);
            insert(&mut registries, "item", &id);
        }
    }

    let block_ids = extractor.read("net/minecraft/references/BlockIds.java")?;
    for id in string_args_in_calls(&block_ids, "create") {
        insert(&mut registries, "block", &id);
    }

    let item_ids = extractor.read("net/minecraft/references/ItemIds.java")?;
    for id in string_args_in_calls(&item_ids, "create") {
        insert(&mut registries, "item", &id);
    }
    // 派生物品：刷怪蛋、音乐唱片、盔甲纹饰模板。
    let entity_constants =
        constant_string_pairs(&extractor.read("net/minecraft/world/entity/EntityTypeIds.java")?);
    for constant in ident_args_in_calls(&item_ids, "createSpawnEgg", "EntityTypeIds") {
        let Some(entity) = entity_constants.get(&constant) else {
            return Err(format!(
                "ItemIds.java 引用未知实体常量 EntityTypeIds.{constant}"
            ));
        };
        insert(&mut registries, "item", &format!("{entity}_spawn_egg"));
    }
    let song_constants =
        constant_string_pairs(&extractor.read("net/minecraft/world/item/JukeboxSongs.java")?);
    for constant in ident_args_in_calls(&item_ids, "createMusicDisc", "JukeboxSongs") {
        let Some(song) = song_constants.get(&constant) else {
            return Err(format!(
                "ItemIds.java 引用未知唱片常量 JukeboxSongs.{constant}"
            ));
        };
        insert(&mut registries, "item", &format!("music_disc_{song}"));
    }
    let trim_pattern_constants = constant_string_pairs(
        &extractor.read("net/minecraft/world/item/equipment/trim/TrimPatterns.java")?,
    );
    for constant in
        ident_args_in_calls(&item_ids, "createArmorTrimSmithingTemplate", "TrimPatterns")
    {
        let Some(pattern) = trim_pattern_constants.get(&constant) else {
            return Err(format!(
                "ItemIds.java 引用未知纹饰常量 TrimPatterns.{constant}"
            ));
        };
        insert(
            &mut registries,
            "item",
            &format!("{pattern}_armor_trim_smithing_template"),
        );
    }
    for base in string_args_in_calls(&item_ids, "createSimpleColored") {
        for color in DYE_COLORS {
            insert(&mut registries, "item", &format!("{color}_{base}"));
        }
    }

    // 代码注册的其它注册表。
    let code_sources: &[(&str, &str, &str)] = &[
        (
            "entity_type",
            "net/minecraft/world/entity/EntityTypeIds.java",
            "create",
        ),
        (
            "biome",
            "net/minecraft/world/level/biome/Biomes.java",
            "register",
        ),
        (
            "dimension",
            "net/minecraft/world/level/dimension/LevelStem.java",
            "withDefaultNamespace",
        ),
        (
            "damage_type",
            "net/minecraft/world/damagesource/DamageTypes.java",
            "withDefaultNamespace",
        ),
        (
            "mob_effect",
            "net/minecraft/world/effect/MobEffects.java",
            "register",
        ),
        (
            "enchantment",
            "net/minecraft/world/item/enchantment/Enchantments.java",
            "key",
        ),
        (
            "point_of_interest_type",
            "net/minecraft/world/entity/ai/village/poi/PoiTypes.java",
            "createKey",
        ),
        (
            "attribute",
            "net/minecraft/world/entity/ai/attributes/Attributes.java",
            "register",
        ),
        (
            "particle",
            "net/minecraft/core/particles/ParticleTypes.java",
            "register",
        ),
        ("sound", "net/minecraft/sounds/SoundEvents.java", "register"),
        (
            "sound",
            "net/minecraft/sounds/SoundEvents.java",
            "registerForHolder",
        ),
        (
            "potion",
            "net/minecraft/world/item/alchemy/PotionIds.java",
            "create",
        ),
        (
            "jukebox_song",
            "net/minecraft/world/item/JukeboxSongs.java",
            "create",
        ),
        (
            "painting_variant",
            "net/minecraft/world/entity/decoration/painting/PaintingVariants.java",
            "create",
        ),
        (
            "instrument",
            "net/minecraft/world/item/Instruments.java",
            "create",
        ),
        (
            "banner_pattern",
            "net/minecraft/world/level/block/entity/BannerPatterns.java",
            "create",
        ),
        (
            "decorated_pot_pattern",
            "net/minecraft/world/level/block/entity/DecoratedPotPatterns.java",
            "create",
        ),
        (
            "trim_material",
            "net/minecraft/world/item/equipment/trim/TrimMaterials.java",
            "registryKey",
        ),
        (
            "trim_pattern",
            "net/minecraft/world/item/equipment/trim/TrimPatterns.java",
            "registryKey",
        ),
        (
            "game_rule",
            "net/minecraft/world/level/gamerules/GameRules.java",
            "registerBoolean",
        ),
        (
            "game_rule",
            "net/minecraft/world/level/gamerules/GameRules.java",
            "registerInteger",
        ),
        (
            "criteria_trigger",
            "net/minecraft/advancements/triggers/CriteriaTriggers.java",
            "register",
        ),
    ];
    for (kind, relative, call) in code_sources {
        for id in extractor.read_ids(relative, call)? {
            insert(&mut registries, kind, &id);
        }
    }

    // 数据包目录：JSON 资源与结构文件。
    for (kind, path, extension) in data_kinds(root)? {
        let ids = scan_resource_ids(&mut extractor, &path, extension)?;
        let entry = registries.entry(kind).or_default();
        for id in ids {
            entry.insert(format!("minecraft:{id}"));
        }
    }

    // 槽位名来自 SlotRanges 的静态表（item 命令与槽位集合使用）。
    let slot_ranges = extractor.read("net/minecraft/world/inventory/SlotRanges.java")?;
    slots.single = string_args_in_calls(&slot_ranges, "addSingleSlot");
    slots.multi = string_args_in_calls(&slot_ranges, "addSlots");
    for inner in call_args(&slot_ranges, "addSlotRange") {
        if let (Some(prefix), Some(count)) = (first_string(inner), last_integer(inner)) {
            slots.ranges.insert(prefix, count);
        }
    }

    // 标签注册表清单：data/minecraft/tags 下含文件的目录路径。
    let tag_registries = scan_tag_registries(&mut extractor, root)?;

    // 资源类型清单：可以出现在 resource 声明里的类型。
    let mut resource_kinds: BTreeSet<String> = BTreeSet::new();
    for (kind, _, _) in data_kinds(root)? {
        if kind == "structure" {
            continue;
        }
        resource_kinds.insert(kind);
    }
    // 没有原版数据文件、但属于数据包 JSON 注册表的类型。
    for kind in ["dimension", "item_modifier"] {
        resource_kinds.insert(kind.to_string());
    }

    let mut root_object = Map::new();
    root_object.insert("source".into(), json!(SOURCE_DIR));
    root_object.insert("digest".into(), json!(extractor.finish()));
    root_object.insert(
        "registries".into(),
        serde_json::to_value(&registries).map_err(|error| error.to_string())?,
    );
    root_object.insert("slots".into(), slots.to_value());
    root_object.insert("tag_registries".into(), json!(tag_registries));
    root_object.insert("resource_kinds".into(), json!(resource_kinds));
    render_json(&Value::Object(root_object))
}

/// 提取 `enums.json`。
pub fn generate_enums(root: &Path) -> Result<String, String> {
    let mut extractor = Extractor::new(root);
    let mut enums: BTreeMap<String, Vec<String>> = BTreeMap::new();

    enums.insert(
        "gamemode".into(),
        enum_literals(
            &extractor.read("net/minecraft/world/level/GameType.java")?,
            "public enum GameType",
        ),
    );
    enums.insert(
        "difficulty".into(),
        enum_literals(
            &extractor.read("net/minecraft/world/Difficulty.java")?,
            "public enum Difficulty",
        ),
    );
    enums.insert(
        "display_slot".into(),
        enum_literals(
            &extractor.read("net/minecraft/world/scores/DisplaySlot.java")?,
            "public enum DisplaySlot",
        ),
    );
    enums.insert(
        "team_color".into(),
        enum_literals(
            &extractor.read("net/minecraft/world/scores/TeamColor.java")?,
            "public enum TeamColor",
        ),
    );
    enums.insert(
        "sound_source".into(),
        enum_literals(
            &extractor.read("net/minecraft/sounds/SoundSource.java")?,
            "public enum SoundSource",
        ),
    );
    enums.insert(
        "anchor".into(),
        enum_literals(
            &extractor.read("net/minecraft/commands/arguments/EntityAnchorArgument.java")?,
            "enum Anchor",
        ),
    );
    enums.insert("swizzle".into(), vec!["x".into(), "y".into(), "z".into()]);
    enums.insert(
        "heightmap".into(),
        enum_literals(
            &extractor.read("net/minecraft/world/level/levelgen/Heightmap.java")?,
            "enum Types",
        ),
    );

    // 时间单位取自 TimeArgument.UNITS 表，空串表示默认刻。
    let time = extractor.read("net/minecraft/commands/arguments/TimeArgument.java")?;
    let mut units = Vec::new();
    for pair in time.match_indices("UNITS.put(") {
        if let Some(inner) = matching_paren(&time, pair.0 + "UNITS.put".len())
            && let Some(unit) = first_string(inner)
            && !unit.is_empty()
        {
            units.push(unit);
        }
    }
    units.sort();
    enums.insert("time_unit".into(), units);

    let mut root_object = Map::new();
    root_object.insert("source".into(), json!(SOURCE_DIR));
    root_object.insert("digest".into(), json!(extractor.finish()));
    root_object.insert(
        "enums".into(),
        serde_json::to_value(&enums).map_err(|error| error.to_string())?,
    );
    render_json(&Value::Object(root_object))
}

/// 提取 `commands.json`：26.3 的 Brigadier 命令树。
///
/// 反编译源码里的注册表达式是 `dispatcher.register(...)` 上的一串
/// `Commands.literal("…")` / `Commands.argument("…", 类型)` 与 `.then(...)`、
/// `.requires(...)`、`.redirect(...)` 调用。生成器只解析这一子集，
/// 记录根命令、字面量子命令、参数类型与 `requires` 权限等级。
pub fn generate_commands(root: &Path) -> Result<String, String> {
    let mut extractor = Extractor::new(root);
    let mut commands: BTreeMap<String, Value> = BTreeMap::new();
    let mut root_count = 0usize;
    for directory in [
        "net/minecraft/server/commands",
        "net/minecraft/server/commands/data",
        "net/minecraft/server/commands/item",
        "net/minecraft/gametest/framework",
        "net/minecraft/commands",
    ] {
        let path = root.join(directory);
        if !path.is_dir() {
            continue;
        }
        for file in java_files(&path)? {
            let relative = file
                .strip_prefix(root)
                .map_err(|error| error.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            let text = extractor.read(&relative)?;
            for node in parse_command_file(&text) {
                root_count += 1;
                commands.insert(node.name.clone(), node.to_value());
            }
        }
    }
    if root_count == 0 {
        return Err("没有解析出任何命令注册，命令树提取逻辑需要更新".into());
    }

    let mut root_object = Map::new();
    root_object.insert("source".into(), json!(SOURCE_DIR));
    root_object.insert("digest".into(), json!(extractor.finish()));
    root_object.insert("root_count".into(), json!(root_count));
    root_object.insert(
        "commands".into(),
        Value::Object(commands.into_iter().collect()),
    );
    render_json(&Value::Object(root_object))
}

/// 命令树节点。
#[derive(Clone, Debug)]
struct CommandNode {
    name: String,
    /// 参数节点的类型表达式；字面量节点为 `None`。
    argument: Option<String>,
    /// `requires` 声明的最低权限等级（0–4）。
    level: Option<u8>,
    executable: bool,
    redirect: bool,
    children: Vec<CommandNode>,
}

impl CommandNode {
    fn to_value(&self) -> Value {
        let mut object = Map::new();
        object.insert("name".into(), json!(self.name));
        if let Some(argument) = &self.argument {
            object.insert("argument".into(), json!(argument));
        }
        if let Some(level) = self.level {
            object.insert("level".into(), json!(level));
        }
        if self.executable {
            object.insert("executable".into(), json!(true));
        }
        if self.redirect {
            object.insert("redirect".into(), json!(true));
        }
        if !self.children.is_empty() {
            object.insert(
                "children".into(),
                Value::Array(self.children.iter().map(CommandNode::to_value).collect()),
            );
        }
        Value::Object(object)
    }
}

/// 解析一个 Java 文件里所有 `dispatcher.register(...)` 调用。
fn parse_command_file(text: &str) -> Vec<CommandNode> {
    let variables = variable_literals(text);
    let mut roots = Vec::new();
    let mut search = 0;
    while let Some(index) = text[search..].find(".register(") {
        let dot = search + index;
        search = dot + 1;
        let receiver_start = text[..dot]
            .rfind(|character: char| {
                !(character.is_ascii_alphanumeric() || character == '_' || character == '.')
            })
            .map(|position| position + 1)
            .unwrap_or(0);
        let receiver = &text[receiver_start..dot];
        if !receiver.to_ascii_lowercase().ends_with("dispatcher") {
            continue;
        }
        let open = dot + ".register".len();
        let Some((inner, _)) = paren_range(text, open) else {
            continue;
        };
        if let Some((node, _)) = parse_node(inner, 0) {
            roots.push(node);
        } else if let Some(node) = fallback_node(inner, &variables) {
            // `dispatcher.register(variable)` 或 `register(helper(variable, ...))`：
            // 反查变量初始化里的字面量名，再从注册表达式补出子命令。
            roots.push(node);
        } else if let Some(node) = first_literal_node(inner) {
            // 注册表达式被辅助函数包住（`addTargets(Commands.literal("loot"), …)`）。
            roots.push(node);
        }
    }
    roots
}

/// 注册表达式里第一个 `Commands.literal(...)` 节点。
fn first_literal_node(argument: &str) -> Option<CommandNode> {
    let index = argument.find("Commands.literal(")?;
    parse_node(argument, index).map(|(node, _)| node)
}

/// 反编译代码常把根节点存进局部变量；这里收集 `变量 = Commands.literal("…")`。
///
/// 变量之间还会互相转手（`b = (LiteralArgumentBuilder) a.then(...)`），
/// 因此再做一轮传递闭包解析：`b` 引用已解析的 `a` 时沿用 `a` 的节点。
fn variable_literals(text: &str) -> BTreeMap<String, CommandNode> {
    let mut variables = BTreeMap::new();
    let mut assignments: BTreeMap<String, String> = BTreeMap::new();
    let mut search = 0;
    while let Some(index) = text[search..].find('=') {
        let equal = search + index;
        search = equal + 1;
        if text[equal..].starts_with("==") {
            continue;
        }
        let before = text[..equal].trim_end();
        if before
            .chars()
            .last()
            .is_some_and(|character| matches!(character, '!' | '<' | '>' | '='))
        {
            continue;
        }
        let name: Vec<char> = before
            .chars()
            .rev()
            .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
            .collect();
        let name: String = name.into_iter().rev().collect();
        if name.is_empty() {
            continue;
        }
        let rest = &text[equal + 1..];
        if let Some((node, _)) = parse_node(rest.trim_start(), 0) {
            variables.entry(name).or_insert(node);
            continue;
        }
        let end = rest.find([';', '\n']).unwrap_or(rest.len());
        assignments
            .entry(name)
            .or_insert_with(|| rest[..end].trim().to_string());
    }
    // 传递闭包：变量转手时沿用最先引用到的已解析变量。
    loop {
        let mut progressed = false;
        for (name, rhs) in &assignments {
            if variables.contains_key(name) {
                continue;
            }
            let best = variables
                .iter()
                .filter_map(|(candidate, node)| {
                    identifier_position(rhs, candidate).map(|position| (position, node))
                })
                .min_by_key(|(position, _)| *position);
            if let Some((_, node)) = best {
                let node = node.clone();
                variables.insert(name.clone(), node);
                progressed = true;
            }
        }
        if !progressed {
            break;
        }
    }
    variables
}

/// 变量注册时的兜底：用变量名查回字面量，再从注册表达式补子命令与权限。
fn fallback_node(argument: &str, variables: &BTreeMap<String, CommandNode>) -> Option<CommandNode> {
    let mut best: Option<(usize, CommandNode)> = None;
    for (name, node) in variables {
        if let Some(position) = identifier_position(argument, name)
            && best
                .as_ref()
                .is_none_or(|(best_position, _)| position < *best_position)
        {
            best = Some((position, node.clone()));
        }
    }
    let (_, mut node) = best?;
    node.level = permission_level(argument).or(node.level);
    if argument.contains(".executes(") {
        node.executable = true;
    }
    if argument.contains(".redirect(") {
        node.redirect = true;
    }
    // 注册表达式里出现的字面量都属于这棵子树；作为一级子命令补入。
    let mut search = 0;
    while let Some(index) = argument[search..].find("Commands.literal(") {
        let start = search + index;
        search = start + 1;
        if let Some((child, _)) = parse_node(argument, start)
            && !node
                .children
                .iter()
                .any(|existing| existing.name == child.name)
        {
            node.children.push(child);
        }
    }
    Some(node)
}

/// `word` 作为独立标识符在 `text` 中第一次出现的位置。
fn identifier_position(text: &str, word: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut search = 0;
    while let Some(index) = text[search..].find(word) {
        let start = search + index;
        let end = start + word.len();
        let before_ok = start == 0 || !is_identifier_byte(bytes[start - 1]);
        let after_ok = end >= bytes.len() || !is_identifier_byte(bytes[end]);
        if before_ok && after_ok {
            return Some(start);
        }
        search = end;
    }
    None
}

/// 解析一个构建器链：`(cast) Commands.literal("x").requires(...).then(...)`。
fn parse_node(text: &str, mut position: usize) -> Option<(CommandNode, usize)> {
    // 跳过 `(LiteralArgumentBuilder)` 之类的强制转换与空白。
    'skip: loop {
        while position < text.len() && text.as_bytes()[position].is_ascii_whitespace() {
            position += 1;
        }
        let rest = &text[position..];
        // 连续多个左括号：`((LiteralArgumentBuilder) ...`。
        let mut cursor = 0;
        while rest.as_bytes().get(cursor) == Some(&b'(') {
            cursor += 1;
        }
        let candidate = rest[cursor..].trim_start();
        if cursor > 0
            && [
                "LiteralArgumentBuilder)",
                "RequiredArgumentBuilder)",
                "ArgumentBuilder)",
            ]
            .iter()
            .any(|cast| candidate.starts_with(cast))
        {
            position += cursor + (rest[cursor..].len() - candidate.len());
            continue 'skip;
        }
        for cast in [
            "LiteralArgumentBuilder",
            "RequiredArgumentBuilder",
            "ArgumentBuilder",
        ] {
            if let Some(after_cast) = rest.strip_prefix(cast)
                && (after_cast.trim_start().starts_with(')')
                    || after_cast.trim_start().starts_with('<'))
                && let Some(close) = rest.find(')')
            {
                position += close + 1;
                continue 'skip;
            }
        }
        break;
    }

    let rest = &text[position..];
    let (name, argument, after) = if let Some(open) =
        ["Commands.literal", "LiteralArgumentBuilder.literal"]
            .iter()
            .find_map(|call| call_open(rest, call))
    {
        let (inner, after) = paren_range(text, position + open)?;
        (first_string(inner)?, None, after)
    } else {
        let open = ["Commands.argument", "RequiredArgumentBuilder.argument"]
            .iter()
            .find_map(|call| call_open(rest, call))?;
        let (inner, after) = paren_range(text, position + open)?;
        let name = first_string(inner)?;
        let argument = inner
            .split_once(',')
            .map(|(_, rest)| rest.trim().to_string())
            .filter(|rest| !rest.is_empty());
        (name, argument, after)
    };

    let mut node = CommandNode {
        name,
        argument,
        level: None,
        executable: false,
        redirect: false,
        children: Vec::new(),
    };
    let mut position = after;
    loop {
        let rest = &text[position..];
        let trimmed = rest.trim_start();
        // 反编译代码会用 `((LiteralArgumentBuilder) 节点链)` 包住子表达式，
        // 越过这些包裹用的右括号后可能还有 `.then(...)`。
        if trimmed.starts_with(')') {
            let mut cursor = position + (rest.len() - trimmed.len());
            while text.as_bytes().get(cursor) == Some(&b')') {
                cursor += 1;
            }
            position = cursor;
            continue;
        }
        if !trimmed.starts_with('.') {
            break;
        }
        let offset = rest.len() - trimmed.len();
        let after_dot = &trimmed[1..];
        let ident_end = after_dot
            .find(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
            .unwrap_or(after_dot.len());
        let method = &after_dot[..ident_end];
        let mut open = position + offset + 1 + ident_end;
        while open < text.len() && text.as_bytes()[open].is_ascii_whitespace() {
            open += 1;
        }
        if text.as_bytes().get(open) != Some(&b'(') {
            break;
        }
        let (inner, after_call) = paren_range(text, open)?;
        match method {
            "then" => {
                if let Some((child, _)) = parse_node(inner, 0) {
                    node.children.push(child);
                }
            }
            "requires" => node.level = permission_level(inner),
            "executes" => node.executable = true,
            "redirect" => node.redirect = true,
            _ => {}
        }
        position = after_call;
    }
    Some((node, position))
}

/// `text` 是否以 `call(` 开头；返回左括号的位置。
fn call_open(text: &str, call: &str) -> Option<usize> {
    text.strip_prefix(call)?;
    let mut offset = call.len();
    while offset < text.len() && text.as_bytes()[offset].is_ascii_whitespace() {
        offset += 1;
    }
    if text.as_bytes().get(offset) == Some(&b'(') {
        Some(offset)
    } else {
        None
    }
}

/// 从 `requires` 表达式里取权限等级。
fn permission_level(text: &str) -> Option<u8> {
    if text.contains("LEVEL_OWNERS") {
        Some(4)
    } else if text.contains("LEVEL_ADMINS") {
        Some(3)
    } else if text.contains("LEVEL_GAMEMASTERS") {
        Some(2)
    } else if text.contains("LEVEL_MODERATORS") {
        Some(1)
    } else if text.contains("LEVEL_ALL") {
        Some(0)
    } else {
        None
    }
}

/// 目录下排序后的 `.java` 文件。
fn java_files(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files: Vec<PathBuf> = sorted_entries(directory)?
        .into_iter()
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("java"))
        .collect();
    files.sort();
    Ok(files)
}

/// 数据包资源目录清单：`(类型, 目录, 扩展名)`。
///
/// 顶层目录直接作为类型；`worldgen` 与 `tags` 之下的子目录单独成类。
fn data_kinds(root: &Path) -> Result<Vec<(String, PathBuf, &'static str)>, String> {
    let data = root.join("data/minecraft");
    let mut kinds = Vec::new();
    for entry in sorted_entries(&data)? {
        let name = entry
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !entry.is_dir() || matches!(name.as_str(), "datapacks" | "tags") {
            continue;
        }
        if name == "worldgen" {
            for sub in sorted_entries(&entry)? {
                if sub.is_dir() {
                    let sub_name = sub
                        .file_name()
                        .map(|value| value.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let kind = format!("worldgen/{sub_name}");
                    kinds.push((kind, sub, "json"));
                }
            }
        } else if name == "structure" {
            kinds.push((name, entry, "nbt"));
        } else {
            kinds.push((name, entry, "json"));
        }
    }
    // 后处理效果注册表的数据在客户端资源目录里。
    let post_effect = root.join("assets/minecraft/post_effect");
    if post_effect.is_dir() {
        kinds.push(("post_effect".into(), post_effect, "json"));
    }
    Ok(kinds)
}

/// 递归列出目录下的资源 id（相对路径去掉扩展名）。
fn scan_resource_ids(
    extractor: &mut Extractor<'_>,
    directory: &Path,
    extension: &str,
) -> Result<BTreeSet<String>, String> {
    let mut ids = BTreeSet::new();
    let mut stack = vec![directory.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in sorted_entries(&current)? {
            if entry.is_dir() {
                stack.push(entry);
            } else if entry.extension().and_then(|value| value.to_str()) == Some(extension) {
                let relative = entry
                    .strip_prefix(directory)
                    .map_err(|error| error.to_string())?
                    .to_string_lossy()
                    .replace('\\', "/");
                let id = relative[..relative.len() - extension.len() - 1].to_string();
                extractor.digest_path(&entry);
                ids.insert(id);
            }
        }
    }
    Ok(ids)
}

/// 扫描 `data/minecraft/tags`，返回含文件的注册表目录清单。
fn scan_tag_registries(extractor: &mut Extractor<'_>, root: &Path) -> Result<Vec<String>, String> {
    let tags = root.join("data/minecraft/tags");
    let mut found = BTreeSet::new();
    let mut stack = vec![tags.clone()];
    while let Some(current) = stack.pop() {
        let mut has_json = false;
        for entry in sorted_entries(&current)? {
            if entry.is_dir() {
                stack.push(entry);
            } else if entry.extension().and_then(|value| value.to_str()) == Some("json") {
                has_json = true;
                extractor.digest_path(&entry);
            }
        }
        if has_json {
            let relative = current
                .strip_prefix(&tags)
                .map_err(|error| error.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            found.insert(relative);
        }
    }
    Ok(found.into_iter().collect())
}

/// 槽位清单：单槽、范围槽（前缀 + 数量）与多槽集合。
#[derive(Default)]
struct Slots {
    single: BTreeSet<String>,
    ranges: BTreeMap<String, u64>,
    multi: BTreeSet<String>,
}

impl Slots {
    fn to_value(&self) -> Value {
        json!({
            "single": self.single,
            "ranges": self.ranges,
            "multi": self.multi,
        })
    }
}

/// 读取源码并按文件内容累计摘要。
struct Extractor<'a> {
    root: &'a Path,
    digest: u64,
}

impl<'a> Extractor<'a> {
    fn new(root: &'a Path) -> Self {
        Self {
            root,
            digest: FNV_OFFSET,
        }
    }

    fn read(&mut self, relative: &str) -> Result<String, String> {
        let path = self.root.join(relative);
        let text = fs::read_to_string(&path)
            .map_err(|error| format!("无法读取 {}：{error}", path.display()))?;
        self.digest = fnv_update(self.digest, relative.as_bytes());
        self.digest = fnv_update(self.digest, text.as_bytes());
        Ok(text)
    }

    fn digest_path(&mut self, path: &Path) {
        let relative = path.strip_prefix(self.root).unwrap_or(path);
        let text = relative.to_string_lossy().replace('\\', "/");
        self.digest = fnv_update(self.digest, text.as_bytes());
    }

    /// 读取源码并提取 `call("id")` 形式的 id。
    fn read_ids(&mut self, relative: &str, call: &str) -> Result<Vec<String>, String> {
        let text = self.read(relative)?;
        Ok(string_args_in_calls(&text, call).into_iter().collect())
    }

    fn finish(&self) -> String {
        format!("fnv1a64:{:016x}", self.digest)
    }
}

/// 注册表插入：统一加上 `minecraft:` 前缀。
fn insert(registries: &mut BTreeMap<String, BTreeSet<String>>, kind: &str, id: &str) {
    registries
        .entry(kind.to_string())
        .or_default()
        .insert(format!("minecraft:{id}"));
}

const DYE_COLORS: [&str; 16] = [
    "white",
    "orange",
    "magenta",
    "light_blue",
    "yellow",
    "lime",
    "pink",
    "gray",
    "light_gray",
    "cyan",
    "purple",
    "blue",
    "brown",
    "green",
    "red",
    "black",
];

const COPPER_PREFIXES: [&str; 8] = [
    "",
    "exposed_",
    "weathered_",
    "oxidized_",
    "waxed_",
    "waxed_exposed_",
    "waxed_weathered_",
    "waxed_oxidized_",
];

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn fnv_update(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// 构造稳定的两空格缩进 JSON。
fn render_json(value: &Value) -> Result<String, String> {
    let mut text = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    text.push('\n');
    Ok(text)
}

fn sorted_entries(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let mut entries: Vec<PathBuf> = fs::read_dir(directory)
        .map_err(|error| format!("无法读取目录 {}：{error}", directory.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<_, _>>()
        .map_err(|error| error.to_string())?;
    entries.sort();
    Ok(entries)
}

/// 提取 `call("...")` 的第一个字符串参数。
fn string_args_in_calls(text: &str, call: &str) -> BTreeSet<String> {
    call_args(text, call)
        .iter()
        .filter_map(|inner| first_string(inner))
        .collect()
}

/// 提取 `call(Prefix.X)` 的第一个 `Prefix.X` 引用中的常量名。
fn ident_args_in_calls(text: &str, call: &str, prefix: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for inner in call_args(text, call) {
        let trimmed = inner.trim();
        if let Some(rest) = trimmed.strip_prefix(prefix)
            && let Some(rest) = rest.strip_prefix('.')
        {
            let constant: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if !constant.is_empty() {
                found.insert(constant);
            }
        }
    }
    found
}

/// 提取 `CONSTANT = call("id")` / `CONSTANT = call("id", ...)` 的常量到 id 映射。
fn constant_string_pairs(text: &str) -> BTreeMap<String, String> {
    let mut pairs = BTreeMap::new();
    for line in text.lines() {
        let trimmed = line.trim();
        let Some((declaration, rest)) = trimmed.split_once('=') else {
            continue;
        };
        // 常量名是 `=` 前最后一个标识符（前面还有修饰符与类型）。
        let Some(name) = declaration.split_whitespace().last() else {
            continue;
        };
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        {
            continue;
        }
        if let Some(open) = rest.find('(')
            && let Some(inner) = matching_paren(rest, open)
            && let Some(id) = first_string(inner)
        {
            pairs.insert(name.to_string(), id);
        }
    }
    pairs
}

/// 找出所有 `call(...)` 调用并返回括号内文本。
fn call_args<'a>(text: &'a str, call: &str) -> Vec<&'a str> {
    let mut args = Vec::new();
    let mut search = 0;
    while let Some(index) = text[search..].find(call) {
        let start = search + index;
        let after = start + call.len();
        search = after;
        if start > 0 && is_identifier_byte(text.as_bytes()[start - 1]) {
            continue;
        }
        let mut open = after;
        while open < text.len() && text.as_bytes()[open].is_ascii_whitespace() {
            open += 1;
        }
        if open >= text.len() || text.as_bytes()[open] != b'(' {
            continue;
        }
        if let Some(inner) = matching_paren(text, open) {
            args.push(inner);
        }
    }
    args
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

/// 返回从 `open` 处左括号开始、配对右括号之间的文本与右括号后的位置。
fn paren_range(text: &str, open: usize) -> Option<(&str, usize)> {
    let bytes = text.as_bytes();
    if bytes.get(open) != Some(&b'(') {
        return None;
    }
    let mut depth = 0i32;
    let mut index = open;
    let mut in_string = false;
    let mut escaped = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
        } else {
            match byte {
                b'"' => in_string = true,
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some((&text[open + 1..index], index + 1));
                    }
                }
                _ => {}
            }
        }
        index += 1;
    }
    None
}

/// 返回从 `open` 处左括号开始、配对右括号之间的文本。
fn matching_paren(text: &str, open: usize) -> Option<&str> {
    paren_range(text, open).map(|(inner, _)| inner)
}

/// 括号内文本的第一个字符串字面量。
fn first_string(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'"' {
            let mut end = index + 1;
            let mut value = String::new();
            while end < bytes.len() && bytes[end] != b'"' {
                if bytes[end] == b'\\' && end + 1 < bytes.len() {
                    end += 1;
                }
                value.push(bytes[end] as char);
                end += 1;
            }
            return Some(value);
        }
        index += 1;
    }
    None
}

/// 括号内文本的最后一个整数字面量（用于 `addSlotRange(..., 0, 54)` 的数量）。
fn last_integer(text: &str) -> Option<u64> {
    let mut last = None;
    let mut current = String::new();
    for character in text.chars() {
        if character.is_ascii_digit() {
            current.push(character);
        } else {
            if let Ok(value) = current.parse::<u64>() {
                last = Some(value);
            }
            current.clear();
        }
    }
    if let Ok(value) = current.parse::<u64>() {
        last = Some(value);
    }
    last
}

/// 从枚举声明开头到第一个方法前的字符串字面量。
fn enum_literals(text: &str, marker: &str) -> Vec<String> {
    let Some(start) = text.find(marker) else {
        return Vec::new();
    };
    let window = &text[start..(start + 8_000).min(text.len())];
    let end = window.find("\n    public ").unwrap_or(window.len());
    let mut literals = Vec::new();
    let block = &window[..end];
    let mut search = 0;
    while let Some(index) = block[search..].find('"') {
        let begin = search + index;
        if let Some(value) = first_string(&block[begin..]) {
            literals.push(value);
        }
        // 跳过这个字面量，继续找下一个。
        let mut cursor = begin + 1;
        let bytes = block.as_bytes();
        while cursor < bytes.len() && bytes[cursor] != b'"' {
            if bytes[cursor] == b'\\' {
                cursor += 1;
            }
            cursor += 1;
        }
        search = cursor + 1;
        if search >= block.len() {
            break;
        }
    }
    literals
}

#[cfg(test)]
mod tests {
    use super::*;

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
