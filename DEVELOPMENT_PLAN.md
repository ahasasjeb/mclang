# Mclang 开发计划

## 目标

用 Rust 实现一门面向 Minecraft Java Edition 26.3-rc-2 的数据包编程语言。编译器把 `.mcl` 源文件转换为可直接放入世界 `datapacks` 目录的数据包，并在编译期报告词法、语法和语义错误。

标准层的判定准则不变：作者能否用有名称、有结构、可补全、可静态检查的语法描述意图；只能原样复制字符串的功能不算完成。本文件在 0.5 版能力之上，给出与 26.3-rc-2 原版命令的逐条对比、全部缺口，以及分阶段路线图。

## 版本基线与函数权限

- 目标版本固定在仓库内 `minecraft_client_26.3-rc-2/` 源码，数据包格式 `121.0`；资源类型、注册表、命令签名一律以该源码为准。
- 函数与函数标签目录依据 `ServerFunctionLibrary` 与 `Registries`，使用 `data/<namespace>/function` 和 `data/<namespace>/tags/function`。
- 26.3 的 `function-permission-level` 服务器属性默认 `GAMEMASTER`（等级 2，见 `DedicatedServerProperties:147`）。函数在编译期（`ServerFunctionLibrary`）与运行期（`FunctionCommand.modifySenderForExecution` 的 `withMaximumPermission`）都被限制在该等级，因此 `ADMIN`（3）/`OWNER`（4）命令无法从数据包函数合法执行。标准层默认按等级 2 规划；越级命令标记为“不可达”，只能出现在 `run` 字符串中并由编译器给出警告，未来可用 `--function-permission-level` 显式放开。

## 一、原版命令覆盖矩阵

状态含义：

| 状态 | 含义 |
| --- | --- |
| 完成 | 结构化语法覆盖该命令的常规数据包用法，编译期检查完整 |
| 部分 | 已有结构化入口，但子命令或参数面存在明确缺口 |
| 缺失 | 没有结构化入口，只能写 `run` 字符串或完全不可用 |
| 不可达 | 需要高于默认等级 2 的函数权限，数据包函数无法合法执行 |
| 不建模 | 可由其他结构化语句等价表达，或对数据包无意义 |

当前统计：26.3-rc-2 共 **97 个根命令名**（含 `xp`、`tp`、`tell`、`w`、`tm`、`me` 等别名）。其中 **14 个完整覆盖**（`return`、`schedule`、`effect`、`experience`/`xp`、`clear`、`stopwatch`、`clone`、`fillbiome`、`forceload`、`time`、`weather`、`gamerule`、`worldborder`、`locate`），14 个有部分结构化入口；33 个缺失；21 个默认权限不可达；15 个只读/工具/开发命令不计划建模。命令之外的数据包内容见 1.6，对应路线图为第 9 阶段。

### 1.1 执行、函数与数据核心

| 命令 | 原版形态（26.3-rc-2） | 状态 | 当前入口与缺口 |
| --- | --- | --- | --- |
| `execute` | `run`；`if`/`unless`（block、biome、loaded、dimension、score、blocks、entity、predicate、function、stopwatch、data、items、slots）；修饰符 as、at、positioned、rotated、facing、align、anchored、in、on（8 种关系）、summon；store result/success | 部分 | 只有 `execute "子句" {}` 字符串包装；缺全部结构化子句、条件与 store |
| `function` | `<fn>`；`<fn> <nbt>`；`<fn> with <entity\|block\|storage> [path]`；`#tag` | 部分 | `call`、表达式调用与 `#tag` 已支持，标签成员在编译期检查执行上下文与参数；缺 `<fn> <nbt>` 宏参数与 `with` |
| `return` | `<int>`；`fail`；`run <命令>` | 完成 | 计分返回值、store ABI、`return fail`、`return run` 全覆盖；`return run` 的命令文本计入 `--deny-raw` |
| `schedule` | `function <fn> <time> [replace\|append]`；`clear <fn>` | 完成 | 整数与浮点时间（按原版 `TimeArgument` 换算为游戏刻）、`replace`/`append`、`schedule.clear`、`#tag` 调度 |
| `data` | get；merge；remove；modify（insert/prepend/append/set/merge；from/string/compute/value；entity/block/storage） | 部分 | 仅 `item_list` 的 `modify set from` 与 `set value []`；无通用访问器与路径 |
| `scoreboard` | objectives（add/remove/list/modify：displayname/rendertype/displayautoupdate/numberformat）；players（set/get/add/remove/reset/enable/operation/display）；display | 部分 | 只有编译器内部 ABI objective；无用户计分板、显示槽、trigger 解锁 |
| `item` | replace/fill/override/modify（entity/block 目标、槽位集合、from/with/loot_modifier） | 部分 | 只有 `give(..., self.item)` 用的 `replace ... from entity ... contents` |
| `loot` | loot/fish/kill/mine + give/insert/replace/spawn | 缺失 | 战利品表资源可声明，但没有取用命令 |
| `advancement` | grant/revoke × only/from/until/through/everything | 缺失 | — |
| `recipe` | give/take `<recipe\|*>` | 缺失 | — |
| `reload` | — | 缺失 | — |
| `datapack` | enable/disable/list；create（不可达，需 OWNER） | 缺失 | — |
| `compute` | default/block/entity × float/integer provider | 缺失 | — |
| `stopwatch` | create/query/restart/remove | 完成 | `stopwatch.create/restart/remove(id)`；`query` 作为表达式并支持可选缩放，失败写入 0 |
| `random` | value/roll/reset | 缺失 | — |
| `test` | gametest 系列 | 不建模 | 测试框架，不属于数据包标准层 |
| `fetchprofile` | name/id/entity | 不建模 | 只输出可点击引用，无返回值 |

### 1.2 实体与玩家

| 命令 | 原版形态 | 状态 | 当前入口与缺口 |
| --- | --- | --- | --- |
| `summon` | `<entity> [pos] [nbt]` | 部分 | `spawn("id") {}` 可进入新实体上下文；缺坐标与初始 NBT |
| `give` | `<players> <item> [count]` | 部分 | 目标必须是已声明玩家查询，物品必须是 `item_stack` 定义；无内联任意组件 |
| `kill` | `[targets]` | 部分 | 只有 `self.remove()` |
| `tag` | `add`/`remove`/`list` | 部分 | 只有 `self.add_tag`/`self.remove_tag`；无多目标与 `list` |
| `team` | list/add/remove/empty/join/leave/modify（displayName/color/friendlyFire/seeFriendlyInvisibles/nametagVisibility/deathMessageVisibility/collisionRule/prefix/suffix） | 缺失 | — |
| `attribute` | get；base set/get/reset；modifier add/remove/value get | 缺失 | — |
| `effect` | give（时长/等级/隐藏粒子/infinite）；clear | 完成 | `effect.give`、`effect.give_infinite`、`effect.clear`；秒数与等级按 26.3 范围编译期检查 |
| `enchant` | `<targets> <enchantment> [level]` | 缺失 | — |
| `experience`（`xp`） | add/set/query（points/levels） | 完成 | `xp.add`/`xp.set`；`xp.query` 作为表达式，要求 `limit(1)` 玩家查询 |
| `clear` | `[targets] [item] [maxCount]` | 完成 | `clear(玩家查询[, 物品][, 数量])`，数量上限 2147483647 |
| `damage` | `<target> <amount> [damage_type] [at <pos>\|by <entity> [from <cause>]]` | 缺失 | — |
| `teleport`（`tp`） | 坐标/实体/朝向 | 缺失 | — |
| `ride` | mount/dismount | 缺失 | — |
| `rotate` | rotation/facing | 缺失 | — |
| `spreadplayers` | 中心/间距/范围 + `under` | 缺失 | — |
| `spectate` | `[target] [player]` | 缺失 | — |
| `trigger` | add/set | 缺失 | — |
| `gamemode` | `<mode> [players]` | 缺失 | — |
| `defaultgamemode` | `<mode>` | 缺失 | — |
| `difficulty` | 查询/设置 | 缺失 | — |
| `spawnpoint` | `[players] [pos] [rotation]` | 缺失 | — |
| `setworldspawn` | `[pos] [rotation]` | 缺失 | — |
| `waypoint` | list；modify color（含 hex/reset）/style | 缺失 | — |
| `swing` | `[targets] [hand] [animation] [duration]` | 缺失 | — |
| `emote`（`me`） | `<action>` | 不建模 | `message.all` 已能广播文本 |
| `kick` | `<players> [reason]` | 不可达 | 需要 ADMIN |
| `list` | `[uuids]` | 缺失 | 只读反馈，低优先 |

### 1.3 世界与方块

| 命令 | 原版形态 | 状态 | 当前入口与缺口 |
| --- | --- | --- | --- |
| `setblock` | `<pos> <block> [destroy\|keep\|replace\|strict]` | 部分 | `set_block(pos, block_state[, 模式])`；缺方块实体 NBT（依赖 1.4） |
| `fill` | `<from> <to> <block> [mode\|replace filter\|keep]` | 部分 | `fill(from, to, block_state[, 模式][, replace 过滤器])`；缺方块实体 NBT（依赖 1.4） |
| `clone` | 同/跨维度 + masked/filtered/force/move/normal/strict | 完成 | `clone(起点, 终点, 目标[, 选项...])`，选项顺序无关、重复报错 |
| `fillbiome` | `<from> <to> <biome> [replace filter]` | 完成 | `fill_biome(from, to, "生物群系"[, replace, "过滤器"])` |
| `place` | feature/jigsaw/structure/template | 部分 | `place.feature/jigsaw/structure/template`；feature 的内联 JSON 未建模 |
| `forceload` | add/remove/query | 完成 | `forceload.add/remove/remove_all/query`，绝对范围检查 256 区块上限 |
| `time` | set/add/pause/resume/rate/query + `of <clock>` | 完成 | `time.set/add/pause/resume/rate(..., [时钟])`；`time.query([时钟])` 与 `time.query_gametime()` 是表达式；26.3 世界时钟模型 |
| `weather` | clear/rain/thunder [duration] | 完成 | `weather.clear/rain/thunder([持续时间])` |
| `gamerule` | `<rule> [value]`，每条规则生成短名与全名两个字面量 | 完成 | `gamerule.set(规则, 值)` 与表达式 `gamerule.query(规则)`；规则名、类型与范围来自 26.3 `GameRules` |
| `worldborder` | add/set/center/damage amount/buffer/get/warning distance/time | 完成 | `worldborder.add/set/center/damage_amount/damage_buffer/warning_distance/warning_time`；`worldborder.get()` 是表达式 |
| `locate` | structure/biome/poi | 完成 | `locate.structure/biome/poi("目标或 #标签")`；仅命令反馈 |

### 1.4 显示、声音与界面

| 命令 | 原版形态 | 状态 | 当前入口与缺口 |
| --- | --- | --- | --- |
| `bossbar` | add/remove/list/set（name/color/style/value/max/visible/players）/get | 缺失 | — |
| `title` | title/subtitle/actionbar/times/clear/reset | 缺失 | — |
| `tellraw` | `<players> <component>` | 部分 | `message.*` 只生成纯文本加颜色 |
| `say` | `<message>` | 不建模 | `message.all` 近似 |
| `msg`（`tell`、`w`） | `<players> <message>` | 缺失 | 私聊，低优先 |
| `teammsg`（`tm`） | `<message>` | 缺失 | 队伍聊天，低优先 |
| `particle` | `<name> [pos] [delta] [speed] [count] [force\|normal] [viewers]` | 缺失 | 目前只能 `run` |
| `playsound` | `<sound> [source] [targets] [pos] [volume] [pitch] [minVolume]` | 部分 | 只有 `sound.self`，音量/音调固定 1 |
| `stopsound` | `<targets> [*\|source] [sound]` | 缺失 | — |
| `posteffect` | add/clear/list/remove | 缺失 | — |
| `dialog` | show/clear | 缺失 | `resource dialog` 已能写数据 |

### 1.5 服务器管理与开发命令

这些命令默认不在数据包标准层内，仍列入清单以保证覆盖完整。

| 命令 | 权限 | 状态 | 说明 |
| --- | --- | --- | --- |
| `ban`、`ban-ip`、`banlist`、`op`、`deop`、`pardon`、`pardon-ip`、`whitelist` | ADMIN | 不可达 | 服务器管理 |
| `perf`、`save-all`、`save-off`、`save-on`、`setidletimeout`、`stop`、`transfer`、`publish`、`unpublish` | OWNER/ADMIN | 不可达 | 服务器管理 |
| `jfr`、`tick`、`debug` | OWNER/ADMIN | 不可达 | 性能与调试 |
| `seed`、`version`、`help` | 视环境 | 不建模 | 只读反馈 |
| `raid`、`debugpath`、`debugmobspawning`、`warden_spawn_tracker`、`spawn_armor_trims`、`serverpack`、`debugconfig`、`chase` | — | 不建模 | 仅开发构建注册 |

### 1.6 数据包内容（非命令）

命令之外的 data pack 组成。除此之外还有两类内容不在范围内：资源包资产（`sounds.json`、模型、纹理、字体、图集、后处理着色器）与客户端行为，mclang 只生成 data pack。

| 内容 | 26.3 形态 | 状态 | 当前入口与缺口 |
| --- | --- | --- | --- |
| `pack.mcmeta` | `description`（文本或组件）、`min_format`/`max_format`、`overlays`、`filters`、`features.enabled` | 部分 | 只写 description 与固定 `[121,0]`；overlay、filter、特性标志均无 |
| `pack.png` | 包图标 | 缺失 | — |
| 函数文件 | `data/<ns>/function/*.mcfunction` | 完成 | 每个函数一个文件，带生成注释头 |
| 函数标签 | `data/<ns>/tags/function/*.json` | 完成 | `fn_tag` 声明输出用户标签，支持函数、嵌套 `#标签`、外部字符串条目与 `replace`、编译期环路检查；`minecraft:load`/`tick` 仍由编译器生成 |
| 注册表标签 | `data/<ns>/tags/<注册表>/**`：16 个顶层注册表（banner_pattern、block、damage_type、dialog、enchantment、entity_type、fluid、game_event、instrument、item、painting_variant、point_of_interest_type、potion、timeline、villager_trade、worldgen）及子目录（`block/mineable`、`item/enchantable`、`item/sulfur_cube_archetype`、`banner_pattern/pattern_item`、`enchantment/exclusive_set`、`villager_trade/<职业>`、`worldgen/biome/has_structure` 等） | 缺失 | 无结构化声明 |
| predicate | JSON | 部分 | 原始 JSON；只检查同命名空间引用 |
| loot_table | JSON | 部分 | 原始 JSON |
| item_modifier | JSON | 部分 | 原始 JSON |
| advancement | JSON | 部分 | 原始 JSON |
| recipe | JSON | 部分 | 原始 JSON |
| 动态注册表 JSON | damage_type、enchantment、enchantment_provider、dialog、timeline、world_clock、trade_set、villager_trade、trial_spawner、trim_material/pattern、jukebox_song、instrument、painting_variant、banner_pattern、装饰陶片、各类变体与声音变体、test_environment/test_instance 等 41 类 | 部分 | 原始 JSON，无字段 schema |
| worldgen | 17 类 JSON：biome、feature、placed_feature、structure、structure_set、template_pool、processor_list、noise、noise_settings、density_function、carver、material_rule、material_condition、block_state_provider、multi_noise_biome_source_parameter_list、flat_level_generator_preset、world_preset | 部分 | 原始 JSON，无 schema 与引用校验 |
| dimension / dimension_type | JSON：`dimension_type` 随原版数据提供；`dimension` 用于自定义维度实例 | 部分 | 原始 JSON |
| structure | `data/<ns>/structure/*.nbt` | 缺失 | 无法生成二进制 NBT |
| 特性包 | `pack.mcmeta` 的 `features.enabled` 与内置数据包布局（`data/minecraft/datapacks/*`） | 缺失 | — |
| 持久化存储 | `data` 的 `storage` 内容 | 部分 | 只有 `item_list`（见 2.4） |
| 打包与分发 | zip 数据包、overlay 目录 | 缺失 | 只输出目录 |
| 资源引用图 | 跨资源引用（advancement 父级、loot table、predicate、item modifier、dialog、tag、function） | 部分 | 只检查同命名空间 predicate |
| JSON 字段校验 | 字段、枚举、未知字段与类型 | 缺失 | 仅 JSON 语法、资源类型、名称与重复声明 |

## 二、已完成能力（0.1–0.5）

编译器闭环与质量：

- [x] 建立 Rust 模块边界：词法分析、语法树、解析、语义检查、代码生成、命令行入口。
- [x] 定义最小语言：命名空间、全局计分变量、函数、`@load`/`@tick`、原生命令、函数调用、赋值、算术、条件分支、调度和返回。
- [x] 实现带文件名、行列和源码片段的诊断，一次返回全部错误。
- [x] 生成 `pack.mcmeta`、`.mcfunction` 文件以及 load/tick 标签；安全重建与 `.mclang-manifest` 输出所有权。
- [x] 提供 `build` 与 `check`，支持单文件与递归项目目录，检查命名空间一致性。
- [x] 词法、解析、语义与数据包输出测试；端到端示例；`cargo fmt`、`cargo clippy`、`cargo test` 零警告；语言参考、编译器设计与快速上手文档。

语言能力：

- [x] 结构化标准层：`query`、`item_stack`、`item_list`、`storage`、`give`、`each`、`spawn`、`in_dimension`、`self.*`、`message.*`、`sound.self`、`predicate`、`schedule`、`resource`。
- [x] 物品组件：名称、Lore、附魔、存储附魔、损伤、无法破坏、稀有度、物品模型、染色、光效覆盖、最大损伤与最大堆叠；`give` 数量按 `最大堆叠数 × 100` 校验。
- [x] 四态执行上下文（无/任意实体/非玩家实体/玩家）与 `@load`/`@tick`/`@entity`/`@non_player`/`@player` 属性；`data` 类操作只允许非玩家实体。
- [x] 函数参数、score 返回值、`let` 词法局部变量、稳定假玩家命名、同步调用图递归拒绝。
- [x] `if`/`while`、`!`/`&&`/`||`、谓词组合、确定求值顺序。
- [x] 中英文双关键词与同文件混用；关键词表单一来源；两种写法产物逐字节一致。
- [x] `--deny-raw` 严格模式；零底层命令字符串的便携箱子示例验收。
- [x] `resource` 声明：26.3 注册表类型限制、编译期 JSON 解析、稳定格式化输出。

命令补全（本次批次）：

- [x] 函数标签：`fn_tag` 声明、`call #tag()`、`schedule #tag()`、嵌套引用与循环引用检查；`function`/`schedule` 的标签成员在编译期检查执行上下文与参数。
- [x] `return fail`、`return run "命令"`；`return run` 计入 `--deny-raw` 的底层语句统计。
- [x] `schedule.clear(函数)` 与浮点延迟（`1.5 s` 按原版 `TimeArgument` 换算，拒绝不足 1 刻的延迟）。
- [x] `effect.give`/`effect.give_infinite`/`effect.clear`，秒数与等级范围检查。
- [x] `xp.add`/`xp.set` 与作为表达式的 `xp.query`（要求 `limit(1)` 玩家查询）。
- [x] `clear(玩家查询[, 物品][, 数量])`。
- [x] `stopwatch.create/restart/remove(id)` 与作为表达式的 `stopwatch.query(id[, 缩放])`。
- [x] `examples/potion_lab.mcl`：严格模式示例，覆盖以上全部能力并通过 `--deny-raw`。

世界与方块（本次批次）：

- [x] 坐标类型：`pos(x, y, z)` 与 `column(x, z)`，支持绝对、`~` 相对与 `^` 局部坐标；`^` 不能与其他写法混用，绝对分量检查世界范围与列坐标 256 区块上限。
- [x] `block_state("id") { 属性 = "值"; }`：属性字符集、重复声明与 `#` 标签谓词的使用位置都在编译期检查。
- [x] `set_block`、`fill`、`fill_biome`、`clone`（含跨维度、filtered/masked、force/move、strict）、`place.feature/jigsaw/structure/template`、`forceload.*`。
- [x] 世界时钟与天气：`time.set/add/pause/resume/rate`、`weather.*`，时间参数按原版 `TimeArgument` 换算。
- [x] 游戏规则与边界：`gamerule.set`/`gamerule.query`（26.3 `GameRules` 表），`worldborder.*` 与表达式 `worldborder.get()`。
- [x] 查询表达式：`time.query([时钟])`、`time.query_gametime()`、`gamerule.query(规则)`、`worldborder.get()`。
- [x] `examples/world_ops.mcl`：覆盖上述能力并通过 `--deny-raw`。

## 三、路线图

实施约定：

- 所有新结构必须提供中英文关键词，两种写法生成逐字节相同的产物。
- 新语句必须下降为真实命令形状，可在 `--deny-raw` 项目中使用；字符串拼接不算完成。
- 资源位置、注册表 id、槽位名、枚举与命令权限一律来自版本数据，不凭记忆。
- 每个阶段结束时：`cargo fmt`、`cargo clippy --all-targets`、`cargo test` 零警告；`examples/` 至少一个项目通过 `--deny-raw`；同步 `docs/language-reference.md`、`docs/compiler-design.md`、`docs/quickstart.md` 与在线手册。

### 第 1 阶段：类型系统与版本数据（基础设施）

- [ ] 1.1 版本数据生成器（`cargo xtask generate-version-data`）
  - 解析 `minecraft_client_26.3-rc-2/` 的 Brigadier 注册（`commands/Commands.java` 与 `server/commands/**`），导出根命令、字面量子命令、重定向、参数类型与 `requires` 权限等级到 `data/version/26.3-rc-2/commands.json`。
  - 从注册表引导代码与 `data/minecraft/**` 导出 id 集合：item、block、entity_type、biome、dimension、damage_type、mob_effect、enchantment、attribute、particle、sound、advancement、recipe、loot_table、predicate、dialog、post_effect、timeline、world_clock、slot、slot_source、worldgen/* 等；同时导出标签注册表清单（16 个顶层注册表与子目录）与数据包资源目录清单（41 类 JSON、17 类 worldgen、structure、tags、function）。
  - 导出枚举表：gamemode、difficulty、display slot、team color、heightmap、execute 关系、声音分类、时间单位、物品槽位。
  - 输出排序且带源哈希，可复现；CI 校验生成物与随附源码一致。
  - 编译器加载快照：资源位置校验从“语法合法”升级为“注册表存在”，未知 id 报错并给出最近候选；`resource` 支持类型改为读取快照。
- [ ] 1.2 坐标与向量类型：`pos`（绝对、`~`、`^`）、`block_pos`、`vec3`、`vec2`、`rotation`、`xyz` 对齐值；校验相对坐标与 `^` 所需的执行位置/朝向上下文。
- [ ] 1.3 文本组件类型：`text("...") { color/bold/italic/underlined/strikethrough/obfuscated }`，以及 `translate`、`keybind`、`score`、`selector`、`nbt`、click/hover 事件；统一 JSON 序列化。
- [ ] 1.4 结构化 NBT 与路径：`nbt { ... }` 复合字面量（数值后缀、字符串、list、compound、array）；绑定访问器的类型化路径（`entity_data`、`block_data`、`storage_data`）；SNBT 输出集中到 `emit`。
- [ ] 1.5 命令结果表达式：`count(q)`、`data.get(...)`、`random(1, 6)`、`xp.query(...)`、`stopwatch.query(...)`、`compute(...)` 等经 `execute store result score` 落入计分，参与现有算术与条件系统。
- [ ] 1.6 函数权限模型：语句与条件记录最低权限等级，默认按 `GAMEMASTER` 校验，越级给出解释 `function-permission-level` 的诊断；预留 `--function-permission-level <2..4>`。

### 第 2 阶段：补齐现有结构化能力

- [ ] 2.1 `execute` 结构化子句与 store
  - 修饰符：`execute as(q) at(q) positioned(pos) rotated(rot) facing(entity, eyes|feet) align(xyz) anchored(eyes|feet) in("dimension") on(relation) summon("id") { ... }`；重复或冲突的子句编译期报错。
  - 条件：`execute if/unless <条件> { ... }` 复用 2.2 的条件实现。
  - store：`store.result(...)`、`store.success(...)`、`store.data(...)`；结果表达式（1.5）隐式生成。
- [ ] 2.2 条件族扩展：`if block(pos, block)`、`if blocks(...)`、`if biome(...)`、`if dimension(...)`、`if loaded(pos)`、`if entity(q)`、`if data(...)`、`if items(...)`、`if slots(...)`、`if function(f)`、`if stopwatch(...)`；沿用“原子冻结到临时计分项再组合”的求值策略，保证一次求值与确定顺序。
- [ ] 2.3 实体查询属性扩展：`type("#tag")` 与否定、`name`、`scores`、`nbt`、坐标盒与 `distance`、`level`、`gamemode`、`team`、`rotate`、`predicate`、`advancements`；物品谓词支持完整组件与多槽位。
- [ ] 2.4 `data` 完整建模：`get`（结果表达式）、`merge`、`remove`、`modify` 全部操作与来源（from/string/compute/value）；沿用非玩家实体写保护。
- [ ] 2.5 `item` 完整建模：`replace`/`fill`/`override`/`modify`，实体与方块目标、槽位集合、`slot_source` 资源引用。
- [ ] 2.6 用户计分板：`objective` 声明（criteria、显示名、渲染类型、数字格式、显示槽）与 `players.enable/operation/display`；与内部 ABI objective 隔离。
- [ ] 2.7 消息组件化：`message.all/self/nearest/player` 接受 1.3 的文本组件。
- [ ] 2.8 声音完整参数：`sound.play(sound, source, targets, pos, volume, pitch, min_volume)`，保留 `sound.self` 简写。
- [ ] 2.9 `spawn` 完整化：`spawn("id", pos) { nbt { ... } ... }`；继续拒绝 `noSummon` 类型。
- [x] 2.10 `function`/`schedule`/`return` 收尾：`#tag` 调用与函数标签声明（9.2）、浮点时间、`schedule.clear`、`return fail`、`return run`；宏参数进阶见 8.3。

### 第 3 阶段：世界与方块命令族

- [x] 3.1 方块状态值：`block_state("minecraft:oak_stairs") { facing = "east"; }`，属性值在编译期检查字符集并拒绝重复声明；`#` 标签谓词用于过滤器。方块实体 `nbt { ... }` 依赖 1.4，未实现。
- [x] 3.2 `set_block(pos, block_state, mode)`，mode 为 `destroy`/`keep`/`replace`/`strict`。
- [x] 3.3 `fill(from, to, block_state [, mode] [, replace filter] [, keep])`，模式含 `outline`/`hollow`/`destroy`/`strict`。
- [x] 3.4 `clone(...)`：同维度与跨维度、`masked`/`filtered`、`force`/`move`/`normal`、`strict`；选项顺序无关，重复报错。
- [x] 3.5 `fill_biome(from, to, biome [, replace filter])`；过滤器接受 `#` 生物群系标签。
- [x] 3.6 `place.feature/jigsaw/structure/template(...)`，含 rotation、mirror、integrity、seed、strict；feature 的内联 JSON 未建模。
- [x] 3.7 `forceload.add/remove/remove_all/query`，绝对范围检查 256 区块上限。
- [x] 3.8 `time.set/add/pause/resume/rate` 与表达式 `time.query([clock])`、`time.query_gametime()`；时钟以可选参数写在方法调用末尾，生成 `time of <clock> ...`。
- [x] 3.9 `weather.clear/rain/thunder(duration)`，持续时间按 `TimeArgument` 换算并拒绝不足 1 刻。
- [x] 3.10 `gamerule.set(name, value)` 与表达式 `gamerule.query(name) -> score`；规则名与值类型对照 26.3 `GameRules` 的静态表（1.1 快照落地前的手工版本）。
- [x] 3.11 `worldborder.add/set/center/damage_amount/damage_buffer/warning_distance/warning_time` 与表达式 `worldborder.get()`。
- [x] 3.12 `locate.structure/biome/poi`（仅日志反馈，目标接受 `#` 标签）。

### 第 4 阶段：实体与玩家命令族

- [x] 4.1 `effect.give(targets, effect, seconds[, amplifier][, hide_particles])`、`effect.give_infinite(...)`、`effect.clear(targets[, effect])`；秒数与等级按 26.3 的 `EffectCommands` 范围检查。
- [ ] 4.2 `enchant(targets, enchantment[, level])`。
- [x] 4.3 `xp.add`/`xp.set`（points/levels）与 `xp.query` 结果表达式。
- [x] 4.4 `clear(targets[, item_filter][, max_count])`；目标为玩家查询，数量上限 2147483647。
- [ ] 4.5 `damage(target, amount[, damage_type][, at pos | by entity [from cause]])`；枚举 damage_type 来自注册表。
- [ ] 4.6 `attribute` 全子命令：`get`、`base set/get/reset`、`modifier add/remove/value get`。
- [ ] 4.7 `teleport(targets, pos[, rotation][, facing ...])` 与 `teleport(targets, entity)`；`tp` 为中文 `传送` 的英文别名。
- [ ] 4.8 `ride.mount(target, vehicle)`、`ride.dismount(target)`。
- [ ] 4.9 `rotate.to(target, rotation)`、`rotate.facing(target, ...)`。
- [ ] 4.10 `spreadplayers(center, spread, max_range, respect_teams, targets)` 与 `under` 变体。
- [ ] 4.11 `spectate([target, player])`。
- [ ] 4.12 `swing([targets][, hand][, animation][, duration])`。
- [ ] 4.13 `trigger(objective[, add|set value])`。
- [ ] 4.14 `gamemode(mode[, players])`、`defaultgamemode(mode)`、`difficulty([mode])`。
- [ ] 4.15 `spawnpoint([players][, pos][, rotation])`、`setworldspawn([pos][, rotation])`。
- [ ] 4.16 `tag` 目标扩展：任意查询的 `tag.add/remove/list`，`self` 版本保留。
- [ ] 4.17 `team` 全子命令，成员参数接受查询或玩家选择器。
- [ ] 4.18 `waypoint.list/modify.color/modify.style`。

### 第 5 阶段：物品、战利品与进度

- [ ] 5.1 `loot` 全形态：上下文来源（loot table、fish、kill、mine）与投放目标（`give`、`insert`、`replace`、`spawn`）。
- [ ] 5.2 `slot_source` 声明与 `item` 联动，替代 `give(..., self.item)` 中的内建空槽来源。
- [ ] 5.3 `advancement.grant/revoke`：`only`（含 criterion）、`from`、`until`、`through`、`everything`。
- [ ] 5.4 `recipe.give/take`（含 `*`）。
- [ ] 5.5 `clear` 与 `give` 使用 2.3 的完整物品谓词。

### 第 6 阶段：界面与感官

- [ ] 6.1 `title.title/subtitle/actionbar/times/clear/reset`，组件使用 1.3 类型。
- [ ] 6.2 `bossbar.add/remove/list/set.name/set.color/set.style/set.value/set.max/set.visible/set.players/get`。
- [ ] 6.3 `dialog.show/clear`，配合 `resource dialog`。
- [ ] 6.4 `particle(name, pos, delta, speed, count, mode, viewers)`。
- [ ] 6.5 `playsound` 全参数（在 2.8 基础上补 min_volume 与多目标）。
- [ ] 6.6 `stopsound(targets[, source][, sound])`。
- [ ] 6.7 `posteffect.add/clear/list/remove`。
- [ ] 6.8 `msg`/`teammsg`（低优先，聊天类型命令；`message` 已覆盖常用广播，`emote` 保持不建模）。

### 第 7 阶段：服务器数据与工具

- [ ] 7.1 `random`：`random(1, 6)` 结果表达式、`random.roll`、`random.reset`（sequence 参数受等级限制）。
- [x] 7.2 `stopwatch.create/query/restart/remove`，query 提供结果表达式。
- [ ] 7.3 `compute`：default/block/entity 上下文 + float/integer provider。
- [ ] 7.4 `reload()`、`datapack.enable/disable/list`。
- [ ] 7.5 `list`、`seed`、`version`、`help`：只读反馈，明确不建模或提供只写日志的语句。
- [ ] 7.6 `fetchprofile`：工具类，按需提供；`serverpack` 仅在开发构建注册，不进入标准层。
- [ ] 7.7 `test`：gametest 工具链保持不建模，需要时通过 `run` 在开发构建中使用。
- [ ] 7.8 权限放开策略：`tick`、`debug`、`jfr`、`kick` 等 ADMIN/OWNER 命令只在 `--function-permission-level` 提升后提供结构化入口，并在文档中说明服务端配置前提。

### 第 8 阶段：语言与工具体验

- [ ] 8.1 模块/import 系统，替代“同命名空间多文件”的项目模型。
- [ ] 8.2 `for`、`break`/`continue` 与更强的控制流优化。
- [ ] 8.3 函数宏高级调用：`function <fn> with <source>` 与 `$(key)` 宏参数，用于文本、坐标与 NBT 动态参数。
- [ ] 8.4 静态命令校验与补全：用 1.1 命令树校验 `run` 字符串的根命令、参数形状与权限等级，在编译期报告不可能加载的命令。
- [ ] 8.5 增量构建与源映射：只重建受影响函数，产物与源码行对应。
- [ ] 8.6 语言服务器与编辑器集成：补全、悬停、跳转、格式化、即时诊断。
  - [x] `mclang lsp` 语言服务器：UTF-16 位置换算、全文同步、项目级即时诊断、声明与关键词补全（`@`/`#` 上下文）、关键词与声明悬停、跨文件跳转；`analysis::analyze` 提供结构化诊断与符号表。
  - [x] VSCode 插件 `editors/vscode`：TextMate 语法高亮（中英文关键词表与解析器同步测试）、语言配置、按 `mclang.server.path`/`target/(release|debug)`/`PATH` 解析可执行文件的语言客户端。
  - [ ] 剩余：文档格式化、代码操作（快速修复）、点号成员（`self.*`、`effect.*` 等）补全与语义高亮。
- [ ] 8.7 文档与示例：每个阶段同步 `docs/` 与 `examples/`，保持 `--deny-raw` 端到端验收。`docs/manual.html` 的在线手册尚未同步本批次的函数标签、`effect`/`xp`/`clear`、`return fail`/`run` 与 `schedule.clear` 章节；`docs/language-reference.md` 与 `docs/compiler-design.md` 已经更新。

### 第 9 阶段：数据包内容与资源 schema（非命令）

- [ ] 9.1 `pack.mcmeta` 完整化：支持文本组件 `description`、`supported_formats` 范围、`overlays`（目录覆盖层）、`filters`（block/allow）、`features.enabled`（特性包）；字段取值来自 1.1 的版本元数据；可选 `pack.png` 图标。
- [ ] 9.2 标签系统：`tag <注册表> <名称> { values = [...]; replace = 假; required = 真; }` 声明，覆盖 16 个标签注册表与子目录（`block/mineable`、`item/enchantable`、`item/sulfur_cube_archetype`、`banner_pattern/pattern_item`、`enchantment/exclusive_set`、`villager_trade/<职业>`、`worldgen/biome/has_structure` 等）；条目支持 `#tag` 嵌套、`required` 与 `replace` 语义。
  - 函数标签：`fn_tag` 声明输出 `data/<ns>/tags/function/<名称>.json`，供 `function #ns:tag` 与 `schedule` 使用；条目支持本命名空间函数、嵌套 `#标签`、外部字符串资源位置与 `replace`，编译期检查引用与循环。`minecraft:load`/`tick` 的生成保留在编译器的内部机制中。
  - 其他注册表标签：16 个顶层注册表与子目录（`block/mineable`、`item/enchantable`、`item/sulfur_cube_archetype`、`banner_pattern/pattern_item`、`enchantment/exclusive_set`、`villager_trade/<职业>`、`worldgen/biome/has_structure` 等）尚未建模，`required` 语义也只在函数标签的外部条目上默认保留原版行为。
  - 校验：注册表与条目 id 存在、嵌套标签可解析；默认拒绝写入 `minecraft:` 命名空间，需要时显式放开。
- [ ] 9.3 资源 schema 化：把 raw JSON 升级为结构化声明，检查字段类型、枚举与未知字段；结构与原始 JSON 可共存，同类型同名称重复声明报错。
  - 第一批：predicate 条件树、loot_table（pool/entry/condition/function）、item_modifier、advancement（criteria/requirements/display/rewards/parent）。
  - 第二批：recipe（配方类型、展示、解锁）、enchantment、damage_type、dialog。
  - 第三批：其余动态注册表（timeline、world_clock、trade_set、villager_trade、trial_spawner、trim_material/pattern、变体与声音变体、test_environment/test_instance 等）。
- [ ] 9.4 资源引用图：advancement 父级、loot table 与 entry、predicate、item modifier、dialog、tag、function、配方解锁等跨文件与跨命名空间引用解析，未解析引用在编译期报错；已建模类型之间不再依赖字符串。
- [ ] 9.5 世界生成：按 biome → structure/structure_set → template_pool/processor_list → feature/placed_feature → noise/noise_settings/density_function/carver → dimension_type/world_preset/flat_level_generator_preset 的顺序补齐 schema 与引用校验。worldgen 是全量覆盖中最大的一块，允许按需推进，未建模类型继续使用原始 JSON。
- [ ] 9.6 二进制 NBT 资源：`structure` 类型输出 `data/<ns>/structure/<名称>.nbt`（基于 1.4 的结构化 NBT 编码），供 `place template` 与结构方块使用。
- [ ] 9.7 打包与分发：`--zip` 输出可直接放入 `datapacks/` 的压缩包；overlay 目录布局；包图标；输出结构与内置特性包对齐。
- [ ] 9.8 版本迁移与多目标：pack format 升级时的资源与命令迁移报告；结合 1.1 快照支持多目标版本后端。

### 依赖关系与验收

- 1.1 是全部注册表校验与补全数据的前提；1.2/1.4 是第 3、4、5 阶段的参数类型前提；1.3 支撑 2.7 与第 6 阶段；1.5 支撑 2.2、4.3、7.1–7.3。
- 第 3 阶段已按上述形式落地：坐标与方块状态内联在语句参数中，资源位置只做语法检查；游戏规则使用对照 26.3 `GameRules` 的静态表。1.1 快照落地后应把方块、生物群系、结构、时钟与规则表升级为注册表校验；方块实体 NBT 与 `place.feature` 内联 JSON 等待 1.4。
- 2.1/2.2 的 `execute` 与条件模型是第 3、4、5 阶段的世界/实体命令在非默认上下文中执行的前提。
- 9.1/9.2 依赖 1.1 的版本元数据与注册表快照；函数标签部分已经落地，不依赖注册表数据；9.3–9.6 依赖 1.4 的结构化 NBT 与值类型；9.7/9.8 依赖输出层与 1.1 的版本数据。函数标签（9.2）是 2.10 的 `#tag` 调用前提。
- 每个阶段的验收：编译器零警告；`examples/` 项目通过 `--deny-raw`；`src/compiler/tests.rs` 断言生成命令与资源文件；中英文关键词产物逐字节一致；文档与手册同步。

## 四、旧待办与设计项映射

| 旧待办或设计项 | 新位置 |
| --- | --- |
| 继续建立方块、坐标、玩家、物品、效果、声音、粒子和结构化 NBT 类型 | 1.2–1.4，第 3–6 阶段 |
| 从 26.3 命令树和注册表生成版本化校验数据 | 1.1 |
| 从目标 Minecraft 源码导出命令签名，提供静态命令校验和补全数据 | 1.1、8.4 |
| 增加结构化 NBT 数据类型和数据包资源 schema，使字段错误在编译期出现 | 1.4、9.3–9.5 |
| 建立中间表示和版本后端，让同一份高层源码选择兼容的 Minecraft 目标 | 9.8 |
| 模块/import 系统 | 8.1 |
| 基于 26.3 函数宏的高级调用 | 8.3 |
| 实体上下文类型、存储/NBT 类型和复合数据 | 1.4、2.4 |
| `for`、break/continue 和更强的控制流优化 | 8.2 |
| 增量构建、源映射、语言服务器与编辑器集成 | 8.5、8.6 |
| 为编辑器提供格式化、补全、跳转、悬停与即时诊断 | 8.6 |
