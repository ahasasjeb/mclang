# Mclang 快速上手

Mclang 把一个 `.mcl` 源文件或包含多个源文件的项目编译为 Minecraft Java Edition 26.3-rc-2 数据包。编译器使用 Rust 实现，运行时不需要 Java。

## 构建编译器

```powershell
cargo build --release
```

可执行文件位于 `target/release/mclang.exe`。

## 编译示例

```powershell
cargo run -- build examples/counter.mcl
```

默认输出到 `build/counter`。也可以指定数据包目录和描述：

```powershell
cargo run -- build examples/counter.mcl -o "C:/path/to/world/datapacks/counter" --description "My counter pack"
```

在 Minecraft 中执行 `/reload`。编译器生成的 load 标签会创建计分板并调用 `@load` 函数，tick 标签会在每个游戏刻调用 `@tick` 函数。

只做静态检查、不写数据包：

```powershell
cargo run -- check examples/counter.mcl
```

项目变大后可以直接传入目录。Mclang 会递归读取其中所有 `.mcl` 文件：

```powershell
cargo run -- check examples/multi_counter
cargo run -- build examples/multi_counter
```

同一项目中的每个源文件都要声明相同的命名空间。函数、计分变量、查询、物品定义和存储可以在任意文件中声明和引用。

`portable_chest` 是不含底层命令字符串的实际项目，展示类型化实体查询、物品组件匹配、跨维度执行和容器持久化：

```powershell
cargo run -- check examples/portable_chest --deny-raw
cargo run -- build examples/portable_chest --deny-raw
```

中文关键词示例可以直接检查和构建：

```powershell
cargo run -- check examples/chinese_counter.mcl --deny-raw
cargo run -- build examples/chinese_counter.mcl --deny-raw
```

中文与英文关键词可以在同一项目、同一文件中混用；编译器会在解析时把它们规范化为同一套语义。

`give_reward` 展示类型化物品定义、名称、Lore、附魔、稀有度、无法破坏、结构化 `give` 语句和一次性给予：

```powershell
cargo run -- check examples/give_reward.mcl --deny-raw
cargo run -- build examples/give_reward.mcl --deny-raw
```

`--deny-raw` 会递归拒绝任何 `run` 或字符串形式的 `execute`，适合要求全部使用 Mclang 标准层的新项目。它也可以用于 `check`。

构建会维护输出目录中的 `.mclang-manifest`。再次构建时只删除上一次由 Mclang 生成的文件，目录中的其他文件保持原样。
