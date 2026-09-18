# Mclang 开发计划

> 本文件随实现持续推进：§二 记录逐命令现状，§三 只列未完成工作，§四 归档已完成能力。
> 维护约定：完成一项能力后，从 §三 删除对应条目、把记录补进 §四，并同步 §二 的状态与 §一 的统计。

## 目标与基线

- **目标**：用 Rust 实现一门面向 Minecraft Java Edition 26.3-rc-2 的数据包编程语言。编译器把 `.mcl` 源文件转换为可直接放入世界 `datapacks` 目录的数据包，并在编译期报告词法、语法和语义错误。
- **标准层判定准则**：作者能否用有名称、有结构、可补全、可静态检查的语法描述意图；只能原样复制字符串的功能不算完成。
- **版本基线**：目标版本固定在仓库内 `minecraft_client_26.3-rc-2/` 源码，数据包格式 `121.0`；资源类型、注册表与命令签名一律以该源码为准。
- **数据包目录**：函数与函数标签依据 `ServerFunctionLibrary` 与 `Registries`，使用 `data/<namespace>/function` 与 `data/<namespace>/tags/function`。
- **函数权限**：26.3 的 `function-permission-level` 服务器属性默认 `GAMEMASTER`（等级 2，见 `DedicatedServerProperties:147`）。函数在编译期（`ServerFunctionLibrary`）与运行期（`FunctionCommand.modifySenderForExecution` 的 `withMaximumPermission`）都被限制在该等级，因此 `ADMIN`（3）/`OWNER`（4）命令无法从数据包函数合法执行。标准层固定按等级 2 规划；越级命令标记为“不可达”，即使出现在 `run` 字符串中也会被编译器拒绝。

## 一、进度总览

### 1.1 命令覆盖统计

26.3-rc-2 的命令树共 97 个根命令名（含 `xp`、`tp`、`tell`、`w`、`tm`、`me` 等别名），逐条清单见 §二。

| 状态 | 数量 | 含义 |
| --- | --- | --- |
| 完成 | 25 | 结构化语法覆盖该命令的常规数据包用法，编译期检查完整 |
| 部分 | 7 | 已有结构化入口，子命令或参数面存在明确缺口 |
| 缺失 | 29 | 没有结构化入口，只能写 `run` 字符串或完全不可用 |
| 不可达 | 21 | 需要高于默认等级 2 的函数权限，数据包函数无法合法执行 |
| 等价覆盖 | 2 | 没有独立结构化入口，但用途已由其他结构化语句覆盖 |
| 只读反馈 | 4 | 只向命令来源输出信息，不改变世界状态 |
| 测试工具 | 1 | gametest 测试框架，不属于数据包标准层 |
| 仅开发构建 | 8 | 只在开发标志或 IDE 环境注册，正式版数据包不可用 |

### 1.2 阶段进度

| 阶段 | 进度 | 剩余重点 |
| --- | --- | --- |
| 1 类型系统与版本数据 | 6/6（余说明性待补） | 动态子命令树与动态 NBT 键、SNBT 备选记法 |
| 2 补齐现有结构化能力 | 10/10 | — |
| 3 世界与方块命令族 | 12/12 | `place.feature` 内联 JSON |
| 4 实体与玩家命令族 | 3/18 | enchant、damage、attribute、team、ride 等 |
| 5 物品、战利品与进度 | 1/5 | loot、recipe、slot_source、完整物品谓词 |
| 6 界面与感官 | 1/8 | title、bossbar、particle、dialog、msg 等 |
| 7 服务器数据与工具 | 2/8（7.1 部分） | random.roll/reset、reload/datapack 等 |
| 8 语言与工具体验 | 2/8（8.6 部分） | 函数宏、参数形状校验、增量构建、标准库 |
| 9 数据包内容与资源 schema | 0/8（9.2 函数标签已完成） | 注册表标签、资源 schema、worldgen、打包 |

## 二、原版命令覆盖矩阵

状态定义（与 §一 的统计一致）：

| 状态 | 含义 |
| --- | --- |
| 完成 | 结构化语法覆盖该命令的常规数据包用法，编译期检查完整 |
| 部分 | 已有结构化入口，但子命令或参数面存在明确缺口 |
| 缺失 | 没有结构化入口，只能写 `run` 字符串或完全不可用 |
| 不可达 | 需要高于默认等级 2 的函数权限，数据包函数无法合法执行 |
| 等价覆盖 | 没有独立结构化入口，但用途已由其他结构化语句覆盖 |
| 只读反馈 | 只向命令来源输出信息，不改变世界状态 |
| 测试工具 | gametest 测试框架的命令，不属于数据包标准层 |
| 仅开发构建 | 只在开发标志或 IDE 环境注册，正式版数据包不可用 |

### 2.1 执行、函数与数据核心

| 命令 | 原版形态（26.3-rc-2） | 状态 | 当前入口与缺口 |
| --- | --- | --- | --- |
| `execute` | `run`；`if`/`unless`（block、biome、loaded、dimension、score、blocks、entity、predicate、function、stopwatch、data、items、slots）；修饰符 as、at、positioned、rotated、facing、align、anchored、in、on（8 种关系）、summon；store result/success | 完成 | 结构化修饰符、条件族与 store 全覆盖（细节见 §四）；旧字符串子句保留为逃生口并计入 `--deny-raw` |
| `function` | `<fn>`；`<fn> <nbt>`；`<fn> with <entity\|block\|storage> [path]`；`#tag` | 部分 | `call`、表达式调用与 `#tag` 已支持；缺 `<fn> <nbt>` 宏参数与 `with`（8.3） |
| `return` | `<int>`；`fail`；`run <命令>` | 完成 | 计分返回值、store ABI、`return fail`、`return run` 全覆盖；`return run` 计入 `--deny-raw` |
| `schedule` | `function <fn> <time> [replace\|append]`；`clear <fn>` | 完成 | 整数与浮点时间（按原版 `TimeArgument` 换算为游戏刻）、`replace`/`append`、`schedule.clear`、`#tag` 调度 |
| `data` | get；merge；remove；modify（insert/prepend/append/set/merge；from/string/compute/value；entity/block/storage） | 完成 | `data.get`（表达式）/`merge`/`remove`/`modify` 与 entity/block/storage 三类目标；实体目标沿用非玩家写保护 |
| `scoreboard` | objectives（add/remove/list/modify：displayname/rendertype/displayautoupdate/numberformat）；players（set/get/add/remove/reset/enable/operation/display）；display | 完成 | `objective` 声明（准则/显示名/渲染类型/数字格式/显示槽）与 `set/get/reset/enable/operation/display`；与内部 ABI 目标隔离 |
| `item` | replace/fill/override/modify（entity/block 目标、槽位集合、from/with/loot_modifier） | 完成 | `replace/fill/override/modify`；实体（`limit(1)`）与方块目标、任意槽位、`with`/`from` 与可选修饰器 |
| `loot` | loot/fish/kill/mine + give/insert/replace/spawn | 缺失 | 战利品表资源可声明，但没有取用命令（5.1） |
| `advancement` | grant/revoke × only/from/until/through/everything | 完成 | `advancement.grant/revoke[_through\|_from\|_until\|_everything]`，目标是玩家；进度引用本命名空间声明或字符串资源位置，`only` 可带准则名 |
| `recipe` | give/take `<recipe\|*>` | 缺失 | — （5.4） |
| `reload` | — | 缺失 | — （7.4） |
| `datapack` | enable/disable/list；create（不可达，需 OWNER） | 缺失 | — （7.4） |
| `compute` | default/block/entity × float/integer provider | 完成 | 结果表达式 `compute(来源, float\|integer, provider[, 缩放])`；provider 与上下文对照注册表校验 |
| `stopwatch` | create/query/restart/remove | 完成 | `stopwatch.create/restart/remove(id)`；`query` 作为表达式并支持可选缩放，失败写入 0 |
| `random` | value/roll/reset | 部分 | `random(1, 6)` 结果表达式已支持；缺 `random.roll` 与 `random.reset`（7.1） |
| `test` | gametest 系列 | 测试工具 | 测试框架，不属于数据包标准层（7.7） |
| `fetchprofile` | name/id/entity | 只读反馈 | 只输出可点击引用，无返回值（7.6） |

### 2.2 实体与玩家

| 命令 | 原版形态 | 状态 | 当前入口与缺口 |
| --- | --- | --- | --- |
| `summon` | `<entity> [pos] [nbt]` | 完成 | `spawn("id", pos\|vec3) { ... }` 进入新实体上下文；初始 NBT 在召唤后合并；拒绝 `noSummon` 类型 |
| `give` | `<players> <item> [count]` | 部分 | 目标必须是已声明玩家查询，物品必须是 `item_stack` 定义；缺内联任意组件与完整物品谓词（5.5） |
| `kill` | `[targets]` | 部分 | 只有 `self.remove()`，无目标参数 |
| `tag` | add/remove/list | 部分 | 只有 `self.add_tag`/`self.remove_tag`；多目标与 `list` 见 4.16 |
| `team` | list/add/remove/empty/join/leave/modify（displayName/color/friendlyFire/seeFriendlyInvisibles/nametagVisibility/deathMessageVisibility/collisionRule/prefix/suffix） | 缺失 | 4.17 |
| `attribute` | get；base set/get/reset；modifier add/remove/value get | 缺失 | 4.6 |
| `effect` | give（时长/等级/隐藏粒子/infinite）；clear | 完成 | `effect.give`、`effect.give_infinite`、`effect.clear`；秒数与等级按 26.3 范围编译期检查 |
| `enchant` | `<targets> <enchantment> [level]` | 缺失 | 4.2 |
| `experience`（`xp`） | add/set/query（points/levels） | 完成 | `xp.add`/`xp.set`；`xp.query` 作为表达式，要求 `limit(1)` 玩家查询 |
| `clear` | `[targets] [item] [maxCount]` | 完成 | `clear(玩家查询[, 物品][, 数量])`，数量上限 2147483647 |
| `damage` | `<target> <amount> [damage_type] [at <pos>\|by <entity> [from <cause>]]` | 缺失 | 4.5 |
| `teleport`（`tp`） | 坐标/实体/朝向 | 部分 | 已有 `pos`/`vec3`、`rotation` 与单个实体查询；缺 `facing` 与朝向变体（4.7） |
| `ride` | mount/dismount | 缺失 | 4.8 |
| `rotate` | rotation/facing | 缺失 | 4.9 |
| `spreadplayers` | 中心/间距/范围 + `under` | 缺失 | 4.10 |
| `spectate` | `[target] [player]` | 缺失 | 4.11 |
| `swing` | `[targets] [hand] [animation] [duration]` | 缺失 | 4.12 |
| `trigger` | add/set | 缺失 | `scoreboard.enable` 已能开启 `trigger` 目标；命令本身见 4.13 |
| `gamemode` | `<mode> [players]` | 缺失 | 4.14 |
| `defaultgamemode` | `<mode>` | 缺失 | 4.14 |
| `difficulty` | 查询/设置 | 缺失 | 4.14 |
| `spawnpoint` | `[players] [pos] [rotation]` | 缺失 | 4.15 |
| `setworldspawn` | `[pos] [rotation]` | 缺失 | 4.15 |
| `waypoint` | list；modify color（含 hex/reset）/style | 缺失 | 4.18 |
| `emote`（`me`） | `<action>` | 等价覆盖 | `message.all` 已能广播文本 |
| `kick` | `<players> [reason]` | 不可达 | 需要 ADMIN |
| `list` | `[uuids]` | 缺失 | 只读反馈，低优先（7.5） |

### 2.3 世界与方块

| 命令 | 原版形态 | 状态 | 当前入口与缺口 |
| --- | --- | --- | --- |
| `setblock` | `<pos> <block> [destroy\|keep\|replace\|strict]` | 完成 | `set_block(pos, block_state[, 模式][, nbt { ... }])` |
| `fill` | `<from> <to> <block> [mode\|replace filter\|keep]` | 完成 | `fill(from, to, block_state[, 模式][, replace 过滤器][, nbt { ... }])` |
| `clone` | 同/跨维度 + masked/filtered/force/move/normal/strict | 完成 | `clone(起点, 终点, 目标[, 选项...])`，选项顺序无关、重复报错 |
| `fillbiome` | `<from> <to> <biome> [replace filter]` | 完成 | `fill_biome(from, to, "生物群系"[, replace, "过滤器"])` |
| `place` | feature/jigsaw/structure/template | 部分 | `place.feature/jigsaw/structure/template`；feature 的内联 JSON 未建模（3.6） |
| `forceload` | add/remove/query | 完成 | `forceload.add/remove/remove_all/query`，绝对范围检查 256 区块上限 |
| `time` | set/add/pause/resume/rate/query + `of <clock>` | 完成 | `time.set/add/pause/resume/rate(..., [时钟])`；`time.query([时钟])` 与 `time.query_gametime()` 是表达式 |
| `weather` | clear/rain/thunder [duration] | 完成 | `weather.clear/rain/thunder([持续时间])` |
| `gamerule` | `<rule> [value]` | 完成 | `gamerule.set(规则, 值)` 与表达式 `gamerule.query(规则)`；规则名、类型与范围来自 26.3 `GameRules` |
| `worldborder` | add/set/center/damage amount/buffer/get/warning distance/time | 完成 | `worldborder.add/set/center/damage_amount/damage_buffer/warning_distance/warning_time`；`worldborder.get()` 是表达式 |
| `locate` | structure/biome/poi | 完成 | `locate.structure/biome/poi("目标或 #标签")`；仅命令反馈 |

### 2.4 显示、声音与界面

| 命令 | 原版形态 | 状态 | 当前入口与缺口 |
| --- | --- | --- | --- |
| `bossbar` | add/remove/list/set（name/color/style/value/max/visible/players）/get | 缺失 | 6.2 |
| `title` | title/subtitle/actionbar/times/clear/reset | 缺失 | 6.1 |
| `tellraw` | `<players> <component>` | 完成 | `message.all/self/nearest/player` 接受文本组件（样式、click/hover、translate/keybind/score/selector/nbt）；纯字符串与末尾颜色保留为旧写法 |
| `say` | `<message>` | 等价覆盖 | `message.all` 已覆盖广播文本 |
| `msg`（`tell`、`w`） | `<players> <message>` | 缺失 | 私聊，低优先（6.8） |
| `teammsg`（`tm`） | `<message>` | 缺失 | 队伍聊天，低优先（6.8） |
| `particle` | `<name> [pos] [delta] [speed] [count] [force\|normal] [viewers]` | 缺失 | 6.4 |
| `playsound` | `<sound> [source] [targets] [pos] [volume] [pitch] [minVolume]` | 完成 | `sound.self` 与 `sound.play(sound, source, targets, pos, volume, pitch, min_volume)` |
| `stopsound` | `<targets> [*\|source] [sound]` | 缺失 | 6.6 |
| `posteffect` | add/clear/list/remove | 缺失 | 6.7 |
| `dialog` | show/clear | 缺失 | 6.3；`resource dialog` 已能写数据 |


### 2.6 数据包内容（非命令）

命令之外的 data pack 组成。除此之外还有两类内容不在范围内：资源包资产（`sounds.json`、模型、纹理、字体、图集、后处理着色器）与客户端行为，mclang 只生成 data pack。

| 内容 | 26.3 形态 | 状态 | 当前入口与缺口 |
| --- | --- | --- | --- |
| `pack.mcmeta` | `description`（文本或组件）、`min_format`/`max_format`、`overlays`、`filters`、`features.enabled` | 部分 | 只写 description 与固定 `[121,0]`；overlay、filter、特性标志均无（9.1） |
| `pack.png` | 包图标 | 缺失 | —（9.1） |
| 函数文件 | `data/<ns>/function/*.mcfunction` | 完成 | 每个函数一个文件，带生成注释头 |
| 函数标签 | `data/<ns>/tags/function/*.json` | 完成 | `fn_tag` 声明输出用户标签，支持函数、嵌套 `#标签`、外部字符串条目与 `replace`、编译期环路检查；`minecraft:load`/`tick` 仍由编译器生成 |
| 注册表标签 | `data/<ns>/tags/<注册表>/**`：16 个顶层注册表（banner_pattern、block、damage_type、dialog、enchantment、entity_type、fluid、game_event、instrument、item、painting_variant、point_of_interest_type、potion、timeline、villager_trade、worldgen）及子目录（`block/mineable`、`item/enchantable`、`item/sulfur_cube_archetype`、`banner_pattern/pattern_item`、`enchantment/exclusive_set`、`villager_trade/<职业>`、`worldgen/biome/has_structure` 等） | 缺失 | 无结构化声明（9.2） |
| predicate | JSON | 部分 | 原始 JSON；只检查同命名空间引用（9.3） |
| loot_table | JSON | 部分 | 原始 JSON（9.3） |
| item_modifier | JSON | 部分 | 原始 JSON（9.3） |
| advancement | JSON | 部分 | 结构化 `advancement` 声明（准则/触发器/奖励/展示/父级）已落地；准则的 `conditions` 仍是原始 JSON（9.3） |
| recipe | JSON | 部分 | 原始 JSON（9.3） |
| 动态注册表 JSON | damage_type、enchantment、enchantment_provider、dialog、timeline、world_clock、trade_set、villager_trade、trial_spawner、trim_material/pattern、jukebox_song、instrument、painting_variant、banner_pattern、装饰陶片、各类变体与声音变体、test_environment/test_instance 等 41 类 | 部分 | 原始 JSON，无字段 schema（9.3） |
| worldgen | 17 类 JSON：biome、feature、placed_feature、structure、structure_set、template_pool、processor_list、noise、noise_settings、density_function、carver、material_rule、material_condition、block_state_provider、multi_noise_biome_source_parameter_list、flat_level_generator_preset、world_preset | 部分 | 原始 JSON，无 schema 与引用校验（9.5） |
| dimension / dimension_type | JSON：`dimension_type` 随原版数据提供；`dimension` 用于自定义维度实例 | 部分 | 原始 JSON（9.5） |
| structure | `data/<ns>/structure/*.nbt` | 缺失 | 无法生成二进制 NBT（9.6） |
| 特性包 | `pack.mcmeta` 的 `features.enabled` 与内置数据包布局（`data/minecraft/datapacks/*`） | 缺失 | —（9.1） |
| 持久化存储 | `data` 的 `storage` 内容 | 部分 | 只有 `item_list` 数据槽（见 §4.4） |
| 打包与分发 | zip 数据包、overlay 目录 | 缺失 | 只输出目录（9.7） |
| 资源引用图 | 跨资源引用（advancement 父级、loot table、predicate、item modifier、dialog、tag、function） | 部分 | 只检查同命名空间 predicate（9.4） |
| JSON 字段校验 | 字段、枚举、未知字段与类型 | 缺失 | 仅 JSON 语法、资源类型、名称与重复声明（9.3） |

## 三、路线图（未完成）

实施约定：

- 所有新结构必须提供中英文关键词，两种写法生成逐字节相同的产物。
- 新语句必须下降为真实命令形状，可在 `--deny-raw` 项目中使用；字符串拼接不算完成。
- 资源位置、注册表 id、槽位名、枚举与命令权限一律来自版本数据，不凭记忆。
- 每个阶段结束时：`cargo fmt`、`cargo clippy --all-targets` 零警告；`tests/valid` 语料全部编译通过并核对产物、`tests/invalid` 语料全部被拒绝；`examples/` 至少一个项目通过 `--deny-raw`；同步 `docs/` 下的语言手册（正文 `docs/content/manual.md`，用 `bun docs/tools/build.mjs --self-test` 重新生成并验证）。

### 阶段 1：类型系统与版本数据（基础设施）

已完成：1.1–1.6 的主体（记录见 §四），余下为说明性待补。

剩余：

- [ ] 1.1 动态生成的子命令树无法静态枚举（`gamerule` 由运行期注册表构建、`time` 的时钟子命令来自数据驱动），命令快照只能以一级清单近似；`slot_source` 没有原版条目。
- [ ] 1.4 SNBT 的其余字面量记法：十六进制 `0x`/二进制 `0b` 与下划线分隔、无符号前缀 `ub`/`us`/`ui`/`ul`、无引号字符串。这些写法都有等值替代，属于表现力无损的扩展。
- [ ] 1.4 动态命名的实体键（`ConversionTracker` 这类由构造参数生成，如 Player 的 `foodData`）与方块实体 NBT 的键校验。

### 阶段 2：补齐现有结构化能力

全部完成（2.1–2.10，记录见 §四），无剩余。

### 阶段 3：世界与方块命令族

已完成：3.1–3.12 的主体（记录见 §四），3.6 还剩内联 JSON。

剩余：

- [ ] 3.6 `place.feature` 的内联 feature JSON 未建模。

### 阶段 4：实体与玩家命令族

已完成：4.1、4.3、4.4（记录见 §四）。

剩余：

- [ ] 4.2 `enchant(targets, enchantment[, level])`。
- [ ] 4.5 `damage(target, amount[, damage_type][, at pos | by entity [from cause]])`；枚举 damage_type 来自注册表。
- [ ] 4.6 `attribute` 全子命令：`get`、`base set/get/reset`、`modifier add/remove/value get`。
- [ ] 4.7 `teleport` 收尾：`facing` 与朝向变体（已有的 `pos`/`vec3`、`rotation` 与单个实体查询见 §四）。
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

### 阶段 5：物品、战利品与进度

已完成：5.3（记录见 §四）。

剩余：

- [ ] 5.1 `loot` 全形态：上下文来源（loot table、fish、kill、mine）与投放目标（`give`、`insert`、`replace`、`spawn`）。
- [ ] 5.2 `slot_source` 声明与 `item` 联动，替代 `give(..., self.item)` 中的内建空槽来源。
- [ ] 5.4 `recipe.give/take`（含 `*`）。
- [ ] 5.5 `clear` 与 `give` 使用完整的物品谓词（实体查询声明中的 `id[...]` 与槽位来源，见 §四）。

### 阶段 6：界面与感官

已完成：6.5（随 §四 的 sound 完整参数落地）。

剩余：

- [ ] 6.1 `title.title/subtitle/actionbar/times/clear/reset`，组件使用 §四 的文本组件类型。
- [ ] 6.2 `bossbar.add/remove/list/set.name/set.color/set.style/set.value/set.max/set.visible/set.players/get`。
- [ ] 6.3 `dialog.show/clear`，配合 `resource dialog`。
- [ ] 6.4 `particle(name, pos, delta, speed, count, mode, viewers)`。
- [ ] 6.6 `stopsound(targets[, source][, sound])`。
- [ ] 6.7 `posteffect.add/clear/list/remove`。
- [ ] 6.8 `msg`/`teammsg`（低优先；`say`/`emote` 已归入等价覆盖）。

### 阶段 7：服务器数据与工具

已完成：7.2、7.3（记录见 §四）。

剩余：

- [ ] 7.1 `random.roll`/`random.reset`（`random(1, 6)` 结果表达式已完成；sequence 参数受等级限制）。
- [ ] 7.4 `reload()`、`datapack.enable/disable/list`。
- [ ] 7.5 `list`：只读反馈，低优先；`seed`/`version`/`help` 归入只读反馈状态，不提供结构化入口，需要时用 `run`（计入 `--deny-raw`）。
- [ ] 7.6 `fetchprofile`：只读反馈，暂不提供结构化入口；`serverpack` 归入仅开发构建，不进入标准层。
- [ ] 7.7 `test`：归入测试工具，不提供结构化入口，需要时在开发构建中用 `run`。
- [ ] 7.8 越级命令策略：`tick`、`debug`、`jfr`、`kick` 等 ADMIN/OWNER 命令在固定的 GAMEMASTER 等级下不可达，不提供结构化入口，只在校验诊断中说明权限缺口。

### 阶段 8：语言与工具体验

已完成：8.1、8.2；8.6 的 LSP 与 VSCode 基础（记录见 §四）。

剩余：

- [ ] 8.1 收尾：未导入引用的诊断仍复用「找不到 X」并附导入提示，尚未在符号表标记“存在但未公开”的候选；不支持包管理、条件导入或跨项目共享模块。
- [ ] 8.3 函数宏高级调用：`function <fn> with <source>` 与 `$(key)` 宏参数，用于文本、坐标与 NBT 动态参数。
  - **评估结论：可以实现，运行时不需要新 ABI，但必须限制类型与调用面。** 26.3 的宏函数就是普通 `.mcfunction`，正文里的 `$(键)` 在调用时做文本替换；调用形式是 `function <fn> {键: "值"}`、`function <fn> with entity <选择器> <路径>`、`with block`、`with storage`。编译器可以把宏参数建模成一种新的形参类别（文本、坐标分量、NBT 片段、整数的十进制文本），在 `run` 字符串、方块 id、坐标、`text()`、`selector()` 等允许动态值的位置写 `$(参数名)`，代码生成时原样写进产物，并把该函数标记为宏函数。
  - 静态检查能覆盖：模板里每个 `$(键)` 都有同名参数、参数表里的每个参数都被使用、同一文件里 `$(...)` 的键名集合一致（宏函数的键在运行期由调用点提供，缺键会在原版加载时报错，因此调用点必须列全）；`with` 的路径语法、目标实体上下文；普通 `call`/`schedule`/`fn_tag` 引用宏函数报错，宏函数形参表不能与计分参数混用。
  - 无法覆盖：替换文本本身。调用点若提供的是运行期表达式（计分变量、NBT 读值），编译器只能退化为“任意文本”，无法保证替换后仍是合法命令；建议这类调用单独统计（计入 `--deny-raw` 的“不安全宏”计数）并在手册里明确标注。
  - 主要技术风险：`with` 的运行期 NBT 只能检查路径与实体类型；宏函数不能返回 score（返回值语义与原生命令一致，需要单独设计）；双语言产物仍可保证逐字节一致，但与手写 `.mcfunction` 没有等价对照，只能用“同一实参 → 同一产物”的回归测试。
  - 工作量：与 8.1 同量级（解析 + 模板校验 + 调用点代码生成 + 语料与文档），约 800–1200 行改动。
- [ ] 8.4 静态命令校验与补全：现有 `run` 校验只覆盖根命令与权限，剩余参数形状校验与补全（结合 1.1 的命令树）。
- [ ] 8.5 增量构建与源映射：只重建受影响函数，产物与源码行对应。
- [ ] 8.6 语言服务器剩余：文档格式化、代码操作（快速修复）、点号成员（`self.*`、`effect.*` 等）补全与语义高亮；实体 NBT 键按上下文补全/悬停。
- [ ] 8.7 文档与示例：每个阶段同步 `docs/` 与 `examples/`，保持 `--deny-raw` 端到端验收（持续事项）。
- [ ] 8.8 标准库与内建表达式：随编译器分发一批 Mclang 源码库，并把最热的原语做成内建表达式。
  - **定位**：标准库是「随编译器分发的 Mclang 源码库」，不是运行时黑盒。库源码用 Mclang 写、与用户代码走同一套检查与产物路径，因此 `--deny-raw`、双语逐字节一致、模块可达性裁剪（导入才编译）都自动适用；它不改变本文件开头的标准层判定准则，只是把已有能力组织成可复用、可版本化的模块。
  - **批 0 内建表达式（不占函数、不走 ABI）**：`min(a, b)`、`max(a, b)`、`abs(a)`、`clamp(value, low, high)` 编译为内联计分板命令（复制到临时项 + 条件赋值），与 `random(a, b)` 同属表达式层，参与现有算术与条件。理由：库函数每次调用要付一次 function 调用与每实参 1–2 条绑定命令，热路径原语必须内联；后续按真实需求增补（如 `sign`），不做无边界扩张。
  - **批 1 纯整数与上下文服务（8.8 机制就绪即可做，不依赖 8.3）**：`std::math`（gcd、整数 pow、isqrt、shl/shr、lerp、min3/max3；纯参数函数，不持有状态）、`std::timers`（`@player` 冷却与间隔计时，库自持限定名 objective，例如 `std.timers.expiry` → `<命名空间>_std.timers.expiry`）。注意计分参数是按值传递的，所以「交换两个外部计分项」这类操作不放进库（调用方用 `let` 临时量即可）。
  - **批 2 参数化与命令面补齐后（依赖 8.3 与第 4–6 阶段）**：`std::storage`（栈/队列/列表；storage id 由宏参数传入，避免固定命名空间跨包冲突）、`std::item`/`std::inventory`（安全给予、空槽查找、容器搬运，依赖 §四 的 `item` 建模与 5.x）、`std::text`（消息/标题/动作栏模板，依赖 6.1–6.3）、`std::team`/`std::damage`/`std::world` 按第 4–6 阶段落地情况增补。
  - **机制**：保留导入根 `std`（与入口别名 `main` 同级）：`import std::math;`；库源码随编译器嵌入（仓库 `stdlib/` + 构建期生成 `include_str!` 清单），`--lib <目录>` 可覆盖或扩展，项目内同名 `std/...` 模块优先（便于 fork 或整体替换）。
  - **约束**：库模块禁止自动副作用——不能声明 `@load`/`@tick`，也不能声明 `advancement`（导入即注册事件）；`@player`/`@non_player` 这类只描述调用上下文、由用户显式调用的函数允许。库自持状态一律走模块限定名；批 2 的 storage 交由调用方用宏参数传入。每个模块附独立语料，必须通过 `--deny-raw` 与双语/文档往返；模块头注释写明每条入口的命令数与函数调用成本，以及依赖的版本快照（26.3）。
  - **风险**：库膨胀与过度封装会放大数据包体积与命令链长度（每条入口写性能预算并实测）；版本漂移需要随 `data/version` 快照评审；导出面要保持小，配合选择性导入与 `as` 别名控制冲突。
  - **验收**：`import std::模块` 的最小项目通过 `--deny-raw`；同一调用重复构建逐字节一致；双语翻译产物一致；文档附录从库源码的导出清单自动生成，不手抄。

### 阶段 9：数据包内容与资源 schema（非命令）

已完成：9.2 的函数标签部分（`fn_tag`，记录见 §四）。

剩余：

- [ ] 9.1 `pack.mcmeta` 完整化：支持文本组件 `description`、`supported_formats` 范围、`overlays`（目录覆盖层）、`filters`（block/allow）、`features.enabled`（特性包）；字段取值来自 1.1 的版本元数据；可选 `pack.png` 图标。
- [ ] 9.2 标签系统：`tag <注册表> <名称> { values = [...]; replace = 假; required = 真; }` 声明，覆盖 16 个标签注册表与子目录（`block/mineable`、`item/enchantable`、`item/sulfur_cube_archetype`、`banner_pattern/pattern_item`、`enchantment/exclusive_set`、`villager_trade/<职业>`、`worldgen/biome/has_structure` 等）；条目支持 `#tag` 嵌套、`required` 与 `replace` 语义。函数标签（`fn_tag`）已完成；16 个顶层注册表与子目录尚未建模，`required` 语义也只在函数标签的外部条目上默认保留原版行为。
  - 校验：注册表与条目 id 存在、嵌套标签可解析；默认拒绝写入 `minecraft:` 命名空间，需要时显式放开。
- [ ] 9.3 资源 schema 化：把 raw JSON 升级为结构化声明，检查字段类型、枚举与未知字段；结构与原始 JSON 可共存，同类型同名称重复声明报错。
  - 第一批：predicate 条件树、loot_table（pool/entry/condition/function）、item_modifier、advancement（criteria/requirements/display/rewards/parent）。advancement 已落地，准则的条件树待做。
  - 第二批：recipe（配方类型、展示、解锁）、enchantment、damage_type、dialog。
  - 第三批：其余动态注册表（timeline、world_clock、trade_set、villager_trade、trial_spawner、trim_material/pattern、变体与声音变体、test_environment/test_instance 等）。
- [ ] 9.4 资源引用图：advancement 父级、loot table 与 entry、predicate、item modifier、dialog、tag、function、配方解锁等跨文件与跨命名空间引用解析，未解析引用在编译期报错；已建模类型之间不再依赖字符串。
- [ ] 9.5 世界生成：按 biome → structure/structure_set → template_pool/processor_list → feature/placed_feature → noise/noise_settings/density_function/carver → dimension_type/world_preset/flat_level_generator_preset 的顺序补齐 schema 与引用校验。worldgen 是全量覆盖中最大的一块，允许按需推进，未建模类型继续使用原始 JSON。
- [ ] 9.6 二进制 NBT 资源：`structure` 类型输出 `data/<ns>/structure/<名称>.nbt`（复用已完成的结构化 NBT 编码），供 `place template` 与结构方块使用。
- [ ] 9.7 打包与分发：`--zip` 输出可直接放入 `datapacks/` 的压缩包；overlay 目录布局；包图标；输出结构与内置特性包对齐。
- [ ] 9.8 版本迁移与多目标：pack format 升级时的资源与命令迁移报告；结合 1.1 快照支持多目标版本后端。

## 四、已完成能力

按领域归档；每项都通过对应语料与示例验证。§三 的条目完成后移动到这里。

### 4.1 编译器、工具链与文档

- **模块边界**：词法（`src/lexer.rs`）、语法树（`src/ast*`）、解析（`src/parser/`）、语义检查（`src/compiler/validate/`）、代码生成（`src/compiler/codegen/`）、命令行（`src/main.rs`）。
- **诊断**：带文件名、行列与源码片段，一次返回全部错误；`src/analysis.rs` 为工具侧提供结构化诊断与符号表。
- **产物**：`pack.mcmeta`、`.mcfunction` 与 load/tick 标签；`.mclang-manifest` 记录输出所有权，重建只清理自己生成的文件。
- **命令**：`build`/`check` 支持单文件与递归项目目录，检查命名空间一致性。
- **质量门槛**：`cargo fmt`、`cargo clippy --all-targets` 零警告；`tests/valid`、`tests/invalid` 编译语料；端到端示例。
- **可复现**：`BTreeMap` 与稳定哈希；`tests/corpus.rs` 断言重复构建逐字节一致。
- **文档**：`docs/content/manual.md` → `docs/index.html`，`bun docs/tools/build.mjs --self-test` 用真实编译器做双语往返验证。
- **LSP 与编辑器**：`mclang lsp`（UTF-16 位置换算、全文同步、项目级即时诊断、声明与关键词补全、悬停、跨文件跳转）；VSCode 插件（TextMate 高亮、语言配置、按 `mclang.server.path`/`target/`/`PATH` 解析可执行文件）。

### 4.2 语言核心与类型系统

- **双语关键词**：任意结构都有中英写法、同文件可混用、关键词表单一来源（`src/parser/keywords.rs`）、两种写法产物逐字节一致；中英关键词互为保留字。
- **中文标识符**：声明名支持 Unicode 字母；编译期按稳定哈希换成 8 个小写字母别名（`compiler/rename.rs`），整程序唯一、避开 ASCII 标识符与内部固定名；`resource`/`advancement`/`fn_tag` 与准则名同样处理；诊断显示源码原名。
- **控制流**：`if`/`while`（常量条件三值折叠、`!`/`&&`/`||` 短路）、`for <变量> in <起点>..<终点>`（半开区间、动态终点只求值一次、空区间不生成命令）、`break`/`continue`（每层循环一个状态计分项，`each`/`spawn` 内报错）。更强的优化（循环不变量外提、小常量区间展开、条件与循环体合并）明确不列入当前计划。
- **函数与调用**：参数、score 返回值、`let` 词法局部量、稳定假玩家命名、同步调用图递归拒绝。
- **`--deny-raw`**：递归拒绝 `run`、字符串 `execute` 与 `return run`；`examples/portable_chest` 零底层命令验收。
- **坐标与向量**：`pos(x, y, z)`、`column(x, z)`（绝对/`~`/`^`，`^` 不可与其它写法混用，绝对分量检查世界范围与 256 区块上限）、`vec3`（`teleport` 落点）、`vec2`（`worldborder.center`）、`rotation(yaw, pitch)`。
- **方块状态**：`block_state("id") { 属性 = "值"; }`，属性字符集、重复声明与 `#` 标签谓词的使用位置在编译期检查。
- **文本组件**：`text`/`translate`（含参数）/`keybind`/`score`/`selector`/`nbt`（entity/block/storage），五种 click 事件与 `show_text` 悬停；`codegen/components.rs` 稳定序列化。
- **实体查询**：`type("#标签")`/`without_type`、`name`、`scores`、`nbt`、坐标盒 `box`、`distance`（与 `within` 取交）、`level`、`gamemode`、`team`、`rotate`、`predicate`、`advancements`；物品谓词支持完整组件文本（`id[...]`）与任意槽位（对照 `SlotRanges` 与 `slot_source`）。
- **结构化 NBT**：`nbt { ... }` 覆盖 26.3 SNBT 的全部 12 种标签类型（`1b`/`1s`/`1`/`1L`/`1.5f`/`1.5d`、字符串、列表、嵌套复合、`[B;]`/`[I;]`/`[L;]`），解析期检查数值范围、单精度可表示性、数组元素后缀与重复键，`emit::nbt_text` 规范化输出；已接入 `set_block`/`fill` 的方块实体数据与 `item_stack` 的 `custom_data`。注意：`s`/`d` 紧贴数字时按 NBT 后缀解析，时间单位需要空格或逗号（`1 s`）。
- **模块系统**：`main.mcl` 入口 + 可达图、可选 `namespace`、默认私有 + `export`、整模块/选择性/别名导入、`mod.mcl` 目录模块、导入环；限定名重写（函数/资源/进度/标签用 `模块/名称`，计分变量/目标/查询/物品/存储/数据槽用 `模块.名称`）；诊断覆盖找不到模块（给出预期路径）、未公开、冲突、命名空间不一致、非法路径分段；合并按 BFS 顺序，重复构建一致。

### 4.3 执行模型与条件

- **四态上下文**（无 / 任意实体 / 非玩家实体 / 玩家）与 `@load`/`@tick`/`@entity`/`@non_player`/`@player`；`data` 类操作只允许非玩家实体。
- **结构化 `execute`**：`as`/`at`/`positioned`/`rotated`/`facing`/`align`/`anchored`/`in`/`on`/`summon` 修饰符；重复修饰符、修饰符写在条件之后、条件写在 store 之后等在编译期报错；按书写顺序下降为真实 `execute` 链；`as` 推导玩家/非玩家上下文、`on` 要求实体上下文、`summon` 的实体类型参与 NBT 校验；旧字符串子句保留为逃生口并计入 `--deny-raw`。
- **条件族**：`if`/`unless` 支持计分比较、谓词、`block`/`blocks`/`biome`/`dimension`/`loaded`/`entity`/`data`/`items`/`slots`/`function`/`stopwatch`；原子条件冻结到临时计分项再组合，保证一次求值与确定顺序；函数条件计入同步调用图递归检查。
- **store**：`store.result/success`（计分、Boss 栏 `value|max`）与 `store.data`（entity/block/storage，实体来源要求 `limit(1)`，`self` 要求非玩家）；store 链包裹块内最后一条命令、可叠加；控制结构尾部与无返回值函数调用报错；`origin` 持有者用临时项捕获 + `on origin` 复制表达。
- **函数、标签与调度**：`call`、表达式调用、`#tag`；`fn_tag` 声明（嵌套 `#标签`、外部字符串条目、`replace`、编译期环路检查），标签成员在编译期检查执行上下文与参数；`schedule`（整数/浮点时间、`replace`/`append`、`#tag`、`schedule.clear`）；`return`/`return fail`/`return run`。
- **结果表达式**：`count(q)`、`data.get`、`random(1, 6)`、`compute`、`xp.query`、`stopwatch.query`、`scoreboard.get`、`time.query`/`query_gametime`、`gamerule.query`、`worldborder.get`，经 `execute store result score` 落入计分并参与算术与条件。
- **函数权限**：`run`/`return run` 的根命令对照 `commands.json`，固定按 GAMEMASTER（2）拒绝 `ADMIN`/`OWNER` 命令并解释权限缺口。

### 4.4 计分板、数据与资源

- **目标与计分板**：`objective` 声明（准则、显示名、渲染类型 integer/hearts、数字格式 blank/fixed/styled、显示槽），运行期名 `<命名空间>_<名称>`，由 `__mcl/load` 创建；与内部 ABI 目标隔离；`scoreboard.set/reset` 与表达式 `get`（持有者 `self`/`origin`/查询，读取要求 `limit(1)`，失败为 0），`scoreboard.enable/operation/display`。
- **数据槽**：`data_slot` 声明（`item_data` → 物品堆 `minecraft:custom_data`、`entity_data` → 26.3 实体通用 `data` 字段），编译期校验键与实体类型（物品槽必须配 `minecraft:item`，实体槽拒绝玩家）。
- **容器搬运**：`self.deposit/withdraw/remove_data`（`Items` 与数据槽互搬；追加成功才清空、取回成功才删除来源）、`self.save_items/restore_items`、`self.remove_preserving_items(slot, query)`（带成功校验的安全移除，不掉落物品）、`self.add_tag/remove_tag/set_invulnerable/set_no_gravity/give_item/clear_items/remove`。
- **`data` 命令**：`data.get`（结果表达式）、`data.merge`、`data.remove`、`data.modify`（insert/prepend/append/set/merge；from/string/value/compute 来源）；entity/block/storage 三类目标；实体目标沿用非玩家写保护。
- **`item` 命令**：`item.replace/fill/override/modify`；实体（`limit(1)`）与方块目标、任意槽位（槽位名或 `slot_source` 资源位置）、`with` 引用已声明物品、`from` 跨容器复制与可选修饰器。
- **物品定义**：名称、Lore、附魔、存储附魔、损伤、无法破坏、稀有度、物品模型、染色、光效覆盖、最大损伤、最大堆叠与 `custom_data`；`give` 数量按 `最大堆叠数 × 100` 校验。
- **JSON 资源**：`resource` 声明（26.3 注册表类型限制、编译期 JSON 解析、稳定格式化）；`predicate`；`advancement` 声明（`parent`/`criterion`（`trigger` + `conditions`）/`requirements`/`reward`/`display`，触发器对照 `CriteriaTriggers` 的 58 项、条件字段对照 `advancement_triggers.json`、26.3 用 `type` 键的内联战利品条件、引用与同名冲突检查）。

### 4.5 世界与方块命令

- `set_block(pos, block_state[, 模式][, nbt])`，模式 destroy/keep/replace/strict。
- `fill(from, to, block_state[, 模式][, replace 过滤器][, nbt])`，模式含 outline/hollow/destroy/strict；`nbt` 可出现在任意可选参数位置。
- `clone(...)`：跨维度、masked/filtered、force/move/normal、strict，选项顺序无关、重复报错。
- `fill_biome(from, to, biome[, replace filter])`，过滤器接受 `#` 生物群系标签。
- `place.feature/jigsaw/structure/template`，含 rotation、mirror、integrity、seed、strict。
- `forceload.add/remove/remove_all/query`，绝对范围检查 256 区块上限。
- `time.set/add/pause/resume/rate` 与表达式 `time.query([clock])`、`time.query_gametime()`；时钟作为可选参数写在末尾，生成 `time of <clock> ...`。
- `weather.clear/rain/thunder(duration)`，按 `TimeArgument` 换算并拒绝不足 1 刻。
- `gamerule.set(name, value)` 与表达式 `gamerule.query(name)`；规则名与值类型对照 26.3 `GameRules`。
- `worldborder.add/set/center/damage_amount/damage_buffer/warning_distance/warning_time` 与表达式 `worldborder.get()`。
- `locate.structure/biome/poi`（接受 `#` 标签，仅命令反馈）。
- 示例：`examples/world_ops.mcl` 覆盖以上能力并通过 `--deny-raw`。

### 4.6 实体、玩家与事件

- `spawn("id", pos|vec3) { nbt { ... } ... }`：进入新实体上下文，首条命令通过 `execute summon` 的 `@s` 合并初始 NBT；拒绝 `noSummon` 类型。
- `nbt { ... }` 语句：生成 `data merge entity @s {...}`；`entity_nbt.json` 快照按实体类型/并集检查键与粗类型，未知键给出编辑距离 ≤ 2 的最近候选；125 个常用标签有中文别名（中英双向一对一，输出逐字节一致）。
- `give(target, item[, count])`（已声明玩家查询 + `item_stack` 定义）。
- `clear(玩家查询[, 物品][, 数量])`，数量上限 2147483647。
- `effect.give/give_infinite/clear`，秒数与等级按 26.3 范围检查。
- `xp.add/set` 与表达式 `xp.query`（要求 `limit(1)` 玩家查询）。
- `stopwatch.create/restart/remove` 与表达式 `query([缩放])`（失败写入 0）。
- `teleport(持有者, pos|vec3[, rotation]，单个实体查询)`：绝对与 `~` 坐标、世界范围校验；持有者支持 `self`/`origin`/查询。
- `message.all/self/nearest/player` 接受文本组件；纯字符串与末尾颜色参数保留为旧写法。
- `sound.self` 与 `sound.play(sound, source, targets, pos, volume, pitch, min_volume)`。
- `advancement.grant/revoke[_through|_from|_until|_everything]`：玩家目标、`only` 可带准则名、引用检查、不计入 `--deny-raw`。
- 示例：`examples/portable_chest`（全中文标识符、矿车按实体计分归属、forceload 屏障盒）、`potion_lab.mcl`、`give_reward.mcl`、`portal.mcl`、`beacon_base.mcl`、`bounty_hunter/`。

### 4.7 版本数据与快照

- `cargo xtask generate-version-data` 生成 `data/version/26.3-rc-2/`：
  - `commands.json`：从 `Commands.java` 与 `server/commands/**` 提取的根命令、字面量子命令、重定向、参数与 `requires` 权限等级（快照含 104 个根名，含别名与开发构建命令；§二 按常规命令逐条计 97 个；动态子树以一级清单近似）。
  - `registries.json`：item/block/entity_type/biome/dimension/damage_type/mob_effect/enchantment/attribute/particle/sound/advancement/recipe/loot_table/predicate/dialog/post_effect/timeline/world_clock/slot/slot_source/worldgen* 等 id 集合，以及标签注册表清单与资源目录清单。
  - `enums.json`：gamemode、difficulty、显示槽、队伍颜色、heightmap、anchor、swizzle、声音分类、时间单位、物品槽位。
  - `entity_nbt.json`：扫描 `world/entity/**` 的 `putX`/`store`/`read` 键并沿 `extends` 求并集，当前 161 个实体类型、277 个标签。
  - `advancement_triggers.json`：`CriteriaTriggers` 各 `TriggerInstance` 的条件字段与粗类型。
- 输出排序且带 FNV-1a 源摘要，可复现；`cargo xtask check-version-data` 与 `tests/corpus.rs` 比对快照与随附源码。
- 编译器加载快照：资源位置从“语法合法”升级为“注册表存在”，未知 id 报错并给出最近候选。
- 26.3 条件格式：内联战利品条件的判别键从 `condition` 改为 `type`（旧写法会在原版加载时报 `No key type in MapLike[...]`），示例、语料与手册已全部修正并对照原版数据核对。

### 4.8 测试语料与示例

- `tests/valid/`：`advancement`、`chinese_identifiers`、`components`、`conditions`、`data_ops`、`effects`、`entities`、`execute`、`expressions`、`language`、`loops`、`modules`、`multifile`、`nbt`、`raw`、`scoreboard`、`world` 等；`tests/dual` 双语往返。
- `tests/invalid/`：模块、命名空间、语法负例与 `advancement_conditions`、`chinese_identifiers`、`context`、`execute`、`items`、`loops`、`nbt_tags`、`player_nbt`、`recursion`、`stopwatch_*`、`unknown_references`、`world` 等，文件头注释写明期望诊断。
- `tests/corpus.rs`：语料编译、产物布局、重复构建一致性与版本数据校验。

## 五、依赖关系与验收

- 已完成的基础设施（1.1–1.6）是后续阶段的共同前提：坐标与值类型（1.2/1.4）支撑第 3–5 阶段，文本组件（1.3）支撑第 6 阶段，结果表达式（1.5）支撑 4.3 与 7.1–7.3，权限校验（1.6）用于 `run` 字符串。
- 第 4–6 阶段的命令依赖已完成的结构化 `execute` 与条件族（见 §四），以便在非默认上下文中执行。
- 8.8 批 0 无前置；批 1 只需要 8.8 自己的导入根与嵌入机制；批 2 依赖 8.3 的宏参数与第 4–6 阶段被包装的命令面（title/bossbar/particle/team/attribute/loot/item/damage 等），并与 5.x 的物品/战利品结构化入口同步扩充。标准库本身不新增 JSON schema；若未来库要携带 predicate/loot_table 资源，则依赖 9.2/9.3。
- 9.1/9.2 依赖 1.1 的版本元数据与注册表快照；函数标签部分已落地，不依赖注册表数据；9.3–9.6 依赖 1.4 的结构化 NBT 与值类型；9.7/9.8 依赖输出层与 1.1 的版本数据。
- 每个阶段的验收：编译器零警告；`examples/` 项目通过 `--deny-raw`；`tests/` 的 mcl 编译语料核对生成的命令与资源文件；中英文关键词产物逐字节一致；文档与手册同步。