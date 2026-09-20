use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::{Map, Value, json};

use super::SOURCE_DIR;
use super::extract::{
    COPPER_PREFIXES, DYE_COLORS, Extractor, Slots, data_kinds, insert, render_json,
    scan_resource_ids, scan_tag_registries,
};
use super::text::{
    call_args, constant_string_pairs, first_string, ident_args_in_calls, last_integer,
    string_args_in_calls,
};

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
            "data_component_type",
            "net/minecraft/core/component/DataComponents.java",
            "register",
        ),
        (
            "data_component_predicate_type",
            "net/minecraft/core/component/predicates/DataComponentPredicates.java",
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

    let mut enchantment_max_levels = BTreeMap::new();
    for id in registries.get("enchantment").into_iter().flatten() {
        let path = format!(
            "data/minecraft/enchantment/{}.json",
            id.trim_start_matches("minecraft:")
        );
        let json: Value =
            serde_json::from_str(&extractor.read(&path)?).map_err(|e| format!("{path}: {e}"))?;
        if let Some(level) = json.get("max_level").and_then(Value::as_u64) {
            enchantment_max_levels.insert(id.clone(), level);
        }
    }
    let mut root_object = Map::new();
    root_object.insert(
        "enchantment_max_levels".into(),
        json!(enchantment_max_levels),
    );
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
