# mclang 项目说明与开发规范

## 项目是什么

mclang 是一门面向 Minecraft Java Edition 26.3-rc-2 的数据包编程语言及其 Rust 编译器。编译器把
`.mcl` 源文件编译成可直接放进世界 `datapacks` 目录的数据包，并在编译期报告词法、语法和语义错误。

- 语言提供命名空间、全局计分变量、函数与参数、score 返回值、`let` 词法局部变量、`if`/`while`、
  实体查询、物品定义、`give`、`self` 操作、`message`/`sound`、`schedule` 和 JSON 资源等结构化语法；
  执行上下文分为无、任意实体、非玩家实体和玩家，对应 `@entity`/`@non_player`/`@player` 属性；
  `run`/`execute` 是底层逃生口，`--deny-raw` 可以强制整个项目只用结构化语法。
- 关键词中英文等价且可混用，两种写法必须生成逐字节相同的产物。
- 目标版本固定为仓库内 `minecraft_client_26.3` 源码，数据包版本 `121.0`。
- 依赖只有 `serde_json`，Rust edition 2024。
- 命令行：
  - `mclang build <源文件.mcl|项目目录> [-o <输出目录>] [--description <文本>] [--deny-raw]`
  - `mclang check <源文件.mcl|项目目录> [--deny-raw]`
  - `mclang lsp`（标准输入输出上的语言服务器，供编辑器插件调用）
  - `mclang help` / `mclang version`
- 默认输出到 `build/<项目名>`；`.mclang-manifest` 记录上次产物，重建只清理自己上次生成的文件，并回收 `data/` 下不再包含文件的空目录。

编译流水线：

```text
.mcl 源文件
  → lexer 词法分析
  → parser 递归下降解析（AST）
  → lib.rs 多文件合并、命名空间一致性检查
  → compiler/validate 只读语义检查（一次返回全部诊断）
  → compiler/codegen 生成函数、辅助函数、pack.mcmeta、资源与函数标签
  → lib.rs 写入输出目录
```

## 目录结构

| 位置 | 职责 |
| --- | --- |
| `src/main.rs` | 命令行参数解析与入口 |
| `src/lib.rs` | 项目级编排：发现/读取源文件、跨文件合并、写数据包 |
| `src/lexer.rs` | 词法分析 |
| `src/ast.rs` | AST 与源范围定义（声明带 `name_span`，供编辑器跳转） |
| `src/analysis.rs` | 工具侧分析入口：内存源文件 → 结构化诊断与符号表 |
| `src/lsp/` | 语言服务器：`rpc`（分帧）、`convert`（位置换算）、`server`（会话与分派）、`features`（补全/悬停/跳转） |
| `src/parser/` | 递归下降解析：`mod.rs`（游标导航与顶层分派）、`declarations`、`items`、`statements`、`conditions`、`expressions`、`keywords` |
| `src/compiler/mod.rs` | `compile()` 入口与 `CompiledPack` |
| `src/compiler/types.rs` | 校验与生成共享的内部类型 |
| `src/compiler/constant.rs` | 编译期常量折叠 |
| `src/compiler/validate/` | 只读语义检查：`rules`（名称/路径/标签等规则）、`items`、`statements`、`expressions`、`recursion` |
| `src/compiler/codegen/` | 代码生成：`statements`（控制流与辅助函数）、`actions`（give 与 self 操作）、`expressions`、`names`（假玩家/objective）、`emit`（命令与 JSON 格式化） |
| `docs/` | 语言与编译器文档 |
| `editors/vscode/` | VSCode 插件：TextMate 语法、语言配置、语言客户端（`npm install` 后 `npx vsce package`） |
| `examples/` | 端到端示例项目 |

模块只向下依赖：`ast` 不认识其他模块；`validate` 只读 AST 并产出 `Diagnostic`；`codegen` 只处理已经
通过检查的程序，不再报告用户错误（可以 `expect` 验证阶段已经保证的前提）。

## 规范要求

### 通用

- 不要被向后兼容绑架，对于目前处于原型期的项目那是个坏习惯：可以随时调整语法、命名和内部 API，
  不保留兼容层、废弃代码或迁移脚本。
- 避免上帝类：
  - 一个类型只承担一个职责，模块边界要与职责对应（校验、生成、命名、格式化各自独立）。
  - 单个文件接近或超过 500 行时，先检查是否混合了多种职责；超过 800 行必须按职责拆分。
  - 单个类型的方法超过约 30 个时，先按职责归类；如果归类后仍显臃肿，再拆分类型本身。
  - 单个函数超过约 80 行时，拆成“分派函数 + 每个分支的小函数”。
  - 状态型类型（如 `Compiler`、`Parser`）允许把 `impl` 分散到子模块，但状态字段只保留一处；
    跨子模块调用的方法标 `pub(super)`，不要扩大可见性。
- 人类可读优先：命名表意，注释解释“为什么”而不是复述代码。
- 生成结果必须可复现：使用 `BTreeMap`/稳定哈希，不要在输出里引入随机顺序。
- 关键词必须存在中英版本，且一个中文关键词不得对应多个英文关键词（反之亦然）；在任务过程中，
  如果发现缺少中文或英文关键词，确认后顺手补上，只读任务除外

### 验证

- 不写单元测试。验证编译器的方式是实际编写 `.mcl` 源码、用真实编译器编译，并检查 `build/`里生成的 mcfunction 与 JSON 产物来判断是否有问题。
- `tests/valid/` 放必须编译成功的语料，覆盖各项语言能力；`tests/invalid/` 放必须被拒绝的语料，文件头注释写明期望诊断，改代码后逐个 `mclang check` 核对。
- 改动编译器后必须运行 `cargo fmt`、`cargo clippy --all-targets` 保持零警告，并运行bun docs/tools/build.mjs --self-test` 验证示例的双语编译与产物逐字节一致。

### 文档

- 主动更新语言文档：语法、语义或运行时行为变化必须同步 `docs/content/manual.md`（单页手册正文），
  并用 `bun docs/tools/build.mjs --self-test` 重新生成 `docs/index.html`；文档工具会从编译器源码
  提取关键词表，并把正文里的完整示例分别用英文与中文关键词编译一遍。
- 新增能力后在 `DEVELOPMENT_PLAN.md` 勾选对应条目，并在 `examples/` 提供或更新示例。
- 不要主动创建 README.md。

### 语言与目标版本

- 一切以仓库内的 `minecraft_client_26.3-rc-2/` 源码和注册表为准，不要凭记忆猜资源类型、注册表
  路径或命令签名；数据包格式保持 `121.0`。
- 新增关键词必须同时提供中英文写法，并保证两种写法产物逐字节一致。
- 代码标识符、内部符号和注释使用 ASCII/英文；用户消息和诊断使用中文。
- 诊断信息要包含具体标识符、期望值和源位置。

### 依赖与提交

- 尽量不新增依赖；确需新依赖时先说明理由。
- 只有用户明确要求时才 `git commit`、`git push`。

## 常用命令

```powershell
cargo fmt
cargo clippy --all-targets

# 编译并检查示例
cargo run -- build examples/portable_chest
cargo run -- check examples/multi_counter --deny-raw
cargo run -- build examples/give_reward.mcl -o build/give_reward --description "奖励示例"

# 编译测试语料
cargo run -- build tests/valid/language -o build/tests/language --deny-raw
cargo run -- check tests/invalid/items.mcl
```

## 文档索引

- `docs/index.html`：单页语言手册，带中英文关键词切换、附录对照表与经验证的完整示例。
- `docs/content/manual.md`：手册正文；`docs/tools/` 从编译器源码提取词表并调用真实编译器验证示例。
- `docs/assets/`：手册样式与交互脚本。
- `DEVELOPMENT_PLAN.md`：已完成能力与后续路线，根据项目实际进展进行主动修改打勾。

给GPT系列：当用户要求收尾时，最好先更新同步DEVELOPMENT_PLAN.md，并立即开始减少工具调用，因为此时额度告急无法支撑太多操作，收尾时无需执行验证。