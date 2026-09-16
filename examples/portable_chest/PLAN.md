# 便携箱子项目执行计划

- [x] 从 Minecraft 26.3-rc-2 源码确认掉落物的 `contents` 槽和物品谓词语法。
- [x] 确认箱子矿车使用 `Items` NBT 列表持久化槽位和完整物品堆数据。
- [x] 确认玩家 NBT 不能通过 `data` 修改（`EntityDataAccessor` 拒绝玩家），因此按玩家状态必须走计分板。
- [x] 确认 26.3 实体带通用 `data` 自定义字段、物品堆带 `minecraft:custom_data` 组件，都可以保存任意 NBT。
- [x] 为语言补充 `objective` 声明与 `scoreboard.set/reset/get` 计分板读写。
- [x] 为语言补充 `data_slot` 声明与 `self.deposit/withdraw/remove_data` 数据槽搬运。
- [x] 用中文关键字重写项目：每位玩家分配独立编号，矿车用实体计分标识归属。
- [x] 收起的物品寄存在触发绿宝石的 `custom_data` 里，使用追加语义避免覆盖。
- [x] 修复收起时 `kill @s` 掉落物品的问题：`自身.保存并移除` 确认写入成功后才清空并移除。
- [x] 矿车补充 `NoGravity` 与 `Invulnerable`，不会下落、滑动或被伤害摧毁。
- [x] 用 `--deny-raw` 严格模式验证源码不含底层命令字符串。
- [x] 通过 Mclang 静态检查并生成 26.3-rc-2 数据包。
- [x] 检查生成的标签、函数、数据包格式和关键命令。
- [x] 运行 Rust 测试、格式检查和 Clippy 回归检查。
- [ ] 在 Minecraft 26.3-rc-2 世界中完成双人交互验收。
