use crate::lexer::lex;
use crate::parser::parse;

use super::*;

fn compile_text(source: &str) -> CompiledPack {
    let program = parse(lex(source, 0).unwrap()).unwrap();
    compile(&program, "test").unwrap()
}

#[test]
fn emits_26_3_pack_and_entry_tags() {
    let pack = compile_text(
        "namespace demo; score timer = 0; @load fn start() {} @tick fn tick() { timer += 1; }",
    );
    assert!(pack.files[&PathBuf::from("pack.mcmeta")].contains("\"min_format\": [121, 0]"));
    assert!(
        pack.files
            .contains_key(&PathBuf::from("data/demo/function/__mcl/load.mcfunction"))
    );
    assert!(
        pack.files[&PathBuf::from("data/minecraft/tags/function/tick.json")].contains("demo:tick")
    );
}

#[test]
fn lowers_arithmetic_and_conditionals() {
    let pack = compile_text(
        "namespace demo; score timer = 0; fn tick() { timer = timer * 2 + 1; if timer >= 20 { run \"say done\"; } }",
    );
    let function = &pack.files[&PathBuf::from("data/demo/function/tick.mcfunction")];
    assert!(function.contains("scoreboard players operation"));
    assert!(function.contains("execute if score"));
    assert!(
        pack.files
            .contains_key(&PathBuf::from("data/demo/function/__mcl/tick/0.mcfunction"))
    );
}

#[test]
fn rejects_unknown_references() {
    let program =
        parse(lex("namespace demo; fn main() { missing(); value = 1; }", 0).unwrap()).unwrap();
    let errors = compile(&program, "test").unwrap_err();
    assert_eq!(errors.len(), 2);
}

#[test]
fn lowers_boolean_conditions_and_while_loops() {
    let pack = compile_text(
        "namespace demo; score a = 1; score b = 2; fn main() { if a == 1 && !b == 0 { while a < 4 { a += 1; } } }",
    );
    let main = &pack.files[&PathBuf::from("data/demo/function/main.mcfunction")];
    assert!(main.contains("matches 1 run function demo:__mcl/main/"));
    assert!(
        pack.files
            .keys()
            .filter(|path| path.to_string_lossy().contains("__mcl/main"))
            .count()
            >= 3
    );
}

#[test]
fn rejects_synchronous_recursion_but_allows_scheduled_self_call() {
    let recursive =
        parse(lex("namespace demo; fn a() { b(); } fn b() { a(); }", 0).unwrap()).unwrap();
    assert_eq!(compile(&recursive, "test").unwrap_err().len(), 2);

    let scheduled =
        compile_text("namespace demo; fn heartbeat() { schedule heartbeat() after 1 t; }");
    assert!(
        scheduled
            .files
            .contains_key(&PathBuf::from("data/demo/function/heartbeat.mcfunction"))
    );

    let tag_recursive = parse(
        lex(
            "namespace demo; fn a() { call #loop(); } fn_tag loop { value(a); }",
            0,
        )
        .unwrap(),
    )
    .unwrap();
    let errors = compile(&tag_recursive, "test").unwrap_err();
    assert_eq!(errors.len(), 1, "{errors:#?}");
    assert!(errors[0].message.contains("同步递归调用环"));
}

#[test]
fn passes_expression_arguments_through_private_score_slots() {
    let pack = compile_text(
        "namespace demo; score total = 0; fn add(amount) { total += amount; } fn main() { add(total + 2); }",
    );
    let caller = &pack.files[&PathBuf::from("data/demo/function/main.mcfunction")];
    let callee = &pack.files[&PathBuf::from("data/demo/function/add.mcfunction")];
    assert!(caller.contains("scoreboard players operation #p_"));
    assert!(caller.contains("function demo:add"));
    assert!(callee.contains("+= #p_"));
}

#[test]
fn validates_and_emits_json_resources() {
    let pack = compile_text(
        "namespace demo; resource predicate coin = \"\"\"{\"condition\":\"minecraft:random_chance\",\"chance\":0.5}\"\"\";",
    );
    let resource = &pack.files[&PathBuf::from("data/demo/predicate/coin.json")];
    assert!(resource.contains("\"chance\": 0.5"));

    let invalid = parse(
        lex(
            "namespace demo; resource predicate broken = \"\"\"{no}\"\"\";",
            0,
        )
        .unwrap(),
    )
    .unwrap();
    assert!(compile(&invalid, "test").is_err());
}

#[test]
fn lowers_predicate_conditions_and_typed_sounds() {
    let pack = compile_text(
        r#"
            namespace demo;
            resource predicate coin = """{"condition":"minecraft:random_chance","chance":0.5}""";
            query players = entity("minecraft:player") {}
            @tick fn tick() {
                if predicate(coin) {
                    each(players) {
                        sound.self("minecraft:block.note_block.pling", master);
                    }
                }
            }
            "#,
    );
    let generated = pack.files.values().cloned().collect::<Vec<_>>().join("\n");
    assert!(generated.contains("execute if predicate demo:coin run scoreboard players set"));
    assert!(generated.contains("playsound minecraft:block.note_block.pling master @s ~ ~ ~ 1 1"));

    let invalid = parse(
        lex(
            r#"
                namespace demo;
                fn broken() {
                    if predicate(missing) {}
                    sound.self("Invalid Sound", invalid_category);
                }
                "#,
            0,
        )
        .unwrap(),
    )
    .unwrap();
    let errors = compile(&invalid, "test").unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("找不到 predicate 资源"))
    );
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("需要玩家执行上下文"))
    );
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("不是有效的声音资源位置"))
    );
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("不是有效的声音分类"))
    );
}

#[test]
fn chinese_and_english_keywords_compile_identically() {
    let english = compile_text(
        r#"
            namespace demo;
            score active = 0;
            query triggers = entity("minecraft:item") {
                without_tag("handled");
                limit(1);
                within(16);
                sort(nearest);
                item(contents) {
                    id = "minecraft:emerald";
                    count = 1;
                    custom_name = "A";
                }
            }
            query players = entity("minecraft:player") { tag("ready"); }
            item reward = item_stack("minecraft:diamond") {
                count = 3;
                custom_name = "Reward";
                lore("First line");
                enchantment("minecraft:fortune", 2);
                stored_enchantment("minecraft:mending", 1);
                damage = 4;
                unbreakable = true;
            }
            item flare = item_stack("minecraft:leather_chestplate") {
                max_stack_size = 16;
                item_name = "Flare";
                rarity = rare;
                item_model = "minecraft:leather_chestplate";
                dyed_color = 16711680;
                enchantment_glint_override = true;
            }
            storage saved = item_list("demo:state", "saved_items");
            resource predicate coin = """{"condition":"minecraft:random_chance","chance":0.5}""";
            @load fn load() { message.all("loaded", green); }
            @entity fn mark() { self.add_tag("handled"); }
            @non_player fn prepare() { self.set_invulnerable(true); self.clear_items(); }
            @tick fn tick() {
                each(triggers) {
                    call mark();
                    if predicate(coin) {
                        spawn("minecraft:chest_minecart") {
                            call prepare();
                            self.restore_items(saved);
                        }
                        message.nearest(16, "ready", gold);
                    } else {
                        in_dimension("minecraft:overworld") {
                            self.remove_preserving_items(saved);
                        }
                    }
                    give(origin, self.item);
                    self.remove();
                }
                each(players) {
                    self.give_item(reward, 2);
                    sound.self("minecraft:block.note_block.pling", master);
                }
                give(players, flare, 16);
            }
            fn plus_one(value) -> score {
                let result = value + 1;
                return result;
            }
            fn later() { schedule later() after 1 s append; }
            fn drain() { while active > 0 { active -= 1; } }
            "#,
    );
    let chinese = compile_text(
        r#"
            命名空间 demo;
            计分 active = 0;
            查询 triggers = 实体("minecraft:item") {
                排除标签("handled");
                上限(1);
                范围(16);
                排序(最近);
                物品(内容) {
                    类型 = "minecraft:emerald";
                    数量 = 1;
                    自定义名称 = "A";
                }
            }
            查询 players = 实体("minecraft:player") { 标签("ready"); }
            物品 reward = 物品堆("minecraft:diamond") {
                数量 = 3;
                自定义名称 = "Reward";
                描述("First line");
                附魔("minecraft:fortune", 2);
                存储附魔("minecraft:mending", 1);
                损伤 = 4;
                无法破坏 = 真;
            }
            物品 flare = 物品堆("minecraft:leather_chestplate") {
                最大堆叠 = 16;
                物品名称 = "Flare";
                稀有度 = 稀有;
                物品模型 = "minecraft:leather_chestplate";
                染色 = 16711680;
                附魔光效 = 真;
            }
            存储 saved = 物品列表("demo:state", "saved_items");
            资源 谓词 coin = """{"condition":"minecraft:random_chance","chance":0.5}""";
            @加载 函数 load() { 消息.全部("loaded", 绿色); }
            @实体 函数 mark() { 自身.添加标签("handled"); }
            @非玩家 函数 prepare() { 自身.设置无敌(真); 自身.清空物品(); }
            @每刻 函数 tick() {
                遍历(triggers) {
                    调用 mark();
                    如果 谓词(coin) {
                        召唤("minecraft:chest_minecart") {
                            调用 prepare();
                            自身.恢复物品(saved);
                        }
                        消息.最近(16, "ready", 金色);
                    } 否则 {
                        在维度("minecraft:overworld") {
                            自身.保存并移除(saved);
                        }
                    }
                    给予(投掷者, 自身.物品);
                    自身.移除();
                }
                遍历(players) {
                    自身.给予物品(reward, 2);
                    声音.自身("minecraft:block.note_block.pling", 主音量);
                }
                给予(players, flare, 16);
            }
            函数 plus_one(value) -> 计分 {
                令 result = value + 1;
                返回 result;
            }
            函数 later() { 调度 later() 延后 1 秒 追加; }
            函数 drain() { 当 active > 0 { active -= 1; } }
            "#,
    );

    assert_eq!(english.files, chinese.files);
}

#[test]
fn enforces_local_scope_and_emits_private_local_slots() {
    let pack = compile_text(
        "namespace demo; score total = 0; fn add(value) { let doubled = value * 2; total += doubled; }",
    );
    let function = &pack.files[&PathBuf::from("data/demo/function/add.mcfunction")];
    assert!(function.contains("#l_"));

    let invalid = parse(
        lex(
            "namespace demo; fn broken() { value = 1; let value = 0; }",
            0,
        )
        .unwrap(),
    )
    .unwrap();
    assert!(compile(&invalid, "test").is_err());
}

#[test]
fn captures_score_function_results_in_expressions() {
    let pack = compile_text(
        "namespace demo; score total = 0; fn double(value) -> score { return value * 2; } fn main() { total = double(20) + 2; }",
    );
    let returning = &pack.files[&PathBuf::from("data/demo/function/double.mcfunction")];
    let caller = &pack.files[&PathBuf::from("data/demo/function/main.mcfunction")];
    assert!(returning.contains("return run scoreboard players get"));
    assert!(caller.contains("execute store result score"));
    assert!(caller.contains("run function demo:double"));

    let invalid = parse(
        lex(
            "namespace demo; score flag = 1; fn wrong_void() { return 1; } fn wrong_value() -> score { return; } fn nested() { if flag == 1 { return; } }",
            0,
        )
        .unwrap(),
    )
    .unwrap();
    let errors = compile(&invalid, "test").unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("无返回值函数"))
    );
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("需要 `return <表达式>;`"))
    );
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("只能直接出现在函数代码块"))
    );
}

#[test]
fn lowers_self_item_give_through_empty_slot_source() {
    let pack = compile_text(
        "namespace demo; item bonus = item_stack(\"minecraft:diamond\") { custom_name = \"B\"; } query drops = entity(\"minecraft:item\") {} fn collect() { each(drops) { give(origin, self.item); give(origin, bonus); } }",
    );
    let generated = pack.files.values().cloned().collect::<Vec<_>>().join("\n");
    assert!(
        generated
            .contains("execute on origin if entity @s[type=minecraft:player] store success score"),
        "missing origin dispatch:\n{generated}"
    );
    assert!(
        generated
            .contains("run item replace entity @s demo:__mcl/empty_slot from entity @e[tag=mcl_")
    );
    assert!(generated.contains("_give,limit=1] contents"));
    assert!(
        generated.contains("matches 1 run item replace entity @s contents with minecraft:air"),
        "missing source clear:\n{generated}"
    );
    assert!(
        generated.contains(
            "execute on origin if entity @s[type=minecraft:player] run give @s minecraft:diamond[minecraft:custom_name={text:\"B\"}] 1"
        ),
        "missing bonus give to origin:\n{generated}"
    );
    let slot_source = &pack.files[&PathBuf::from("data/demo/slot_source/__mcl/empty_slot.json")];
    assert!(slot_source.contains("\"type\": \"minecraft:filtered\""));
    assert!(slot_source.contains("\"type\": \"minecraft:group\""));
    assert!(slot_source.contains("\"slots\": \"hotbar.*\""));
    assert!(slot_source.contains("\"slots\": \"inventory.*\""));
    assert!(!slot_source.contains("container.*"));
    assert!(slot_source.contains("\"count\": 0"));

    let players = compile_text(
        "namespace demo; item gem = item_stack(\"minecraft:emerald\") {} query players = entity(\"minecraft:player\") {} fn collect() { each(players) { give(origin, gem); give(players, self.item); } }",
    );
    let generated = players
        .files
        .values()
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    assert!(generated.contains(
        "execute on origin if entity @s[type=minecraft:player] run give @s minecraft:emerald 1"
    ));
    assert!(
        generated.contains(
            "execute as @e[type=minecraft:player] at @s run function demo:__mcl/collect/"
        )
    );
}

#[test]
fn lowers_typed_minecraft_queries_storage_and_actions() {
    let pack = compile_text(
        r#"
            namespace demo;
            score active = 0;
            query triggers = entity("minecraft:item") {
                without_tag("handled");
                item(contents) {
                    id = "minecraft:emerald";
                    count = 1;
                    custom_name = "A";
                }
            }
            storage saved = item_list("demo:state", "saved_items");
            @tick fn tick() {
                each(triggers) {
                    self.add_tag("handled");
                    spawn("minecraft:chest_minecart") {
                        self.set_invulnerable(true);
                        self.restore_items(saved);
                    }
                    self.remove_preserving_items(saved);
                    message.nearest(16, "Ready", green);
                }
            }
            "#,
    );
    let generated = pack.files.values().cloned().collect::<Vec<_>>().join("\n");
    assert!(generated.contains(
        "if items entity @s contents minecraft:emerald[minecraft:custom_name={text:\"A\"},minecraft:count=1]"
    ));
    assert!(generated.contains("execute summon minecraft:chest_minecart run function demo:"));
    assert!(generated.contains(
        "execute unless data storage demo:state saved_items run data modify storage demo:state saved_items set value []"
    ));
    assert!(
        generated.contains("data modify storage demo:state saved_items set from entity @s Items")
    );
    assert!(
        generated.contains("data modify entity @s Items set from storage demo:state saved_items")
    );
    assert!(generated.contains("data modify entity @s Items set value []"));
    assert!(generated.contains("tellraw @a[sort=nearest,limit=1,distance=..16]"));
}

#[test]
fn lowers_typed_item_giving_and_checks_player_context() {
    let pack = compile_text(
        r#"
            namespace demo;
            item reward = item_stack("minecraft:diamond") {
                count = 3;
                custom_name = "Explorer's Gem";
                lore("First line");
                lore("Second line");
                enchantment("minecraft:fortune", 2);
                stored_enchantment("minecraft:mending", 1);
                damage = 4;
                unbreakable = true;
            }
            item flare = item_stack("minecraft:leather_chestplate") {
                max_stack_size = 16;
                item_name = "Signal Flare";
                rarity = rare;
                item_model = "minecraft:leather_chestplate";
                dyed_color = 16711680;
                enchantment_glint_override = true;
            }
            item blade = item_stack("minecraft:diamond_sword") {
                max_damage = 100;
            }
            query players = entity("minecraft:player") {}
            @player fn reward_player() { self.give_item(reward); }
            @tick fn tick() {
                each(players) { reward_player(); }
                give(players, flare, 1600);
                give(players, blade);
                give(players, reward, 4);
            }
            "#,
    );
    let generated = pack.files.values().cloned().collect::<Vec<_>>().join("\n");
    assert!(generated.contains(
        "give @s minecraft:diamond[minecraft:custom_name={text:\"Explorer's Gem\"},minecraft:lore=[{text:\"First line\"},{text:\"Second line\"}],minecraft:enchantments={\"minecraft:fortune\":2},minecraft:stored_enchantments={\"minecraft:mending\":1},minecraft:damage=4,minecraft:unbreakable={}] 3"
    ));
    assert!(generated.contains(
        "execute as @e[type=minecraft:player] at @s run give @s minecraft:leather_chestplate[minecraft:item_name={text:\"Signal Flare\"},minecraft:max_stack_size=16,minecraft:rarity=\"rare\",minecraft:item_model=\"minecraft:leather_chestplate\",minecraft:dyed_color=16711680,minecraft:enchantment_glint_override=true] 1600"
    ));
    assert!(generated.contains("run give @s minecraft:diamond_sword[minecraft:max_damage=100] 1"));
    assert!(generated.contains("run give @s minecraft:diamond[") && generated.contains("] 4"));

    let invalid = parse(
        lex(
            r#"
                namespace demo;
                item broken = item_stack("Invalid Item") {
                    count = 101;
                    enchantment("Invalid Enchantment", 0);
                    enchantment("Invalid Enchantment", 256);
                    damage = 2147483648;
                }
                item bounded = item_stack("minecraft:diamond") { max_stack_size = 16; }
                item conflicting = item_stack("minecraft:diamond") {
                    max_stack_size = 16;
                    max_damage = 10;
                }
                query item_entities = entity("minecraft:item") {}
                query players = entity("minecraft:player") {}
                @player fn needs_player() {}
                @entity fn wrong_entity() {
                    needs_player();
                    self.give_item(missing);
                }
                @tick fn wrong_give() {
                    give(item_entities, bounded);
                    give(players, bounded, 1601);
                    give(players, bounded, 0);
                }
                fn wrong_origin() { give(origin, self.item); }
                fn wrong_self_item() { give(players, self.item, 3); }
                fn wrong_call() { wrong_entity(); }
                fn wrong_schedule() { schedule needs_player() after 1 t; }
                "#,
            0,
        )
        .unwrap(),
    )
    .unwrap();
    let errors = compile(&invalid, "test").unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("物品资源位置"))
    );
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("count 必须是 1 到 100"))
    );
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("必须匹配 minecraft:player"))
    );
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("给予数量必须是 1 到 1600"))
    );
    assert!(errors.iter().any(|error| {
        error
            .message
            .contains("不能同时设置大于 1 的 max_stack_size 和 max_damage")
    }));
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("附魔资源位置"))
    );
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("附魔等级"))
    );
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("重复声明"))
    );
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("2147483647"))
    );
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("需要玩家执行上下文"))
    );
    assert!(errors.iter().any(|error| {
        error
            .message
            .contains("@player 函数 `needs_player` 需要玩家执行上下文")
    }));
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("找不到物品定义"))
    );
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("不能调度需要执行上下文"))
    );
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("投掷者目标需要实体执行上下文"))
    );
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("原样给予 self.item 时不能指定数量"))
    );
}

#[test]
fn checks_entity_context_and_typed_references() {
    let source = r#"
            namespace demo;
            @entity fn entity_only() { self.remove(); }
            @tick fn tick() {
                entity_only();
                each(missing_query) { self.save_items(missing_storage); }
                message.all("bad color", orange);
            }
        "#;
    let program = parse(lex(source, 0).unwrap()).unwrap();
    let errors = compile(&program, "test").unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("需要实体执行上下文"))
    );
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("找不到实体查询"))
    );
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("找不到物品存储"))
    );
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("不是有效的文本颜色"))
    );
}

#[test]
fn rejects_player_nbt_mutations_and_accepts_non_player_contexts() {
    let invalid = parse(
        lex(
            r#"
                namespace demo;
                query players = entity("minecraft:player") {}
                storage saved = item_list("demo:state", "saved_items");
                @player fn player_nbt() {
                    self.set_invulnerable(true);
                    self.save_items(saved);
                }
                @entity fn unknown_entity_nbt() {
                    self.clear_items();
                }
                @tick fn tick() {
                    each(players) { self.remove_preserving_items(saved); }
                }
                "#,
            0,
        )
        .unwrap(),
    )
    .unwrap();
    let errors = compile(&invalid, "test").unwrap_err();
    assert_eq!(errors.len(), 4, "{errors:#?}");
    assert!(
        errors
            .iter()
            .all(|error| error.message.contains("Minecraft 不允许修改玩家数据"))
    );

    let valid = compile_text(
        r#"
            namespace demo;
            query mobs = entity("minecraft:zombie") {}
            storage saved = item_list("demo:state", "saved_items");
            @non_player fn mob_init() {
                self.set_invulnerable(true);
                self.restore_items(saved);
            }
            fn spawn_all() {
                spawn("minecraft:armor_stand") { mob_init(); }
                each(mobs) { mob_init(); self.add_tag("ready"); }
            }
            "#,
    );
    let generated = valid.files.values().cloned().collect::<Vec<_>>().join("\n");
    assert!(generated.contains("data merge entity @s {Invulnerable:1b}"));
    assert!(
        generated.contains("data modify entity @s Items set from storage demo:state saved_items")
    );
    assert!(generated.contains("tag @s add ready"));

    let wrong_caller = parse(
        lex(
            r#"
                namespace demo;
                @non_player fn mob_only() {}
                @entity fn any_entity() { mob_only(); }
                fn no_context() { mob_only(); }
                "#,
            0,
        )
        .unwrap(),
    )
    .unwrap();
    let errors = compile(&wrong_caller, "test").unwrap_err();
    assert_eq!(errors.len(), 2, "{errors:#?}");
    assert!(errors.iter().all(|error| {
        error
            .message
            .contains("@non_player 函数 `mob_only` 需要非玩家实体执行上下文")
    }));
}

#[test]
fn rejects_non_summonable_entity_types() {
    let program = parse(
        lex(
            r#"
                namespace demo;
                fn bad() {
                    spawn("minecraft:player") { self.add_tag("x"); }
                    spawn("minecraft:fishing_bobber") { self.add_tag("x"); }
                }
                "#,
            0,
        )
        .unwrap(),
    )
    .unwrap();
    let errors = compile(&program, "test").unwrap_err();
    assert_eq!(errors.len(), 2, "{errors:#?}");
    assert!(
        errors
            .iter()
            .all(|error| error.message.contains("不支持实体类型"))
    );
}

#[test]
fn lowers_return_fail_and_run() {
    let pack = compile_text(
        r#"
            namespace demo;
            fn fail_now() { return fail; }
            fn gametime() -> score { return run "time query gametime"; }
            "#,
    );
    let fail_now = &pack.files[&PathBuf::from("data/demo/function/fail_now.mcfunction")];
    assert!(fail_now.contains("return fail"));
    let gametime = &pack.files[&PathBuf::from("data/demo/function/gametime.mcfunction")];
    assert!(gametime.contains("return run time query gametime"));

    let invalid =
        parse(lex("namespace demo; fn bad() { return run \"/say hi\"; }", 0).unwrap()).unwrap_err();
    assert!(invalid[0].message.contains("不能以 `/` 开头"));
}

#[test]
fn lowers_schedule_clear_and_fractional_delays() {
    let pack = compile_text(
        r#"
            namespace demo;
            fn cleanup() {}
            fn main() {
                schedule cleanup() after 1.5 s append;
                schedule cleanup() after 2 d;
                schedule cleanup() after 30 t replace;
                schedule.clear(cleanup);
            }
            "#,
    );
    let main = &pack.files[&PathBuf::from("data/demo/function/main.mcfunction")];
    assert!(main.contains("schedule function demo:cleanup 1.5s append"));
    assert!(main.contains("schedule function demo:cleanup 2d replace"));
    assert!(main.contains("schedule function demo:cleanup 30t replace"));
    assert!(main.contains("schedule clear demo:cleanup"));

    let too_small = parse(lex("namespace demo; fn f() { schedule f() after 0.01 s; }", 0).unwrap());
    assert!(
        too_small
            .unwrap_err()
            .iter()
            .any(|error| error.message.contains("至少为 1 游戏刻"))
    );
}

#[test]
fn emits_function_tags_and_tag_calls() {
    let pack = compile_text(
        r#"
            namespace demo;
            fn a() {}
            fn b() {}
            fn main() { call #cleanup(); schedule #cleanup() after 2 s; }
            fn_tag cleanup {
                value(a);
                value(#nested);
                value("minecraft:tick");
                replace = true;
            }
            fn_tag nested { value(b); }
            "#,
    );
    let main = &pack.files[&PathBuf::from("data/demo/function/main.mcfunction")];
    assert!(main.contains("function #demo:cleanup"));
    assert!(main.contains("schedule function #demo:cleanup 2s replace"));
    let cleanup = &pack.files[&PathBuf::from("data/demo/tags/function/cleanup.json")];
    assert!(cleanup.contains("\"demo:a\""), "{cleanup}");
    assert!(cleanup.contains("\"#demo:nested\""), "{cleanup}");
    assert!(cleanup.contains("\"minecraft:tick\""), "{cleanup}");
    assert!(cleanup.contains("\"replace\": true"), "{cleanup}");
    let nested = &pack.files[&PathBuf::from("data/demo/tags/function/nested.json")];
    assert!(nested.contains("\"demo:b\""), "{nested}");
    assert!(!nested.contains("replace"), "{nested}");

    let empty = compile_text("namespace demo; fn_tag nothing { }");
    let empty = &empty.files[&PathBuf::from("data/demo/tags/function/nothing.json")];
    assert!(empty.contains("\"values\": []"), "{empty}");
}

#[test]
fn lowers_effect_xp_and_clear_commands() {
    let pack = compile_text(
        r#"
            namespace demo;
            score total = 0;
            query players = entity("minecraft:player") { limit(1); }
            query mobs = entity("minecraft:zombie") {}
            @tick fn tick() {
                effect.give(mobs, "minecraft:speed", 30);
                effect.give(mobs, "minecraft:speed", 30, 2);
                effect.give(mobs, "minecraft:speed", 30, 0, true);
                effect.give_infinite(players, "minecraft:night_vision");
                effect.give_infinite(players, "minecraft:night_vision", 1, true);
                effect.clear(mobs);
                effect.clear(mobs, "minecraft:speed");
                xp.add(players, points, 10);
                xp.set(players, levels, 3);
                total = xp.query(players, levels) + 1;
                if xp.query(players, levels) >= 5 {
                    message.all("veteran", gold);
                }
                clear(players);
                clear(players, "minecraft:diamond");
                clear(players, "minecraft:diamond", 5);
            }
            "#,
    );
    let generated = pack.files.values().cloned().collect::<Vec<_>>().join("\n");
    assert!(
        generated.contains("run effect give @s minecraft:speed 30\n"),
        "{generated}"
    );
    assert!(generated.contains("run effect give @s minecraft:speed 30 2\n"));
    assert!(generated.contains("run effect give @s minecraft:speed 30 0 true\n"));
    assert!(generated.contains("run effect give @s minecraft:night_vision infinite\n"));
    assert!(generated.contains("run effect give @s minecraft:night_vision infinite 1 true\n"));
    assert!(generated.contains("run effect clear @s\n"));
    assert!(generated.contains("run effect clear @s minecraft:speed\n"));
    assert!(generated.contains("run xp add @s 10 points\n"));
    assert!(generated.contains("run xp set @s 3 levels\n"));
    assert!(generated.contains("run xp query @s levels"));
    assert_eq!(generated.matches("run xp query @s levels").count(), 2);
    assert!(generated.contains("run clear @s\n"));
    assert!(generated.contains("run clear @s minecraft:diamond\n"));
    assert!(generated.contains("run clear @s minecraft:diamond 5\n"));
}

#[test]
fn rejects_invalid_effects_xp_clear_and_tags() {
    let program = parse(
        lex(
            r#"
            namespace demo;
            query players = entity("minecraft:player") {}
            query mobs = entity("minecraft:zombie") {}
            fn ok() {}
            fn wrong() {
                effect.give(mobs, "Invalid Effect", 30);
                effect.give(mobs, "minecraft:speed", 0);
                effect.give(mobs, "minecraft:speed", 30, 300);
                xp.add(mobs, points, 5);
                xp.set(players, levels, -1);
            }
            fn wrong_query() -> score {
                return xp.query(players, levels);
            }
            fn wrong_clear() {
                clear(mobs);
                clear(players, "Invalid Item");
                clear(players, "minecraft:diamond", 4294967295);
            }
            fn_tag broken {
                value(missing_function);
                value(#missing_tag);
                value("Invalid Location");
            }
            fn call_missing() { call #missing_tag(); }
            fn call_args() { call #broken_function(1); }
            fn_tag broken_function { value(ok); }
            fn_tag self_cycle { value(#self_cycle); }
            fn schedule_problems() {
                schedule.clear(missing_function);
                schedule missing_function() after 1 t;
            }
            "#,
            0,
        )
        .unwrap(),
    )
    .unwrap();
    let errors = compile(&program, "test").unwrap_err();
    let messages = errors
        .iter()
        .map(|error| error.message.as_str())
        .collect::<Vec<_>>();
    for expected in [
        "不是有效的效果资源位置",
        "effect 持续秒数必须是 1 到 1000000",
        "effect 等级必须是 0 到 255",
        "查询 `mobs` 必须匹配 minecraft:player",
        "xp.set 的数量必须是非负整数",
        "xp.query 需要 limit(1) 的单个玩家查询",
        "`Invalid Item` 不是有效的物品资源位置",
        "clear 最大数量不能超过 2147483647",
        "引用了不存在的函数 `missing_function`",
        "引用了不存在的标签 `missing_tag`",
        "`Invalid Location` 不是有效的资源位置",
        "函数标签 `self_cycle` 形成循环引用",
        "找不到函数标签 `missing_tag`",
        "函数标签调用不接受参数",
        "找不到函数 `missing_function`",
    ] {
        assert!(
            messages.iter().any(|message| message.contains(expected)),
            "missing `{expected}` in: {messages:#?}"
        );
    }

    let duplicate_replace = parse(
        lex(
            "namespace demo; fn_tag t { replace = true; replace = false; }",
            0,
        )
        .unwrap(),
    );
    assert!(
        duplicate_replace
            .unwrap_err()
            .iter()
            .any(|error| error.message.contains("只能声明一次 replace"))
    );

    let tag_context = parse(
        lex(
            r#"
            namespace demo;
            query players = entity("minecraft:player") {}
            @player fn needs_player() {}
            fn_tag needs_context { value(needs_player); }
            fn main() {
                call #needs_context();
                schedule #needs_context() after 1 t;
            }
            "#,
            0,
        )
        .unwrap(),
    )
    .unwrap();
    let errors = compile(&tag_context, "test").unwrap_err();
    assert!(errors.iter().any(|error| {
        error.message.contains(
            "函数标签 `needs_context` 中的 @player 函数 `needs_player` 需要玩家执行上下文",
        )
    }));
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("不能调度函数标签 `needs_context`"))
    );

    let tag_parameters = parse(
        lex(
            r#"
            namespace demo;
            fn add(amount) {}
            fn_tag with_args { value(add); }
            fn main() { call #with_args(); }
            "#,
            0,
        )
        .unwrap(),
    )
    .unwrap();
    let errors = compile(&tag_parameters, "test").unwrap_err();
    assert!(errors.iter().any(|error| {
        error
            .message
            .contains("函数标签 `with_args` 中的函数 `add` 需要 1 个参数")
    }));
}

#[test]
fn lowers_stopwatch_commands() {
    let pack = compile_text(
        r#"
            namespace demo;
            score elapsed = 0;
            fn main() {
                stopwatch.create("demo:timer");
                stopwatch.restart("demo:timer");
                stopwatch.remove("demo:timer");
                elapsed = stopwatch.query("demo:timer");
                elapsed = stopwatch.query("demo:timer", 1000);
                elapsed = stopwatch.query("demo:timer", 0.5);
                if stopwatch.query("demo:timer", -2) >= 0 {
                    message.all("wrapped", yellow);
                }
            }
            "#,
    );
    let main = &pack.files[&PathBuf::from("data/demo/function/main.mcfunction")];
    assert!(main.contains("stopwatch create demo:timer\n"), "{main}");
    assert!(main.contains("stopwatch restart demo:timer\n"));
    assert!(main.contains("stopwatch remove demo:timer\n"));
    assert!(main.contains("run stopwatch query demo:timer\n"));
    assert!(main.contains("run stopwatch query demo:timer 1000\n"));
    assert!(main.contains("run stopwatch query demo:timer 0.5\n"));
    assert!(main.contains("run stopwatch query demo:timer -2\n"));
    assert_eq!(main.matches("run stopwatch query demo:timer").count(), 4);
}

#[test]
fn rejects_invalid_stopwatch_usage() {
    let program = parse(
        lex(
            r#"
            namespace demo;
            fn bad_create() { stopwatch.create("Bad Id"); }
            fn bad_query() -> score { return stopwatch.query("also bad"); }
            "#,
            0,
        )
        .unwrap(),
    )
    .unwrap();
    let errors = compile(&program, "test").unwrap_err();
    assert_eq!(errors.len(), 2, "{errors:#?}");
    assert!(
        errors
            .iter()
            .all(|error| error.message.contains("不是有效的秒表资源位置"))
    );
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("`Bad Id`"))
    );
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("`also bad`"))
    );

    let statement_query =
        parse(lex("namespace demo; fn f() { stopwatch.query(\"demo:t\"); }", 0).unwrap());
    assert!(
        statement_query.unwrap_err()[0]
            .message
            .contains("只能出现在表达式里")
    );

    let unknown = parse(lex("namespace demo; fn f() { stopwatch.list(\"demo:t\"); }", 0).unwrap());
    assert!(
        unknown.unwrap_err()[0]
            .message
            .contains("未知 stopwatch 方法")
    );
}

#[test]
fn new_commands_are_keyword_symmetric() {
    let english = compile_text(
        r#"
            namespace demo;
            query players = entity("minecraft:player") { limit(1); }
            query mobs = entity("minecraft:zombie") {}
            fn a() {}
            fn b() { schedule #cleanup() after 1.5 s; }
            fn main() {
                call #cleanup();
                effect.give(mobs, "minecraft:speed", 30, 2, true);
                effect.give_infinite(players, "minecraft:night_vision");
                effect.clear(mobs, "minecraft:speed");
                xp.add(players, points, 5);
                xp.set(players, levels, 2);
                let level = xp.query(players, levels);
                clear(players, "minecraft:diamond", 3);
                stopwatch.create("demo:timer");
                stopwatch.restart("demo:timer");
                let elapsed = stopwatch.query("demo:timer", 0.5);
                stopwatch.remove("demo:timer");
                return fail;
            }
            fn_tag cleanup { value(a); value(#nested); replace = true; }
            fn_tag nested { value(b); }
            "#,
    );
    let chinese = compile_text(
        r#"
            命名空间 demo;
            查询 players = 实体("minecraft:player") { 上限(1); }
            查询 mobs = 实体("minecraft:zombie") {}
            函数 a() {}
            函数 b() { 调度 #cleanup() 延后 1.5 秒; }
            函数 main() {
                调用 #cleanup();
                效果.给予(mobs, "minecraft:speed", 30, 2, 真);
                效果.给予无限(players, "minecraft:night_vision");
                效果.清除(mobs, "minecraft:speed");
                经验.增加(players, 点数, 5);
                经验.设置(players, 等级, 2);
                令 level = 经验.查询(players, 等级);
                清除(players, "minecraft:diamond", 3);
                秒表.创建("demo:timer");
                秒表.重启("demo:timer");
                令 elapsed = 秒表.查询("demo:timer", 0.5);
                秒表.移除("demo:timer");
                返回 失败;
            }
            函数标签 cleanup { 值(a); 值(#nested); 替换 = 真; }
            函数标签 nested { 值(b); }
            "#,
    );
    assert_eq!(english.files, chinese.files);
}
