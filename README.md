# Realm of Us

多人分布式开放 3D 世界协作框架。原生 Git 管完整工程与唯一历史，标准 Git LFS 管源工程和运行时资产，`rou` 提供空间选区、构建、校验与固定版本浏览。当前范围为 S1 本地闭环；P2P、公开签名与节点管理属于 S2。

**2.1 核心已迁移到 Rust。** Blender 通过 ctypes 调用 `librou`，Godot 通过 GDExtension 调用同一份 `rou_core::ops`。旧 Go CLI 和 Python 领域层已删除，没有 2.0 兼容 reader。

[设计](doc/README.md) · [数据契约](doc/protocol/README.md) · [架构](doc/Realm_of_Us_Architecture.zh-CN.md) · [迁移验收记录](doc/experiments/rust-migration-status.zh-CN.md) · [工具链与打包](toolchain/README.md)

| 目录 | 责任 |
| --- | --- |
| `rou/` | Cargo workspace、统一 Rust 源码、测试、fuzz 与构建产物；见 [模块说明](rou/README.md) |
| `rou/crates/rou-core/src` | 纯规则、文档类型、严格 JSON/JCS、空间、GLB、哈希、投影与 diff |
| `rou/crates/rou-git/src`、`rou/app/src/main.rs` | Git/LFS、快照、sparse、构建编排与 `rou` |
| `rou/crates/rou-ffi/src` | C ABI 与生成的 `include/rou.h` |
| `rou/crates/rou-godot/src/lib.rs` | Godot 原生适配器 |
| `toolchain/blender/rou_blender` | bpy 操作、完整源工程会话与插件界面 |
| `client/app` | 通用查看器；`samples/` 单独保存房屋演示数据 |
| `toolchain/build`、`toolchain/tests/blender`、`client/tests` | 验收、打包与开发工具 |
| `examples` | 原创校准街区与 Villa Anahid 实时/离线美术实验 |

`realm.json` 只有 `format/id/profile`，初始化后不可更改。`realm/models/` 定义源文件、模型入口和依赖，`realm/entities/` 定义模型引用与毫米位置，完整工程在 `sources/`，构建输出在 `runtime/<model>.glb` 与同名记录。浏览包不含观看入口；最近位置保存在本机用户目录。

## 构建与运行

需要 Git、Git LFS、Blender 4.2+ 与 Godot 4.7。本机验收使用 Blender 5.2.1 / Godot 4.7.2 / macOS arm64。`rou/rust-toolchain.toml` 固定 Rust 1.94.0；其他平台尚未实机验收。

```bash
pushd rou
cargo build --workspace
popd
mkdir -p build/bin client/app/bin
cp rou/target/debug/rou build/bin/rou
# macOS；Linux 使用 .so，Windows 使用 rou_godot.dll。
cp rou/target/debug/librou_godot.dylib client/app/bin/
export ROU_TOOLCHAIN_ROOT="$PWD"
export PATH="$PWD/build/bin:$PATH"

mkdir -p build/my-realm
pushd build/my-realm
rou init
# 在 Blender 保存完整 .blend、登记模型和实体，然后显式构建、暂存、提交。
rou build
rou add .
rou commit -m "Create a realm"
rou view HEAD --output .rou/view/current
popd

godot --path client/app -- --realm="$PWD/build/my-realm/.rou/view/current/realm.json"
```

Git 身份使用本机配置；测试使用隔离身份。`init` 不暂存，`build` 不执行 `add`。可用 `rou sparse set --chunk '1,0,1..-1,0,-1'` 选择范围，`rou sparse add --entity house` 追加，`rou sparse set --all` 取得全部创作依赖。

内置命令用 `-h` 查看简短用法，`--help` 查看详细规则和示例，例如 `rou sparse set --help`、`rou build --help`、`rou diff --help`；见 [CLI 帮助说明](rou/README.md#cli-help)。

查看器默认在全部实体包围盒并集中心上方，距离为并集对角线，朝向中心；空世界在原点上方 2 m。可通过地图、实体列表或米制坐标选点，也可传 `--at=12,3,5`。加载半径为 48 m；全景起点较高时先选地点即可进入内容区域。

## 验证

```bash
python3 -m venv .cache/s1-validation-venv
.cache/s1-validation-venv/bin/python -m pip install -r requirements-dev.txt
.cache/s1-validation-venv/bin/python toolchain/build/check_code.py
pushd rou
cargo test --workspace
popd
.cache/s1-validation-venv/bin/python -m unittest discover -s toolchain/tests -v
.cache/s1-validation-venv/bin/python toolchain/build/verify_spatial.py
.cache/s1-validation-venv/bin/python toolchain/build/verify_git_realm.py --windowed
```

最后一项在新 build 目录创建真实 Blender 场景和隔离 bare Git/LFS 远端，检查双人协作、选区、原工程再打开、并行修改、合并与固定版本浏览。插件安装、独立应用及其空 PATH 验收命令见[工具链说明](toolchain/README.md)。

## 美术实验与历史

[样例](examples/README.md)保留原创 CC0 校准街区和 Villa Anahid 重建。[V3 实时整屋](doc/experiments/walsh-v3.zh-CN.md)与[Cycles 离线样片](doc/experiments/walsh-offline.zh-CN.md)的旧资产、应用、回执保持原样；新生成器默认写入带 `-2.1` 的新目录。2.0 浏览包不能直接由新查看器读取，需从源工程重新导出。房屋导览通过 `--sample=villa-anahid` 显式启用，路点属于本地样例数据。
