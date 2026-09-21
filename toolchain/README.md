# 创作、验收与打包

生产链路：Rust `rou` → 原生 Git/LFS → headless Blender → 固定版本浏览包 → Godot。Python 只负责宿主 IO、bpy 与验收编排，规则经 `librou` 调用 Rust。数据与命令语义见[规范](../doc/protocol/spec/s1-repository.zh-CN.md)。

| 模块 | 作用 |
| --- | --- |
| `../rou/` | 核心、Git/CLI、C ABI 与 GDExtension |
| `blender/rou_blender/` | 完整原工程、放置、会话守卫、本地预览 |
| `blender/export_git_realm.py` | 冻结工程的 object/Collection 导出 helper |
| `build/native.py` | 开发脚本构建与加载原生库；无领域规则 |
| `build/`、`tests/blender/` | 黑盒验收与打包 |

## 开发检查

先安装根目录 `requirements-dev.txt`，在 `rou/` 中运行 `cargo build --workspace`。Rust 版本由 `rou/rust-toolchain.toml` 固定。完整检查：

```bash
.cache/s1-validation-venv/bin/python toolchain/build/check_code.py
pushd rou
cargo test --workspace
popd
.cache/s1-validation-venv/bin/python -m unittest discover -s toolchain/tests -v
.cache/s1-validation-venv/bin/python toolchain/build/verify_spatial.py
.cache/s1-validation-venv/bin/python toolchain/build/verify_git_realm.py --windowed
```

`verify_spatial.py` 自动构建本机 GDExtension 并复制到 `client/app/bin/`。`verify_git_realm.py` 自动构建 CLI 和扩展，所有 Git 操作都在新建验收目录内，报告记录每条命令、结果和日志。它不修改源码仓库的 index。

macOS 上可额外从 C 调用生成的头文件，检查 ABI、成功/失败响应与释放：

```bash
mkdir -p build/rust-validation
clang -std=c11 -Wall -Wextra -Werror -I rou/include \
  rou/crates/rou-ffi/tests/c_smoke.c -L rou/target/debug -lrou \
  -Wl,-rpath,"$PWD/rou/target/debug" -o build/rust-validation/ffi-c-smoke
build/rust-validation/ffi-c-smoke
```

## Blender 扩展

```bash
python3 toolchain/build/package_addon.py --output dist/rou_blender.zip
blender --background --factory-startup --python-exit-code 1 \
  --python toolchain/tests/blender/verify_addon.py -- \
  --addon-zip "$PWD/dist/rou_blender.zip" --output "$PWD/build/addon-preview-check"
blender --background --factory-startup --python-exit-code 1 \
  --python toolchain/tests/blender/verify_git_authoring.py -- \
  --addon-zip "$PWD/dist/rou_blender.zip" --output "$PWD/build/addon-authoring-check"
```

ZIP 使用 Blender 4.2+ 扩展清单，根目录为 Python 模块和 `blender_manifest.toml`，`lib/<platform>/` 携带当前机器架构的 release 库；生成包不会依赖开发机的 target 目录。显示名 Realm of Us，扩展 ID `rou_blender`，操作符仍为 `rou.*`。在 Preferences → Install from Disk 安装后，于 3D 视图 N 面板使用 Realm。

首次启用或重新启用后，重新打开作者工程建立会话。`rou.register_entities` 登记模型和实体；`rou.write_placement` 写放置；“仅保存源”保留外部 JSON，“重应用摆放”在明确操作后重新采用当前场景位置。基线只覆盖当前源相关记录，完整元数据仍检查引用与预算。linked 库、外部贴图和 `//` 相对路径均有真实 Blender 回归。

作者根按对象的 `rou_entity_id` 定位；`.blend` 的 Scene 属性 `rou_model_owners_json` 保存拥有模型定义的实体 ID，重开、重应用摆放、撤销后重新按 ID 找对象。模型 ID 从实体记录读取，不要求与实体 ID 相同。已有工程首次建立绑定时，单实例直接采用实体 ID；共享模型要求现有入口对象带有匹配的实体 ID，无法唯一确认则报错。登记时才把作者根的新名字同步到模型 JSON，共享实例的名字不改变入口。Shift+D 后两个存活根不能共用 ID；删除原件后，同 ID 副本可以继续作为作者根。绑定随完整源文件和“仅保存源”一起保存，不增加 realm JSON 字段，也不进入浏览包。

已登记的作者根改名后，先执行“登记实体”，再保存源和构建。插件的完整保存及“仅保存源”都会检查入口名与当前作者根一致，失败不写源文件或模型/放置 JSON。原生 Blender 保存不经过插件按钮；后续 `rou build` 会用冻结快照里的实体关联核对源内作者身份，发现名字漂移或作者根缺失时拒绝导出，并提示重新登记、保存。

真实 Preferences 安装/启停测试在隔离目录执行，避免写用户配置：

```bash
ADDON_CHECK_DIR="$(mktemp -d "$PWD/build/addon-install.XXXXXX")"
mkdir -p "$ADDON_CHECK_DIR/user/config" "$ADDON_CHECK_DIR/user/scripts"
BLENDER_USER_RESOURCES="$ADDON_CHECK_DIR/user" \
BLENDER_USER_CONFIG="$ADDON_CHECK_DIR/user/config" \
BLENDER_USER_SCRIPTS="$ADDON_CHECK_DIR/user/scripts" \
blender --background --factory-startup --python-exit-code 1 \
  --python toolchain/tests/blender/verify_addon_install.py -- \
  --addon-zip "$PWD/dist/rou_blender.zip" --output "$ADDON_CHECK_DIR"
```

插件本地预览只写 `realm.json` 与 `objects/`。没有版本侧车时，查看器显示未绑定版本。Git 提交仍由作者显式暂存和提交；会话守卫不能拦截外部程序或 Blender 原生保存。

## 独立查看器

先 `rou view` 生成固定版本目录，再打包当前平台的原生扩展：

```bash
python3 toolchain/build/fetch_templates.py
python3 toolchain/build/package_viewer.py --platform macos \
  --realm /absolute/path/to/view/realm.json --output-dir dist/viewer-2.1
python3 toolchain/build/verify_git_viewer.py \
  --app "dist/viewer-2.1/macos/Realm of Us.app" \
  --output build/viewer-acceptance --seconds 12 --activate --at=12,3,5
```

`--at` 按实际世界选择米制位置。应用无 Git/LFS/Blender 运行依赖，包内没有源工程。Mac 打包从官方 universal 模板提取 arm64 二进制，原模板保持原样，应用做 ad-hoc 签名。本轮只验收 macOS arm64；跨平台导出要求对应 Rust target 的原生库和模板，不把导出成功视为实机通过。

查看器默认起点、地图/实体/坐标选点和本机最近位置遵循[浏览包规范](../doc/protocol/spec/s1-preview.zh-CN.md)。最近位置按 realm ID 存在 Godot 本机用户目录，`--at` 优先于最近位置，`--no-recent` 用于可重复测试。地图可选择 Y 层。加载半径始终为 48 m，较高的默认全景可能没有附近实体，界面提示选点。

`godot` crate 固定 0.5.5，API 4.7。通用接口为 `Rou.invoke`，不再遮蔽 `Object.call`；`load_catalog` / `catalog_query` 复用 Rust 校验后的目录索引。

## 独立工具

- `verify_s1.py`：Rust 空间测试、真实 GLB 导入/AABB 与查看器生命周期。
- `compare_grids.py`：经原生库计算 16/32/64 m 校准实验，观察位置属于测试数据。
- `benchmark_viewer.py`：实际窗口帧耗时、RSS 与启动时间；单独记录焦点有效性，支持 `--at` 和 `--sample`。
- `client/tests/rendered_workflow.gd`：真实窗口中以脚本输入验证导航、文件打开/重载和样例房间地图。
- `examples/villa_anahid/`：建模和本地导览样例，不向通用浏览包写样例路点。

旧 Go/Python 代码及 G0 探针已删除。历史报告保留；本轮回执与边界见[迁移记录](../doc/experiments/rust-migration-status.zh-CN.md)。

macOS 插件打包会对 ZIP 内的 dylib 显式执行 ad-hoc 签名与 `codesign --verify --strict`。这不是 Developer ID 签名或公证。交付探针：

```bash
python toolchain/build/verify_addon_macos.py --addon-zip dist/rou_blender.zip --output build/macos-delivery-probe
```

探针仅修改隔离副本，记录 quarantine、`spctl` 和真实 Blender 加载结果，不更改系统安全策略。普通安装或手动解除隔离后的加载通过，不代表公开下载可自动放行；正式分发仍需签名、公证与实际下载路径验收。
