use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Map, Value, json};

use super::SOURCE_DIR;
use super::extract::{Extractor, render_json};
use super::text::{enum_literals, first_string, matching_paren};

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
