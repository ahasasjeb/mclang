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
    let output = repo.join("target/control-optimization-test");
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

    let mut effectful = function(&output, "effectful_guards");
    let helper_directory = output.join("data/optimizations/function/__mcl/effectful_guards");
    for entry in fs::read_dir(helper_directory).unwrap() {
        effectful.push_str(&fs::read_to_string(entry.unwrap().path()).unwrap());
    }
    assert_eq!(
        effectful
            .matches("function optimizations:repeated_effect")
            .count(),
        2,
        "函数条件含副作用，不能合并或删除：{effectful}"
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
    let output = repo.join("target/control-optimization-test");
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
    let output = repo.join("target/control-optimization-test");
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

/// `&&` 右侧是比较时，编译期不再为它单独分配标志计分项；
/// 用解释器在边界值上核对生成的命令与源码语义一致。
#[test]
fn and_operand_fusion_matches_source_semantics() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = repo.join("target/control-optimization-test");
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
struct Simulator {
    scores: std::collections::BTreeMap<String, i32>,
}

impl Simulator {
    fn new(initial: &[(&str, i32)]) -> Self {
        Self {
            scores: initial
                .iter()
                .map(|(holder, value)| ((*holder).to_owned(), *value))
                .collect(),
        }
    }

    fn get(&self, holder: &str) -> i32 {
        self.scores.get(holder).copied().unwrap_or(0)
    }

    fn run(&mut self, commands: &[&str]) {
        for command in commands {
            self.run_one(command);
        }
    }

    fn run_one(&mut self, command: &str) {
        if let Some(rest) = command.strip_prefix("execute ") {
            let (guard, body) = rest
                .split_once(" run ")
                .unwrap_or_else(|| panic!("generate 只使用带 run 的 execute 守卫：{command}"));
            if self.guard_holds(guard) {
                self.run_one(body);
            }
            return;
        }
        let tokens = command.split_whitespace().collect::<Vec<_>>();
        match tokens.as_slice() {
            ["scoreboard", "players", "set", holder, _, value] => {
                self.scores
                    .insert((*holder).to_owned(), value.parse().expect("score literal"));
            }
            ["scoreboard", "players", "add", holder, _, value] => {
                let value = value.parse::<i32>().expect("score literal");
                let updated = self.get(holder).wrapping_add(value);
                self.scores.insert((*holder).to_owned(), updated);
            }
            ["scoreboard", "players", "remove", holder, _, value] => {
                let value = value.parse::<i32>().expect("score literal");
                let updated = self.get(holder).wrapping_sub(value);
                self.scores.insert((*holder).to_owned(), updated);
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
            }
            other => panic!("解释器不支持的命令：{other:?}"),
        }
    }

    fn guard_holds(&self, guard: &str) -> bool {
        let tokens = guard.split_whitespace().collect::<Vec<_>>();
        let mut index = 0;
        while index < tokens.len() {
            let negated = match tokens[index] {
                "if" => false,
                "unless" => true,
                other => panic!("不支持的 execute 关键字 `{other}`：{guard}"),
            };
            assert_eq!(tokens[index + 1], "score", "{guard}");
            let left = self.get(tokens[index + 2]);
            let (holds, step) = match tokens[index + 4] {
                "matches" => (score_matches(left, tokens[index + 5]), 6),
                symbol => (compare_scores(left, symbol, self.get(tokens[index + 5])), 7),
            };
            if holds == negated {
                return false;
            }
            index += step;
        }
        true
    }
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
