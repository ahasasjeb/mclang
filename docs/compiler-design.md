# 编译器与目标数据包设计

## 编译流水线

输入文件依次经过 UTF-8 词法分析、递归下降解析、项目合并、全程序语义检查和数据包代码生成。目录输入会递归发现 `.mcl` 文件，并跳过隐藏目录、`target` 和 `build`。解析器生成带源文件身份和字节范围的语法树，诊断层再把范围转换为 Unicode 感知的行列和源码标记。只有所有静态检查都通过后，构建命令才会更新输出文件。

词法器接受 Unicode 文字，以便识别中文关键词；解析器在构造 AST 时把中英文关键词、方法和枚举值规范化为同一内部表示。用户标识符仍由语义阶段限制为稳定的 ASCII 名称，后续阶段无需处理双语分支。

实体查询、物品定义、物品存储、维度、消息、`give`、`effect`、`xp`、`clear` 和 `self` 操作在 AST 中保留为结构化节点。物品定义中的名称、Lore、普通附魔、存储附魔、损伤、最大损伤、最大堆叠、稀有度、物品模型、染色、附魔光效覆盖和无法破坏也是独立字段。语义阶段检查资源位置、查询、物品和存储引用、执行上下文、范围、计数、附魔等级、标签及枚举值；代码生成阶段才创建选择器、物品组件谓词、`give` 参数、NBT 命令和文本组件 JSON。Mclang 源码因此不依赖这些底层字符串的拼写。

`give` 语句的目标写成查询时必须解析为 `minecraft:player`，后端生成 `execute as <选择器> at @s run give @s <物品> <数量>`。数量省略时取物品定义的 `count`；上限按 26.3 `GiveCommand` 的 `最大堆叠数 × 100` 计算，未声明 `max_stack_size` 时按最小堆叠数 1 取保守上限 100。`self.give_item` 生成同形状但没有目标包装的 `give @s` 命令。

语义阶段使用四种执行上下文：无、任意实体、非玩家实体和玩家。任意实体来自 `@entity` 函数，非玩家实体来自非玩家查询的 `each`、`spawn` 和 `@non_player` 函数，玩家来自玩家查询的 `each` 和 `@player` 函数。`spawn` 因为 `noSummon` 限制不能生成 `minecraft:player` 和 `minecraft:fishing_bobber`，语义阶段会拒绝这两个类型。`@entity`、`@non_player` 和 `@player` 把最低上下文要求加入函数签名，调用点用 `satisfies` 判定：非玩家实体满足任意实体，玩家也满足任意实体，但两者互不满足。`self.give_item`、`message.self` 和 `sound.self` 要求玩家；`set_invulnerable`、`save_items`、`restore_items`、`remove_preserving_items` 和 `clear_items` 通过 `data` 命令修改实体 NBT，26.3 的 `EntityDataAccessor` 对玩家抛出 `commands.data.entity.invalid`，因此要求非玩家实体；其余 `self` 操作只要求任意实体。`give` 语句自带目标查询，不要求当前上下文。上下文函数不能被调度。`storage ... = item_list(...)` 还会向合成 load 函数加入幂等的空列表初始化。

`give` 在目标为 `origin`（原版实体关系，掉落物对应 `Thrower`）或物品来源为 `self.item` 时改变形状：`origin` 生成 `execute on origin if entity @s[type=minecraft:player] run ...`；`self.item` 额外生成临时标签、成功标志与辅助函数，用 `item replace … from entity … contents` 把源实体槽位 0 的物品堆复制进目标玩家背包的第一个空槽。编译器为此输出 `data/<命名空间>/slot_source/__mcl/empty_slot.json`：一个 `filtered` 槽位来源，底层 `group` 组合 `hotbar.*`（0 到 8 号槽）与 `inventory.*`（9 到 35 号槽），正好覆盖快捷栏与主背包而不含盔甲、副手和合成槽；物品谓词只接受空堆（`"count": 0`）。复制成功后清空源槽，掉落物随之下一次 tick 自行消失；投掷者不存在或背包已满时不修改源实体。物品堆的组件不重新构造，原样保留。

算术值存储在同一个内部 scoreboard objective。用户变量使用 `#v_<name>` 假玩家，表达式临时值使用 `#t<number>`，函数参数和局部变量使用函数名与变量名的稳定哈希假玩家，避免与真实玩家冲突。调用前先从左到右计算全部实参，再复制到被调用函数的参数计分项。语义检查器单独维护词法块作用域，保证局部变量在生成阶段已经完成名称解析。objective 名由命名空间的稳定 FNV-1a 哈希生成，格式为 16 字符的 `mcl_<12 hex>`，满足 Minecraft 的 objective 长度限制。

结构化代码块通过辅助函数实现。例如 `if` 先计算两侧表达式，把比较结果冻结到临时计分项，再用两条 `execute if score ... matches` 分派 then 和 else 辅助函数。`execute` 块也调用辅助函数，从而自然继承 Minecraft 命令源上下文。

`while` 由条件辅助函数和循环体辅助函数互相调度实现，继续受 Minecraft 命令链长度规则约束。用户函数的普通调用会进入同步调用图分析；任何直接或间接调用环都会被拒绝。`schedule` 在未来游戏刻建立新调用，因此不属于同步调用图。

返回 score 的函数把常量编译为 `return <整数>`，把运行时值编译为 `return run scoreboard players get`，`return run` 原样转发命令并沿用它的结果，`return fail` 生成失败返回。调用表达式先绑定参数，再以 `execute store result score ... run function` 捕获函数结果。表达式中的调用同样进入同步调用图。结构块需要辅助函数，而 Minecraft 的 `return` 只退出当前 `.mcfunction`，因此语义检查明确禁止结构块中的 return，避免它被误解为退出外层源码函数。

函数标签是独立的顶层声明。`fn_tag` 的条目在语义阶段解析：裸标识符必须对应本命名空间的函数，`#名称` 必须对应本命名空间的标签，字符串条目按资源位置校验但不做存在性检查。编译器检测标签循环引用，在 `call #标签()` 与 `schedule #标签()` 处展开标签（含嵌套引用）并检查每个可达函数的执行上下文与参数；同步调用图同样包含经标签形成的边。代码生成把可达函数集合写成 `data/<命名空间>/tags/function/<名称>.json`，默认省略 `replace` 字段以保持与 26.3 `TagFile` 编解码器的默认值一致。

`effect`、`xp` 和 `clear` 都以具名查询作为目标，生成 `execute as <选择器> at @s run <命令>`。等级为 0 且不隐藏粒子时 `effect give` 省略可选参数；需要隐藏粒子时补上等级占位。`xp.query` 只能出现在表达式里，编译器要求它的查询带 `limit(1)`（原版只接受单个玩家），生成 `execute ... store result score ... run xp query @s <类型>`，并先把临时计分项置零，避免选择器没有匹配玩家时读到旧值。`schedule` 的延迟在解析期按原版 `TimeArgument` 的浮点规则换算为游戏刻，拒绝不足 1 刻或超出 32 位范围的延迟，`schedule.clear` 只接受函数。

`resource` 声明中的原始文本先由 `serde_json` 解析，语义阶段检查资源类型、资源路径和项目内重复项。代码生成阶段重新序列化为稳定缩进的 JSON，并按照注册表目录写入用户命名空间。

## 26.3-rc-2 兼容依据

仓库随附源码的 `version.json` 声明数据包版本为 major 121、minor 0。生成的 `pack.mcmeta` 使用 `[121, 0]` 作为 `min_format` 和 `max_format`。

该版本 `ServerFunctionLibrary` 通过注册表 `minecraft:function` 加载函数；`Registries.elementsDirPath` 返回 `function`，`tagsDirPath` 返回 `tags/function`。因此输出使用：

```text
pack.mcmeta
data/<namespace>/function/*.mcfunction
data/minecraft/tags/function/load.json
data/minecraft/tags/function/tick.json
```

load/tick 标签属于 `minecraft` 命名空间，而标签值引用用户命名空间中的函数。编译器生成 `__mcl/load` 包装函数，先创建 objective、仅为尚不存在的计分项写初始值，再按源码顺序调用所有 `@load` 函数。

## 输出所有权

`.mclang-manifest` 记录本次生成的相对文件。下次构建只清理清单中的普通相对路径，并拒绝绝对路径、父目录跳转或损坏的条目。这样能移除不再需要的辅助函数，同时避免覆盖范围扩展到数据包目录外。
