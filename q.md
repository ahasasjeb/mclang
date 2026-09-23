# mclang × minecraft_client_26.3 命令对照审计总报告

**范围**：68 个已实现命令族（DEVELOPMENT_PLAN 口径）+ 5 个别名。6 个批次中 5 份已完成；**批次 4（title/bossbar/dialog/particle/stopsound/posteffect/msg/teammsg/tellraw/playsound + 文本组件 JSON）子代理返回空结果，未审计**，需接管模型补做。
**方法**：逐条比对 mclang 代码生成形状（`src/compiler/codegen/`）与客户端 `dispatcher.register(...)` 全树（`minecraft_client_26.3/net/minecraft/server/commands/`），含参数类解析约束；并交叉核对 `data/version/26.3/commands.json`。

---

## 一、项目理解摘要

- mclang 把 `.mcl` 编译为 26.3 数据包（格式 121.0）；命令字符串在 `src/compiler/codegen/` 纯格式化拼出，语义阶段已保证引用合法。
- 真值源：`minecraft_client_26.3/`（反编译源码）。`data/version/26.3/commands.json` 由 `src/version/generate/commands.rs` 从 Java 注册表达式提取（仅解析内联 `literal/argument/then/requires/redirect` 子集）。
- raw 逃生口只校验根命令权限等级（`validate_raw_command`，≤2 GAMEMASTER）；全命令树校验尚未实现。
- 计分板 ABI：内部 objective + 假玩家 `#…`；辅助函数 `__mcl/…`；宏 `$()` + `function … {snbt}`。

---

## 二、A 级——**MC 会直接拒绝**（必须修，共 8 处根因）

| # | 位置 | 问题 | 原版要求 | 后果 |
|---|---|---|---|---|
| A1 | `src/compiler/codegen/emit.rs:47-57` `entity_query_selector` | 先发 `type=<entity_type>` 再拼 `type_filters` 的 `type=`/`type=!` → **同一 `type` 选项重复** | 26.3 `InvertableSetOptionState`（`selector/options/InvertableSetOptionState.java:28-50`）只允许「单个正类型」或「一组负类型/标签」，之后 `type` 选项不可再用（`EntitySelectorOptions.java:311-384` 抛 `ERROR_INAPPLICABLE_OPTION`） | **任何带类型过滤器的查询都生成非法选择器**（如 `@e[type=minecraft:player,type=!minecraft:zombie]`），全部命令族通用 |
| A2 | `src/compiler/codegen/scoreboard.rs:77` | `scoreboard players numberformat <targets> <obj> …` **缺 `display` 关键字** | 真实路径是 `scoreboard players display numberformat …`（`ScoreboardCommand.java:170`） | 100% 解析拒绝 |
| A3 | `src/compiler/codegen/world.rs:157-185` `clone_command` | `to <dim>` 拼在 **destination 之后** | 原版 `clone [from <dim>] <begin> <end> [to <dim>] <destination> …`（`CloneCommands.java:62-67`），`to` 在 end 与 destination **之间** | 带 `to` 维度即拒绝 |
| A4 | 同上 | `strict` 拼在**最后** | `strict` 是 destination 的直接子节点、位于过滤/模式**之前**（`CloneCommands.java:76-88`） | `… masked force strict` 等组合拒绝（裸 `dest strict` 碰巧合法） |
| A5 | 同上 + `CloneMode::as_str` | 过滤缺省但模式≠normal 时生成 `clone … dest move/force` | 模式字面量只挂在 `replace|masked|filtered <filter>` **之下**（`wrapWithCloneMode`，`CloneCommands.java:106-122`），destination 无裸模式子节点 | **拒绝**；且 `CloneFilter::Replace→""` 导致无法显式输出 `replace` 来修复表达 |
| A6 | `src/compiler/codegen/expressions.rs:397-399` 条件 `if\|unless stopwatch` | 生成 `stopwatch <id>` **缺范围参数** | 原版 `if\|unless stopwatch <id> <range:RangeArgument.floatRange()>`（`ExecuteCommand.java:371-373`，id 节点无 executes） | 100% 解析拒绝 |
| A7 | `validate_nbt_source`（`src/compiler/validate/components.rs:198-220` 及 4 类入口：DataMerge/DataRemove/DataModify 目标、from/string 来源、`data get` 表达式、`if data` 条件） | `Holder::Query` **不强制 `limit(1)`** | 原版 data 族实体参数是 `EntityArgument.entity()` **单实体**（`EntityDataAccessor.java:29`） | `data get/merge/modify/remove entity @e[…]` 多匹配选择器触发 `ERROR_NOT_SINGLE_ENTITY`，解析拒绝（self.* 数据槽路径已有 limit(1) 校验，`validate/statements/entities.rs:205-208`，可照抄） |
| A8 | `src/compiler/validate/core_commands.rs`（say/me） | **不校验消息长度 >256** | `MessageArgument.java:134-135` 剩余长度 >256 抛 `TOO_LONG` | 长文本拒绝 |
| A9 | `src/parser/core_commands/remaining.rs:90`（test 的 times） | `unsigned()` 无上界，可输出 >2147483647 | `IntegerArgumentType.integer(0)`（`TestCommand.java:318+`）i32 范围 | 超界拒绝 |
| A10 | `src/compiler/validate/world/gamerules.rs:51-54` | `max_minecart_speed` 无特性约束 | 该规则要求 `minecart_improvements` 特性（`GameRules.java:52`），默认特性集**不注册该字面量** | 默认特性集下 `gamerule max_minecart_speed …` 解析拒绝 |
| A11 | `src/compiler/codegen/emit.rs:60-66`（`name=` 过滤） | 裸值不加引号 | 原版 `name` 用 `readString()`，含空格/特殊字符必须引号 | 名字含空格时拒绝（边缘） |
| A12 | `src/compiler/validate/world/border.rs:54-58` | damage amount/buffer 按 f64 校验 ≥0 | 原版 `floatArg(0.0F)`（f32，`WorldBorderCommand.java:36-66`） | >f32::MAX 时原版拒绝（边缘） |
| A13 | `src/compiler/validate/scoreboard.rs` + `scoreboard.rs:51-53` | `scoreboard players list <holder>` 不限制单持有者 | `ScoreHolderArgument.scoreHolder()` **单**（`ScoreboardCommand.java:147`） | 多匹配选择器拒绝 |

---

## 三、B 级——**MC 接受但值/语义错误**（应修）

| # | 位置 | 问题 | 说明 |
|---|---|---|---|
| B1 | `src/compiler/codegen/world.rs:58-61` `vec2_text` + `signed_number_text`（`functions.rs:112-125`） | **`worldborder center` 整数值被加 +0.5** | `Vec2Argument.vec2()` 为 `centerCorrect=true`（`WorldCoordinate.java:26-38`），无小数点的整数文本会 +0.5；Rust `Display` 把 `10.0` 打成 `10` → `worldborder center 0 0` 实际中心 **(0.5, 0.5)**。修法：绝对整数分量补 `.0` |
| B2 | `src/compiler/codegen/actions.rs:257-258` `compile_item_action` With 分支 | **物品定义的 `count` 被静默丢弃** | `item replace … with <item>` 不输出 count（原版可选 `count:integer(1,99)`，`ItemCommands.java:63-110`）；定义 count>1 时语义丢失 |
| B3 | say/me 文本原样输出 | MessageArgument 会把 `@` 开头片段解析为选择器并替换 | 消息里的 `@e` 等会被实体名替换（语义差异，不拒绝） |
| B4 | 查询自带物品过滤在 data 路径被丢弃 | `component_holder` 直出选择器的路径（`components.rs:178-187`）不含 item 过滤，data 语句无 `capture_command_targets` 兜底 | 语义风险；teleport 有兜底（`command_targets.rs:25-104`） |

---

## 四、C 级——表达缺口（mclang 写不出合法原版形状，不产生非法命令）

- `tag`：标签名不允许 `+`（`rules.rs:67-74`；原版 `word()` 允许）
- `team join/leave`：裸成员名不支持 `#假玩家`（`validate/entity_commands/teams.rs:62-71`；原版 `ScoreHolderArgument` 接受）
- `waypoint`：hex 只收 6 位（原版 `HexColorArgument` 收 3 位缩写）；style id 强制带命名空间
- `spreadplayers`：vec2 禁 `^` 局部坐标（原版允许）
- `test runfailed`：强制解析 onlyRequiredTests，缺「无参 runfailed」「runfailed <times>…」两变体（`remaining.rs:81-84`）
- `fill`：无法表达 `replace <filter> <outline|hollow|destroy|strict>`（有过滤即忽略模式，`world.rs:130-133`）；过滤器不能带 `{nbt}`、`#tag[a=b]`（vagueProperties）
- `clone`：过滤器同上；`replace`/`normal` 显式字面量无法输出（连带 A5）
- `time`：`set <timemarker>`（day/noon/night/midnight/…，`ClockTimeMarkers.java:14-19`）与 `query <timeline> [repetition]` 无法表达（26.3 **没有** `time set day` 字面量、也**没有** `daytime/day` 查询）
- `loot`：不支持内联 loot table（`ResourceOrIdArgument` 可收 SNBT 值）
- `data get` 无 `scale`；`from/string` 的 sourcePath 必给（原版可省）
- `execute`：缺 `positioned as/over`、`rotated as` 子句（可走 Raw）
- `advancement … only <criterion>`：criterion 只收标识符（原版 greedyString 可含空格）
- `item` 的 entity 目标强制 limit(1)（原版 `entities()` 复数，`EntityItemAccessor.java:27`）——过紧，属能力缺口
- `datapack create`（权限 4）有意不支持 ✓
- 资源位置强制显式命名空间（原版裸 id 补 `minecraft:`）——普遍更严，不产生非法命令
- place template / locate 的 `minecraft:` 存在性快照检查可能**误拒数据包新增资源**（不产生非法命令）

## 五、D 级——等价但不同形（可不修）

give/clear 恒包 `execute … @s` 并恒输出 count；rotate/teleport 的 facing anchor 恒显式 `feet|eyes`；schedule 恒输出 `replace|append`；`return 0` 代替裸 return；setblock 的 `replace` 省略；datapack name 恒引号；effect 从不输出 `hideParticles false`；`scoreboard objectives add` 不用可选 displayName 参数；teleport 多目标拆成逐实体 `tp @s`；execute store 拆进辅助函数；xp query 强制 limit(1)；`if data` 路径恒给（原版恰也必填）。

---

## 六、commands.json 提取器失真（不影响生成，影响树快照/raw 校验完整性）

**根因**：提取器只解析注册表达式内联的 `literal/argument/then/requires/redirect`，不展开 helper 返回的 builder，且多次 `register` 按名错误合并。失真清单：

| 命令 | 失真 |
|---|---|
| swing | 缺 mainhand/offhand、animation（SwingAnimationArgument）、duration（TimeArgument.time(1)）整棵子树（`handSwing(...)`） |
| gamemode | 漏 `level: 2`（`Commands.hasPermission(PERMISSION_CHECK)` 不识别；`Commands.java:172` 与 LEVEL_GAMEMASTERS 同权） |
| difficulty | 缺 peaceful/easy/normal/hard 字面量（`getSerializedName()` 循环注册） |
| test | 缺 verify/locate/reset*/clear*/stop/pos/create 与 run 全部参数 |
| random | 缺 `value`/`roll` 子树 |
| loot | 空壳 |
| item | 空壳 |
| data | 空壳（全文件 NbtPathArgument 出现 0 次） |
| compute | 空壳 |
| gamerule | 空树（59 规则 × 双字面量 + value 全缺） |
| fill | `to` 以下整棵缺失（`wrapWithMode`） |
| clone | 只剩 `from→sourceDimension`（`beginEndDestinationAndModeSuffix`/`modeSuffix`） |
| time | 错乱：缺 set/add/pause/resume/rate；query 只剩 gametime+of；凭空 `gametime{of}`；根错误标 executable（`addClockNodes` + 两次 register 合并） |
| scoreboard objectives modify | 缺 rendertype、numberformat（`createRenderTypeModify`/`addNumberFormats`） |
| scoreboard players display numberformat | 缺 `<objective>` 与 blank/fixed/styled 分支 |
| function | 缺 `with block/entity/storage [path]`（`ArgProvider.wrap`） |
| execute | 缺 if/unless 全树、on 全树、store 的 score/bossbar/data 子树（`addConditionals`/`createRelationOperations`/`wrapStores`） |
| return | `run` redirect 未展开 |

其余（kill/tag/enchant/damage/attribute/ride/rotate/spreadplayers/spectate/trigger/setworldspawn/spawnpoint/team/waypoint/list/reload/help/version/seed/say/me/fetchprofile/recipe/datapack/give/clear/schedule/advancement/stopwatch/setblock/fillbiome/place/forceload/weather/worldborder/locate/teleport/effect/experience）与 Java 注册一致。

---

## 七、逐命令判定汇总（5/6 批次）

| 命令 | 判定 | 关键问题 |
|---|---|---|
| kill/tag/enchant/damage/attribute/ride/rotate/spreadplayers/spectate/swing/trigger/gamemode/defaultgamemode/difficulty/spawnpoint/setworldspawn/team/waypoint/list | 一致（形状） | A1 共性；C 级缺口若干 |
| reload/help/version/seed/fetchprofile/recipe/random/loot/give/clear | 一致 | D 级不同形 |
| say/me | **有差异** | A8 |
| test | **有差异** | A9 + runfailed 缺形 |
| datapack/item | **有差异** | item：B2 count 丢弃 + entity 过紧 |
| scoreboard | **有差异** | A2、A13 |
| function/schedule/return/advancement/stopwatch | 一致 | — |
| setblock/fillbiome/place/forceload/time/weather/locate | 一致 | time 有 C 级缺口 |
| fill | **有差异** | C 级 ×3 |
| **clone** | **有差异** | **A3/A4/A5（3 处会拒绝）** |
| gamerule | **有差异** | A10 |
| worldborder | **有差异** | B1（+0.5 值错误）、A12 |
| execute/teleport/effect/experience/compute | 一致 | D 级不同形 |
| data + 条件表达式 | **有差异** | **A7、A6（会拒绝）** |
| **title/bossbar/dialog/particle/stopsound/posteffect/msg/teammsg/tellraw/playsound（批次 4）** | **未审计** | 子代理空结果，**需补做**（含 26.3 文本组件 codec：`object` 组件、click show_dialog/custom、hover show_item/show_entity、颜色枚举） |

---

## 八、建议修复顺序（给接管模型）

1. **A1**（选择器 `type=` 去重/合并逻辑，影响面最大）
2. **A2**（一行修）
3. **A3/A4/A5**（clone 输出顺序与 `replace` 显式化，需动 `CloneFilter` 语义）
4. **A6**（stopwatch 条件补 range，需动 AST/解析/校验/生成）
5. **A7**（`validate_nbt_source` 加 limit(1)，照抄 `entities.rs:205-208`）
6. **A8/A9/A10/A11/A12/A13**（校验层小补丁）
7. **B1**（vec2 补 `.0`）
8. **B2**（item with 补 count）
9. 补做批次 4 UI 审计
10. （可选）提取器修复或标注 commands.json「树不全」

**验证方式**（项目无单测）：改后写 `.mcl` 语料实测生成命令文本 + `cargo fmt` / `cargo clippy --all-targets` + `bun docs/tools/build.mjs --self-test`。`tests/invalid/` 语料需按新校验逐个 `mclang check` 核对。

---

## 附录：六批次明细审计要点

### 批次 1（实体命令族）关键结论

- **kill/tag/enchant/damage/attribute/ride/rotate/spreadplayers/spectate/swing/trigger/gamemode/defaultgamemode/difficulty/spawnpoint/setworldspawn/team/waypoint/list**：生成形状与 Java 注册全部一致。
- damage 的 `at/by` 挂在 damageType 之下（无 type 不能 at/by），mclang 解析器同样强制 ✓；attribute 的 operation 字面量 `add_value/add_multiplied_base/add_multiplied_total`（26.3 现行名）✓；swing 的 hand→animation→duration 父子嵌套 ✓；spreadplayers 的 respectTeams 是 `BoolArgumentType`（非字面量）、under 在 maxRange 后 ✓；spawnpoint 的 rotation 是 RotationArgument 节点 ✓；team 的 camelCase 枚举映射（`hideForOtherTeams` 等）✓、color 用 TeamColorArgument 的 snake_case ✓；waypoint 的 `color hex` 是 color 的子字面量 ✓。
- 共性：**A1 `type=` 重复**、**A11 `name=` 不加引号**。

### 批次 2（核心/物品/loot）关键结论

- give/clear/loot/recipe/datapack/random/fetchprofile/help/version/seed/reload 形状一致；`clear` 的 maxCount 仅在 item 后（与原版树一致）✓；loot 的 target×source 全连接与 slot `accepts_single` 等价 SlotArgument ✓；item 谓词文法 `id[!c|c~v,c2=v2]` 与 `ComponentPredicateParser` 逐项一致 ✓。
- 差异：say/me 超长未校验（A8）、test times 超界（A9）、item with 丢 count（B2）、datapack name 恒引号（D 级）、loot 缺内联表（C 级）。

### 批次 3（计分板/函数类）关键结论

- function 的 `{snbt}` + `with block/entity/storage [path]`、schedule 的 `function <id|#tag> <time> [replace|append]`、return 全形态、advancement 的 only/from/until/through/everything 组合、stopwatch 四子命令：全部一致。
- 差异：**A2 numberformat 缺 `display`**、**A13 list 单持有者**、objectives add 拆两条命令（D 级）。

### 批次 5（世界命令）关键结论

- setblock/fillbiome/place/forceload/time/weather/locate 一致；place 的 rotation/mirror 枚举逐字一致、max_depth 1..=20 与 integrity 0..=1 与原版范围一致、可选参数逐级递进一致 ✓；26.3 **没有** `time set day|noon|night|midnight` 字面量（是 `<timemarker:Identifier>`），也没有 `query daytime/day` ✓ mclang 恰好未生成这些。
- 差异：**clone 三处会拒绝（A3/A4/A5）**、**gamerule max_minecart_speed 特性门控（A10）**、worldborder center +0.5（B1）、fill 缺 filter+mode 组合（C 级）。

### 批次 6（execute/teleport/effect/xp/data）关键结论

- execute 的 as/at/positioned/rotated/facing/align/anchored/in/on/summon 与 store 三目标（score/bossbar/data）形状一致；teleport 的三形式（坐标、实体、facing 两变体）一致且正确拒绝「实体落点+旋转」；effect 的秒数 1..=1000000、amplifier 0..=255、infinite 字面量一致；xp 别名树与 experience 完全一致；compute 的 provider 形状与 scale 约束一致。
- 差异：**A6 if stopwatch 缺 range**、**A7 data 实体来源缺 limit(1)**、execute 缺 positioned as/over 与 rotated as（C 级）。
