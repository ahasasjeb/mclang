# Mclang

面向 **Minecraft Java Edition 26.3-rc-2** 的数据包编程语言及其 Rust 编译器。

Mclang 把 `.mcl` 源码编译成可以直接放进世界 `datapacks/` 目录的数据包。作者只描述意图，实体选择器、`execute` 上下文、NBT 路径、文本组件 JSON、计分板 ABI、函数标签和 26.3 的目录结构全部由编译器生成并检查；`run` 和字符串形式的 `execute` 只是最后手段的逃生口。

```mcl
namespace demo;

score ticks = 0;
query players = entity("minecraft:player") {}
item reward = item_stack("minecraft:emerald") {
    count = 1;
    custom_name = "计时奖励";
}

@tick
fn tick() {
    ticks += 1;
    if ticks >= 100 {
        give(players, reward);
        ticks = 0;
    }
}
```

中文关键词同样合法：`命名空间`、`计分`、`查询`、`物品`、`物品堆`、`每刻`、`函数`、`给予`……两种写法可以混用。

中文标识符也合法，例如 `计分 次数 = 0;` 或 `fn 检查基座(参数)`。编译时非 ASCII 名字会被替换成确定性的随机 ASCII 别名：同一份源码总是得到同一批产物，别名在项目内唯一并自动避开已有的 ASCII 标识符，因此不会与现有变量冲突；诊断信息仍使用源码里的名字。

## 特性

- **结构化标准层**：`query`、`item_stack`、`item_list`、`give`、`each`、`spawn`、`in_dimension`、`teleport`、`self.*`、`message.*`、`sound.self`、`effect.*`、`xp.*`、`clear`、`stopwatch.*`、`objective` 与 `scoreboard.*`、`data_slot` 与数据搬运、`advancement` 声明与 `advancement.grant/revoke`、`fn_tag`、`predicate`、`schedule`、`return fail`/`return run`、`score` 返回值等都是独立的 AST 节点，而不是命令字符串。
- **编译期检查**：命名、资源位置、标签、范围、枚举、执行上下文、返回值、调用图递归、JSON 资源都在写出数据包之前报错，并一次返回全部诊断。
- **精确的执行上下文**：区分“无 / 任意实体 / 非玩家实体 / 玩家”，`data` 类 NBT 操作只允许非玩家实体，玩家数据不会被错误修改。
- **中英文双关键词**：任意结构都有英文和中文写法，可以在同一文件里混用，两种写法生成逐字节相同的产物。
- **中文标识符**：声明名可以写中文等非 ASCII 字母；编译期自动换成互不冲突的随机 ASCII 别名（稳定可复现），产物里不会出现非 ASCII 标识符。
- **可复现产物**：输出使用 `BTreeMap` 与稳定哈希，同一份源码总是生成同样的文件与内容。
- **严格模式**：`--deny-raw` 递归拒绝 `run`、字符串 `execute` 和 `return run`，让项目完全停留在标准层。
- **安全重建**：`.mclang-manifest` 记录上次生成的文件，重建只清理自己的产物，不碰目录里的其他文件，并回收 `data/` 下不再包含文件的空目录。

## 快速开始

```powershell
# 构建编译器（可执行文件在 target/release/mclang.exe）
cargo build --release

# 静态检查，不写文件
cargo run -- check examples/portable_chest --deny-raw

# 编译为数据包，默认输出到 build/<项目名>
cargo run -- build examples/portable_chest --deny-raw
```

把输出目录复制到目标世界的 `datapacks/` 下，执行 `/reload` 即可。

一个最小的单文件示例：

```mcl
namespace counter;

score ticks = 0;
query all_players = entity("minecraft:player") {}

@load
fn load() {
    message.all("Counter loaded", green);
}

@tick
fn tick() {
    ticks += 1;
    if ticks >= 100 {
        each(all_players) {
            message.self("Five seconds passed.", gold);
            sound.self("minecraft:block.note_block.pling", master);
        }
        ticks = 0;
    }
}
```

## 命令行

```text
mclang build <源文件.mcl|项目目录> [-o <输出目录>] [--description <文本>] [--deny-raw]
mclang check <源文件.mcl|项目目录> [--deny-raw]
mclang lsp
mclang help | version
```

- 输入可以是单个 `.mcl` 文件，也可以是递归包含 `.mcl` 的项目目录；项目内所有文件必须声明相同命名空间，声明与引用在整个项目内可见。
- `check` 只做检查并打印统计；`build` 通过全部检查后才写出文件。
- 默认输出目录是 `build/<源文件名>`。
- `lsp` 在标准输入输出上启动语言服务器，供编辑器插件调用，不面向终端交互。

## 编辑器支持

仓库自带 VSCode 插件（[`editors/vscode`](editors/vscode)）和语言服务器：

```powershell
cargo build --release
cd editors/vscode
npm install
npx vsce package
code --install-extension mclang-0.5.0.vsix
```

插件提供中英文关键词的语法高亮、即时诊断（与命令行同一套检查，未保存的编辑也会检查）、声明与关键词补全、悬停说明和跨文件跳转。语言服务器是编译器的一部分（`mclang lsp`），插件会在工作区 `target/release`、`target/debug` 与 `PATH` 中自动查找可执行文件，也可以用 `mclang.server.path` 指定。

## 与原生命令的对应关系

标准层是原生命令的类型化外壳，每条语句都下降为确定的命令形状。逐条对照表见手册的
[语句参考](docs/index.html#statements)与[世界与方块](docs/index.html#statements-world)，常见示例：

| Mclang | 生成的命令 |
| --- | --- |
| `each(q) { ... }` | `execute as <选择器> at @s run function <辅助函数>` |
| `spawn("minecraft:chest_minecart") { ... }` | `execute summon <类型> run function <辅助函数>` |
| `give(players, item[, n])` | `execute as <玩家选择器> at @s run give @s <物品> <数量>` |
| `give(origin, self.item)` | `execute on origin … item replace entity @s <空槽来源> from entity <源> contents` |
| `self.add_tag("x")` / `self.remove()` | `tag @s add x` / `kill @s` |
| `self.save_items(s)` / `self.restore_items(s)` | `data modify … entity @s Items`（非玩家实体上下文） |
| `objective box_key;` | `__mcl/load` 中的 `scoreboard objectives add <命名空间>_box_key dummy` |
| `scoreboard.set(origin, box_key, 1)` | `execute on origin run scoreboard players set @s <目标> 1` |
| `scoreboard.get(self, box_key) == key` | `execute store result score … run scoreboard players get @s <目标>` 后比较 |
| `data_slot stash = item_data("pc_items");` | 物品堆 `Item.components."minecraft:custom_data".pc_items` |
| `self.deposit(stash, q)` / `self.withdraw(stash, q)` | `data modify entity <选择器> <槽路径> append/set from entity @s Items` |
| `self.remove_preserving_items(stash, q)` | 追加成功才清空 `Items`、容器为空才 `kill @s`（不会掉落物品） |
| `self.set_no_gravity(true)` / `self.set_invulnerable(true)` | `data merge entity @s {NoGravity:1b}` / `{Invulnerable:1b}` |
| `advancement x { criterion c { trigger = placed_block; } … }` | `data/<命名空间>/advancement/x.json`（准则、奖励函数与展示信息） |
| `advancement.revoke(self, x)` | `advancement revoke @s only <命名空间>:x`（让事件进度可重复触发） |
| `teleport(self, pos(0, 64, 0))` | `tp @s 0 64 0` |
| `teleport(self, 单个实体查询)` | `tp @s <单个实体选择器>`（跟随目标的位置、朝向与维度） |
| `message.self("x", gold)` | `tellraw @s <文本组件 JSON>` |
| `sound.self("…", master)` | `playsound <声音> <分类> @s ~ ~ ~ 1 1` |
| `effect.give(q, "…", 30[, 等级][, 隐藏粒子])` | `execute as <选择器> at @s run effect give @s …` |
| `xp.add/set(q, points\|levels, n)` | `execute as <选择器> at @s run xp add/set @s n <类型>` |
| `xp.query(q, levels)` | `store result score … run xp query @s levels`（查询需要 `limit(1)`） |
| `clear(q[, "…"][, n])` | `execute as <选择器> at @s run clear @s …` |
| `stopwatch.create/restart/remove("id")` | `stopwatch create/restart/remove <id>` |
| `stopwatch.query("id"[, 缩放])` | `execute store result score … run stopwatch query <id> …` |
| `call #标签()` / `schedule #标签() after 2 s` | `function #<命名空间>:标签` / `schedule function #…` |
| `fn_tag 名称 { value(函数); }` | `data/<命名空间>/tags/function/<名称>.json` |
| `return fail` / `return run "…"` | `return fail` / `return run …` |
| `schedule f() after 20 t` | `schedule function <命名空间>:f 20t replace` |
| `schedule.clear(f)` | `schedule clear <命名空间>:f` |
| `@load` / `@tick` | `minecraft:load` / `minecraft:tick` 函数标签 |

生成的数据包结构与计分板 ABI 约定见手册的[编译产物与运行模型](docs/index.html#artifacts)。

## 仓库结构

| 位置 | 内容 |
| --- | --- |
| `src/lexer.rs` | 词法分析（Unicode 感知，用于识别中文关键词） |
| `src/parser/` | 递归下降解析：声明、语句、条件、表达式、关键词表 |
| `src/ast.rs` | 语法树与源范围 |
| `src/analysis.rs` | 工具侧分析入口：结构化诊断与符号表 |
| `src/lsp/` | 语言服务器：JSON-RPC、位置换算、补全、悬停、跳转 |
| `src/compiler/validate/` | 整程序语义检查（名称、资源、上下文、递归、JSON） |
| `src/compiler/codegen/` | 函数、辅助函数、资源与函数标签的代码生成 |
| `src/lib.rs` / `src/main.rs` | 项目编排、数据包写入与命令行入口 |
| `docs/` | 语言手册：正文 `content/manual.md`、构建工具 `tools/`、静态页面 `index.html` 与 `assets/` |
| `editors/vscode/` | VSCode 插件：语法高亮、语言配置与语言客户端 |
| `examples/` | 端到端示例项目 |
| `minecraft_client_26.3-rc-2/` | 随仓库固定的目标版本源码（用于核对注册表与命令签名） |

## 文档

- [`docs/index.html`](docs/index.html)：可离线打开的单页语言手册，带中英文关键词一键切换、附录对照表与经验证的完整示例。
- 手册正文在 `docs/content/manual.md`，由 `docs/tools/build.mjs` 生成：关键词表取自 `src/parser/keywords.rs`，正文里的完整示例会用真实编译器分别以英文与中文关键词编译，并要求两种写法的产物逐字节一致。
  重新生成：`bun docs/tools/build.mjs --self-test`。
- [`DEVELOPMENT_PLAN.md`](DEVELOPMENT_PLAN.md)：已完成能力与后续路线。
- [`AGNETS.md`](AGNETS.md)：项目开发规范。

## 目标版本

目标版本固定在仓库内的 `minecraft_client_26.3-rc-2/` 源码，数据包格式为 `121.0`。
资源类型、物品组件、槽位名、声音分类和命令签名都以该源码和注册表为准，不依赖记忆。

## 开发

```powershell
cargo fmt
cargo clippy --all-targets

cargo run -- check examples/portable_chest --deny-raw
cargo run -- build examples/portable_chest --deny-raw

# 编译测试语料：valid 必须成功，invalid 必须报错（文件头注释写明期望诊断）
cargo run -- build tests/valid/language -o build/tests/language --deny-raw
cargo run -- check tests/invalid/items.mcl
```

编译器不含单元测试：验证方式是实际编写 `.mcl`、用真实编译器编译，并检查 `build/` 里生成的
mcfunction 与 JSON 产物；`tests/valid` 与 `tests/invalid` 是随仓库维护的编译测试语料。

依赖只有 `serde_json`，Rust edition 2024。
