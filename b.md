# 过度防御审计报告

> 2026-10-01 已完成独立复核与修复。前七节保留原始审计记录，当前位置及最终判定以第八节为准。第 5.1 节的“未复现”结论已纠正：最小用例确实会 panic，现已修复；公开版本 API 和真实输入/IO 边界检查保留。

审计对象：`src/` 全部 179 个 Rust 文件（Minecraft 脚本语言 mclang 的编译器与语言服务器）。
审计方式：逐文件通读 + 交叉核对「谁已经保证过这个不变量」；全部结论都给出两处位置（防御点与已经保证它的地方）。
验证状态：`cargo clippy --all-targets` 零警告，`cargo test` 全部通过；报告中标注「已实测」的结论实际运行过；一处子审计提出的疑似 panic 已实测**未能复现**（见第五节）。

## 总体结论

**不存在「同一条件被三层完整重复校验」的系统性问题。** 分层（词法 → 语法 → 语义校验 → 代码生成）职责清楚，绝大多数检查是对用户源码的正当校验。

真正的过度防御集中在三个模式：

1. **对自产数据/不可能缺失的字段写静默兜底**——`src/version/` 最集中，其次是 LSP 的位置换算；
2. **保护远处不变量的 `unreachable!` / `expect`**——parser 有 26 处，其中 24 处把「关键词表 ↔ match 分支必须手工同步」变成运行期 panic（在 LSP 里是崩进程，不是报错）；
3. **同一判据在多处重复实现**——已开始出现不一致（同一处拼写错误，一个入口给建议、另一个不给）。

另有一个方向性问题值得单独强调：**兜底的方向错了**。多处「不可能缺失」的查表失败后退化成「静默少一条诊断 / 静默当成另一种含义」，而不是响亮失败；同一份数据在隔壁文件却是直接索引（会 panic）。两种态度并存本身就是信号。

---

## 一、删掉即无行为变化（置信度最高，建议先做）

### 1.1 版本快照生成器的 FNV `digest` 是死重

- 位置：`src/version/generate.rs:3-5`、`src/version/generate/extract.rs:134-171`、`src/version/generate/extract.rs:211-220`，写入点 `generate/registries.rs:302`、`generate/enums.rs:91`、`generate/commands.rs:50`、`generate/triggers.rs:49`
- 防御写法：`Extractor` 维护 `digest: u64`，`read()` 对「相对路径 + 全文」做 FNV-1a，`digest_path()` 遍历整个数据包目录对每个资源路径哈希，`finish()` 输出 `fnv1a64:...`，四份快照各写一个 `"digest"` 字段。
- 为什么多余：**全仓库只有写入方，没有任何读取者**（已 grep `digest` 确认）。`check_command`（`src/version/generate.rs:93-112`）是把重新生成的完整 JSON 与磁盘文件逐字节比对，摘要完全不参与；文档声称的用途（供 `check-version-data` 校验）并不存在。旁证：`entity_nbt.json` 与 `block_states.json` 根本没有这个字段。
- 建议：删掉 `digest` 字段与 `digest`/`digest_path`/`finish`/`fnv_update`，文档改为「比对全文」。（注意：删除后需重跑生成器刷新快照字节。）

### 1.2 自产快照的加载被包成 `Result`，唯一调用点立刻 `expect`

- 位置：`src/version/entity_nbt.rs:260-295`
- 防御写法：`EntityNbtCatalog::from_json` 返回 `Result<Self, String>`，三条错误分支（`快照 JSON 无效`、`标签 \`{key}\` 的类型无法识别`、`实体 \`{id}\` 的标签表必须是数组`）；唯一调用者 `catalog()` 直接 `.expect("...必须是有效的实体 NBT 快照")`。
- 为什么多余：`Err` 内容永不外传。而三条分支本身也不构成校验：`tags`/`entities` 用 `if let Some` 取值，字段缺失时**静默产出空目录**而不是报错——校验只做了一半。类型串来自 `EntityTagType::as_str()`（同文件 57-86 一一对应），`entities` 的值在 `generate()`（163-177）恒为数组，文件一致性由 `tests/corpus.rs:469` 逐字节重生成比对保证。
- 建议：删掉 `Result` 与内层三条 `Err`，改成直取字段 + 边界处一个 `expect`。

### 1.3 快照解析辅助为「无条件写入的字段」准备了一整套默认值兜底

- 位置：`src/version/snapshot.rs:43-64`、`318-382`（另 `268-271`、`280`、`289-299`、`309`）
- 防御写法：`object_sets` / `object_arrays` / `object_maps` / `string_set` / `Slots::from_value` 全链 `value.and_then(Value::as_object) → .map(...) → .unwrap_or_default()`，内部再 `as_array()...unwrap_or_default()`、`filter_map(as_str)`、`as_object()?` 静默丢元素；四个 `load()` 都以可选取值调用。
- 为什么多余：`registries`/`slots`/`tag_registries`/`resource_kinds`（`generate/registries.rs:296-309`）、`enums`（`generate/enums.rs:89-96`）、`commands`（`generate/commands.rs:48-56`）、`triggers`（`generate/triggers.rs:47-54`）全部**无条件 insert**，元素类型就是序列化类型；兜底永不执行。
- 同一文件里的反向写法：`snapshot.rs:258-273` 先用 `unwrap_or_else(|| json!({}))` 兜底，紧接着 `.expect("valid enchantment level snapshot")` —— 两种相反策略保护同一个不可能前提，且 panic 文案是英文，与同文件中文风格不一致。
- 建议：改成一次 `serde_json::from_str::<SnapshotSchema>`（每个文件一个 `expect`），辅助函数整批删除。

### 1.4 LSP 位置换算里的重复守卫与死参数

- `src/lsp/convert.rs:76-80`：`if line as usize >= index.starts_len() { return text.len(); }`。
  `LineIndex::line_start`（`src/lines.rs:47-49`）对越界行号本身就返回 `self.len`，此时 `text[line_start..]` 为空 → `find('\n')` 为 `None` → 返回 `text.len()`，与原早退**完全同值**。`starts_len()`（`src/lines.rs:52`）全仓库只为这一处存在 → 两者一起删。
- `src/lsp/convert.rs:68-73`：`offset_to_position_in(text, offset, index: Option<&LineIndex>)` 的 `None` 分支「按需现建一次索引」。
  唯一调用者是 `src/lsp/server.rs:146-147`，两处都传 `Some(index)`；`convert` 是私有模块，crate 内无其它调用 → 参数改成 `&LineIndex`。
- `src/lsp/features.rs:488-502`：`offset.min(text.len())` 在 489 与 501 各夹一次；循环里 `match text[..start].chars().next_back() { None => break }` 不可达（`start > 0` 时切片非空）。同族：`convert.rs:127-132` 的 `clamp_boundary` 在调用链上再夹一次。
- 建议：`None` 分支改成 `while let Some(previous) = ...`，夹紧只保留一处。

### 1.5 对自家源码做「用户输入级」词法扫描

- 位置：`src/translate/tables.rs:471-507`
- 防御写法：`string_literals()` 手写 Rust 字符串字面量提取，包含「跳过 `//` 注释」与「处理 `\` 转义」两段。
- 为什么多余：输入是 `include_str!("../parser/keywords/*.rs")` 的 5 个**固定文件**（`tables.rs:451-457`），是编译器自己的源码。实测这 5 个文件既无任何 `\` 转义，也无含 `"` 的注释行（`//` 出现处全是 `///` 文档注释），两段处理永不触发。而且这套转义逻辑本身只对 `\"`/`\\` 正确（`\n` 会被当成字母 n 推进字面量），不构成真保护。
- 建议：删掉注释跳过与转义分支，只按 `"` 切分（或在 `build()` 里改成显式断言）。

### 1.6 少量零散死代码

- `src/parser/statements/scoreboard.rs:216-219`：`fn score_target_body(&mut self, _label: &str)`，函数体一行未读该参数，4 个调用点（`scoreboard.rs:212`、`scoreboard_commands.rs:76,86,95`）却都认真传了标签 → 删参数，或让它替换硬编码的 `"计分板目标名称"`。
- `src/version/snapshot.rs:85-95, 187-189, 201-203, 252-254`：`Slots::names()`（自带范围槽展开逻辑，注释说「用于提示与补全」）、`knows_registry`、`enum_values`、`command_names`，crate 内零调用点。（`pub mod version` 是公开 API，删除前需确认无下游消费者。）

---

## 二、静默兜底：内部不变量被破坏时表现为「悄悄少一点」

这是方向性问题，比多写几行更值得改：出问题时应该响亮失败，而不是退化成另一个看似正常的结果。

- `src/modules/resolve.rs:179` —— `module_paths.get(index).cloned().unwrap_or_default()`
  一旦查表失败，子模块被**当成入口模块**（`segments = []` → 名字不再限定），静默产出重名产物。同一循环的 `src/modules/resolve.rs:180` 却是直接索引 `by_source[index]`，两种态度并存。同族：`resolve.rs:63-65` 的 `let (Some(directory), Some(path)) = ... else { continue }`、`src/modules/paths.rs:57` 的 `if let Some(targets) = edges.get(&index)`。
- `src/analysis.rs:130`、`150-151`、`313-316` —— `programs_by_root.remove(&root).unwrap_or_default()`、`filter_map` 里的 `?`、`let Some(source) = sources.get(*source_id) else { continue }`
  效果是**诊断与符号凭空消失**。这些索引都由本函数自己建立，永不缺失。对比 `src/lines.rs:84` 的 `sources[span.source]` 是直接索引。
- `src/compiler/codegen/macros.rs:110-113` —— 为「当前函数名 + 它自己的 helper」写 `let Some(commands) = self.functions.get_mut(&path) else { continue }`
  函数名在 `mod.rs:150-154` 必被 insert，每个 helper 都在 `next_helper_path` 调用点立即 insert，且无处删除表项 → 应直接索引，把「helper 一定存在」变成显式不变量。
- `src/translate/token.rs:30`、`src/translate/rewrite.rs:326`、`src/parser/mod.rs:40` —— `unwrap_or_default()` / `unwrap_or(frame)`
  `token.rs:30` 在 `while index < source.len()` 内，`rest` 必非空，`'\0'` 兜底会静默落进 Punct 分支；`split` 至少产出一个元素。`parser/mod.rs:40` 更微妙：`tokens` 为空时它返回默认 span，但紧接着 `parser.program()` 会在 `self.tokens[self.cursor]`（`mod.rs:396`）越界 panic —— **兜了一个兜不住的底**（`lexer::scan` 结尾无条件追加 `Eof`，见 `lexer.rs:166-173`）。

---

## 三、保护远处不变量的 `unreachable!` / `expect`

### 3.1 parser 的 26 处 `_ => unreachable!()`（唯一会崩 LSP 的一类）

完整清单：`components.rs:414,466`；`execute.rs:168,325`；`core_commands.rs:68`；`entity_commands.rs:188`；`items.rs:203`；`declarations/basic.rs:120`；`declarations/tags.rs:47`；`declarations/query.rs:201,320`；`declarations/advancement.rs:79,172,244,355`；`world/blocks.rs:178,287`；`statements/ui.rs:44`；`statements/messages.rs:40,227`；`statements/self_actions.rs:145,196`；`statements/scoreboard.rs:85`；`keywords/values.rs:16`。

- 其中 24 处的 match 对象是关键词表返回的 `&'static str`，分支已逐一穷举表内全部取值，`_` 永不进入。表与分支的对应关系已核对：`keywords/values.rs:114-133`（16 值）↔ `items.rs:64-203`；`keywords/world_values.rs:159-172`（10 值）↔ `execute.rs:98-168`；`entity_commands.rs:41-64`（19 root）↔ `entity_commands.rs:74-188`；`core_commands.rs:8-21`（12 root）↔ `core_commands.rs:35-68`。表改了而分支忘了改 → panic（LSP 直接崩）。
- 另有 2 处是**同一函数内刚收窄过的集合又 match 一遍**：`src/parser/statements/ui.rs:99-117`（外层列 7 个字面量，内层 `:106-116` 再 match 同样 7 个）、`src/parser/statements/self_actions.rs:88-95`（外层 2 个，内层同 2 个）。旁边分支已有正确写法（`self_actions.rs:74,82,123` 用 `if method_kind == "…"`）。
- 建议：改成返回诊断（这些函数都返回 `Result`，span 在作用域内）；根治是让表直接返回枚举——项目里 `set_block_mode`/`fill_mode`/`locate_kind` 已经这么做，match 就恢复编译期穷尽。

### 3.2 codegen 把 validate 的结论又用运行期代码复查一遍

- `src/compiler/codegen/emit.rs:54-64` —— `entity_query_selector` 里 `if base_is_tag { for filter ... match { Include → type=v, Exclude → type=!v } }`，注释自陈 "keep formatting defensive"。
  **已核实**：`src/compiler/validate/items.rs:161-167` 对 `EntityTypeFilter::Include` 无条件报错，`validate/items.rs:168-176` 对「非 `#标签` 基类型 + `without_type`」也报错，而 `validate_entity_query` 对每个查询声明都会执行（`validate/collect.rs:200`）。因此 `type_filters` 非空时基类型必为标签 → `base_is_tag` 恒真、`Include` 臂不可达。
  建议：删掉守卫与 `Include` 臂，把「Brigadier 拒绝第二个 `type=`」的理由留成注释。
- `src/compiler/codegen/mod.rs:191-199` 与 `src/compiler/codegen/scoreboard.rs:96-99` —— styled JSON 在 codegen 里**第二次** `serde_json::from_str::<Value>(style).expect("validated ... style JSON")`。
  validate 已经解析过同一字符串（`validate/collect.rs:94-98`、`validate/scoreboard.rs:82-91`），失败即 Error，`compiler/mod.rs:81-86` 在此之前就返回；`rename` 也不改这些字符串（`src/name_walk.rs:110` 只处理 `NumberFormat::Fixed`）。项目自己已有「validate 解析一次、把 `Value` 传下去」的范式（`resource_json`，`codegen/mod.rs:63-64`、`293-300`），这里没用。
- `src/compiler/codegen/advancement.rs:67-70` —— `serde_json::to_value(requirements).expect("criterion names serialize as strings")`，而 `requirements` 是同文件 50-66 行刚构造的 `Vec<Vec<String>>`。整层 serde 转换可直接换成 `Value::Array` 构造，`expect` 随之消失。
- `src/compiler/codegen/expressions.rs:408-416` —— 先 `matches!` 得到 `reuse`，下一行再 `match &left_value { Value::Score(s) => s.clone(), _ => unreachable!() }`。`reuse` 为真时必是 `Value::Score`；合并成一次 `if let Value::Score(score) = &left_value && score.starts_with("#t")`。
- `src/compiler/codegen/dispatch.rs:140-153` —— `frozen_operand` 先 `if operand_stable(...) { return operand }`，随后 match 里仍留 `Literal(v) => Literal(v)`；`operand_stable` 对 `Literal` 恒为 true（`dispatch.rs:173-185`）→ 改成 `if let Operand::Cell { .. }`。

### 3.3 真正删不掉的 expect（记录耦合，不必改）

以下确认不可达，但受 Rust 类型系统限制无法删除，作为「已知耦合」记录即可：`codegen/mod.rs:295-297` 的 `resource_json[index].expect(...)`（validate 用 `Vec<Option<Value>>` 传递失败信息，删掉要改类型）、`actions/operands.rs:190/197/204/214/221` 的五个查询/物品/存储/数据槽/函数 expect、`control.rs:690/693` 的 `loop_state()` 双 expect、`execute.rs:114/131` 与 `helpers.rs:21` 的 `pop().expect("one command")`（紧随 `len() == 1` 判断）、`advancement.rs:41-43` 的 `unreachable!("...valid conditions JSON")`。

---

## 四、同一判据多处实现（已开始不一致）

- **`run` 字符串行尾反斜杠被查三层**：解析层 `src/parser/statements/dispatch.rs:349`（另有 `:337` 查 `/` 前缀、`:343` 查换行）→ `src/compiler/validate/statements/execute.rs:369`（`validate_raw_command`，经 `statements/statement.rs:36`、`flow.rs:18` 对同一字符串调用）→ `src/compiler/codegen/mod.rs:276-284`。三处各写各的文案。
  建议：解析层是唯一能给解析期 span 的，保留它，删掉 `validate_raw_command` 里的重复项。
- **codegen `finish()` 的命令自检是第三层**（`codegen/mod.rs:260-285`：UTF-16 长度、换行/NUL、行尾反斜杠）。**已实测**：`help("a\nb")` 与 `say("x\nrun \"stop\"")` 都在 validate 层就被拦住（`validate/core_commands.rs:23`、`validate/components.rs:16-18`），可达路径上够不到这里。它是真·最后一道保险（输出正确性，文档也承诺了 2000000 上限），建议保留，但把诊断点名到具体命令，别只报「函数 `x` 第 N 条」。
- **「静态注册表存在性 + 最近候选」抄了 4 份**：`src/compiler/validate/registry.rs:32-55`（`validate_static_id`，唯一调用点 `statements/ui.rs:167`）、`world/features.rs:54-67`、`advancement.rs:394-416`、`resource_schema.rs:266-285`。四处各自 `valid_resource_location` + `registry_contains*` + `suggest_registry_id` + 拼消息；`features.rs:59` 用 `!= Some(true)`、`advancement.rs:402` 用 `registry_contains_exact`，语义等价却要改四处。
  建议：统一走 `validate_static_id`，registry/label/措辞参数化。
- **同一份 LootCondition codec 校验写了两套**：`src/compiler/validate/resource_schema.rs:71-139`、`226-257` 与 `src/compiler/validate/advancement.rs:353-495`。同一份 26.3 codec（字符串引用 / 对象形状 / `inverted.term` / `all_of|any_of.terms`）实现两遍，靠人工同步；**已经不一致**——同样写错 `type`，`advancement.rs:409-413` 给拼写建议，`resource_schema.rs:275-283` 不给。注意删除任意一份会改变另一个入口的校验，所以这是「同层重复实现」，应抽公共 helper 而不是直接删。
- **中文别名两套机制**：`src/parser/statements/scoreboard.rs:14-19` 用 `self.ident()` 取原文再 `matches!(method.as_str(), "objectives" | "目标集")`、`"players" | "玩家分数"`；`src/parser/core_commands/remaining.rs:18-22` 手写 `"运行测试" => "run"`、`"停止测试" => "stop"`。同层 `.{方法}` 在别处统一走 `command_word()`（`entity_commands.rs:204-216`：`command_value` → `ui_value` → `KEYWORDS`）出规范英文。「目标集」在 `keywords/commands.rs:91` 已有一份（重复），「玩家分数」「运行测试」「停止测试」任何表里都没有，而 `ui_value` 给 `players` 配的「玩家列表」在这条路径永远用不到 —— `scoreboard.玩家列表` 会报「未知 scoreboard 方法」。
- 其它小重复实现：`src/compiler/validate/rules.rs:72-74` 的 `valid_nbt_path` 与 `components.rs:419-421` 的 `valid_nbt_component_path` **函数体逐字相同**；`components.rs:386-395` 的 `valid_uuid` 与 `core_commands.rs:29-36` 的内联 UUID 检查逻辑完全一致；`collect.rs:94-98` 与 `scoreboard.rs:82-91` 的 styled 对象检查两遍。
- `src/parser/expressions.rs:200-201` —— `word_matches(&method, "get") || command_value(&method) == Some("get")`。**已核实**前者严格被后者包含（`keywords/commands.rs:28` 是 `"get" | "取"`，且 `"get"` 不在 `KEYWORDS` 里）→ 只留后者。
- `src/lib.rs:181-191` —— 先 `fs::metadata(...).permissions().readonly()` 再 `OpenOptions::new().write(true).open(path)`，同一「可写性」目的两层，第二层已完全覆盖第一层。
- 重复诊断（靠 `validate/mod.rs:113-120` 的 `deduplicate` 掩盖，用户可见结果不变）：
  - `src/compiler/validate/statements/macros.rs:56-60`：`validate_nbt_source` 内部经 `validate_component_holder` → `validate_holder`，紧接着 `entity_target(holder, ...)`（`entity_commands.rs:171-208`）**再次** `validate_holder`，对 `Holder::Query` 产出完全相同的 span 与消息；对 `Holder::Origin` 则各报一条不同措辞。
  - `src/compiler/validate/components.rs:289-311`、`statements/statement.rs:257-266`：通用 holder 校验 + 命令专用规则对同一 span 各报一条，前者在 `context = None` 时已被后者完全覆盖。
- 可证明永不生效的守卫 3 处：`validate/statements/actions.rs:144-146` 与 `validate/statements/execute.rs:134-135` 的 `if valid_resource_location(x) && non_summonable_entity(x)`（`non_summonable_entity` 只匹配两个恒合法的字面量，且紧前一行刚调用过内部第一步就是 `valid_resource_location` 的 `validate_id`）；`validate/resource_schema.rs:223` 的 `!id.is_empty() && ...`（`canonical_json_id("")` 是 `"minecraft:"`，必为 false）。
- 重复状态：`src/compiler/validate/statements/execute.rs:22` 的 `has_stores` 与 `stage == 2` 是同一个状态（全文件只有两处同时赋值），可删掉 `has_stores`。
- 重复计算（效率问题，非校验）：`src/modules/resolve.rs:69-79` 用同一实参调用 `is_virtual_path` 两次，且在 `for segment` 循环里每个分段重算 `first_span(program)`（内部为全部声明建 10 段 Vec 再取首个）；`src/lsp/` 每次 hover 请求重建 `LineIndex` 三次（`position_to_offset`、`features.rs:299`、`features.rs:505`）；`src/version/entity_nbt.rs:319-325` 的 `let allowed = allowed.cloned()`（`Option<&HashSet<_>>` 是 `Copy`）。

---

## 五、附注：断言说谎与待确认项

### 5.1 一处 expect 声称的不变量比 validate 实际保证的更强（**实测未复现**）

`src/compiler/codegen/statements/execute.rs:136` 的 `.expect("semantic validation guarantees the stored block is not empty")`：validate 只保证**源语句块**非空（`validate/statements/execute.rs:197-202`），不保证它**生成**至少一条命令。

子审计据此静态推导：块尾是零命令的 `for`（常量上下界满足 `start >= end`，如 `for i in 5..5 {}`）时，`compile_for` 在 `control.rs:474-478` 直接返回 → `body_commands` 为空 → 落到 136 行 panic；并指出 validate 的 store-块黑名单（`validate/statements/execute.rs:166-177`）列了 If/While/Each/InDimension/Spawn/Execute 而**漏了 `For`**。

**实测结果**：按此构造 `execute store.result(self, stored) { counter += 1; for i in 5..5 {} }` 构建**正常通过**（3 个函数、5 条命令），未 panic。因此不作为缺陷处理，仅记录「该 expect 的声称强于 validate 的保证」这一耦合。

### 5.2 两个非防御问题的附带发现

- `src/analysis.rs:133-140`：LSP 每次按键都跑完整 `compile()`（含 rename 与 codegen）只为拿少量 warning。
- `src/compiler/validate/advancement.rs:24-83`：硬编码 `TRIGGERS` 常量与 `data/version/26.3/advancement_triggers.json`（由 `version/generate/triggers.rs:19-53` 生成）疑似重复数据源，注释也自认「数据生成器落地后改为读取快照」。未逐项 diff，待确认。
- `src/compiler/validate/items.rs:31-38`：`limit` 取自 `component_stack_size(item)`（会读组件里的 `max_stack_size`），但消息打印 `item.max_stack_size.unwrap_or(1)`；只用组件语法声明堆叠数时会打印「最大堆叠数为 1」而实际上限是 64×100。
- `src/version/entity_nbt/aliases.rs:4` 引用的测试 `chinese_aliases_resolve_to_real_tags` 在仓库中不存在（grep 无果），文档与实际不符。

---

## 六、判定为合理、未计入的项（避免误改）

- 解析**用户输入**的全部校验：语法、SNBT 形状与数值范围、资源位置、重复声明、坐标 `^` 混用、保留字等。
- 安全边界与真实 IO：符号链接拒绝、路径穿越拒绝、`strip_parent`/`strip_prefix`、ZIP 布局上限、`archive.rs:15-23` 的输出目录检查。
- 防栈溢出与展开爆炸：`src/parser/limits.rs` 的 `MAX_NESTING`、`EXPANSION_BUDGET`、`check_expression_depth`，以及 `src/stack.rs` 的 16 MiB 工作线程（`parse_in_place` 复用外层大栈是有意为之）。
- 面向用户的诊断质量：`src/constant.rs:29-50` 的第二次遍历（只为给出「为什么不能折叠」）、各处 span 定位与文案。
- 确需的运行期兜底：`emit.rs:146` 的 `intersect_ranges(...).unwrap_or_else(...)`（`distance(3..10)` + `within(2)` 是合法组合，回退真会走到）、`emit.rs:214-223` 的 `items.rs:509-516` 允许的原样透传形状、`ui.rs:123-124` 与 `calls.rs:31/35/38` 的可选参数补位、`names.rs:26-27` 的「参数→局部→全局」第三级。
- `src/lsp/rpc.rs`、`server.rs:63-120`、`dispatch.rs`、`lifecycle.rs:29-46`：JSON-RPC 报文来自外部客户端，畸形/半包处理必需；`lifecycle.rs:77-90` 在 mtime 缺失时不复用缓存、`221-223` 在 IO 失败时清空发现结果，都是合理取舍。
- `src/version/generate/**` 对随附 Minecraft 反编译源码/资源的校验：外部数据形状变了必须硬失败。
- `src/compiler/validate/mod.rs:113-120` 的 `deduplicate`：`unroll` 会复制语句体（`parser/statements/unroll.rs:84-110`），去重是必需兜底。
- `src/name_walk.rs` 及 `src/name_walk/**`：全部 match 无通配兜底，名字遍历只有一份实现，无发现。
- `src/stdlib.rs`、`src/modules.rs`、`src/modules/declarations.rs`、`src/bin/xtask.rs`、`src/translate/context.rs`、`src/translate/syntax.rs`、`src/main.rs`：无发现。

---

## 七、建议动手顺序

1. **纯删除**（零行为变化，风险最低）：`digest` 链、`entity_nbt` 的 `Result`、snapshot 默认值兜底族、`starts_len`/`Option<&LineIndex>`、`tables.rs` 源码扫描、死参数 `_label`、四个无调用点访问器。
2. **把静默兜底改成索引 / `expect`**：`modules/resolve.rs:179`、`analysis.rs:130/150/313`、`codegen/macros.rs:110`、`translate/token.rs:30`、`parser/mod.rs:40`。收益最大——把「悄悄错」变成「响亮错」。
3. **让 codegen 只信任 validate**：styled JSON 按 `resource_json` 的范式传 `Value`；删 `emit.rs` 的 `base_is_tag` 守卫与 `Include` 臂；`advancement.rs` 的 `to_value` 换直接构造。
4. **收敛重复**：三层反斜杠检查收敛成一层；四处注册表检查收敛成一个 helper；LootCondition codec 抽公共函数；中文别名并回关键词表。
5. **单独排期**：表驱动的 24 处 `unreachable!` 改成「表返回枚举」——这是唯一会崩 LSP 的一类。

---

## 八、2026-10-01 独立复核与实际处理结果

复核以本次修改前的 Git 版本及实际调用链为依据，并针对有行为影响的结论运行最小用例。原 `src/` 的 179 个 Rust 文件这一数量属实；新增公共 LootCondition 校验模块后为 180 个。没有将原报告的“已实测”或测试通过声明当成独立证据。

### 8.1 第一节：删除与快照加载

| 原条目 | 独立判定与处理 | 检查依据 |
| --- | --- | --- |
| 1.1 FNV digest 链 | 属实，已删除四份快照的摘要字段、哈希累计与路径摘要参数；Extractor 改为只读借用。重新生成全部快照，文档改成全文逐字节比对。 | 全仓库没有摘要读取者；`check_command` 实际比较完整文本；重生成检查通过。编译器用于生成名称的 `stable_hash` 保留，它有实际调用。 |
| 1.2 EntityNbtCatalog 的 Result 与半截校验 | 属实，改成类型化反序列化，只有快照边界的一个 expect；字段缺失、未知类型和非字符串标签不再产出空目录。 | 随附快照由生成器写入；类型字符串与所有 EntityTagType 的反序列化逐项对应；畸形快照测试通过。 |
| 1.3 snapshot 默认值辅助族 | 属实，registries/enums/commands/triggers/slots 按各自结构反序列化，删除 object_sets、object_arrays、object_maps、string_set 和 Slots::from_value。 | 生成器必填字段与类型核对；缺字段、错数组元素类型、错误字段类型、负槽位数均被拒绝；真实快照与语料全部通过。 |
| 1.4 越界行号重复守卫 | 属实，删除 starts_len 和位置换算的重复越界检查。 | line_start 越界返回 EOF；空文本、尾换行、CRLF、极大行号测试通过。 |
| 1.4 Option&lt;&LineIndex&gt; | 属实，改成必传索引，并进一步让项目缓存索引供诊断、定位、悬停与跳转复用。 | 唯一调用点原本必传 Some；编辑后 hover/definition 的行号缓存更新测试通过。 |
| 1.4 prefix_at 重复夹紧与不可达 None | 属实，统一计算一次偏移并用 while let 扫描。保留字符边界夹紧，而不是只做长度 min。 | 中文多字节字符内部的偏移不能直接切片；对文本每个字节偏移及越界偏移检查 completion/word_at 无 panic。 |
| 1.5 自家关键词源码扫描 | 属实，按引号分段，删除注释扫描与伪转义处理；明确断言源码不得含转义或带引号的行注释。 | 五个 include_str 文件满足条件；所有翻译语料、幂等、双向翻译与产物一致性检查通过。 |
| 1.6 score_target_body 的死参数 | 属实，定义及所有调用统一去掉 _label；score_target 自己用于括号诊断的 label 保留。 | 全仓库调用点检查、计分板语料与生成命令检查。 |
| 1.6 四个无仓库内调用的版本访问器 | “仓库内未调用”属实，但不能据此认定为可安全删除。保留 Slots::names、knows_registry、enum_values、command_names。 | `lib.rs` 公开导出 version，snapshot 为公开模块；本地搜索不能证明不存在下游使用。 |

### 8.2 第二节：静默兜底

| 原条目 | 独立判定与处理 | 检查依据 |
| --- | --- | --- |
| modules.resolve 模块路径默认空数组 | 默认空数组确实存在，已改成建立路径表后按索引访问。但原报告把路径转换失败一概当成内部不变量，这不严谨：module_segments 可以返回 None。 | 非项目内 .mcl 路径现在明确报诊断并在使用表前返回；专门回归测试验证不会按入口模块处理或索引 panic。 |
| resolve 缺路径/根目录的 continue | 原写法存在；路径来源数组的下标由项目加载建立，可以直接访问；根目录/路径转换失败属于需要明确处理的分支，改为路径诊断。 | 路径生成、项目分组、模块加载调用链核对。 |
| paths 的 edges.get | 属实，每个已解析模块都无条件插入依赖图，目标也来自模块表，改成索引。 | 跨模块、限定名、标准库与导入环相关语料通过。 |
| analysis 分组默认值、诊断 filter_map 与符号 continue | 属实，索引来源全部由解析、分组及扩展标准库过程建立；改成索引/明确 expect。空输入直接返回空分析。 | 内存模块分析、解析失败恢复、空输入、LSP 测试与完整语料。 |
| codegen 宏函数表缺项 continue | 属实，用户函数和 helper 在宏处理前均插入，且不删除；改为带明确不变量说明的 expect。 | 生成器插入点与删除点搜索；宏转发命令检查与双语生成一致性检查。 |
| translate/token 的字符默认值 | 属实，扫描循环保证剩余切片非空，改为 expect。 | 全翻译测试及 Unicode 输入覆盖。 |
| translate/rewrite 的 split 默认 frame | 属实，split 必定至少产生一个片段，改为明确 expect。 | 具名调用、条件、宏、嵌套调用翻译回归。 |
| parser 空 token 流默认 span | 属实，本 crate 私有解析入口均接收 lexer 的 Eof 结尾流，默认 span 无法防止后续越界；改为明确 expect。 | lexer 结尾无条件追加 Eof，全部解析调用点与空源码路径核对。 |

### 8.3 第三节：parser 与 codegen

| 原条目 | 独立判定与处理 | 检查依据 |
| --- | --- | --- |
| parser 的 26 处 unreachable | 数量属实，全部清除。23 处分派失败改为带关键词的解析诊断；attribute 展示表和 AST 值从同一宏表生成；self 保存/恢复合并成 if；BossBar setter 使用局部枚举穷尽分派。 | Git 原版 26 处与现源码逐项核对；现 parser 中无 unreachable；深嵌套、语法恢复、LSP 后续编辑存活测试通过。没有大范围改写其它关键词函数的返回类型。 |
| “只有 parser 的这一类会崩 LSP” | 不属实，store 的代码生成也存在真实 panic，见 8.5。 | 本次运行的最小用例得到 exit code 101。 |
| entity_query_selector 重复守卫与 Include 臂 | 属实，移除 base_is_tag 守卫与 Include 分派；保留 Brigadier 限制注释。 | validate_entity_query 对每个声明执行，并拒绝 Include 和具体基类型后的排除；实体命令输出及错误语料通过。 |
| styled JSON 第二次解析 | 属实，两个校验入口共用 StyleCache，每种原始字符串只解析一次；合法对象由 Validated 传入 codegen。 | rename 只遍历 Fixed 组件、不改 styled 字符串；声明、运行期 numberformat、非法 JSON/非对象输入及原生命令检查通过。 |
| requirements 的 to_value/expect | 属实，直接构造 Value::Array/Value::String。 | all/any 的原有 AND/OR 嵌套结构不变，进度生成与原版进度语料通过。 |
| expressions 临时值复用的重复 match | 属实，合并成一次 if let，返回 target 与 reuse 标记。 | 运算、短路、写集、溢出与控制流语义测试通过。 |
| frozen_operand 的 Literal 兜底臂 | 属实，常量在稳定性检查时提前返回，改成 if let Cell，保留原 operand 的返回，不增加字符串复制。 | 操作数冻结与有副作用表达式的语义测试通过。 |
| 3.3 记录的其它 expect | 原调用链确有相应类型或长度不变量，保留 resource_json、查询/物品/存储/数据槽/函数查询、loop_state、单命令 pop、已解析 advancement conditions 的耦合。 | 校验先于 codegen、声明表建立、长度判断及循环上下文路径核对。 |

### 8.4 第四节：重复实现与重复计算

| 原条目 | 独立判定与处理 | 检查依据 |
| --- | --- | --- |
| run 反斜杠与斜杠前缀重复检查 | 属实，用户命令字符串统一在 parser::command_string 检查；删除 validate_raw_command 的重复项及前缀剥离。 | run 与 return run 共用该解析函数；原始命令错误语料、权限校验通过。 |
| finish 命令自检 | 真实输出边界，保留，并提取为 check_commands。错误信息附前 80 个字符的命令预览，只在出错时构造预览。 | 大于 2000000 UTF-16 码元的结构化 help 命令依然被 CLI 与 analyze 同样拒绝；换行/NUL/续行守卫保留。 |
| 四份静态注册表存在性/候选逻辑 | 重复实现属实，统一调用返回 bool 的 validate_static_id。必须精确存在的代码注册表使用 registry_contains_exact；允许数据包扩展的 validate_id 保留原语义。 | 地物类型、战利品类型、recipe serializer、界面类型的入口核对；外部命名空间的静态类型仍拒绝，拼写候选与原错误语料通过。 |
| 两份 LootCondition codec | 重复且不一致属实，抽取 loot_conditions 公共模块，统一 type 建议、inverted、terms 和 random_chance 检查。资源根节点保持 direct codec，进度/嵌套字段支持 holder；listOf 字段保持数组限制。 | 原源码 LootItemCondition、RegistryCodecs、HolderSetCodec 与 RandomChance codec 核对；两个入口同样的合法/非法形状回归测试通过。 |
| 组合 terms 只允许数组/标签的判定 | 原进度入口过严：holderSet 使用 alwaysUseList=false 的 compactListCodec，允许单个条件对象或普通谓词引用。已修复，并从 invalid 语料删除原来标错的合法用例。 | 随附 HolderSetCodec.java 的 directCodec；单对象、单引用、数组、空数组、#标签的两个入口测试。 |
| 中文别名两套机制 | 属实，计分板分组归入 keywords::scoreboard_group，翻译从该表派生；运行测试/停止测试归入 command_value；新增 test 接收者翻译。 | 保留“玩家分数”术语，并接受已有 ui_value 的“玩家列表”；两个术语位于不同语境表，避免破坏单表中英一对一；解析、翻译往返与命令输出检查。 |
| NBT path 两个相同函数 | 属实，组件路径直接复用 rules::valid_nbt_path。 | NbtPath::parse 调用相同，相关错误与正确语料通过。 |
| UUID 两份检查 | 属实，复用 rules::valid_uuid。 | 原长度、ASCII 十六进制与连字符位置约束一致，文本组件与 fetchprofile 语料通过。 |
| styled 对象检查两份 | 属实，与前述 StyleCache 同时收敛为 parse_style。 | 声明与运行期格式共同测试。 |
| bossbar.get 的 word_matches/command_value | 属实，前者包含于后者，保留 command_value 单判据。 | get/取 的词表与表达式、界面命令语料。 |
| metadata.readonly + OpenOptions | 重复属实，保留整批 OpenOptions 可写性预检，删除 metadata 只读检查。 | 现有 project_failures_leave_earlier_files_unchanged 实测后续只读文件失败时前面文件不被修改。 |
| 宏实体来源重复 validate_holder | 属实，实体来源只走 entity_target；非实体来源仍走 NBT 来源校验。 | 未声明查询、单实体限制、origin 和宏转发路径核对。 |
| 通用 holder 与专用约束重复诊断 | 属实，data 写入的无上下文 self 只报告通用上下文错误；不允许 origin 的命令优先报告专用错误；scoreboard.operation 分开校验 objective 与受限 holder。 | 所有相关入口仍检查查询和目标声明，不删 deduplicate；完整错误语料通过。 |
| 三个永不生效的守卫 | 属实，删除两处 summonable 判定前的 valid_resource_location 和 valid_json_id 的额外空串判定。 | non_summonable_entity 的两个字面量均合法；空串规范化为 minecraft: 后仍无效。 |
| has_stores 与 stage 重复状态 | 属实，删除 has_stores，统一用 stage == 2。 | stage 只向更大值推进，store.data/result/success 语料与空块拒绝检查。 |
| 重复 is_virtual_path/first_span | 属实，模块路径循环每个模块只计算一次 builtin 与 span；first_span 也改成惰性查询首个声明，不建立临时 Vec。 | 保持原声明类别优先级；模块路径诊断与跨模块语料通过。 |
| hover 重建 LineIndex | 属实，索引由 Project 按源码建立，内容不变时复用，编辑后重建；诊断、hover、definition 和位置换算使用同一缓存。 | 中文、emoji、CRLF、越界位置与编辑后缓存更新测试通过。 |
| entity_nbt::suggest 复制 HashSet | 属实，删除 allowed.cloned，直接借用候选集合。 | 所有中文别名实际存在、双向唯一及实体 NBT 语料通过。 |

### 8.5 第五节：原报告未确认及附带问题

1. **store panic 已独立复现并修复。** 最小源代码：

   ```mcl
   namespace audit;
   objective stored;
   @entity fn main() {
       execute store.result(self, stored) {
           for i in 5..5 {}
       }
   }
   ```

   修改前运行 build，在 compile_execute_body 的 expect 处 panic，exit code 101。原报告的 counter += 1 前缀让命令数组非空，因此未复现崩溃，但存在把 store 捕获到前缀命令上的错误风险。现在把终止于 For 的 store 块纳入与其它不传递结果控制流相同的诊断规则；空/反向/非空 for 和带前缀情况均有 CLI 与 analyze 回归测试。

2. **LSP 跑完整 compile 属实，已减少无用产物构造。** analyze 改为 check：共用语义检查、rename、命令生成及输出自检，不生成 mcfunction 文本、资源 JSON、进度 JSON、标签 JSON 和 pack.mcmeta。仍保留命令生成，因为完全跳过它会遗漏输出长度等真实错误；超长结构化 help 的 CLI/analyze 一致性测试证明了这一点。

3. **硬编码 TRIGGERS 与快照重复属实，但不能直接以旧快照替换。** 原常量有 58 个注册名，旧快照只有 57 个，唯一差异是生成器有意跳过无字段的 impossible。先修改生成器保留空字段条目，再以 trigger_fields 查询替代硬编码常量。impossible 的 Codec.unit 无字段校验语义保留，原版进度全部条件语料通过。

4. **give 诊断打印错误最大堆叠数属实，已修复。** 消息读取 component_stack_size，与实际上限使用同一来源；只通过组件设置为 64 时，超量诊断现在正确显示范围 1 到 6400、最大堆叠数 64。

5. **别名文档引用不存在的测试属实，已补齐测试。** tests/audit_regressions.rs 实际定义 chinese_aliases_resolve_to_real_tags，检查每个中文/英文键唯一、实际快照键存在且两个查询方向对应，全部通过。

### 8.6 第六节：保留项复核

用户源码的语法、数值/资源位置/重复声明/坐标约束仍是输入边界；真实 IO 的符号链接、路径穿越、ZIP 容量和输出路径检查仍处理可发生的失败。MAX_NESTING、表达式深度、unroll 展开预算与 16 MiB 工作线程分别保护不同阶段；Windows 子进程的深输入测试仍通过。

constant_blocker 的第二次遍历提供无法折叠的原因；空距离交集回退确实可达；原样物品查询拼接、粒子/声音可选参数补位与参数/局部/全局名字查找均有实际语义。JSON-RPC 字段和报文校验面对外部输入；mtime 缺失时不复用磁盘缓存、发现失败时清空列表均保留。版本生成器继续严格检查外部 Minecraft 数据，名字遍历仍是一份实现。

deduplicate 不能删除：unroll 复制语句体后相同 span 的重复诊断仍可能发生；本次仅删除具体重复入口。对原报告列为“无发现”的文件未做无关改写，src/modules/declarations.rs 只做本报告指出的 first_span 分配优化。

### 8.7 最终验证

- cargo test --offline -- --test-threads=4：**59 个普通测试及 1 个文档测试，全部通过**，其中新增 14 个测试。
- cargo clippy --offline --all-targets -- -D warnings：**通过，零警告**。
- cargo fmt --all --check：通过。
- cargo xtask check-version-data：通过，所有版本快照与重生成结果逐字节一致。
- git diff --check：通过。
- 提交前复审发现文档工具仍读取旧 ATTRIBUTES 常量与 TRIGGERS 常量，导致构建失败；已改为提取 attributes! 表与触发器快照，并同步网页翻译器的 test 接收者和 scoreboard 分组表，重新生成 docs/index.html。手册补充 store 块尾 for 的拒绝规则，依赖说明同步 serde。
- bun test docs/tools/translate.test.mjs：30 个测试通过；新增 test.run/test.stop 共享翻译语料同时由 Rust 与网页翻译器验证。
- bun docs/tools/build.mjs --self-test：16 个完整示例、148 个生成文件，以及仓库示例的 39 个源码文件均通过双语产物一致性检查。
- 专项覆盖包括：双语产物逐字节一致、可复现构建/ZIP、原生命令形状、输出预算、控制流副作用、只读文件整批预检、深输入与 LSP 存活、Unicode 位置、快照错误结构、两个 codec 入口的一致性、store 崩溃及样式 JSON 复用。

所有原报告的具体条目均已给出处理或保留判定。公开 API、真实边界保护和必要的类型耦合有明确保留理由；没有遗留未实现的修复项。
