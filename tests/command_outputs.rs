//! End-to-end corpus assertions against native command shapes.
use mclang::{BuildOptions, build_file};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

fn files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut result = BTreeMap::new();
    let mut pending = vec![root.to_owned()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                result.insert(
                    path.strip_prefix(root).unwrap().to_owned(),
                    fs::read(path).unwrap(),
                );
            }
        }
    }
    result
}

#[test]
fn native_command_shapes_and_macro_forwarding() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cases: &[(&str, &str, &[&str])] = &[
        (
            "entity_commands",
            "commands",
            &[
                "damage @e[type=minecraft:zombie,limit=1] 1.5 minecraft:arrow by @e[type=minecraft:player,limit=1] from @e[type=minecraft:player,limit=1]",
                "kill @e[type=#minecraft:undead,name=\"Bob Smith\",limit=1]",
                "attribute @e[type=minecraft:zombie,limit=1] minecraft:max_health modifier add entity_commands:bonus 2 add_value",
                "rotate @e[type=minecraft:zombie,limit=1] facing entity @e[type=minecraft:player,limit=1] eyes",
                "spreadplayers 0 0 4 32 under 100 true @e[type=minecraft:player]",
                "team modify red color reset",
                "team modify red nametagVisibility hideForOtherTeams",
                "waypoint modify @e[type=minecraft:zombie,limit=1] color hex ABCDEF",
                "spawnpoint @e[type=minecraft:player] 0 64 0 90 0",
                "swing @e[type=minecraft:zombie] offhand stab 2s",
            ],
        ),
        (
            "core_commands",
            "commands",
            &[
                "datapack enable \"file/example\" after \"vanilla\"",
                "random reset * 42 false true",
                "loot give @e[type=minecraft:player] loot core_commands:treasure",
                "loot replace block 0 64 0 container.0 fish minecraft:gameplay/fishing 0 62 0",
                "loot replace entity @e[type=minecraft:zombie,limit=1] weapon.mainhand 1 loot minecraft:chests/simple_dungeon",
                "recipe take @e[type=minecraft:player] core_commands:crafted",
            ],
        ),
        (
            "item_components",
            "inventory",
            &[
                "give @s minecraft:stone[minecraft:max_stack_size=99,minecraft:custom_data={group:\"building\"}] 9900",
                "clear @s *[minecraft:custom_data,!minecraft:damage|minecraft:damage=0,minecraft:custom_data~{owner:\"Alex\"},minecraft:count~{min:1,max:64}] 0",
            ],
        ),
        (
            "ui_commands",
            "display",
            &[
                "title @e[type=minecraft:player] title {\"bold\":true,\"color\":\"gold\",\"text\":\"欢迎\"}",
                "title @e[type=minecraft:player] times 10t 3s 1s",
                "bossbar set ui_commands:task players",
                "run bossbar get ui_commands:task players",
                "dialog show @e[type=minecraft:player] ui_commands:links",
            ],
        ),
        (
            "ui_commands",
            "sensory",
            &[
                "particle minecraft:dust{color:16711680,scale:1.0f} ~ ~1 ~ 0.1 0.2 0.1 0.05 12 force @e[type=minecraft:player]",
                "stopsound @e[type=minecraft:player] * minecraft:entity.player.levelup",
                "posteffect list @e[type=minecraft:player,limit=1]",
                "msg @e[type=minecraft:player,limit=1] 任务已更新",
            ],
        ),
        ("ui_commands", "personal", &["teammsg 集合！"]),
        (
            "remaining_commands",
            "commands",
            &[
                "scoreboard players display numberformat @e[type=minecraft:player,limit=1] remaining_commands_points blank",
            ],
        ),
    ];
    for (fixture, function, expected) in cases {
        let output = root.join("target/command-outputs").join(fixture);
        let options = BuildOptions {
            description: "命令产物验收".to_owned(),
            deny_raw: true,
        };
        build_file(&root.join("tests/valid").join(fixture), &output, &options).unwrap();
        let source = fs::read_to_string(
            output.join(format!("data/{fixture}/function/{function}.mcfunction")),
        )
        .unwrap();
        for expected in *expected {
            assert!(
                source.contains(expected),
                "{fixture} 缺少原版命令形状：{expected}\n{source}"
            );
        }
        let before = files(&output);
        build_file(&root.join("tests/valid").join(fixture), &output, &options).unwrap();
        assert_eq!(before, files(&output), "{fixture} 重复构建必须一致");
    }
    let conditions_output = root.join("target/command-outputs/conditions");
    let options = BuildOptions {
        description: "条件短路产物验收".to_owned(),
        deny_raw: true,
    };
    build_file(
        &root.join("tests/valid/conditions"),
        &conditions_output,
        &options,
    )
    .unwrap();
    let short_circuit = fs::read_to_string(
        conditions_output.join("data/conditions_test/function/short_circuit_probe.mcfunction"),
    )
    .unwrap();
    assert!(short_circuit.contains("matches 1 store result score"));
    assert!(short_circuit.contains("matches 0 store result score"));
    assert!(short_circuit.contains("run function conditions_test:mark_side_effect"));
    assert!(
        short_circuit
            .lines()
            .filter(|line| line.contains("function conditions_test:mark_side_effect"))
            .all(|line| line.starts_with("execute if score ")),
        "短路条件右侧的副作用调用必须受左侧标志保护：{short_circuit}"
    );
    let condition_checks = fs::read_to_string(
        conditions_output.join("data/conditions_test/function/checks.mcfunction"),
    )
    .unwrap();
    assert!(condition_checks.contains("stopwatch conditions_test:timer 0.."));
    let advancement_output = root.join("target/command-outputs/advancement");
    build_file(
        &root.join("tests/valid/advancement"),
        &advancement_output,
        &options,
    )
    .unwrap();
    let portal: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(
            advancement_output.join("data/advancement_test/advancement/portal_frame.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let any = portal["requirements"].as_array().unwrap();
    assert_eq!(any.len(), 1, "any 应生成一个 OR 组：{portal}");
    assert_eq!(any[0].as_array().unwrap().len(), 3);
    let all_gate: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(
            advancement_output.join("data/advancement_test/advancement/all_gate.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let all = all_gate["requirements"].as_array().unwrap();
    assert_eq!(all.len(), 2, "all 应生成两个 AND 组：{all_gate}");
    assert!(all.iter().all(|group| group.as_array().unwrap().len() == 1));
    let ui_files = files(&root.join("target/command-outputs/ui_commands"));
    let ui_text = ui_files
        .values()
        .map(|bytes| String::from_utf8_lossy(bytes))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(ui_text.contains("run title @e[scores={"));
    assert!(ui_text.contains("run posteffect add @e[scores={"));
    let output = root.join("target/command-outputs/macros");
    let options = BuildOptions {
        description: "宏产物验收".to_owned(),
        deny_raw: true,
    };
    build_file(&root.join("tests/valid/macros"), &output, &options).unwrap();
    let invoke =
        fs::read_to_string(output.join("data/macro_test/function/invoke.mcfunction")).unwrap();
    assert!(invoke.contains("function macro_test:welcome {label:\"Alex\",x:5,z:0.5d}"));
    assert!(invoke.contains("schedule clear external:plain"));
    let welcome =
        fs::read_to_string(output.join("data/macro_test/function/welcome.mcfunction")).unwrap();
    assert!(
        welcome.contains("matches 1 run tellraw @a"),
        "单命令 if 分支应直接内联：{welcome}"
    );
    let helper = files(&output)
        .into_iter()
        .filter(|(path, _)| path.to_string_lossy().contains("__mcl\\welcome"))
        .map(|(_, bytes)| String::from_utf8(bytes).unwrap())
        .find(|source| source.contains("{\"label\":\"$(label)\",\"x\":$(x),\"z\":$(z)}"))
        .expect("宏循环辅助函数必须转发参数");
    assert_eq!(
        helper.lines().filter(|line| line.starts_with('$')).count(),
        2
    );
    assert!(helper.contains("{\"label\":\"$(label)\",\"x\":$(x),\"z\":$(z)}"));
    let before = files(&output);
    build_file(&root.join("tests/valid/macros"), &output, &options).unwrap();
    assert_eq!(before, files(&output));
    assert!(
        build_file(&root.join("tests/valid/macro_sources"), &output, &options).is_err(),
        "运行期宏参数必须计入严格模式检查"
    );
    let filtered = root.join("target/command-outputs/filtered");
    build_file(
        &root.join("tests/valid/filtered_targets"),
        &filtered,
        &options,
    )
    .unwrap();
    let helpers = files(&filtered);
    let helpers = helpers
        .values()
        .map(|bytes| String::from_utf8_lossy(bytes))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        helpers.contains("run tag @e[scores={"),
        "物品查询应当先捕获再执行多目标命令"
    );
    assert!(helpers.contains("run return fail"));
    assert!(helpers.contains("return run scoreboard players get"));
    assert!(
        !helpers.contains("run tag @s add __"),
        "不能给 tag.list 引入内部标签"
    );
    let data_output = root.join("target/command-outputs/data-filter");
    build_file(&root.join("tests/valid/data_ops"), &data_output, &options).unwrap();
    let data_files = files(&data_output);
    let data_text = data_files
        .values()
        .map(|bytes| String::from_utf8_lossy(bytes))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(data_text.contains("if items entity @s weapon.mainhand minecraft:diamond_sword"));
    assert!(data_text.contains("run data get entity @e[scores={"));
    assert!(data_text.contains("item replace entity @e[type=minecraft:player,limit=1] weapon.mainhand with minecraft:diamond[minecraft:custom_name={text:\"奖励\"}] 2"));
    let world_output = root.join("target/command-outputs/world");
    build_file(&root.join("tests/valid/world"), &world_output, &options).unwrap();
    let world_build =
        fs::read_to_string(world_output.join("data/world_test/function/build.mcfunction")).unwrap();
    for expected in [
        "clone 0 64 0 4 64 4 10 64 0 strict masked force",
        "clone 0 64 0 4 64 4 10 64 0 replace force",
        "clone from minecraft:the_nether 0 64 0 4 64 4 to minecraft:overworld 10 64 0",
    ] {
        assert!(
            world_build.contains(expected),
            "缺少 clone 形状：{expected}"
        );
    }
    let world_environment =
        fs::read_to_string(world_output.join("data/world_test/function/environment.mcfunction"))
            .unwrap();
    assert!(world_environment.contains("worldborder center 10.0 20.0"));
    let world_journey =
        fs::read_to_string(world_output.join("data/world_test/function/journey.mcfunction"))
            .unwrap();
    assert!(world_journey.contains("tp @s 2.0 64.0 4.0"));
}
