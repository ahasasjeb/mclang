# Mclang DEVELOPMENT_PLAN — Java Edition 26.3

## 1. 命令覆盖

统计口径：只统计 Java Edition 26.3 中按默认函数权限等级 2 可从数据包函数调用的命令族。管理员、OWNER、开发构建和 IDE 条件命令不计入完成度；原始命令字符串不计结构化覆盖。

- 命令族：68
- 已完成：68
- 部分完成：0
- 未实现：0
- 结构化覆盖：68/68（100%）

### 1.1 已完成

`advancement`, `attribute`, `bossbar`, `clear`, `clone`, `compute`, `damage`, `data`, `datapack`, `defaultgamemode`, `dialog`, `difficulty`, `effect`, `enchant`, `execute`, `experience`, `fetchprofile`, `fill`, `fillbiome`, `forceload`, `function`, `gamemode`, `gamerule`, `give`, `help`, `item`, `kill`, `list`, `locate`, `loot`, `me`, `msg`, `particle`, `place`, `playsound`, `posteffect`, `random`, `recipe`, `reload`, `return`, `ride`, `rotate`, `say`, `schedule`, `scoreboard`, `seed`, `setblock`, `setworldspawn`, `spawnpoint`, `spectate`, `spreadplayers`, `stopsound`, `stopwatch`, `summon`, `swing`, `tag`, `team`, `teammsg`, `teleport`, `tellraw`, `test`, `time`, `title`, `trigger`, `version`, `waypoint`, `weather`, `worldborder`。

### 1.2 本轮补全

- `return run` 可接结构化命令语句；字符串形式仍作为兼容入口，并计入严格模式的 raw 统计。
- `scoreboard` 补齐运行期 objectives list/remove/modify 与 players list/add/remove/reset/display name/numberformat；目标声明仍负责 load 时创建。
- `fetchprofile`、`help`、`test`、`version`、`seed`、`me`、`say` 已有结构化入口；`test` 覆盖数据包可调用分支，IDE-only export 分支不计。

### 1.3 未实现

本统计口径内无未实现的命令族。

### 1.4 别名

别名不单独计入 68 个命令族：`tell`、`w` 跟随 `msg`；`tm` 跟随 `teammsg`；`tp` 跟随 `teleport`；`xp` 跟随 `experience`。

### 1.5 不计入完成度

`debug`, `kick`, `tick`, `jfr`, `ban`, `ban-ip`, `banlist`, `deop`, `op`, `pardon`, `pardon-ip`, `perf`, `save-all`, `save-off`, `save-on`, `setidletimeout`, `stop`, `transfer`, `whitelist`, `publish`, `unpublish`, `chase`, `raid`, `debugpath`, `debugmobspawning`, `warden_spawn_tracker`, `spawn_armor_trims`, `serverpack`, `debugconfig`。

这些命令可保留在版本数据中供原始命令校验和诊断使用，不计入数据包命令完成度。

## 2. 共享参数能力

| 能力 | 当前状态 | 计划 |
| --- | --- | --- |
| 资源位置 / 注册表引用 | 较高 | 统一资源引用类型；覆盖普通资源与 tag 引用。 |
| 实体选择器 | 较高 | 已补重复 scores 合并、非负区间、SNBT/advancements 形状和 team 单词检查；后续让版本数据提供完整 option 形状。 |
| NBT / SNBT | 较高 | 结构化 NBT 已校验并序列化；实体标签快照现覆盖 26.3 源码中的直接键、列表、接口/辅助方法及动态传入的已知键（161 种实体、305 个顶层键）；NBT path 匹配复合检查键值与嵌套结构。仍需补完整 SNBT 数值/内置运算语法、方块实体字段级校验和 round-trip 语料。 |
| TextComponent | 较高 | 已补 `object` 的 atlas/player、click `show_dialog/custom`、hover `show_item/show_entity`；后续扩展 item 组件补丁与 player profile。 |
| 物品组件 | 部分 | 已按 26.3 codec 检查常用字段类型、范围、枚举、资源引用及 `food`、`use_cooldown`、`use_effects`、`weapon`、`attack_range`、`enchantable`、`custom_model_data` 结构；继续覆盖其余组件。 |
| 物品谓词 | 部分 | 已校验 `count`、`damage`、`potion_contents`、附魔、纹饰、烟花、成书、唱片和村民类型的常用字段、引用与区间；继续覆盖集合等复杂子谓词。 |
| 方块状态 | 较高 | 已从 26.3 方块模型资产生成逐方块属性取值快照；继续补不影响模型的状态属性。 |
| 粒子 options | 较高 | 已按 ParticleType codec 检查必需字段、类型、范围和常用嵌套结构；继续扩展特殊粒子。 |
| NBT path | 较高 | 已建成员、索引、全列表、列表匹配、成员/根复合匹配 AST；匹配复合现检查 SNBT 键值、列表与嵌套结构，继续覆盖完整数值及内置运算语法。 |
| MessageArgument | 部分 | `msg`/`teammsg`/`say`/`me` 已检查长度与续行风险；带选项选择器暂时拒绝，后续接入完整 Brigadier 解析。 |
| 坐标 / 旋转 / 时间 / 范围 | 较高 | 统一 value type 与范围检查接口。 |

共享参数示例在 `examples/shared_arguments.mcl`，有效/无效语料在 `tests/valid/shared_arguments` 与 `tests/invalid/shared_*`。方块状态快照位于 `block_states.json`，由 `cargo xtask generate-version-data` 重建。本轮扩充了物品组件、物品子谓词与 NBT path 匹配复合的检查。

## 3. 数据包资源能力

| 能力 | 当前状态 | 计划 |
| --- | --- | --- |
| `pack.mcmeta` | 已支持 | 默认生成 description、min/max format；可用 `assets/pack.mcmeta` 提供完整模板并校验已知字段。 |
| overlays | 已支持 | 校验 overlay entries/版本范围，并复制 `assets/overlays/<目录>/` 布局。 |
| `filter` | 已支持 | 校验 block 列表与 namespace/path 字段。 |
| features | 已支持 | 校验 enabled 资源位置列表。 |
| `pack.png` | 已支持 | 自动校验并复制 `assets/pack.png`。 |
| `.mcfunction` | 已支持 | 保持函数生成与命名空间输出。 |
| load/tick function tag | 已支持 | 保持自动生成与用户声明合并。 |
| 普通 function tag | 已支持 | 保持 `replace`、嵌套 tag 和引用检查。 |
| 其它 registry tag | 部分 | 已可用 `resource "tags/<注册表>"` 输出标签 JSON，并检查 `values`、`replace` 与可选 `required`；后续提供结构化 typed tag 和引用图。 |
| predicate | 部分 | 建 typed predicate schema，并与 execute/item/advancement 复用。 |
| loot table | 部分 | 分阶段覆盖 pools、entries、functions、conditions、number providers。 |
| item modifier | 部分 | 与 loot function 模型共用 schema。 |
| advancement | 部分 | `requirements = all/any` 已按原版“外层 AND、内层 OR”生成；conditions 会按 26.3 trigger codec 检查常见形状、注册 loot condition type 并递归验证组合节点，`victims`/`items`/`ingredients` 这类 `listOf()` 字段按数组校验；继续结构化其余条件 codec。 |
| recipe | 部分 | 覆盖常用 vanilla recipe serializer。 |
| dialog | 部分 | 根据 26.3 dialog codec 建 typed schema。 |
| enchantment / provider / trade / timeline 等动态注册表资源 | 部分 | 使用通用 codec schema 框架提供字段校验。 |
| worldgen 资源 | 部分 | 提供 typed reference、codec 校验和常用 DSL。 |
| dimension / dimension_type | 部分 | 补 typed schema 与跨资源引用检查。 |
| structure `.nbt` | 部分 | 已按 `assets/structure/**/*.nbt` 复制 gzip NBT 并加入构建清单；后续校验完整 NBT 结构及引用。 |
| test_instance / test_environment | 部分 | 与 `test` 命令一起提供 typed resource。 |
| storage | 部分 | 支持显式 storage ID、初始化和 typed helper。 |
| 跨资源引用图 | 部分 | 检查函数、tag、advancement、predicate、loot、dialog、worldgen 等引用。 |
| JSON codec 字段校验 | 较少 | 已按 26.3 注册表核对 predicate/recipe 类型，并检查常见配方与战利品池的必需字段；仍需统一 schema IR、完整字段和原版加载验收。 |
| 数据包 ZIP | 已支持 | `mclang build --zip` 生成根目录正确且字节可复现的 ZIP。 |
| feature flag 诊断 | 待实现 | 对依赖 feature 的资源或命令给出静态提示。 |

资源层可建设可复用 schema IR，再逐类增加更易写的语法。提取不稳定的 codec 可暂时保留受控 raw，并在诊断中标明未做字段级校验。

## 4. 编译器与语言功能

| 功能 | 当前状态 | 计划 |
| --- | --- | --- |
| 词法 / 语法 / AST | 较高 | 给 AST 节点提供稳定 span/ID，供诊断、LSP 和 source map 使用。 |
| 模块系统 | 较高 | 增加项目根和 manifest；继续检查循环、冲突和导入导出。 |
| 类型系统 | 中等 | 增加 `const`、bool、resource/tag 等常用类型。 |
| 表达式 | 中等 | 常量比较直接下降为 `matches` 范围，常量除零与 i32 溢出会诊断；继续增加一等 bool，并明确运行期计分板除法、取模与溢出语义。 |
| 控制流 | 较高 | 保留短路与表达式求值顺序；简单条件和安全单命令块直接内联，无跳转循环省略状态。多实体执行、store 赋值结果和失败条件取反均保留语义边界；后续可增加 `match/switch`。 |
| 函数调用 | 较高 | 整理参数、返回值、macro 调用规则和诊断。 |
| raw 入口 | 已有 | 保留兼容入口；增加严格模式统计哪些代码仍依赖 raw。 |
| 诊断 | 较高 | 增加诊断码、warning 等级、JSON 输出、related span、fix-it。 |
| 项目配置 | 待实现 | 增加项目配置文件，保存 namespace、description、output、strict policy、libraries、pack metadata 等。 |
| source map | 待实现 | 记录生成 `.mcfunction` 行与源文件 span/symbol 的对应关系。 |
| 多版本数据 | 待实现 | 把版本号和生成快照路径从散落常量收敛到统一版本配置；支持 26.3，并保留扩展其它版本的数据结构。 |
| 本地库 | 待实现 | 支持只读 library roots、稳定解析顺序和冲突诊断。 |

`examples/monster_market` 的复杂市场语料用于检查真实产物。当前产物为 528 条函数命令、66 个 mcfunction，其中 24 个位于 `__mcl`；安全单命令块与简单条件已直接内联，算术赋值直接写入目标，复用的常量在加载时统一初始化。多实体执行仍保留逐来源求值边界，store 不会丢失自赋值结果，查询失败后的取反与是否带 else 无关。

编译期优化的分层与适用条件：

- **原生条件链**：连续嵌套 `if` 合并成一条 `execute if a if b if c run …`，不再产生 `run execute …` 的嵌套派发；`a && a`、`if a { if a { … } }` 这类同一纯条件只保留一次，含函数调用或随机 predicate 的条件不去重。
- **整数范围传播**：沿同一条条件链维护 `x ∈ [min, max]` 与单个排除值，`x > 10` 之后 `x > 5` 直接判定为真、`x < 5` 判定为假，矛盾条件整段删除，`== v` 会收紧成单点。
- **副作用保护**：`f() || 已知为真` 仍必须调用 `f()`，所以“编译期可判定为真”的条件只有在无副作用时才允许删除；带副作用的条件退回原标志路径（`ScoreFacts` 对含调用/谓词/随机的条件一律返回 Unknown）。
- **`&&` 右侧比较折叠**：右侧是比较且左侧已算出 0/1 标志时，把右侧的否定子句直接并进合并命令，省掉第二个标志计分项的 `set 0` / `run set 1` 两条命令（例如 `choice == 1 && charge(price()) == 1`）。
- **常量与不可达**：常量条件在编译期选定分支，`return` 之后同一生成块里的语句不再输出，被紧跟其后的常量赋值覆盖的赋值也会裁掉。

对应回归语料位于 `tests/valid/control_semantics/` 和 `tests/valid/optimizations/`；`tests/control_optimization.rs` 里的 mini 解释器会在边界值上对比源码语义与生成命令语义。在 `examples/**` + `tests/valid/**` 共 54 个语料上，优化后命令总数 3029 → 2970、`execute` 1080 → 1026、`scoreboard` 1434 → 1376、临时计分项 409 → 383、产物字节数 −2.4%（monster_market 540 → 528）。

**有意不做的优化**（都会改变运行时行为，或收益不抵风险）：

- `if c { A } else { B }` 不能改写成 `execute if c run A` + `execute unless c run B`：两条命令之间 B 的求值发生在 A 之后，A 若写入 `c` 读取的计分项，`unless` 会看到新值而多执行一次 B。除非能证明 A/B 不写 `c` 的输入，否则保留一次性标志。
- 不用 `execute store success … run execute if …` 合并标志：26.3 的 `ExecuteCommand.addConditional(...).executes(...)` 在条件为假时抛 `ERROR_CONDITIONAL_FAILED`（见 `createNumericConditionalHandler`），裸条件没有 `run` 就没有 fork 兜底。
- `&&` 之外的标志合并、跨语句的区间传播（例如 `if x > 10 { …; if x > 5 { … } }`）需要先有“语句是否写入某计分项”的写集分析；本次没有引入，相关形状保持原样。

后续可继续让 `scoreboard.get(self, objective)` 保留原目标作为表达式操作数，避免复制到内部临时项，并把区间传播扩展到能证明无写入的跨语句分支。

编译器自身重复工作的消除（1024 个 advancement 的压力语料，release 构建，均为最小值）：`check` 63.02 ms → 26.02 ms，目录构建 402 ms → 384 ms，无变化的重复构建 485 ms → 137 ms。收益来自模块只解析一次、conditions 只做一次 JSON parse、版本快照按注册表/枚举/命令/触发器分别惰性解析，以及“内容未变不重写文件、只删除清单里消失的文件”的增量输出。

版本数据生成器建议补一份机器可读命令可用性文件，记录根命令、别名、注册条件、权限节点和可执行叶；命令覆盖统计由该文件校验。

## 5. 内置标准库

已提供源码内嵌的 `std::math`、`std::state`、`std::time`、`std::random`。编译器与内存分析入口均按显式 import 加载；无需项目本地库文件，严格模式可用。示例见 `examples/stdlib_demo/`，有效语料见 `tests/valid/stdlib/`。导入时不会自动创建 tick/load 逻辑。后续模块按数据包需求逐步扩展。

| 模块 | 建议内容 |
| --- | --- |
| `std/math` | 已有 `abs`、`min/max`、`clamp`、`sign`；待补 `gcd/lcm`、整数 `pow`、`isqrt`、floor_div/mod helper。 |
| `std/state` | 已有 0/1 规范化、toggle、latch、rising_edge、falling_edge；待补 once。 |
| `std/time` | 已有 tick/second 换算、interval、cooldown ready、remaining；待补 deadline、elapsed、debounce。 |
| `std/debug` | `assert`、条件 assert、score/NBT dump、日志和 trace marker。 |
| `std/test` | assertion、fixture setup/teardown、GameTest helper、失败上下文。 |
| `std/random` | 已有百分比与千分比 chance；待补 weighted choice、shuffle index、range helper。 |
| `std/storage` | typed get/set/remove、list push/pop、stack/queue、小型 map 约定。 |
| `std/entity` | 单实体选择、存在性、nearest、临时 tag、origin helper。 |
| `std/inventory` | give-or-drop、检测/计数/移动槽位、容器搬运、保存/恢复模板。 |
| `std/text` | component builder、变量插值、title/actionbar/message wrapper。 |
| `std/world` | region/box、维度执行、时间/天气/边界组合 helper。 |
| `std/collections` | 基于 storage 的 list/stack/queue/set-like 约定。 |
| `std/fixed` | 定点数乘除、比例和百分比。 |

后续需要内部 objective、tag 或 storage 的模块应使用编译器保留命名空间。

## 6. 工具链功能

| 功能 | 计划 |
| --- | --- |
| 项目配置 | 从配置文件读取 namespace、description、output、strict policy、libraries、pack metadata；CLI 可覆盖。 |
| `mclang fmt` | 稳定格式化、保留注释、中英文关键词不互改、支持 `--check`。 |
| JSON diagnostics | 输出 code、severity、message、file、range、related、fixes。 |
| `mclang build --zip` | 已实现；生成可直接分发且可复现的数据包 ZIP。 |
| warning policy | 支持 warning 类别与 allow/deny。 |
| `mclang init` | 生成最小项目结构。 |
| `mclang clean` | 清理当前项目构建产物。 |
| `mclang test` | 编译测试项目，并提供可选 GameTest/服务器适配入口。 |
| `--emit` | 输出 lowered IR、命令树、resource graph、source map 等调试信息。 |
| 本地库路径 | 从项目配置声明 library roots。 |
| 文档生成 | 从符号与注释生成 API / stdlib 文档。 |
| 第三方包管理 | 增加 lockfile、内容哈希和 namespace 冲突处理。 |

## 7. LSP / VS Code 功能

当前已有 completion、hover、definition、diagnostics。

| 功能 | 计划 |
| --- | --- |
| member completion | 支持命令模块、stdlib 模块和声明成员补全。 |
| signature help | 显示函数、宏、内置 helper 参数。 |
| find references | 支持跨模块函数、目标、query、item、resource、tag。 |
| rename | 只修改符号引用；资源路径按规则处理。 |
| document/workspace symbols | 导航函数、资源、objective、query、item、storage、tag。 |
| formatting | 复用 CLI formatter。 |
| code actions | 未知 ID 候选、缺失 import、可修复语法。 |
| semantic tokens | 区分命令、资源、宏参数、objective、query、raw。 |
| inlay hints | 展示推断类型和资源类型。 |
| call hierarchy | 展示函数、tag、schedule 调用关系。 |

## 8. 测试与覆盖清单

建议维护三份机器可检查清单：

- `command_coverage.json`：68 个命令族和 5 个别名，记录状态、已覆盖叶、排除叶和对应测试。
- `resource_coverage.json`：每类资源记录 raw output、typed schema、DSL 三档支持情况。
- `argument_coverage.json`：TextComponent、NBT path、item component、particle options、block state 等共享参数能力。

命令测试建议覆盖有效输入、无效输入、最终 `.mcfunction` golden、中英文语法产物一致性，以及严格模式下的 raw 依赖检查。资源测试建议覆盖 schema 正例、字段错误、资源引用错误、结构 NBT fixture 和 ZIP 目录结构。
