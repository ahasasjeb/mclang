# 告示牌账户银行 v2

这是面向 Minecraft Java Edition 26.3 的 Mclang 压力测试数据包，包含 32 个告示牌账户槽位、独立银行余额和 32 间可扩建实体仓库。

## 正确构建

当前版本依赖提交 `57eae8d` 对 `execute if blocks` 的修复。必须先重编译编译器本体，不能复用旧的 `target/debug/mclang.exe`：

```powershell
cargo build --bin mclang
target/debug/mclang.exe check examples/sign_bank --deny-raw
target/debug/mclang.exe build examples/sign_bank -o build/sign_bank --deny-raw
```

旧编译器会生成缺少比较模式的命令：

```mcfunction
execute if blocks ... run ...
```

26.3 要求显式写成：

```mcfunction
execute if blocks ... all run ...
```

账号、密码和空白牌检测全部依赖该命令，缺少 `all` 时会出现“按钮有声音但页面不变化”。

## 使用

1. 大厅位于 `(0.5, 64, 5.5)`。
2. 首屏下方分别是登录和注册命令牌。
3. 选择功能后，中间牌才变为可编辑；账号和密码始终分两次输入。
4. 所有提交都会立即显示 actionbar 状态。
5. 注册费为 1 枚绿宝石。
6. 仓库初始 3×3，最高 10×10；升级费用等于升级后的边长。

## 数据结构

- 机房账号索引牌存在：槽位已被占用或正在注册。
- 同槽位凭据牌存在：账户注册完成。
- 隐藏 marker：保存账户槽位、绿宝石余额和仓库边长。
- 实体仓库：物品和方块始终留在世界中，不做 NBT 序列化。

命令木牌同时设置 `is_waxed = true` 与 `allow_op_features = true`。原版会执行一块牌四行中的全部点击事件，因此每块命令牌只含一个 `run_command`，存入、取出、升级、退出分别使用独立牌。

## 世界区域

- 大厅与机房：`x=-10..10, y=63..82, z=-12..12`
- 仓库阵列：`x=100..545, y=63..70, z=0..13`
- 常加载区域：72 个区块

`installed` 已存在时不会重建房间外壳，因此更新数据包不会主动清除已有仓库方块。
