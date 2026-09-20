# Mclang DEVELOPMENT_PLAN — Java Edition 26.3

## 1. 命令覆盖

统计口径：只统计 Java Edition 26.3 中按默认函数权限等级 2 可从数据包函数调用的命令族。管理员、OWNER、开发构建和 IDE 条件命令不计入完成度；原始命令字符串不计结构化覆盖。

- 命令族：68
- 已完成：59
- 部分完成：2
- 未实现：7
- 结构化覆盖：59/68（约 86.8%）

### 1.1 已完成

`advancement`, `attribute`, `bossbar`, `clear`, `clone`, `compute`, `damage`, `data`, `datapack`, `defaultgamemode`, `dialog`, `difficulty`, `effect`, `enchant`, `execute`, `experience`, `fill`, `fillbiome`, `forceload`, `function`, `gamemode`, `gamerule`, `give`, `item`, `kill`, `list`, `locate`, `loot`, `msg`, `particle`, `place`, `playsound`, `posteffect`, `random`, `recipe`, `reload`, `ride`, `rotate`, `schedule`, `setblock`, `setworldspawn`, `spawnpoint`, `spectate`, `spreadplayers`, `stopsound`, `stopwatch`, `summon`, `swing`, `tag`, `team`, `teammsg`, `teleport`, `tellraw`, `time`, `title`, `trigger`, `waypoint`, `weather`, `worldborder`。

### 1.2 部分完成

| 命令 | 缺口 |
| --- | --- |
| `return` | `return run <command>` 仍依赖原始命令字符串，需要结构化命令节点。 |
| `scoreboard` | 补 `objectives list/remove/modify`，包括 `displayname`、`displayautoupdate`、numberformat/rendertype；补 `players list/add/remove/display name/display numberformat`。 |

### 1.3 未实现

| 命令 | 需要实现 |
| --- | --- |
| `fetchprofile` | `name`、`id`、`entity`。 |
| `help` | 数据包可调用的命令入口。 |
| `test` | `run`、`runmultiple`、`runthese`、`runclosest`、`runthat`、`runfailed`、`verify`、`locate`、reset/clear、`stop`、`pos`、`create`；IDE-only export 分支不计。 |
| `version` | 零参数入口。 |
| `seed` | 零参数入口。 |
| `me` | 原生命令入口，不用 `message.all` 代替。 |
| `say` | 原生命令入口。 |

### 1.4 别名

别名不单独计入 68 个命令族：`tell`、`w` 跟随 `msg`；`tm` 跟随 `teammsg`；`tp` 跟随 `teleport`；`xp` 跟随 `experience`。

### 1.5 不计入完成度

`debug`, `kick`, `tick`, `jfr`, `ban`, `ban-ip`, `banlist`, `deop`, `op`, `pardon`, `pardon-ip`, `perf`, `save-all`, `save-off`, `save-on`, `setidletimeout`, `stop`, `transfer`, `whitelist`, `publish`, `unpublish`, `chase`, `raid`, `debugpath`, `debugmobspawning`, `warden_spawn_tracker`, `spawn_armor_trims`, `serverpack`, `debugconfig`。

这些命令可保留在版本数据中供原始命令校验和诊断使用，不计入数据包命令完成度。

## 2. 共享参数能力

| 能力 | 当前状态 | 计划 |
| --- | --- | --- |
| 资源位置 / 注册表引用 | 较高 | 统一资源引用类型；覆盖普通资源与 tag 引用。 |
| 实体选择器 | 较高 | 补互斥条件、上下文诊断，并让版本数据提供 option 形状。 |
| NBT / SNBT | 较高 | 补 round-trip 测试；把 SNBT 语法校验和资源 codec 字段校验分开。 |
| TextComponent | 部分 | 补 `object` 内容、click `show_dialog/custom`、hover `show_item/show_entity`。 |
| 物品组件 | 部分 | 根据 26.3 component codec 补字段类型、范围、枚举、资源引用与嵌套结构校验。 |
| 物品谓词 | 部分 | 根据 component predicate codec 补字段 schema。 |
| 方块状态 | 部分 | 按具体方块校验 property 名和值。 |
| 粒子 options | 部分 | 根据 ParticleType codec 建字段 schema。 |
| NBT path | 部分 | 建结构化 AST，覆盖成员、索引、列表元素、match compound 等常用形式。 |
| MessageArgument | 部分 | 建独立 Message 类型，避免与 TextComponent、普通字符串混用。 |
| 坐标 / 旋转 / 时间 / 范围 | 较高 | 统一 value type 与范围检查接口。 |

## 3. 数据包资源能力

| 能力 | 当前状态 | 计划 |
| --- | --- | --- |
| `pack.mcmeta` | 部分 | 支持 description、min/max format 等常用字段。 |
| overlays | 待实现 | 支持 overlay entries、版本范围和目录布局。 |
| `filter` | 待实现 | 支持 namespace/path pattern。 |
| features | 待实现 | 支持 26.3 对应 feature flags。 |
| `pack.png` | 待实现 | 项目配置指定并复制。 |
| `.mcfunction` | 已支持 | 保持函数生成与命名空间输出。 |
| load/tick function tag | 已支持 | 保持自动生成与用户声明合并。 |
| 普通 function tag | 已支持 | 保持 `replace`、嵌套 tag 和引用检查。 |
| 其它 registry tag | 待增强 | 提供通用 typed tag，支持普通值和 `#tag`。 |
| predicate | 部分 | 建 typed predicate schema，并与 execute/item/advancement 复用。 |
| loot table | 部分 | 分阶段覆盖 pools、entries、functions、conditions、number providers。 |
| item modifier | 部分 | 与 loot function 模型共用 schema。 |
| advancement | 部分 | 将 conditions 等 raw JSON 字段逐步结构化。 |
| recipe | 部分 | 覆盖常用 vanilla recipe serializer。 |
| dialog | 部分 | 根据 26.3 dialog codec 建 typed schema。 |
| enchantment / provider / trade / timeline 等动态注册表资源 | 部分 | 使用通用 codec schema 框架提供字段校验。 |
| worldgen 资源 | 部分 | 提供 typed reference、codec 校验和常用 DSL。 |
| dimension / dimension_type | 部分 | 补 typed schema 与跨资源引用检查。 |
| structure `.nbt` | 待实现 | 支持读取、复制和打包已有结构文件。 |
| test_instance / test_environment | 部分 | 与 `test` 命令一起提供 typed resource。 |
| storage | 部分 | 支持显式 storage ID、初始化和 typed helper。 |
| 跨资源引用图 | 部分 | 检查函数、tag、advancement、predicate、loot、dialog、worldgen 等引用。 |
| JSON codec 字段校验 | 较少 | 建统一 schema IR，表示字段、可选值、范围、枚举、资源引用和 union/dispatch。 |
| 数据包 ZIP | 待实现 | 提供构建为 ZIP 的输出方式。 |
| feature flag 诊断 | 待实现 | 对依赖 feature 的资源或命令给出静态提示。 |

资源层可建设可复用 schema IR，再逐类增加更易写的语法。提取不稳定的 codec 可暂时保留受控 raw，并在诊断中标明未做字段级校验。

## 4. 编译器与语言功能

| 功能 | 当前状态 | 计划 |
| --- | --- | --- |
| 词法 / 语法 / AST | 较高 | 给 AST 节点提供稳定 span/ID，供诊断、LSP 和 source map 使用。 |
| 模块系统 | 较高 | 增加项目根和 manifest；继续检查循环、冲突和导入导出。 |
| 类型系统 | 中等 | 增加 `const`、bool、resource/tag 等常用类型。 |
| 表达式 | 中等 | 增加一等 bool；明确整数除法、取模和溢出诊断。 |
| 控制流 | 较高 | 保持 if/while/for/break/continue/each；后续可增加 `match/switch`。 |
| 函数调用 | 较高 | 整理参数、返回值、macro 调用规则和诊断。 |
| raw 入口 | 已有 | 保留兼容入口；增加严格模式统计哪些代码仍依赖 raw。 |
| 诊断 | 较高 | 增加诊断码、warning 等级、JSON 输出、related span、fix-it。 |
| 项目配置 | 待实现 | 增加项目配置文件，保存 namespace、description、output、strict policy、libraries、pack metadata 等。 |
| source map | 待实现 | 记录生成 `.mcfunction` 行与源文件 span/symbol 的对应关系。 |
| 多版本数据 | 待实现 | 把版本号和生成快照路径从散落常量收敛到统一版本配置；支持 26.3，并保留扩展其它版本的数据结构。 |
| 本地库 | 待实现 | 支持只读 library roots、稳定解析顺序和冲突诊断。 |

版本数据生成器建议补一份机器可读命令可用性文件，记录根命令、别名、注册条件、权限节点和可执行叶；命令覆盖统计由该文件校验。

## 5. 建议内置标准库

| 模块 | 建议内容 |
| --- | --- |
| `std/math` | `abs`、`min/max`、`clamp`、`sign`、`gcd/lcm`、整数 `pow`、`isqrt`、floor_div/mod helper。 |
| `std/bool` / `std/state` | 0/1 规范化、toggle、latch、rising_edge、falling_edge、once。 |
| `std/time` | tick/second 换算、deadline、elapsed、interval、cooldown、debounce。 |
| `std/debug` | `assert`、条件 assert、score/NBT dump、日志和 trace marker。 |
| `std/test` | assertion、fixture setup/teardown、GameTest helper、失败上下文。 |
| `std/random` | chance、weighted choice、shuffle index、range helper。 |
| `std/storage` | typed get/set/remove、list push/pop、stack/queue、小型 map 约定。 |
| `std/entity` | 单实体选择、存在性、nearest、临时 tag、origin helper。 |
| `std/inventory` | give-or-drop、检测/计数/移动槽位、容器搬运、保存/恢复模板。 |
| `std/text` | component builder、变量插值、title/actionbar/message wrapper。 |
| `std/world` | region/box、维度执行、时间/天气/边界组合 helper。 |
| `std/collections` | 基于 storage 的 list/stack/queue/set-like 约定。 |
| `std/fixed` | 定点数乘除、比例和百分比。 |

标准库模块以显式调用为主；导入模块时不自动创建 tick/load 逻辑。需要内部 objective、tag 或 storage 时使用编译器保留命名空间。

## 6. 工具链功能

| 功能 | 计划 |
| --- | --- |
| 项目配置 | 从配置文件读取 namespace、description、output、strict policy、libraries、pack metadata；CLI 可覆盖。 |
| `mclang fmt` | 稳定格式化、保留注释、中英文关键词不互改、支持 `--check`。 |
| JSON diagnostics | 输出 code、severity、message、file、range、related、fixes。 |
| `mclang build --zip` | 生成可直接分发的数据包 ZIP。 |
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
