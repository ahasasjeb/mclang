# 便携箱子项目执行计划

- [x] 从 Minecraft 26.3-rc-2 源码确认掉落物的 `contents` 槽和物品谓词语法。
- [x] 确认箱子矿车使用 `Items` NBT 列表持久化槽位和完整物品堆数据。
- [x] 用多文件 Mclang 源码实现精确触发、召唤、保存、清空、移除和恢复流程。
- [x] 覆盖主世界、下界和末地中的矿车收取，并保存全世界共享状态。
- [x] 推翻字符串命令作为常规接口的设计，为 Minecraft 行为增加结构化 AST 和类型检查。
- [x] 用 `query`、`storage`、`each`、`spawn`、`in_dimension`、`self.*` 和 `message.*` 重写项目，源码保持零底层命令字符串。
- [x] 用 `remove_preserving_items` 把容器保存、清空和移除固化为单个安全语言操作。
- [x] 增加 `--deny-raw` 严格模式，并以它验证本项目没有 `run` 或字符串 `execute`。
- [x] 通过 Mclang 静态检查并生成 26.3-rc-2 数据包。
- [x] 检查生成的标签、函数、数据包格式和关键命令。
- [x] 运行 Rust 测试、格式检查和 Clippy 回归检查。
- [ ] 在 Minecraft 26.3-rc-2 世界中完成交互验收。
