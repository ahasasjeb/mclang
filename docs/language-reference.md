# Mclang 语言参考

输入可以是单个源文件，也可以是递归包含 `.mcl` 文件的项目目录。项目内每个文件声明相同的命名空间，声明和引用在整个项目内可见。标识符只能包含小写 ASCII 字母、数字和下划线，不能以数字开头，最长 32 字节。`//` 开始单行注释。

## 中英文双关键词

所有语言结构都接受英文或中文关键词，两种写法可以在同一文件中混用。解析器会立即把中文别名规范化，因此类型检查和生成结果没有差异。函数名、变量名、查询名、存储名、命名空间和 Minecraft 资源位置仍使用稳定的 ASCII/Minecraft 标识符；它们不是关键词，不会被翻译。

核心声明和控制流：

| English | 中文 | English | 中文 |
| --- | --- | --- | --- |
| `namespace` | `命名空间` | `score` | `计分` |
| `query` | `查询` | `entity` | `实体` |
| `item` | `物品` | `item_stack` | `物品堆` |
| `storage` | `存储` | `item_list` | `物品列表` |
| `resource` | `资源` | `predicate` | `谓词` |
| `contents` | `内容` | `fn` | `函数` |
| `fn_tag` | `函数标签` | `fail` | `失败` |
| `effect` | `效果` | `xp` | `经验` |
| `clear` | `清除` | `stopwatch` | `秒表` |
| `let` | `令` | `return` | `返回` |
| `if` | `如果` | `else` | `否则` |
| `while` | `当` | `each` | `遍历` |
| `call` | `调用` | `schedule` | `调度` |
| `after` | `延后` | `append` | `追加` |
| `replace` | `替换` | `give` | `给予` |
| `in_dimension` | `在维度` | `spawn` | `召唤` |
| `origin` | `投掷者` | `self` | `自身` |
| `message` | `消息` | `sound` | `声音` |
| `run` | `原生命令` | `execute` | `原生执行` |
| `set_block` | `设置方块` | `fill` | `填充` |
| `fill_biome` | `填充生物群系` | `clone` | `复制` |
| `place` | `放置` | `forceload` | `强制加载` |
| `time` | `时间` | `weather` | `天气` |
| `gamerule` | `游戏规则` | `worldborder` | `世界边界` |
| `locate` | `定位` | `pos` | `坐标` |
| `column` | `列坐标` | `block_state` | `方块状态` |

函数属性为 `@load`/`@加载`、`@tick`/`@每刻`、`@entity`/`@实体`、`@non_player`/`@非玩家`
和 `@player`/`@玩家`。返回类型 `-> score` 也可以写成 `-> 计分`。

查询、物品和实体操作：

| English | 中文 | English | 中文 |
| --- | --- | --- | --- |
| `tag` | `标签` | `without_tag` | `排除标签` |
| `limit` | `上限` | `within` | `范围` |
| `sort` | `排序` | `item` | `物品` |
| `contents` | `内容` | `id` | `类型` |
| `count` | `数量` | `custom_name` | `自定义名称` |
| `item_name` | `物品名称` | `rarity` | `稀有度` |
| `item_model` | `物品模型` | `max_stack_size` | `最大堆叠` |
| `max_damage` | `最大损伤` | `dyed_color` | `染色` |
| `enchantment_glint_override` | `附魔光效` | `lore` | `描述` |
| `enchantment` | `附魔` | `stored_enchantment` | `存储附魔` |
| `damage` | `损伤` | `unbreakable` | `无法破坏` |
| `give_item` | `给予物品` |  |  |
| `add_tag` | `添加标签` | `remove_tag` | `移除标签` |
| `set_invulnerable` | `设置无敌` | `save_items` | `保存物品` |
| `restore_items` | `恢复物品` | `remove_preserving_items` | `保存并移除` |
| `clear_items` | `清空物品` | `remove` | `移除` |

目标和枚举值：

| 类型 | English | 中文 |
| --- | --- | --- |
| 消息目标 | `all` / `self` / `nearest` | `全部` / `自身` / `最近` |
| 查询排序 | `nearest` / `furthest` / `random` / `arbitrary` | `最近` / `最远` / `随机` / `任意` |
| 物品稀有度 | `common` / `uncommon` / `rare` / `epic` | `普通` / `罕见` / `稀有` / `史诗` |
| 布尔值 | `true` / `false` | `真` / `假` |
| 调度单位 | `t` / `s` / `d` | `刻` / `秒` / `天` |
| 经验类型 | `points` / `levels` | `点数` / `等级` |
| 声音分类 | `master` / `music` / `record` / `weather` / `block` | `主音量` / `音乐` / `唱片` / `天气` / `方块` |
| 声音分类 | `hostile` / `neutral` / `player` / `ambient` / `voice` / `ui` | `敌对` / `中立` / `玩家` / `环境` / `语音` / `界面` |

方法名也可以写中文：`效果.给予`、`效果.给予无限`、`效果.清除`、`经验.增加`、`经验.设置`、`经验.查询`、`秒表.创建`、`秒表.查询`、`秒表.重启`、`秒表.移除`，以及函数标签里的 `值` 和 `替换`。

世界命令的方法名同样有中文写法：`放置.地物`/`拼图`/`结构`/`模板`、`强制加载.添加`/`移除`/`全部移除`/`查询`、`时间.设置`/`增加`/`暂停`/`恢复`/`速率`/`查询`/`查询游戏时间`、`天气.晴朗`/`下雨`/`雷暴`、`游戏规则.设置`/`查询`、`世界边界.增加`/`设置`/`中心`/`伤害量`/`伤害缓冲`/`获取`/`警告距离`/`警告时间`、`定位.结构`/`生物群系`/`兴趣点`。

文本颜色支持 `black`/`黑色`、`dark_blue`/`深蓝色`、`dark_green`/`深绿色`、`dark_aqua`/`深青色`、`dark_red`/`深红色`、`dark_purple`/`深紫色`、`gold`/`金色`、`gray`/`灰色`、`dark_gray`/`深灰色`、`blue`/`蓝色`、`green`/`绿色`、`aqua`/`青色`、`red`/`红色`、`light_purple`/`亮紫色`、`yellow`/`黄色` 和 `white`/`白色`。

完整中文示例：

```mcl
命名空间 demo;
计分 ticks = 0;
查询 players = 实体("minecraft:player") {}
物品 reward = 物品堆("minecraft:emerald") {
    数量 = 1;
    自定义名称 = "计时奖励";
}

@每刻
函数 tick() {
    ticks += 1;
    如果 ticks >= 100 {
        给予(players, reward);
        遍历(players) {
            声音.自身("minecraft:block.note_block.pling", 主音量);
            消息.自身("已经过五秒。", 金色);
        }
        ticks = 0;
    }
}
```

## 程序结构

每个文件先声明一个命名空间，然后声明全局计分变量和函数：

```mcl
namespace demo;

score counter = 0;

@load fn load() {
    message.all("loaded", green);
}
```

`score` 是有符号 32 位整数，所有函数共享。初始值只在计分项不存在时写入，所以 `/reload` 不会清空运行状态。编译器为每个命名空间生成一个不超过 Minecraft 长度限制的内部 objective。

## 函数、参数和入口属性

```mcl
@load fn initialize() { }
@tick fn update() { }
fn add(amount, multiplier) {
    total += amount * multiplier;
}

fn caller() {
    add(2, total + 1);
}
```

- `@load`：数据包加载或 `/reload` 后调用。可以标记多个函数，按源码顺序调用。
- `@tick`：每个游戏刻调用。可以标记多个函数。
- 普通函数通过 `add(2, 3);` 或 `call add(2, 3);` 调用。实参是从左到右计算的计分表达式。

参数是函数私有的 32 位计分值，可以参与表达式和赋值。同步递归被禁止，因此嵌套调用不会覆盖仍在使用的参数。入口函数不能带参数；需要参数的函数也不能直接 `schedule`，可以调度一个无参数包装函数。参数化函数应由 Mclang 代码调用，直接从 Minecraft 执行它会沿用上次调用留下的参数值。

函数默认没有值返回。`return;` 会提前结束这种函数。追加 `-> score` 后，函数必须以 `return` 结束，调用可以出现在任意计分表达式中：

```mcl
fn double(value) -> score {
    return value * 2;
}

fn use_value() {
    let result = double(21) + 1;
    total = result;
}
```

返回值使用 Minecraft 原生 `return` 和 `execute store result score` 传递。返回 score 的函数还可以用 `return run "命令"` 把一条命令的结果作为返回值，或用 `return fail` 报告失败；无返回值函数省略分数即可：

```mcl
fn gametime() -> score {
    return run "time query gametime";
}

fn abort() {
    return fail;
}
```

`return run` 的命令字符串与 `run` 使用同一套检查（不能为空、不能以 `/` 开头、只能一行）。它是唯一需要原始命令文本的结构化形式，因此 `--deny-raw` 会把 `return run` 计为一条底层语句。

当前版本只允许 `return` 直接位于函数最外层代码块，不能放入 `if`、`while`、`each`、`spawn`、`in_dimension` 或 `execute` 块；需要条件结果时先赋给局部变量，在函数末尾返回。

## 局部变量

`let` 声明函数内的计分变量，初始值可以使用全局变量、参数和此前声明的局部变量：

```mcl
fn update(amount) {
    let adjusted = amount * 2;
    total += adjusted;

    if adjusted > 10 {
        let excess = adjusted - 10;
        total += excess;
    }
}
```

局部变量采用词法块作用域，不能在声明前使用，也不能在声明它的代码块外使用。为保持生成名称与诊断清晰，同一函数内不允许重复局部名，也不允许局部变量或参数遮蔽全局计分变量。

## 实体查询

`query` 用结构化属性声明实体集合。下面的查询只匹配单个、名称精确为 `A` 的绿宝石掉落物：

```mcl
query triggers = entity("minecraft:item") {
    without_tag("handled");
    item(contents) {
        id = "minecraft:emerald";
        count = 1;
        custom_name = "A";
    }
}
```

查询还支持 `tag("name")`、`limit(1)`、`within(16)` 和 `sort(nearest)`；排序值可以是 `nearest`、`furthest`、`random` 或 `arbitrary`。实体类型、物品类型、标签、范围、数量和 `contents` 槽都会在编译期检查。

`each` 为每个匹配实体建立 `self` 和实体所在位置。实体类型为 `minecraft:player` 的查询会建立玩家上下文，其他查询建立非玩家实体上下文：

```mcl
each(triggers) {
    self.add_tag("handled");
    self.remove();
}
```

## 实体和维度上下文

`spawn` 生成实体，块内的 `self` 指向新实体。`in_dimension` 切换维度，避免手写 `execute`：

```mcl
spawn("minecraft:chest_minecart") {
    self.add_tag("portable_box");
    self.set_invulnerable(true);
}

in_dimension("minecraft:the_nether") {
    each(boxes) {
        self.remove();
    }
}
```

可用实体方法按所需上下文分成三层：

- `add_tag`、`remove_tag` 和 `remove` 只要求任意实体上下文；
- `set_invulnerable`、`save_items`、`restore_items`、`remove_preserving_items` 和
  `clear_items` 通过 `data` 命令修改实体 NBT，而 Minecraft 拒绝修改玩家数据，
  因此要求非玩家实体上下文；
- `give_item` 要求玩家上下文。

编译器跟踪四种执行上下文，并在编译期检查每条语句需要哪一种：

| 上下文 | 来源 | `self` 指向 |
| --- | --- | --- |
| 无 | `@load`、`@tick`、普通函数 | 没有实体 |
| 任意实体 | `@entity` 函数 | 实体，但可能是玩家 |
| 非玩家实体 | 非玩家查询的 `each`、`spawn`、`@non_player` 函数 | 确定不是玩家的实体 |
| 玩家 | 玩家查询的 `each`、`@player` 函数 | 确定是玩家 |

`spawn` 只生成非玩家实体：`minecraft:player` 和 `minecraft:fishing_bobber` 在 26.3 中标记为 `noSummon`，编译器会拒绝 `spawn` 它们。

```mcl
@entity
fn mark_current_entity() {
    self.add_tag("marked");          // 任意实体都可以
}

@non_player
fn prepare_box() {
    self.set_invulnerable(true);     // 只对非玩家实体安全
}

@player
fn reward_current_player() {
    self.give_item(welcome_gift);
}
```

`@entity` 函数可以被任何实体上下文调用，其中不能使用只对非玩家实体安全的 NBT 方法。
需要这类方法的可复用逻辑应声明为 `@non_player`，并只从非玩家查询的 `each`、`spawn`
或另一个 `@non_player` 函数调用。玩家上下文满足 `@entity` 的调用要求，但反过来不成立。
`@entity`、`@non_player` 和 `@player` 函数都不能被 `schedule`，因为原版调度不会保留执行实体。

## 类型化物品与给予

物品先声明为可复用的类型化值，再给予玩家。`give` 语句直接对应原版 `give <目标> <物品> [<数量>]` 的形状：

```mcl
item welcome_gift = item_stack("minecraft:emerald") {
    count = 3;
    custom_name = "Welcome Gift";
    lore("First lore line");
    lore("Second lore line");
    enchantment("minecraft:fortune", 2);
    unbreakable = true;
}

query players = entity("minecraft:player") {}

fn grant() {
    give(players, welcome_gift);       // 使用物品定义的 count
    give(players, welcome_gift, 64);   // 本次给予 64 个
}

@player
fn grant_self() {
    self.give_item(welcome_gift);      // 当前玩家，使用物品定义的 count
    self.give_item(welcome_gift, 5);   // 当前玩家，本次给予 5 个
}

fn return_drop() {
    each(drops) {
        give(origin, self.item);       // 把当前实体的物品原样交给投掷者
    }
}
```

`give` 的目标写成查询时必须是 `minecraft:player` 类型，编译器为每个匹配玩家生成 `execute as <选择器> at @s run give @s ...`。`self.give_item` 是当前玩家上下文的简写，只能用于玩家查询建立的 `each` 块或 `@player` 函数。

目标还可以写成 `origin`（中文 `投掷者`）：它对应原版实体关系 `origin`，对掉落物来说是 `Thrower`，因此可以把物品交还给投掷者。投掷者运行时必须解析到玩家，否则不执行。物品来源除了已声明的物品定义，还可以写成 `self.item`（中文 `自身.物品`）：它读取当前实体槽位 0 的物品堆 NBT（掉落物就是它的 `Item`），把它原样放进目标玩家快捷栏或主背包的第一个空槽（不含盔甲、副手和合成槽），不合并、不改变组件。原样给予不能附带数量；复制成功后源槽被清空，掉落物自行消失，背包已满时源实体保持原样。对普通玩家查询使用 `self.item` 时，只有第一个有空位的匹配玩家会收到物品。

物品来源是物品定义时走普通 `/give` 路径：会与背包中已有的同名物品堆叠，装不下时掉在玩家脚边，数量和上限规则与查询目标相同。因此额外奖励只需再写一条 `give(origin, <物品定义>);`，与 `self.item` 的原样交还可以并用。

数量省略时使用物品定义的 `count`，显式数量总是覆盖它。与 26.3 `GiveCommand` 一致，数量上限是物品最大堆叠数乘以 100：未声明 `max_stack_size` 时物品的原型堆叠数至少为 1，因此保守上限为 100；声明 `max_stack_size = n` 后上限为 `n × 100`。

物品定义的可选组件：

| 写法 | 组件 | 说明 |
| --- | --- | --- |
| `custom_name = "..."` | `minecraft:custom_name` | 自定义名称，物品栏中显示为斜体 |
| `item_name = "..."` | `minecraft:item_name` | 基础物品名称，不带斜体 |
| `lore("...")` | `minecraft:lore` | 描述行，最多 256 行 |
| `enchantment(id, level)` | `minecraft:enchantments` | 普通附魔，等级 1 到 255 |
| `stored_enchantment(id, level)` | `minecraft:stored_enchantments` | 附魔书的存储附魔 |
| `damage = n` | `minecraft:damage` | 当前损伤值，非负 |
| `max_damage = n` | `minecraft:max_damage` | 最大损伤值，正整数 |
| `max_stack_size = n` | `minecraft:max_stack_size` | 最大堆叠数，1 到 99 |
| `rarity = 稀有度` | `minecraft:rarity` | `common`、`uncommon`、`rare` 或 `epic` |
| `item_model = "..."` | `minecraft:item_model` | 物品模型资源位置 |
| `dyed_color = n` | `minecraft:dyed_color` | 0 到 16777215 的 RGB 颜色 |
| `enchantment_glint_override = 真/假` | `minecraft:enchantment_glint_override` | 强制显示或隐藏附魔光效 |
| `unbreakable = 真` | `minecraft:unbreakable` | 无法破坏 |

`damage` 是非负的物品损伤值。大于 1 的 `max_stack_size` 不能和 `max_damage` 同时声明，因为 26.3 的 `ItemStack.validateStrict` 拒绝既可堆叠又可损伤的物品。

编译器检查定义名称、物品与附魔资源位置、数量、等级、文本和引用，并生成 `give` 所需的物品组件语法。所有属性都可以写成中文形式，例如 `物品名称`、`稀有度`、`最大堆叠`、`染色` 和 `附魔光效`。

## 类型化物品存储

```mcl
storage saved = item_list("demo:state", "saved_items");
```

声明创建一个保存容器 `Items` 列表的持久位置。编译器在数据包 load 入口中仅于路径不存在时初始化空列表，因此 `/reload` 不会覆盖内容。在非玩家实体上下文（非玩家查询的 `each`、`spawn` 或 `@non_player` 函数）中使用 `self.save_items(saved)` 和 `self.restore_items(saved)` 保存或恢复完整槽位数据。`self.remove_preserving_items(saved)` 是安全收起容器的原子语言操作，后端固定按保存、清空、删除的顺序生成命令。

## 消息

```mcl
message.all("Data pack loaded", green);
message.self("Only the current player sees this", yellow);
message.nearest(16, "Container stored", gold);
```

编译器生成玩家选择器和文本组件 JSON，并检查距离和原版颜色名称。`message.self` 需要玩家上下文。

## 声音

```mcl
sound.self("minecraft:block.note_block.pling", master);
```

`sound.self` 在当前玩家位置向该玩家播放声音，需要玩家上下文。编译器检查声音资源位置和 26.3 的声音分类。当前分类包括 `master`、`music`、`record`、`weather`、`block`、`hostile`、`neutral`、`player`、`ambient`、`voice` 和 `ui`。

## 与原生命令的对应关系

标准层是原生命令的类型化外壳：每条结构化语句都会在编译期检查，再下降为确定的命令形状。下表给出完整对应关系，便于和 `run` 中的手写命令互相换算。

| Mclang | 生成的命令 | 说明 |
| --- | --- | --- |
| `score x = 1;`、赋值 | `scoreboard players set/add/remove/operation` | 假玩家 `#v_x`，32 位整数 |
| 算术表达式 | `scoreboard players operation` 与 `#t<n>` 临时项 | 常量在编译期折叠 |
| `if`、`while`、`&&`、`\|\|`、`!` | `execute if score` 与辅助函数 | 条件先求值为 0/1 |
| `call f(...)`、`f(...)` | `function <ns>:f` | 实参经假玩家传递 |
| `call #标签()` | `function #<ns>:标签` | 标签成员在编译期检查上下文与参数 |
| `return e;` | `return <值>`、`return run scoreboard players get` | 计分返回约定 |
| `return fail;` | `return fail` | 失败返回，store result 得到 0 |
| `return run "命令";` | `return run <命令>` | 命令结果作为返回值；计入底层语句 |
| `@load`、`@tick` | `minecraft:load`、`minecraft:tick` 函数标签 | 入口函数 |
| `fn_tag 名称 { ... }` | `data/<ns>/tags/function/<名称>.json` | 函数标签声明 |
| 普通函数 | `data/<ns>/function/<名称>.mcfunction` | 每个函数一个文件 |
| `each(q) {}` | `execute as <选择器> at @s run function <辅助函数>` | 查询下降为选择器 |
| `spawn(t) {}` | `execute summon <t> run function <辅助函数>` | 暂不支持初始 NBT |
| `in_dimension(d) {}` | `execute in <d> run function <辅助函数>` | |
| `execute "子句" {}` | `execute <子句> run function <辅助函数>` | 底层接口 |
| `give(q, 物品[, n])` | `execute as <选择器> at @s run give @s <物品> <数量>` | 目标必须是玩家查询 |
| `give(origin, 物品)` | `execute on origin if entity @s[type=minecraft:player] run give ...` | 原版实体关系 |
| `give(q, self.item)` | `item replace entity @s <空槽来源> from entity <源实体> contents` | 原样复制后清空源槽 |
| `self.give_item(...)` | `give @s <物品> <数量>` | 玩家上下文 |
| `self.add_tag(x)`、`self.remove_tag(x)` | `tag @s add/remove x` | 任意实体上下文 |
| `self.remove()` | `kill @s` | 任意实体上下文 |
| `self.set_invulnerable(b)` | `data merge entity @s {Invulnerable:1b/0b}` | 非玩家实体上下文 |
| `self.save_items(s)` | `data modify storage ... set from entity @s Items` | 非玩家实体上下文 |
| `self.restore_items(s)` | `data modify entity @s Items set from storage ...` | 非玩家实体上下文 |
| `self.clear_items()` | `data modify entity @s Items set value []` | 非玩家实体上下文 |
| `self.remove_preserving_items(s)` | 保存、清空、`kill @s` | 非玩家实体上下文 |
| `message.all/self/nearest` | `tellraw <玩家选择器> <文本组件 JSON>` | 文本组件由编译器生成 |
| `sound.self(声音, 分类)` | `playsound <声音> <分类> @s ~ ~ ~ 1 1` | 玩家上下文 |
| `effect.give(q, 效果, 秒[, 等级][, 隐藏粒子])` | `execute as <选择器> at @s run effect give @s ...` | 目标为实体查询 |
| `effect.give_infinite(q, 效果[, 等级][, 隐藏粒子])` | `... effect give @s <效果> infinite ...` | 无限时长 |
| `effect.clear(q[, 效果])` | `... effect clear @s [<效果>]` | |
| `xp.add/set(q, points\|levels, 数量)` | `... xp add/set @s <数量> <类型>` | 目标必须是玩家查询 |
| `xp.query(q, points\|levels)` | `... store result score #tN ... run xp query @s <类型>` | 表达式；查询需要 `limit(1)` |
| `clear(q[, 物品][, 数量])` | `... clear @s [<物品>] [<数量>]` | 目标必须是玩家查询 |
| `stopwatch.create/restart/remove("id")` | `stopwatch create/restart/remove <id>` | id 是完整资源位置 |
| `stopwatch.query("id"[, 缩放])` | `execute store result score ... run stopwatch query <id> [<缩放>]` | 表达式；失败写入 0 |
| `set_block(pos, 方块[, 模式])` | `setblock <坐标> <方块>[ destroy\|keep\|strict]` | `replace` 为默认，省略 |
| `fill(起点, 终点, 方块[, 模式][, replace 过滤器])` | `fill <起点> <终点> <方块>[ 模式][ replace <过滤器>]` | 模式 replace/outline/hollow/destroy/strict/keep |
| `fill_biome(起点, 终点, "生物群系"[, replace, "过滤器"])` | `fillbiome <起点> <终点> <生物群系>[ replace <过滤>]` | 过滤器可带 `#` 标签 |
| `clone(起点, 终点, 目标[, 选项...])` | `clone [from <维度>] <起点> <终点> <目标> [to <维度>][ masked\|filtered <谓词>][ force\|move][ strict]` | 选项顺序无关 |
| `place.feature/structure/jigsaw/template` | `place feature/structure/jigsaw/template ...` | 旋转、镜像、完整度、种子、strict |
| `forceload.add/remove/query(column)[, column]` | `forceload add/remove/query <列>[ <列>]` | 一次最多 256 个区块 |
| `forceload.remove_all()` | `forceload remove all` | |
| `time.set/add/pause/resume/rate(..., [时钟])` | `time [of <时钟>] set/add/pause/resume/rate ...` | 编译期换算时间单位 |
| `time.query([时钟])`、`time.query_gametime()` | `execute store result score ... run time [of <时钟>] query time/gametime` | 表达式 |
| `weather.clear/rain/thunder([时间])` | `weather clear/rain/thunder [<时间>]` | |
| `gamerule.set("规则", 值)` | `gamerule <规则> <值>` | 规则表来自 26.3 `GameRules` |
| `gamerule.query("规则")` | `execute store result score ... run gamerule <规则>` | 表达式 |
| `worldborder.add/set(...)`、`center`、`damage_amount`、`damage_buffer`、`warning_distance`、`warning_time` | `worldborder add/set/center/damage amount/damage buffer/warning distance/warning time ...` | 距离与中心检查范围 |
| `worldborder.get()` | `execute store result score ... run worldborder get` | 表达式 |
| `locate.structure/biome/poi("目标")` | `locate structure/biome/poi <目标>` | 只产生反馈，可带 `#` 标签 |
| `predicate(p)` | `execute if predicate <ns>:p` | 可与 `!`、`&&`、`\|\|` 组合 |
| `schedule f() after n t [append]` | `schedule function <ns>:f <n>t [append]` | 单位 `t`、`s`、`d`，支持小数 |
| `schedule #标签() after n t` | `schedule function #<ns>:标签 <n>t` | |
| `schedule.clear(f)` | `schedule clear <ns>:f` | 只接受函数 |
| `run "命令"` | 命令原样写入 `.mcfunction` | 底层接口 |
| `query ... = entity(...)` | 选择器 `@e[...]`、`if items entity @s <槽> <物品谓词>` | 编译期检查全部参数 |
| `item ... = item_stack(...)` | 物品组件 SNBT | 编译期检查全部组件 |
| `storage ... = item_list(...)` | `data` 路径；load 时 `execute unless data ... run data modify ... set value []` | 幂等初始化 |
| `resource ... = """JSON"""` | `data/<ns>/<类型>/<名称>.json` | 编译期解析并统一格式化 |

## 底层兼容接口

```mcl
run "particle minecraft:happy_villager ~ ~1 ~ 0.2 0.2 0.2 0 3";
```

`run` 只用于标准层尚未覆盖的 Minecraft 能力，定位类似内联汇编。字符串内容原样成为一行 `.mcfunction` 命令，无法得到实体、资源位置、NBT 或参数的静态检查。常规项目应优先使用类型化语句。命令不要写开头的 `/`；字符串支持 `\"`、`\\`、`\n`、`\r` 和 `\t` 转义，实际换行会被拒绝。

新项目可以启用严格检查，递归拒绝 `run`、字符串形式的 `execute` 和 `return run`：

```powershell
mclang check path/to/project --deny-raw
mclang build path/to/project --deny-raw
```

## JSON 数据包资源

`resource` 把经过检查的 JSON 写入当前命名空间。跨行原始字符串以三个双引号开始和结束，内容不处理转义：

```mcl
resource predicate coin_flip = """
{
  "condition": "minecraft:random_chance",
  "chance": 0.5
}
""";
```

输出路径为 `data/<namespace>/predicate/coin_flip.json`。资源名称需要子目录时使用字符串，例如 `resource advancement "story/custom" = """...""";`；worldgen 类型也使用字符串，例如 `resource "worldgen/biome" custom = """...""";`。

支持的资源类型依据随附 26.3-rc-2 数据和注册表确定，包括 advancement、predicate、loot_table、item_modifier、recipe、dialog、enchantment、各类动态注册表以及 `worldgen/*`。编译器检查类型、路径、重复声明和 JSON 语法，并统一格式化输出。JSON 字段的 Minecraft codec 语义仍由游戏加载时验证。

当前命名空间中声明的 predicate 可以直接进入布尔条件，并与 `!`、`&&`、`||` 组合：

```mcl
if predicate(coin_flip) {
    message.all("Heads");
} else {
    message.all("Tails");
}
```

## 赋值和表达式

```mcl
counter = 10;
counter += 1;
counter -= 2;
counter *= scale;
counter /= 4;
counter %= 20;
counter = (counter + 3) * 2;
```

支持 `+`、`-`、`*`、`/`、`%`、一元负号和括号，乘除和取模的优先级高于加减。表达式由 Minecraft scoreboard 命令计算，结果保持 32 位计分板语义。编译器会拒绝能够静态确定的除零操作。

## 条件分支

```mcl
if counter >= 20 {
    message.all("ready", green);
} else {
    message.all("waiting", yellow);
}
```

比较运算符为 `==`、`!=`、`<`、`<=`、`>` 和 `>=`，两侧都可以是算术表达式。条件在进入分支前求值一次，因此 then 分支修改参与比较的变量也不会错误触发 else 分支。

条件可以用 `!`、`&&` 和 `||` 组合，也可以用括号分组。优先级依次为 `!`、`&&`、`||`：

```mcl
if enabled == 1 && ticks < 100 || override == 1 {
    message.all("active", green);
}

if enabled == 1 && !(ticks < 100 || override == 1) {
    message.all("paused", yellow);
}
```

条件表达式只读取计分值，目前不包含会改变状态的表达式。编译器按从左到右的确定顺序计算组合条件。

## 循环

```mcl
while pending > 0 {
    pending -= 1;
    message.all("processed");
}
```

`while` 每次迭代都会重新计算条件。循环由生成的函数链实现；无法结束的循环最终会触及世界的 `maxCommandChainLength`，因此循环体必须推进退出条件。同步函数递归会在编译期被拒绝，以免重入覆盖表达式临时值。需要跨刻重复执行时使用 `schedule`。

## 字符串执行上下文（兼容接口）

```mcl
execute "as @a at @s" {
    run "particle minecraft:happy_villager ~ ~1 ~ 0.2 0.2 0.2 0 3";
}
```

字符串只写 `execute` 和 `run` 之间的子句。代码块被编译为私有辅助函数，并继承原版 `execute` 建立的执行者、位置、维度和朝向上下文。代码块可以嵌套 `if`、`execute` 和其他语句。

## 调度

```mcl
schedule cleanup() after 20 t;
schedule cleanup() after 5 s append;
schedule cleanup() after 1.5 s;
schedule cleanup() after 1 d replace;
schedule.clear(cleanup);
```

单位支持游戏刻 `t`、秒 `s` 和游戏日 `d`，数量可以是整数或小数；编译器按原版 `TimeArgument` 的规则四舍五入到游戏刻，并拒绝不足 1 刻或超出 32 位范围的延迟。默认模式为 `replace`；`append` 允许保留同函数已有的调度。`schedule.clear` 取消尚未执行的一次调度，它只接受函数名：原版无法取消标签调度。

## 效果与经验

```mcl
query mobs = entity("minecraft:zombie") { within(32); }
query player = entity("minecraft:player") { limit(1); }

fn buff() {
    effect.give(mobs, "minecraft:speed", 30);          // 30 秒
    effect.give(mobs, "minecraft:speed", 30, 1);       // 等级 1
    effect.give(mobs, "minecraft:speed", 30, 0, true); // 隐藏粒子
    effect.give_infinite(mobs, "minecraft:night_vision");
    effect.clear(mobs);
    effect.clear(mobs, "minecraft:speed");
}

fn reward() {
    xp.add(player, points, 10);      // 增加 10 点经验
    xp.add(player, levels, 1);       // 增加 1 级
    xp.set(player, points, 0);       // 把经验点设为 0
    xp.set(player, levels, 5);       // 把等级设为 5
    let level = xp.query(player, levels);
}
```

`effect.give` 的持续秒数范围是 1 到 1000000，等级范围是 0 到 255；`effect.give_infinite` 生成无限时长。`xp.query` 只能作为表达式使用（原版查询命令没有副作用），它要求目标查询是 `limit(1)` 的玩家查询，因为原版只接受单个玩家。`xp.set` 的数量不能为负；`xp.add` 可以为负，表示扣除。

## 清除物品

```mcl
clear(player);                          // 清空全部物品
clear(player, "minecraft:diamond");     // 只清除钻石
clear(player, "minecraft:diamond", 5);  // 最多清除 5 个
```

目标必须是玩家查询，物品是资源位置，数量最大为 2147483647。

## 秒表

```mcl
fn main() {
    stopwatch.create("demo:timer");            // 创建，重复创建会失败
    stopwatch.restart("demo:timer");           // 重新计时
    let seconds = stopwatch.query("demo:timer");
    let millis = stopwatch.query("demo:timer", 1000);
    stopwatch.remove("demo:timer");
}
```

秒表 id 是完整的资源位置，保存在世界的 `stopwatches` 存档数据里：`/reload` 不会清空它，因此 `stopwatch.create` 只在第一次加载时成功。`stopwatch.query` 只能作为表达式使用，返回 `(int)(已过秒数 × 缩放)`；缩放是可选的双精度参数，省略时为 1，`1000` 得到毫秒。查询不存在的秒表时命令失败，表达式得到 0（原版 `store result` 在失败时写入 0）。`restart` 和 `remove` 对不存在的秒表会在运行时记录错误，但不会中断后续命令。

## 世界与方块

`pos(x, y, z)` 描述方块坐标：绝对分量是整数，`~` 是相对偏移，`^` 是局部偏移，`^` 不能与 `~` 或绝对坐标混用；`column(x, z)` 是 `forceload` 使用的两段列坐标（不支持 `^`）。绝对坐标在编译期检查世界范围（水平 -30000000 到 29999999，垂直 -2032 到 2031）。

`block_state("minecraft:oak_stairs") { facing = "east"; }` 描述方块状态；属性值写成字符串，输出时去掉引号。`#` 前缀的 `block_state("#minecraft:planks")` 是方块标签谓词，只能出现在 `fill` 的替换过滤器与 `clone filtered` 里。

```mcl
fn build() {
    set_block(pos(0, 64, 0), block_state("minecraft:stone"));
    set_block(pos(~, ~1, ~), block_state("minecraft:oak_stairs") { facing = "east"; }, keep);
    fill(pos(0, 64, 1), pos(4, 64, 5), block_state("minecraft:glass"), outline);
    fill(pos(0, 64, 1), pos(4, 64, 5), block_state("minecraft:oak_planks"), replace, block_state("#minecraft:planks"));
    fill_biome(pos(0, 0, 0), pos(15, 0, 15), "minecraft:plains");
    fill_biome(pos(0, 0, 0), pos(15, 0, 15), "minecraft:plains", replace, "#minecraft:is_forest");
}
```

`set_block` 的模式是 `replace`（默认）、`destroy`、`keep` 或 `strict`；`fill` 的模式还接受 `outline`、`hollow`。`clone` 的选项按任意顺序写在目标位置之后：`replace`/`masked`/`filtered(方块谓词)`、`normal`/`force`/`move`、`strict`、`from_dimension("维度")`、`to_dimension("维度")`，每个选项最多一次：

```mcl
clone(pos(0, 64, 0), pos(8, 64, 8), pos(20, 64, 0), masked, force);
clone(pos(0, 64, 0), pos(8, 64, 8), pos(40, 64, 0), filtered, block_state("#minecraft:logs"), move, strict);
clone(pos(0, 64, 0), pos(8, 64, 8), pos(60, 64, 0), from_dimension("minecraft:the_nether"), to_dimension("minecraft:overworld"));
```

放置命令对应原版子命令：`place.feature("地物"[, pos])`、`place.structure("结构"[, pos])`、`place.jigsaw("模板池", "目标", 最大深度[, pos])`、`place.template("模板", pos[, 旋转][, 镜像][, 完整度][, 种子][, strict])`。旋转是 `none`、`clockwise_90`、`180`、`counterclockwise_90`，镜像是 `none`、`left_right`、`front_back`，完整度必须在 0.0 到 1.0 之间。

```mcl
place.feature("minecraft:oak_tree", pos(0, 64, 20));
place.structure("minecraft:village_plains");
place.jigsaw("minecraft:village/plains/houses", "minecraft:bottom", 4, pos(64, 64, 20));
place.template("minecraft:empty", pos(96, 64, 20), clockwise_90, none, 1.0, 7, strict);

forceload.add(column(0, 0), column(31, 31));   // 两端都写时是矩形范围
forceload.query(column(0, 0));
forceload.remove(column(0, 0));
forceload.remove_all();
```

时间参数写成 `数值, 单位`，单位沿用 `t`、`s`、`d`；编译器按原版 `TimeArgument` 换算成游戏刻并检查范围，输出保留原始写法。`time` 的时钟是可选参数：

```mcl
time.set(1000, t);
time.add(1, d, "minecraft:the_end");   // 可选的世界时钟
time.pause();
time.resume();
time.rate(1.5);
weather.rain(30, s);
weather.clear();

let ticks = time.query();              // 默认时钟的总游戏刻
let end = time.query("minecraft:the_end");
let game = time.query_gametime();
```

`gamerule` 的规则名与取值类型来自 26.3 的 `GameRules` 注册表，写 `keep_inventory` 或 `minecraft:keep_inventory` 都可以；查询是表达式：

```mcl
gamerule.set("keep_inventory", true);
gamerule.set("random_tick_speed", 5);
if gamerule.query("keep_inventory") == 1 {
    message.all("keep inventory is on", green);
}
```

`worldborder` 的距离与中心在编译期检查原版范围；`get` 是表达式。`locate` 只产生命令反馈：

```mcl
worldborder.set(1000, 5 s);
worldborder.add(-100, 30 s);
worldborder.center(0, 0);
worldborder.damage_amount(0.5);
worldborder.damage_buffer(5);
worldborder.warning_distance(10);
worldborder.warning_time(20 s);
let size = worldborder.get();

locate.structure("minecraft:village_plains");
locate.biome("#minecraft:is_forest");
locate.poi("minecraft:home");
```

方块实体 NBT 尚未建模：`block_state` 只描述方块属性，`setblock`/`fill` 的 `{...}` NBT 后缀需要时暂用 `run`。`place.feature` 的原版内联地物 JSON 同样未建模。

## 函数标签

`fn_tag` 声明输出 `data/<命名空间>/tags/function/<名称>.json`，用 `call #标签()` 同步调用、用 `schedule #标签() after ...` 调度：

```mcl
fn cleanup() { effect.clear(mobs); }
fn report() { message.all("cleanup done", green); }

fn_tag tick_work {
    value(cleanup);
    value(report);
    value(#aliases);        // 嵌套标签
    value("minecraft:tick"); // 其他命名空间的资源
    replace = true;
}

fn main() {
    call #tick_work();
    schedule #tick_work() after 2 s append;
}
```

标签条目有三种写法：裸标识符是本命名空间的函数，`#名称` 是本命名空间的标签，字符串是外部资源位置（以 `#` 开头表示标签），外部条目只做语法检查。`replace` 对应标签文件的同名字段，默认 `false`（与低优先级数据包合并），写成 `true` 时替换。

编译器会检查条目引用的函数和标签存在、标签不能循环引用、`call #标签()` 不能带参数；被调用标签里的所有函数都必须满足当前执行上下文且没有参数，被调度的标签里的所有函数都不能要求执行上下文或参数。

## 诊断和静态检查

编译器会在写入前检查重复声明、未知变量、未知函数、无效属性、非法名称、整数范围和静态除零。错误包含文件、行列、源码行和标记范围。`run` 中原生命令的完整语法仍由 Minecraft 在加载数据包时验证。
