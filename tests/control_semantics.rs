use mclang::{BuildOptions, build_file};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

fn function(root: &Path, name: &str) -> String {
    fs::read_to_string(root.join(format!("data/control_semantics/function/{name}.mcfunction")))
        .unwrap()
}

fn owner_commands(root: &Path, owner: &str) -> String {
    let mut text = function(root, owner);
    let helpers = root.join(format!("data/control_semantics/function/__mcl/{owner}"));
    if helpers.exists() {
        let mut paths = fs::read_dir(helpers)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<PathBuf>>();
        paths.sort();
        for path in paths {
            text.push_str(&fs::read_to_string(path).unwrap());
        }
    }
    text
}

fn generated_files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut pending = vec![root.to_owned()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                files.insert(
                    path.strip_prefix(root).unwrap().to_owned(),
                    fs::read(path).unwrap(),
                );
            }
        }
    }
    files
}

#[test]
fn control_flow_effects_and_small_command_shapes() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = repo.join("target/control-semantics-test");
    build_file(
        &repo.join("tests/valid/control_semantics"),
        &output,
        &BuildOptions {
            description: "控制流语义回归".to_owned(),
            deny_raw: true,
        },
    )
    .unwrap();

    let absorbed = owner_commands(&output, "absorbed_conditions");
    assert_eq!(
        absorbed
            .matches("run function control_semantics:side_effect")
            .count(),
        4,
        "if/while 的右侧吸收常量不能删除左侧调用：{absorbed}"
    );
    assert!(absorbed.contains("scoreboard players set #loop_state_"));

    let bounds = function(&output, "dynamic_bounds");
    let lower = bounds
        .find("function control_semantics:lower_bound")
        .unwrap();
    let upper = bounds
        .find("function control_semantics:upper_bound")
        .unwrap();
    assert!(lower < upper, "for 边界须按源码顺序求值：{bounds}");
    assert_eq!(
        bounds
            .matches("function control_semantics:upper_bound")
            .count(),
        1
    );
    assert!(!owner_commands(&output, "dynamic_bounds").contains("#loop_state_"));

    let chain = owner_commands(&output, "condition_chain");
    assert!(chain.contains(
        "if function control_semantics:false_condition if function control_semantics:side_effect"
    ));
    assert!(
        chain.lines().any(|line| {
            line.contains("if function control_semantics:side_effect run")
                && line.starts_with("execute if function control_semantics:__mcl/")
        }),
        "后一个动态条件必须受前一个条件保护：{chain}"
    );
    let dynamic_root = function(&output, "condition_chain_dynamic");
    let condition_helpers = dynamic_root
        .split("if function control_semantics:")
        .skip(1)
        .map(|part| part.split_whitespace().next().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        condition_helpers.len(),
        2,
        "动态条件必须参与原版条件链：{dynamic_root}"
    );
    let entry = function(&output, condition_helpers[0]);
    assert!(entry.contains("function control_semantics:false_condition"));
    assert!(!entry.contains("function control_semantics:side_effect"));
    assert!(entry.contains("return run scoreboard players get"));
    assert!(
        function(&output, condition_helpers[1]).contains("function control_semantics:side_effect")
    );

    let one_branch = function(&output, "simple_if");
    assert!(one_branch.contains("execute if score "));
    assert!(one_branch.contains("matches ..9 run scoreboard players add"));
    assert!(one_branch.contains("execute if block 0 64 0 minecraft:stone run"));
    assert!(
        !one_branch.contains("#t"),
        "简单条件不需要 flag：{one_branch}"
    );
    assert_eq!(
        one_branch
            .matches("scoreboard players add #v_counter")
            .count(),
        3
    );
    let atomic = function(&output, "atomic_if");
    assert!(atomic.contains("execute if entity @e[type=minecraft:zombie,limit=1] run"));
    assert!(atomic.contains("execute if predicate control_semantics:always_true run"));
    assert!(!atomic.contains("#t"));

    let simple_while = owner_commands(&output, "simple_while");
    assert!(
        simple_while.contains("matches ..2 run function control_semantics:__mcl/simple_while/")
    );
    assert!(!simple_while.contains("#t"));
    assert!(!simple_while.contains("#loop_state_"));
    let simple_for = owner_commands(&output, "simple_for");
    assert!(
        simple_for
            .lines()
            .any(|line| line.starts_with("scoreboard players add #v_counter"))
    );
    assert!(!simple_for.contains("matches ..2 run scoreboard players add"));
    assert!(!simple_for.contains("#loop_state_"));
    assert_eq!(
        fs::read_dir(output.join("data/control_semantics/function/__mcl/simple_for"))
            .unwrap()
            .count(),
        1,
        "单命令 for 循环体应直接进入循环命令"
    );

    let small = function(&output, "small_blocks");
    assert!(
        small.contains("execute as @e[type=minecraft:zombie,limit=1] at @s run tag @s add seen")
    );
    assert!(small.contains("execute in minecraft:overworld run scoreboard players add"));
    assert!(small.contains("execute summon minecraft:armor_stand run tag @s add new"));
    assert!(!small.contains("function control_semantics:__mcl/"));
    assert!(!small.contains("execute run "));

    let stored = function(&output, "store_one");
    assert!(stored.contains(
        "execute store result score @s control_semantics_stored run function control_semantics:side_effect"
    ));
    assert!(!stored.contains("function control_semantics:__mcl/"));
    let multi_store = function(&output, "store_multi");
    let multi_store_helper = owner_commands(&output, "store_multi");
    assert!(multi_store.contains("function control_semantics:__mcl/store_multi/"));
    assert!(multi_store_helper.contains("scoreboard players add #v_counter"));
    assert!(multi_store_helper.contains(
        "execute store result score @s control_semantics_stored run function control_semantics:side_effect"
    ));

    let nested = owner_commands(&output, "nested_no_outer_jump");
    let states = nested
        .split_whitespace()
        .filter(|token| token.starts_with("#loop_state_"))
        .collect::<HashSet<_>>();
    assert_eq!(states.len(), 1, "只有内层循环需要跳转状态：{nested}");
    assert!(!function(&output, "nested_no_outer_jump").contains("#loop_state_"));

    let arithmetic = function(&output, "arithmetic");
    let load = function(&output, "__mcl/load");
    assert!(
        !arithmetic.contains("#t"),
        "算术赋值应直接写到目标：{arithmetic}"
    );
    assert_eq!(load.matches("scoreboard players set #c_3 ").count(), 1);
    assert_eq!(arithmetic.matches("#c_3 ").count(), 3);
    assert!(!arithmetic.contains("scoreboard players set #c_"));

    let observed = function(&output, "observe_left_before_call");
    let lines = observed.lines().collect::<Vec<_>>();
    let calls = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.contains("run function control_semantics:side_effect"))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 3);
    assert!(
        calls.iter().all(|index| {
            lines[index - 1].starts_with("scoreboard players operation #t")
                && lines[index - 1].contains(" = #v_counter ")
        }),
        "右侧调用前必须冻结已求值的左侧：{observed}"
    );

    let if_else = function(&output, "if_else_once");
    assert_eq!(
        if_else
            .matches("run function control_semantics:side_effect")
            .count(),
        1
    );
    assert!(if_else.contains("matches 1 run scoreboard players add"));
    assert!(if_else.contains("matches 0 run scoreboard players add"));
    let returned = function(&output, "return_execute");
    assert!(returned.contains("return run function control_semantics:__mcl/return_execute/"));
    assert!(owner_commands(&output, "return_execute").contains("say result boundary"));

    for owner in ["forked_each", "forked_nested_if"] {
        let root = function(&output, owner);
        let entry = root
            .lines()
            .find(|line| line.starts_with("execute "))
            .unwrap();
        let helper = entry
            .split_once("run function control_semantics:")
            .unwrap()
            .1;
        assert!(
            !entry.contains("if score"),
            "多实体条件不能提前求值：{entry}"
        );
        let body = function(&output, helper);
        assert!(
            body.contains("matches 0 run scoreboard players add #v_counter"),
            "每个实体必须独立求值并执行：{body}"
        );
    }

    for owner in ["forked_execute", "forked_at", "forked_passengers"] {
        let root = function(&output, owner);
        assert!(
            root.lines().any(|line| line.starts_with("execute ")
                && line.contains("if score #v_counter ")
                && line.contains("matches 0 run scoreboard players add #v_counter")),
            "execute 条件必须先筛选全部来源，再执行块体：{root}"
        );
    }
    let forked_dynamic = function(&output, "forked_dynamic_chain");
    assert!(
        forked_dynamic
            .contains("execute as @e[type=minecraft:zombie] if function control_semantics:__mcl/")
    );
    assert!(
        forked_dynamic
            .contains("if function control_semantics:side_effect run scoreboard players add")
    );

    let extreme = function(&output, "empty_extreme_ranges");
    assert!(!owner_commands(&output, "empty_extreme_ranges").contains("-2147483649"));
    assert_eq!(
        extreme
            .lines()
            .filter(
                |line| line.contains("run function control_semantics:__mcl/empty_extreme_ranges/")
            )
            .count(),
        2
    );
    assert!(
        !extreme.lines().any(|line| line.starts_with("function ")),
        "空动态区间必须在首次进入循环前检查：{extreme}"
    );
    let signed = function(&output, "signed_constant_conditions");
    assert_eq!(
        signed
            .lines()
            .filter(|line| line.starts_with("scoreboard "))
            .count(),
        5,
        "常量运算应与原版 floorDiv/floorMod 一致：{signed}"
    );
    assert!(!signed.contains("execute "));
    let macro_root = function(&output, "macro_condition");
    let snapshot = "$data modify storage control_semantics:__mcl/macro_conditions/macro_condition arguments set value";
    assert!(
        macro_root.contains(snapshot),
        "宏条件需要保留调用参数：{macro_root}"
    );
    let wrapper = macro_root
        .split("if function control_semantics:")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap();
    let wrapper_text = function(&output, wrapper);
    assert!(wrapper_text.contains("return run function control_semantics:__mcl/macro_condition/"));
    assert!(wrapper_text.contains(
        "with storage control_semantics:__mcl/macro_conditions/macro_condition arguments"
    ));
    assert!(
        !wrapper_text.contains("$("),
        "原版 function 条件只能调用普通函数"
    );

    let unconditional = function(&output, "forked_execute_unconditional");
    assert!(
        unconditional
            .contains("execute as @e[type=minecraft:zombie] run scoreboard players add #v_counter")
    );
    assert!(
        !unconditional.contains("function control_semantics:__mcl/"),
        "无条件多来源 execute 的单命令块不需要辅助函数：{unconditional}"
    );

    let assignments = owner_commands(&output, "stored_assignments");
    let stores = assignments
        .lines()
        .filter(|line| {
            line.contains("store result score @s control_semantics_stored")
                || line.contains("store success score @s control_semantics_stored")
        })
        .collect::<Vec<_>>();
    assert_eq!(stores.len(), 4);
    assert!(
        stores
            .iter()
            .all(|line| line.contains("run scoreboard players operation #v_counter ")),
        "store 必须捕获最后的赋值，不能捕获先前命令或右侧调用：{assignments}"
    );
    assert_eq!(
        stores
            .iter()
            .filter(|line| line.contains(" = #v_counter "))
            .count(),
        3
    );
    assert_eq!(
        assignments
            .matches("run function control_semantics:failed_result")
            .count(),
        1
    );

    let negated = owner_commands(&output, "negated_queries");
    for atom in ["data", "block", "items", "slots"] {
        assert!(
            !negated.contains(&format!("unless {atom} ")),
            "可能失败的条件必须先捕获布尔值再取反：{negated}"
        );
        assert!(negated.contains(&format!("if {atom} ")));
    }
    assert_eq!(negated.matches("run execute if data entity").count(), 4);

    let first_build = generated_files(&output);
    build_file(
        &repo.join("tests/valid/control_semantics"),
        &output,
        &BuildOptions {
            description: "控制流语义回归".to_owned(),
            deny_raw: true,
        },
    )
    .unwrap();
    assert_eq!(first_build, generated_files(&output));
}

#[test]
fn raw_return_keeps_a_function_boundary() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = repo.join("target/control-raw-source");
    fs::create_dir_all(&source).unwrap();
    fs::write(
        source.join("main.mcl"),
        "namespace raw_boundary;\nscore ticks = 0;\nfn probe() {\n    while ticks < 3 {\n        run \"return 0\";\n    }\n    execute if ticks < 3 {\n        run \"return 0\";\n    }\n}\n",
    )
    .unwrap();
    let output = repo.join("target/control-raw-output");
    build_file(
        &source,
        &output,
        &BuildOptions {
            description: "return 边界回归".to_owned(),
            deny_raw: false,
        },
    )
    .unwrap();
    let caller =
        fs::read_to_string(output.join("data/raw_boundary/function/probe.mcfunction")).unwrap();
    assert!(!caller.contains("run return 0"));
    let helpers = output.join("data/raw_boundary/function/__mcl/probe");
    let helper_text = fs::read_dir(helpers)
        .unwrap()
        .map(|entry| fs::read_to_string(entry.unwrap().path()).unwrap())
        .collect::<Vec<_>>();
    assert!(helper_text.iter().any(|text| text.contains("return 0")));
    assert!(helper_text.iter().any(|text| {
        text.contains("function raw_boundary:__mcl/probe/") && !text.contains("return 0")
    }));
    assert!(
        build_file(
            &source,
            &output,
            &BuildOptions {
                description: "return 边界回归".to_owned(),
                deny_raw: true,
            },
        )
        .is_err()
    );
}
