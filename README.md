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

## 特性

- **结构化标准层**：`query`、`item_stack`、`item_list`、`give`、`each`、`spawn`、`in_dimension`、`self.*`、`message.*`、`sound.self`、`predicate`、`schedule`、`score` 返回值等都是独立的 AST 节点，而不是命令字符串。
- **编译期检查**：命名、资源位置、标签、范围、枚举、执行上下文、返回值、调用图递归、JSON 资源都在写出数据包之前报错，并一次返回全部诊断。
- **精确的执行上下文**：区分“无 / 任意实体 / 非玩家实体 / 玩家”，`data` 类 NBT 操作只允许非玩家实体，玩家数据不会被错误修改。
- **中英文双关键词**：任意结构都有英文和中文写法，可以在同一文件里混用，两种写法生成逐字节相同的产物。
- **可复现产物**：输出使用 `BTreeMap` 与稳定哈希，同一份源码总是生成同样的文件与内容。
- **严格模式**：`--deny-raw` 递归拒绝 `run` 和字符串 `execute`，让项目完全停留在标准层。
- **安全重建**：`.mclang-manifest` 记录上次生成的文件，重建只清理自己的产物，不碰目录里的其他文件。

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
mclang help | version
```

- 输入可以是单个 `.mcl` 文件，也可以是递归包含 `.mcl` 的项目目录；项目内所有文件必须声明相同命名空间，声明与引用在整个项目内可见。
- `check` 只做检查并打印统计；`build` 通过全部检查后才写出文件。
- 默认输出目录是 `build/<源文件名>`。

## 与原生命令的对应关系

标准层是原生命令的类型化外壳，每条语句都下降为确定的命令形状。完整对照表见
[`docs/language-reference.md`](docs/language-reference.md#与原生命令的对应关系)，常见示例：

| Mclang | 生成的命令 |
| --- | --- |
| `each(q) { ... }` | `execute as <选择器> at @s run function <辅助函数>` |
| `spawn("minecraft:chest_minecart") { ... }` | `execute summon <类型> run function <辅助函数>` |
| `give(players, item[, n])` | `execute as <玩家选择器> at @s run give @s <物品> <数量>` |
| `give(origin, self.item)` | `execute on origin … item replace entity @s <空槽来源> from entity <源> contents` |
| `self.add_tag("x")` / `self.remove()` | `tag @s add x` / `kill @s` |
| `self.save_items(s)` / `self.restore_items(s)` | `data modify … entity @s Items`（非玩家实体上下文） |
| `message.self("x", gold)` | `tellraw @s <文本组件 JSON>` |
| `sound.self("…", master)` | `playsound <声音> <分类> @s ~ ~ ~ 1 1` |
| `schedule f() after 20 t` | `schedule function <命名空间>:f 20t replace` |
| `@load` / `@tick` | `minecraft:load` / `minecraft:tick` 函数标签 |

生成的数据包结构与计分板 ABI 约定见
[`docs/compiler-design.md`](docs/compiler-design.md)。

## 仓库结构

| 位置 | 内容 |
| --- | --- |
| `src/lexer.rs` | 词法分析（Unicode 感知，用于识别中文关键词） |
| `src/parser/` | 递归下降解析：声明、语句、条件、表达式、关键词表 |
| `src/ast.rs` | 语法树与源范围 |
| `src/compiler/validate/` | 整程序语义检查（名称、资源、上下文、递归、JSON） |
| `src/compiler/codegen/` | 函数、辅助函数、资源与函数标签的代码生成 |
| `src/lib.rs` / `src/main.rs` | 项目编排、数据包写入与命令行入口 |
| `docs/` | 语言参考、语言设计、编译器设计和在线手册 |
| `examples/` | 端到端示例项目 |
| `minecraft_client_26.3-rc-2/` | 随仓库固定的目标版本源码（用于核对注册表与命令签名） |

## 文档

- [`docs/language-reference.md`](docs/language-reference.md)：完整语法、语义与原生命令对应表。
- [`docs/language-design.md`](docs/language-design.md)：语言设计取舍与标准层边界。
- [`docs/compiler-design.md`](docs/compiler-design.md)：编译流水线、代码生成约定与 26.3 兼容依据。
- [`docs/quickstart.md`](docs/quickstart.md)：从零构建第一个数据包。
- [`docs/manual.html`](docs/manual.html)：可离线打开的单页手册（带中英文关键词切换）。
- [`DEVELOPMENT_PLAN.md`](DEVELOPMENT_PLAN.md)：已完成能力与后续路线。
- [`AGNETS.md`](AGNETS.md)：项目开发规范。

## 目标版本

目标版本固定在仓库内的 `minecraft_client_26.3-rc-2/` 源码，数据包格式为 `121.0`。
资源类型、物品组件、槽位名、声音分类和命令签名都以该源码和注册表为准，不依赖记忆。

## 开发

```powershell
cargo fmt
cargo clippy --all-targets
cargo test

cargo run -- check examples/portable_chest --deny-raw
cargo run -- build examples/portable_chest --deny-raw
```

依赖只有 `serde_json`，Rust edition 2024。
