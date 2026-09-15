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

`potion_lab` 展示效果、经验、清空物品、秒表、函数标签和带小数的调度，同样完全不含底层命令字符串：

```powershell
cargo run -- check examples/potion_lab.mcl --deny-raw
cargo run -- build examples/potion_lab.mcl --deny-raw
```

`world_ops` 展示世界与方块命令：`set_block`、`fill`、`clone`、`fill_biome`、`place.*`、`forceload`、`time`、`weather`、`gamerule`、`worldborder` 与 `locate`，坐标和方块状态都是结构化参数：

```powershell
cargo run -- check examples/world_ops.mcl --deny-raw
cargo run -- build examples/world_ops.mcl --deny-raw
```

`--deny-raw` 会递归拒绝任何 `run` 或字符串形式的 `execute`，适合要求全部使用 Mclang 标准层的新项目。它也可以用于 `check`。

构建会维护输出目录中的 `.mclang-manifest`。再次构建时只删除上一次由 Mclang 生成的文件，目录中的其他文件保持原样。

## 编辑器支持（VSCode）

仓库自带 VSCode 插件（`editors/vscode`）与配套语言服务器。先构建编译器，再打包安装：

```powershell
cargo build --release
cd editors/vscode
npm install
npx vsce package
code --install-extension mclang-0.5.0.vsix
```

打开包含 `.mcl` 文件的工作区后，插件提供：

- 语法高亮：中英文关键词、函数属性、`#函数标签`、字符串与注释；
- 即时诊断：词法、语法和整项目语义错误直接标在编辑器里，保存与否都即时更新；
- 补全：已声明的函数、计分变量、查询、物品、存储、函数标签，以及全部中英文关键词；`@` 后补全函数属性，`#` 后补全函数标签；
- 悬停：关键词显示中英文对照与一句话说明，声明显示摘要和定义位置；
- 跳转：从引用跳到声明，支持跨文件。

语言服务器可以独立使用：`mclang lsp` 在标准输入输出上说 LSP。插件依次在工作区的 `target/release`、`target/debug` 和 `PATH` 中查找 `mclang` 可执行文件，也可以用设置 `mclang.server.path` 显式指定。

编辑器按打开的文档推断项目：同一命名空间、同一目录树下的文件属于一个项目，工作区里互不相关的项目（比如仓库里的多个示例）不会互相干扰；如果目录里出现命名空间不一致的文件，命令行 `mclang check` 仍会报错。
