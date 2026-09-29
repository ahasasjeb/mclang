use mclang::{BuildOptions, build_file};
use std::fs;
use std::path::Path;

fn function(root: &Path, name: &str) -> String {
    fs::read_to_string(root.join(format!("data/optimizations/function/{name}.mcfunction"))).unwrap()
}

fn commands(source: &str) -> Vec<&str> {
    source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

#[test]
#[allow(clippy::redundant_comparisons)] // The source deliberately contains a provably redundant guard.
fn nested_guards_fold_constants_and_preserve_effects() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = repo.join("target/control-optimization-nested-guards");
    build_file(
        &repo.join("tests/valid/optimizations"),
        &output,
        &BuildOptions {
            description: "控制流优化回归".to_owned(),
            deny_raw: true,
        },
    )
    .unwrap();

    let nested = function(&output, "nested");
    let nested_commands = commands(&nested);
    assert_eq!(
        nested_commands.len(),
        1,
        "三层 guard 应当合成一条：{nested}"
    );
    assert_eq!(nested_commands[0].matches("if score ").count(), 3);
    assert!(
        !nested.contains("run execute"),
        "不得嵌套执行 execute：{nested}"
    );

    let ranges = function(&output, "nested_ranges");
    let ranges_commands = commands(&ranges);
    assert_eq!(
        ranges_commands.len(),
        1,
        "连续范围 guard 应当合并：{ranges}"
    );
    assert!(ranges.contains("matches 11.."), "保留 x > 10：{ranges}");
    assert!(ranges.contains("unless score "), "保留 x != 13：{ranges}");
    assert_eq!(ranges.matches("matches 11..").count(), 1);
    assert_eq!(ranges.matches("matches 13").count(), 1);
    for value in [i32::MIN, -1, 0, 1, 5, 10, 11, 12, 13, 14, i32::MAX] {
        assert_eq!(
            execute_score_guard(ranges_commands[0], value),
            value > 10 && value > 5 && value != 13,
            "生成的 execute guard 与 MCL 对 value={value} 的结果不一致：{ranges}"
        );
    }

    for name in ["impossible_range", "contradictory_condition"] {
        let output = function(&output, name);
        assert!(
            commands(&output).is_empty(),
            "矛盾范围不应生成运行时命令：{name}: {output}"
        );
    }

    let equality = function(&output, "equality_ranges");
    let equality_commands = commands(&equality);
    assert_eq!(equality_commands.len(), 1);
    assert_eq!(
        equality.matches("matches ").count(),
        1,
        "== 推导出的比较应消失：{equality}"
    );
    for value in [i32::MIN, 0, 4, 5, 6, i32::MAX] {
        assert_eq!(
            execute_score_guard(equality_commands[0], value),
            value == 5 && value >= 5 && value > 4 && value != 6,
            "相等范围传播不一致：value={value}, {equality}"
        );
    }
    assert!(commands(&function(&output, "impossible_equality")).is_empty());

    let upper_range = function(&output, "upper_range");
    let upper_commands = commands(&upper_range);
    assert_eq!(upper_commands.len(), 1);
    assert_eq!(
        upper_range.matches("matches ").count(),
        1,
        "<= 应推出 < 后续整数：{upper_range}"
    );
    for value in [i32::MIN, 4, 5, 6, i32::MAX] {
        assert_eq!(
            execute_score_guard(upper_commands[0], value),
            value <= 5 && value < 6,
            "上界范围传播不一致：value={value}, {upper_range}"
        );
    }

    let constants = function(&output, "constants");
    let constant_commands = commands(&constants);
    assert_eq!(
        constant_commands.len(),
        2,
        "常量分支已在编译期选择：{constants}"
    );
    assert!(!constants.contains("execute "));
    assert!(constant_commands[0].ends_with(" 1"));
    assert!(constant_commands[1].ends_with(" 2"));
    let booleans = function(&output, "boolean_identities");
    assert!(booleans.contains("execute if score "));
    assert!(
        !booleans.contains("#t"),
        "x && true / x || false 不需临时值：{booleans}"
    );

    let effectful = owner_commands(&output, "effectful_guards");
    assert_eq!(
        effectful
            .matches("function optimizations:repeated_effect")
            .count(),
        2,
        "函数条件含副作用，不能合并或删除：{effectful}"
    );
    assert!(
        effectful.lines().any(|line| {
            line.starts_with("execute if score #t")
                && line.contains("store result score #t")
                && line.contains("run function optimizations:repeated_effect")
        }),
        "内层函数条件必须受外层结果守卫：{effectful}"
    );
    assert!(
        !output
            .join("data/optimizations/function/__mcl/effectful_guards")
            .exists(),
        "嵌套 then 链不再为每层生成辅助函数：{effectful}"
    );

    let effectful_contradiction = function(&output, "effectful_contradiction");
    assert!(
        effectful_contradiction.contains("if function optimizations:repeated_effect if score "),
        "矛盾分支仍须执行前面的函数条件：{effectful_contradiction}"
    );

    let mutated = function(&output, "mutate_between_guards");
    assert!(mutated.contains("matches 11.. run function optimizations:__mcl/"));
    let mutation_helper = output.join("data/optimizations/function/__mcl/mutate_between_guards");
    let mutation_body = fs::read_dir(mutation_helper)
        .unwrap()
        .map(|entry| fs::read_to_string(entry.unwrap().path()).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(mutation_body.contains("scoreboard players set #p_"));
    assert!(
        mutation_body.contains("matches ..4 run scoreboard players add"),
        "条件变量修改后，内层条件必须重新读取：{mutation_body}"
    );

    let short_circuit = function(&output, "short_circuit_effect");
    assert!(short_circuit.contains("matches ..-1"));
    assert!(short_circuit.contains("run function optimizations:repeated_effect"));
    assert!(
        short_circuit
            .lines()
            .filter(|line| line.contains("function optimizations:repeated_effect"))
            .all(|line| line.starts_with("execute if score ")),
        "&& 右侧调用仍须受左侧 guard 保护：{short_circuit}"
    );

    let returned = function(&output, "return_stops_block");
    assert!(returned.contains("return 0"));
    let returned_commands = commands(&returned);
    assert_eq!(
        returned_commands.len(),
        2,
        "return 后的死写入应删除：{returned}"
    );
    assert!(returned_commands.iter().any(|line| {
        line.starts_with("scoreboard players add #v_counter ") && line.ends_with(" 1")
    }));
    assert!(!returned_commands.iter().any(|line| line.ends_with(" 2")));

    let dead_assignment = function(&output, "dead_assignment");
    let dead_assignment_commands = commands(&dead_assignment);
    assert_eq!(dead_assignment_commands.len(), 1);
    assert!(dead_assignment_commands[0].starts_with("scoreboard players set #v_x "));
    assert!(dead_assignment_commands[0].ends_with(" 2"));
}

/// 编译期能证明为真的条件不能连带丢掉它的副作用：
/// `f() || 已知为真` 仍然必须先调用 `f()`。
#[test]
fn proven_conditions_keep_their_side_effects() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = repo.join("target/control-optimization-proven-effects");
    build_file(
        &repo.join("tests/valid/optimizations"),
        &output,
        &BuildOptions {
            description: "控制流优化回归".to_owned(),
            deny_raw: true,
        },
    )
    .unwrap();

    let left_effect = owner_commands(&output, "effectful_proven_true");
    assert_eq!(
        left_effect
            .matches("function optimizations:repeated_effect")
            .count(),
        1,
        "`f() || 已知为真` 必须保留左侧调用：{left_effect}"
    );
    assert!(
        left_effect
            .lines()
            .any(|line| line.contains("if score #p_") && line.contains("matches 11.. run function")),
        "调用必须只在外层 guard 成立时发生：{left_effect}"
    );

    let right_effect = owner_commands(&output, "effectful_proven_true_right");
    assert!(
        !right_effect.contains("function optimizations:repeated_effect"),
        "`已知为真 || f()` 按短路语义不会调用右侧：{right_effect}"
    );

    for owner in ["mutating_guards_nested", "mutating_guards_and"] {
        let mutation = owner_commands(&output, owner);
        assert_eq!(
            mutation.matches("matches 11..").count(),
            2,
            "函数条件可能修改计分项，调用后的 guard 必须重新读取：{owner}: {mutation}"
        );
        assert_eq!(
            mutation
                .matches("function optimizations:reset_guard_score")
                .count(),
            1,
            "修改计分项的函数条件必须且只需执行一次：{owner}: {mutation}"
        );
    }

    let contradiction = owner_commands(&output, "effectful_contradiction");
    assert_eq!(
        contradiction
            .matches("function optimizations:repeated_effect")
            .count(),
        1,
        "矛盾分支仍须执行条件里的调用：{contradiction}"
    );
}

/// 同一纯条件在一次 execute 条件链里只应出现一次。
#[test]
fn duplicate_pure_guards_are_merged() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = repo.join("target/control-optimization-duplicate-guards");
    build_file(
        &repo.join("tests/valid/optimizations"),
        &output,
        &BuildOptions {
            description: "控制流优化回归".to_owned(),
            deny_raw: true,
        },
    )
    .unwrap();

    let duplicates = function(&output, "duplicate_guards");
    let duplicated = commands(&duplicates);
    assert_eq!(duplicated.len(), 2, "{duplicates}");
    assert_eq!(
        duplicates
            .matches("run scoreboard players add #v_counter")
            .count(),
        2,
        "两个分支各自只保留一条命令：{duplicates}"
    );
    assert!(
        !duplicates.contains("run execute "),
        "重复条件不应留下嵌套 execute：{duplicates}"
    );

    let effectful = function(&output, "duplicate_effectful_guards");
    assert_eq!(
        effectful
            .matches("if function optimizations:repeated_effect")
            .count(),
        2,
        "有副作用的同一个条件必须求值两次：{effectful}"
    );
}

/// Loop entry edges carry range facts into their bodies. This removes guards
/// already enforced by a counted loop, while a source write or a potentially
/// overflowing induction step must keep the runtime test.
#[test]
fn loop_ranges_propagate_without_crossing_mutations_or_overflow() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = repo.join("target/control-optimization-loop-ranges");
    build_file(
        &repo.join("tests/valid/optimizations"),
        &output,
        &BuildOptions {
            description: "控制流优化回归".to_owned(),
            deny_raw: true,
        },
    )
    .unwrap();

    let counted_for = owner_commands(&output, "for_loop_ranges");
    assert!(
        !counted_for.contains("matches 0..") && !counted_for.contains("matches 11.."),
        "for 的循环范围应删除体内冗余与矛盾 guard：{counted_for}"
    );
    assert_eq!(
        counted_for.matches("matches ..3").count(),
        1,
        "只应保留循环回边的上界检查：{counted_for}"
    );
    assert!(
        !counted_for.contains(" 100"),
        "矛盾分支应消失：{counted_for}"
    );

    let nested = owner_commands(&output, "nested_for_loop_ranges");
    assert!(!nested.contains("matches -2.."), "外层下界已知：{nested}");
    assert!(!nested.contains("matches 1.."), "内层下界已知：{nested}");
    assert_eq!(nested.matches("matches ..2").count(), 1, "{nested}");
    assert_eq!(nested.matches("matches ..3").count(), 1, "{nested}");

    let mutated = owner_commands(&output, "mutated_for_loop_range");
    assert_eq!(
        mutated.matches("matches ..3").count(),
        2,
        "源码改写循环变量后，体内 guard 与循环回边都必须保留：{mutated}"
    );

    let counted_while = owner_commands(&output, "counted_while_ranges");
    assert!(
        !counted_while.contains("matches 0.."),
        "单调下界已知：{counted_while}"
    );
    assert_eq!(
        counted_while.matches("matches ..3").count(),
        1,
        "while 条件本身只应在循环入口求值：{counted_while}"
    );
    assert!(
        counted_while
            .contains("matches ..3 run function optimizations:__mcl/counted_while_ranges/"),
        "已知首次成立的计数 while 应使用尾部条件递归：{counted_while}"
    );
    assert_eq!(
        fs::read_dir(output.join("data/optimizations/function/__mcl/counted_while_ranges"))
            .unwrap()
            .count(),
        1,
        "计数 while 不需要单独的条件转发 helper"
    );

    let overflowing = owner_commands(&output, "overflowing_while_range");
    assert!(
        overflowing.contains("matches 2147483646.."),
        "可能溢出的步进不能传播错误下界：{overflowing}"
    );

    let known_empty = owner_commands(&output, "known_empty_while");
    assert_eq!(
        commands(&known_empty).len(),
        1,
        "紧邻常量赋值已使首次条件为假，while 本身不应生成命令：{known_empty}"
    );
    assert!(!known_empty.contains("function optimizations:__mcl/"));
}

/// `&&` 右侧是比较时，编译期不再为它单独分配标志计分项；
/// 用解释器在边界值上核对生成的命令与源码语义一致。
#[test]
fn and_operand_fusion_matches_source_semantics() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = repo.join("target/control-optimization-and-fusion");
    build_file(
        &repo.join("tests/valid/optimizations"),
        &output,
        &BuildOptions {
            description: "控制流优化回归".to_owned(),
            deny_raw: true,
        },
    )
    .unwrap();

    let values = [i32::MIN, -1, 0, 1, 3, 4, 5, 6, 7, 8, i32::MAX];
    // `left + 1` 在 i32::MAX 上会触及整数溢出语义，那是另一条规则；
    // 这里只核对 `&&` 合并在边界附近的真假。
    let arithmetic = [i32::MIN, -1, 0, 1, 3, 4, 5, 6, 7, 8, i32::MAX - 1];

    let computed_text = function(&output, "and_fusion_computed");
    let computed = commands(&computed_text);
    assert_eq!(computed.len(), 6, "右侧比较应折进合并命令：{computed:#?}");
    for left in arithmetic {
        for right in values {
            assert_eq!(
                run_guard(&computed, left, right),
                i32::from(left > 3 && left + 1 < right),
                "`left > 3 && left + 1 < right` 不一致：left={left}, right={right}"
            );
        }
    }

    let reversed_text = function(&output, "and_fusion_reversed");
    let reversed = commands(&reversed_text);
    assert_eq!(reversed.len(), 6, "常量在左侧时同样折进：{reversed:#?}");
    for left in arithmetic {
        for right in values {
            assert_eq!(
                run_guard(&reversed, left, right),
                i32::from(left > 3 && 7 > left + 1),
                "`left > 3 && 7 > left + 1` 不一致：left={left}"
            );
        }
    }

    let scores_text = function(&output, "and_fusion_scores");
    let scores = commands(&scores_text);
    assert_eq!(scores.len(), 1, "可原生表达的比较直接进条件链：{scores:#?}");
    for left in values {
        for right in values {
            assert_eq!(
                run_guard(&scores, left, right),
                i32::from(left > 3 && left < right),
                "`left > 3 && left < right` 不一致：left={left}, right={right}"
            );
        }
    }
}

/// 链式扁平化与「计分单元直接比较」的语义回归：
/// 用解释器在边界取值上对比源码语义与生成命令（含辅助函数）的语义。
#[test]
fn chain_dispatch_and_cell_operands_match_source_semantics() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = repo.join("target/control-optimization-chain-dispatch");
    build_file(
        &repo.join("tests/valid/optimizations"),
        &output,
        &BuildOptions {
            description: "控制流优化回归".to_owned(),
            deny_raw: true,
        },
    )
    .unwrap();

    // 用户计分项没有记录时，scoreboard.get 的表达式值是 0；必须先捕获到
    // 已预置为 0 的临时项，不能直接用会把“缺失”视为不匹配的原生条件。
    let cells = function(&output, "dispatch_cells");
    assert_eq!(commands(&cells).len(), 5, "{cells}");
    assert_eq!(
        cells.matches("scoreboard players set #t").count(),
        2,
        "两个读取各预置一次 0：{cells}"
    );
    assert_eq!(
        cells.matches("store result score #t").count(),
        2,
        "两个用户计分项各捕获一次：{cells}"
    );
    let cells_else = function(&output, "dispatch_cells_else");
    assert_eq!(commands(&cells_else).len(), 6, "{cells_else}");
    assert!(
        cells_else.contains("store result score #t"),
        "缺失安全的读取需要临时项：{cells_else}"
    );
    assert_eq!(cells_else.matches("run scoreboard players add").count(), 2);

    // 分支体调用函数：用户计分项可能被改写，两个操作数各冻结一次，
    // 两个分支测试同一对冻结值，且不再生成反转标志。
    let effectful = function(&output, "dispatch_cells_else_effectful");
    let effectful_commands = commands(&effectful);
    assert_eq!(effectful_commands.len(), 6, "{effectful}");
    assert!(
        !effectful.contains("matches 0"),
        "不生成反转标志：{effectful}"
    );
    assert_eq!(
        effectful.matches("execute store result score #t").count(),
        2,
        "两个操作数各捕获一次：{effectful}"
    );

    // 自复制不能使用“先清零目标再复制”的快速路径，否则来源会在读取前丢失。
    let self_copy = function(&output, "copy_cell_to_itself");
    assert_eq!(commands(&self_copy).len(), 3, "{self_copy}");
    assert!(
        !self_copy.contains("scoreboard players set @s optimizations_box_a 0"),
        "不能在读取前清零同一个源计分项：{self_copy}"
    );
    assert!(
        self_copy.contains("store result score #t")
            && self_copy.contains("scoreboard players get @s optimizations_box_a")
            && self_copy.contains("scoreboard players operation @s optimizations_box_a = #t"),
        "自复制应通过临时项保留原值：{self_copy}"
    );

    let helper_root = output.join("data/optimizations/function");
    let helper_dir = helper_root.clone();

    // 4 层纯条件链：每层一条命令，不再为 else 生成辅助函数。
    let flat = function(&output, "else_chain_flat");
    assert_eq!(commands(&flat).len(), 4, "{flat}");
    assert!(
        !helper_root.join("__mcl/else_chain_flat").exists(),
        "链式分派不再生成辅助函数：{flat}"
    );
    let flat_holder = first_param(&flat);
    for (value, expected) in [(-1, 4), (0, 4), (1, 1), (2, 2), (3, 3), (4, 4), (5, 4)] {
        let mut simulator = Simulator::new(&[(flat_holder.as_str(), value)]);
        simulator.run(&commands(&flat));
        assert_eq!(
            simulator.get("#v_counter"),
            expected,
            "else_chain_flat 在 value={value} 上不一致：{flat}"
        );
    }

    // 分支体改写条件读取的计分项：分派必须测试冻结后的旧值，
    // 否则 value=1 会先写 9，再被 else 层重复计入。
    let frozen = function(&output, "else_chain_frozen");
    assert!(
        frozen.contains("scoreboard players operation #t"),
        "{frozen}"
    );
    assert!(
        !helper_root.join("__mcl/else_chain_frozen").exists(),
        "冻结链同样不生成辅助函数：{frozen}"
    );
    let frozen_holder = first_param(&frozen);
    for (value, expected) in [(1, 0), (2, 2), (7, 3)] {
        let mut simulator = Simulator::new(&[(frozen_holder.as_str(), value)]);
        simulator.run(&commands(&frozen));
        assert_eq!(
            simulator.get("#v_counter"),
            expected,
            "else_chain_frozen 在 value={value} 上不一致：{frozen}"
        );
    }

    // 嵌套 then 链的叶子带 else：外层条件并进守卫，两层共两条命令。
    let nested = function(&output, "nested_then_flat");
    assert_eq!(commands(&nested).len(), 2, "{nested}");
    assert!(
        !helper_root.join("__mcl/nested_then_flat").exists(),
        "{nested}"
    );
    let nested_holder = first_param(&nested);
    for (value, expected) in [(10, 0), (11, 2), (20, 2), (21, 1)] {
        let mut simulator = Simulator::new(&[(nested_holder.as_str(), value)]);
        simulator.run(&commands(&nested));
        assert_eq!(
            simulator.get("#v_counter"),
            expected,
            "nested_then_flat 在 value={value} 上不一致：{nested}"
        );
    }

    // 函数条件链：只有分支体需要辅助函数；条件本身短路，只调用一次。
    let effectful_chain = function(&output, "else_chain_effectful");
    assert!(
        !helper_root.join("__mcl/else_chain_effectful").exists(),
        "函数条件不再为每层生成辅助函数：{effectful_chain}"
    );
    let chain_holder = first_param(&effectful_chain);
    for (value, returns, expected, calls) in
        [(1, 1, 1, 0), (2, 1, 2, 1), (3, 1, 2, 1), (2, 0, 3, 1)]
    {
        let mut simulator = Simulator::new(&[(chain_holder.as_str(), value)])
            .with_functions(&[("repeated_effect", returns)]);
        simulator.run(&commands(&effectful_chain));
        assert_eq!(
            simulator.get("#v_counter"),
            expected,
            "else_chain_effectful 在 value={value}, repeated_effect={returns} 上不一致：{effectful_chain}"
        );
        assert_eq!(
            simulator.calls("repeated_effect"),
            calls,
            "else 层只在前面都不成立时求值：value={value}, {effectful_chain}"
        );
    }

    // 分支体中的表达式调用可能改写外层全局分数；两个分支必须测试冻结值。
    let expression_effect = function(&output, "branch_expression_mutates_guard");
    assert!(
        expression_effect.contains("scoreboard players operation #t")
            && expression_effect.contains("= #v_x"),
        "表达式调用前应冻结外层条件：{expression_effect}"
    );
    assert_eq!(
        expression_effect
            .matches("function optimizations:reset_guard_score")
            .count(),
        1,
        "表达式调用只能执行一次：{expression_effect}"
    );

    // 后续 else 条件本身也可能改写先前守卫读取的全局分数。
    let condition_effect = function(&output, "else_condition_mutates_guard");
    assert!(
        condition_effect.contains("scoreboard players operation #t")
            && condition_effect.contains("= #v_x"),
        "后续条件求值前应冻结前层条件：{condition_effect}"
    );
    assert_eq!(
        condition_effect
            .matches("function optimizations:reset_guard_score")
            .count(),
        1,
        "后续条件调用只能执行一次：{condition_effect}"
    );

    // preserve_command_result 路径必须经已初始化临时项复制；否则源记录缺失时
    // 最后一条 operation 会失败，store.success 会从原来的 1 变成 0。
    let stored_copy = owner_commands(&output, "copy_cell_store_success");
    assert!(
        stored_copy.contains("scoreboard players get @s optimizations_box_a")
            && stored_copy.contains("scoreboard players operation @s optimizations_box_b = #t"),
        "store.success 下的读取应保留成功复制语义：{stored_copy}"
    );
    assert!(
        !stored_copy.contains(
            "scoreboard players operation @s optimizations_box_b = @s optimizations_box_a"
        ),
        "保留命令结果时不能使用可能失败的直接复制：{stored_copy}"
    );

    // 超过守卫子句上限：剩余链回退成带守卫的辅助函数，语义不变。
    let capped = function(&output, "guard_cap");
    assert!(
        helper_root.join("__mcl/guard_cap/0.mcfunction").is_file(),
        "深链需要回退辅助函数：{capped}"
    );
    let capped_holder = first_param(&capped);
    for value in -1..12 {
        let mut simulator =
            Simulator::new(&[(capped_holder.as_str(), value)]).with_helpers(helper_dir.clone());
        simulator.run(&commands(&capped));
        let expected = if (1..=10).contains(&value) { value } else { 10 };
        assert_eq!(
            simulator.get("#v_counter"),
            expected,
            "guard_cap 在 value={value} 上不一致：{capped}"
        );
    }
}

/// 生成文本里第一个参数持有者（`#p_...`），用于给解释器设置入参。
fn first_param(source: &str) -> String {
    source
        .split_whitespace()
        .find(|token| token.starts_with("#p_"))
        .unwrap_or_else(|| panic!("没有找到参数持有者：{source}"))
        .to_owned()
}

/// 在 `#v_left`/`#v_right` 取给定值的前提下执行生成的函数体，
/// 返回 `#v_counter` 的增量（0 或 1）。
fn run_guard(commands: &[&str], left: i32, right: i32) -> i32 {
    let mut simulator = Simulator::new(&[("#v_left", left), ("#v_right", right)]);
    simulator.run(commands);
    simulator.get("#v_counter")
}

/// 比较表达式编译出的 `matches` 范围必须与原版语义逐值一致。
///
/// 覆盖六种运算符、常量在左右两侧、以及 `i32` 边界常量：
/// `x < c` 会编译成 `matches ..c-1`、`c > x` 会翻转成 `matches ..c-1`，
/// 越界字面量与矛盾范围则完全不生成条件（原版 `Integer.parseInt` 会拒绝
/// `..-2147483649`，`min > max` 会抛 `ERROR_SWAPPED`）。
#[test]
fn comparison_ranges_match_source_semantics() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let constants = [i32::MIN, i32::MIN + 1, -1, 0, 1, 2147483646, i32::MAX];
    let operators = ["<", "<=", ">", ">=", "==", "!="];
    let mut source =
        String::from("namespace range_semantics;\n\nscore left = 0;\nscore counter = 0;\n\n");
    let mut cases = Vec::new();
    for operator in operators {
        for (constant_index, constant) in constants.iter().enumerate() {
            let constant = *constant;
            for constant_first in [false, true] {
                let name = format!(
                    "case_{}_{}_{}",
                    match operator {
                        "<" => "lt",
                        "<=" => "le",
                        ">" => "gt",
                        ">=" => "ge",
                        "==" => "eq",
                        _ => "ne",
                    },
                    constant_index,
                    u8::from(constant_first),
                );
                let condition = if constant_first {
                    format!("{constant} {operator} left")
                } else {
                    format!("left {operator} {constant}")
                };
                source.push_str(&format!(
                    "fn {name}() {{\n    if {condition} {{ counter += 1; }}\n}}\n\n"
                ));
                cases.push((name, operator, constant, constant_first));
            }
        }
    }

    let project = repo.join("target/range-semantics-source");
    let _ = fs::remove_dir_all(&project);
    fs::create_dir_all(&project).unwrap();
    fs::write(project.join("main.mcl"), source).unwrap();
    let output = repo.join("target/range-semantics-test");
    build_file(
        &project,
        &output,
        &BuildOptions {
            description: "范围语义回归".to_owned(),
            deny_raw: true,
        },
    )
    .unwrap();

    let mut values = vec![i32::MIN, i32::MIN + 1, i32::MIN + 2, -2, -1, 0, 1, 2];
    values.extend([2147483644, 2147483645, 2147483646, i32::MAX - 1, i32::MAX]);
    for constant in constants {
        for offset in [-1i32, 0, 1] {
            values.push(constant.saturating_add(offset));
        }
    }
    values.sort_unstable();
    values.dedup();

    for (name, operator, constant, constant_first) in cases {
        let text = fs::read_to_string(
            output.join(format!("data/range_semantics/function/{name}.mcfunction")),
        )
        .unwrap();
        let commands = commands(&text);
        let condition = if constant_first {
            format!("{constant} {operator} left")
        } else {
            format!("left {operator} {constant}")
        };
        for value in &values {
            let expected = if constant_first {
                match operator {
                    "<" => constant < *value,
                    "<=" => constant <= *value,
                    ">" => constant > *value,
                    ">=" => constant >= *value,
                    "==" => constant == *value,
                    _ => constant != *value,
                }
            } else {
                match operator {
                    "<" => *value < constant,
                    "<=" => *value <= constant,
                    ">" => *value > constant,
                    ">=" => *value >= constant,
                    "==" => *value == constant,
                    _ => *value != constant,
                }
            };
            let mut scores = Simulator::new(&[("#v_left", *value)]);
            scores.run(&commands);
            assert_eq!(
                scores.get("#v_counter"),
                i32::from(expected),
                "`{condition}` 在 left={value} 上不一致：{commands:?}"
            );
        }
    }
}

fn owner_commands(root: &Path, owner: &str) -> String {
    let mut text = function(root, owner);
    let helpers = root.join(format!("data/optimizations/function/__mcl/{owner}"));
    if helpers.exists() {
        let mut paths = fs::read_dir(helpers)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        paths.sort();
        for path in paths {
            text.push_str(&fs::read_to_string(path).unwrap());
        }
    }
    text
}

fn execute_score_guard(command: &str, value: i32) -> bool {
    let (guard, _) = command
        .strip_prefix("execute ")
        .unwrap()
        .split_once(" run ")
        .unwrap();
    let tokens = guard.split_whitespace().collect::<Vec<_>>();
    assert!(matches!(tokens.first(), Some(&"if") | Some(&"unless")));
    assert_eq!(tokens.get(1), Some(&"score"));
    let objective = tokens[3];
    let mut simulator = Simulator::new(&[(tokens[2], value)]);
    simulator.run(&[&format!(
        "execute {guard} run scoreboard players add #marker {objective} 1"
    )]);
    simulator.get("#marker") == 1
}

/// 极简 scoreboard/execute 解释器：只覆盖优化器会生成的命令形状，
/// 用它在边界取值上对比“MCL 源码语义”和“生成的命令语义”。
///
/// 函数调用按配置表返回结果；配置了辅助函数目录时，`__mcl/<owner>/<n>` 会真的
/// 读取生成的 `.mcfunction` 并继续解释（模拟原版 `function` 调用）。
struct Simulator {
    scores: std::collections::BTreeMap<String, i32>,
    functions: std::collections::BTreeMap<String, i32>,
    calls: std::collections::BTreeMap<String, usize>,
    helpers: Option<std::path::PathBuf>,
    last_result: i32,
}

impl Simulator {
    fn new(initial: &[(&str, i32)]) -> Self {
        Self {
            scores: initial
                .iter()
                .map(|(holder, value)| ((*holder).to_owned(), *value))
                .collect(),
            functions: std::collections::BTreeMap::new(),
            calls: std::collections::BTreeMap::new(),
            helpers: None,
            last_result: 0,
        }
    }

    /// 配置条件里用到的函数返回值。
    fn with_functions(mut self, functions: &[(&str, i32)]) -> Self {
        self.functions = functions
            .iter()
            .map(|(name, value)| ((*name).to_owned(), *value))
            .collect();
        self
    }

    /// 允许 `function <命名空间>:__mcl/...` 递归解释磁盘上的辅助函数。
    fn with_helpers(mut self, directory: std::path::PathBuf) -> Self {
        self.helpers = Some(directory);
        self
    }

    fn get(&self, holder: &str) -> i32 {
        self.scores.get(holder).copied().unwrap_or(0)
    }

    fn calls(&self, name: &str) -> usize {
        self.calls.get(name).copied().unwrap_or(0)
    }

    fn run(&mut self, commands: &[&str]) {
        for command in commands {
            self.run_one(command);
        }
    }

    fn run_one(&mut self, command: &str) {
        if let Some(rest) = command.strip_prefix("execute ") {
            let (head, body) = rest
                .split_once(" run ")
                .unwrap_or_else(|| panic!("generate 只使用带 run 的 execute 守卫：{command}"));
            let (guard, store) = split_store(head);
            if self.guard_holds(guard) {
                self.last_result = 0;
                self.run_one(body);
                if let Some(target) = store {
                    let result = self.last_result;
                    self.scores.insert(target.to_owned(), result);
                }
            }
            return;
        }
        if let Some(target) = command.strip_prefix("function ") {
            self.call(short_function_name(target));
            return;
        }
        let tokens = command.split_whitespace().collect::<Vec<_>>();
        match tokens.as_slice() {
            ["scoreboard", "players", "set", holder, _, value] => {
                self.scores
                    .insert((*holder).to_owned(), value.parse().expect("score literal"));
                self.last_result = 1;
            }
            ["scoreboard", "players", "add", holder, _, value] => {
                let value = value.parse::<i32>().expect("score literal");
                let updated = self.get(holder).wrapping_add(value);
                self.scores.insert((*holder).to_owned(), updated);
                self.last_result = 1;
            }
            ["scoreboard", "players", "remove", holder, _, value] => {
                let value = value.parse::<i32>().expect("score literal");
                let updated = self.get(holder).wrapping_sub(value);
                self.scores.insert((*holder).to_owned(), updated);
                self.last_result = 1;
            }
            [
                "scoreboard",
                "players",
                "operation",
                holder,
                _,
                "=",
                source,
                _,
            ] => {
                let value = self.get(source);
                self.scores.insert((*holder).to_owned(), value);
                self.last_result = 1;
            }
            other => panic!("解释器不支持的命令：{other:?}"),
        }
    }

    /// 调用一个函数：优先解释辅助函数文件，否则按配置表返回结果。
    fn call(&mut self, name: &str) {
        *self.calls.entry(name.to_owned()).or_default() += 1;
        if let Some(directory) = self.helpers.clone() {
            let path = directory.join(format!("{name}.mcfunction"));
            if path.is_file() {
                let text = std::fs::read_to_string(&path).expect("读取辅助函数");
                let commands = text
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty() && !line.starts_with('#'))
                    .map(str::to_owned)
                    .collect::<Vec<_>>();
                let commands = commands.iter().map(String::as_str).collect::<Vec<_>>();
                self.run(&commands);
                return;
            }
        }
        self.last_result = self.functions.get(name).copied().unwrap_or(0);
    }

    fn guard_holds(&mut self, guard: &str) -> bool {
        let tokens = guard.split_whitespace().collect::<Vec<_>>();
        let mut index = 0;
        while index < tokens.len() {
            let negated = match tokens[index] {
                "if" => false,
                "unless" => true,
                other => panic!("不支持的 execute 关键字 `{other}`：{guard}"),
            };
            let (holds, step) = match tokens[index + 1] {
                "score" => {
                    let left = self.get(tokens[index + 2]);
                    match tokens[index + 4] {
                        "matches" => (score_matches(left, tokens[index + 5]), 6),
                        symbol => (compare_scores(left, symbol, self.get(tokens[index + 5])), 7),
                    }
                }
                "function" => {
                    let name = short_function_name(tokens[index + 2]).to_owned();
                    self.call(&name);
                    (self.last_result == 1, 3)
                }
                other => panic!("不支持的 execute 谓词 `{other}`：{guard}"),
            };
            if holds == negated {
                return false;
            }
            index += step;
        }
        true
    }
}

/// `execute` 头部的 `store result score <持有者> <目标>` 子句。
fn split_store(head: &str) -> (&str, Option<&str>) {
    match head.split_once("store ") {
        Some((guard, store)) => {
            let tokens = store.split_whitespace().collect::<Vec<_>>();
            assert_eq!(
                tokens.first(),
                Some(&"result"),
                "只支持 store result：{head}"
            );
            assert_eq!(
                tokens.get(1),
                Some(&"score"),
                "只支持 store result score：{head}"
            );
            (guard.trim_end(), Some(tokens[2]))
        }
        None => (head, None),
    }
}

fn short_function_name(target: &str) -> &str {
    let name = target.split_whitespace().next().expect("函数名");
    name.rsplit_once(':').map_or(name, |(_, path)| path)
}

fn compare_scores(left: i32, symbol: &str, right: i32) -> bool {
    match symbol {
        "=" => left == right,
        "!=" => left != right,
        "<" => left < right,
        "<=" => left <= right,
        ">" => left > right,
        ">=" => left >= right,
        other => panic!("不支持的比较运算符 `{other}`"),
    }
}

fn score_matches(value: i32, range: &str) -> bool {
    if let Some((minimum, maximum)) = range.split_once("..") {
        let minimum = if minimum.is_empty() {
            i64::from(i32::MIN)
        } else {
            minimum.parse::<i64>().unwrap()
        };
        let maximum = if maximum.is_empty() {
            i64::from(i32::MAX)
        } else {
            maximum.parse::<i64>().unwrap()
        };
        (minimum..=maximum).contains(&i64::from(value))
    } else {
        range.parse::<i32>().unwrap() == value
    }
}
