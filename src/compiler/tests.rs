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
            storage saved = items("demo:state", "saved_items");
            resource predicate coin = """{"condition":"minecraft:random_chance","chance":0.5}""";
            @load fn load() { message.all("loaded", green); }
            @entity fn mark() { self.add_tag("handled"); }
            @tick fn tick() {
                each(triggers) {
                    call mark();
                    if predicate(coin) {
                        spawn("minecraft:chest_minecart") {
                            self.set_invulnerable(true);
                            self.restore_items(saved);
                        }
                        message.nearest(16, "ready", gold);
                    } else {
                        in_dimension("minecraft:overworld") {
                            self.remove_preserving_items(saved);
                        }
                    }
                    self.consume();
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
            存储 saved = 物品("demo:state", "saved_items");
            资源 谓词 coin = """{"condition":"minecraft:random_chance","chance":0.5}""";
            @加载 函数 load() { 消息.全部("loaded", 绿色); }
            @实体 函数 mark() { 自身.添加标签("handled"); }
            @每刻 函数 tick() {
                遍历(triggers) {
                    调用 mark();
                    如果 谓词(coin) {
                        召唤("minecraft:chest_minecart") {
                            自身.设置无敌(真);
                            自身.恢复物品(saved);
                        }
                        消息.最近(16, "ready", 金色);
                    } 否则 {
                        在维度("minecraft:overworld") {
                            自身.保存并移除(saved);
                        }
                    }
                    自身.消耗();
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
            storage saved = items("demo:state", "saved_items");
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
