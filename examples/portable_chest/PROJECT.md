# 便携箱子矿车测试项目

这个项目面向 Minecraft Java Edition 26.3-rc-2，用来验证 Mclang 能否编译一个带持续状态、实体检测、NBT 存储和多文件函数调用的实际数据包。

项目源码不包含 `run` 或字符串形式的 `execute`。实体选择器、物品组件谓词、维度切换、NBT 命令和文本组件 JSON 都由编译器根据类型化语句生成。

## 构建与安装

在 Mclang 仓库根目录执行：

```powershell
target/release/mclang.exe check examples/portable_chest --deny-raw
target/release/mclang.exe build examples/portable_chest --deny-raw
```

默认产物位于 `build/portable_chest`。把这个目录复制到目标世界的 `datapacks` 目录，进入世界后执行 `/reload`。也可以直接把输出目录指向世界：

```powershell
target/release/mclang.exe build examples/portable_chest -o "C:/path/to/world/datapacks/portable_chest" --deny-raw
```

## 操作方法

1. 用铁砧把一个绿宝石命名为大写单字母 `A`。
2. 只丢出一个该绿宝石。数据包会在落点召唤一辆不可被普通伤害破坏的箱子矿车；绿宝石不会被消耗，而是原样交还投掷者并立即回到背包。
3. 打开矿车并放入任意物品。
4. 再次丢出绿宝石。源码中的 `remove_preserving_items` 会让编译器固定生成“保存完整槽位列表、清空实体、移除实体”三个步骤。
5. 第三次触发会重新召唤矿车，并把保存的物品恢复到原槽位。

名称匹配区分大小写，也要求物品组件的值精确等于纯文本 `A`。未命名绿宝石、小写 `a`、其他名称和数量大于一的掉落堆都不会触发。

## 状态和边界

这是一个全世界共享的单箱子实现，不按玩家分别保存。`active` 计分值记录箱子是否放出，物品存储会随世界保存，执行 `/reload` 不会清空内容。矿车可以在主世界、下界和末地之间收取；当前实现不扫描数据包添加的自定义维度。

正常切换时，删除矿车之前会先保存并清空 `Items`，所以矿车不会把箱内物品掉出。管理员直接删除实体、清除计分板或删除命令存储属于外部状态修改，不在切换协议内。

触发用的绿宝石不会被消耗或复制：数据包先给掉落物实体打上 `portable_chest_processed` 标签，避免它每刻重复切换；随后 `self.return_to_owner()` 通过原版 `execute on owner` 找到掉落物 `Thrower` NBT 里的投掷者，把同一个实体传送到投掷者并把 `PickupDelay` 置零，由原版拾取逻辑放回背包。名称、附魔、Lore 等组件原样保留；投掷者不存在或背包已满时，实体保持原样留在世界。

## 手工验收

- 首次触发后只有一辆带 `portable_chest_box` 标签的箱子矿车，触发用的绿宝石立即回到投掷者背包且名称与组件不变。
- 反复投掷同一个绿宝石可以持续切换；不会消耗或复制绿宝石。
- 背包已满时触发，绿宝石留在投掷者脚边而不是消失。
- 箱内物品包含不同槽位、堆叠数、耐久和组件时，收起后地面没有掉落物。
- 再次放出后，物品的槽位、数量和组件保持不变。
- 连续执行 `/reload` 后，已放出的矿车和已收起的物品都不会被重置。
- 普通绿宝石、名称 `a`、名称 `AA` 和两个一组的 `A` 绿宝石不会触发。
