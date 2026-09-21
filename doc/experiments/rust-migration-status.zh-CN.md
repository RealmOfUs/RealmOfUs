# 2.1 Rust 迁移验收

已按[架构 §10](../Realm_of_Us_Architecture.zh-CN.md)完成五步迁移。核心规则统一由 Rust 提供，CLI、Blender ctypes 与 Godot GDExtension 已接通；旧 Go/Python 规则实现已在新实现通过验收后备份并删除，删除后重新执行回归。此记录区分自动实测与尚未确认的性能、人工体验。

下表及旧回执保留各阶段当时的验收结果；当前使用限制见本文“已知边界”。

随后按用户要求将全部 Rust 代码收拢到 `rou/`，再调整为 `app/` + `crates/` 的 Cargo workspace；当前布局与构建命令见 [rou/README](../../rou/README.md)，本次目录调整的复验见文末。下面五步迁移表中的 crate 名保留为当时阶段名称。

| 顺序 | 实现与结果 |
| --- | --- |
| 1. rou-core | 无 IO 的类型、严格 JSON/JCS、空间、GLB、路径/LFS、构建与版本规则；30 项测试覆盖 77 个审阅夹具，三个 fuzz target 均运行 21 秒无崩溃 |
| 2. rou-git / rou-cli | 原生 Git/LFS、快照、锁、缓存、稀疏物化、分组构建与命令集；16 项真实 Git/LFS 集成测试，覆盖原八类场景与迁移必修缺陷 |
| 3. rou-ffi / Blender | ABI 1、生成的 C 头文件、ctypes 与按平台携带原生库的扩展 ZIP；C 调用、3 项 ABI 测试及真实 Blender 安装/作者流程通过 |
| 4. rou-godot / 查看器 | 校验与空间换算调用同一核心；通用选点、最近位置、流式加载、版本显示与独立应用已实现；房屋导览仅使用样例本地数据 |
| 5. 删除与回归 | 旧实现和 G0 探针已删除，规范路径已接管；Rust、宿主、Git/Blender/Godot 联合流程与真实窗口回归通过 |

## 数据与查看器规则

- `realm.json` 只有 `format/id/profile`，初始化后不可变；`realm/` 只有 `models/` 与 `entities/`。没有第四个 ID 命名空间、观看入口文件或 `origin.json`。
- 浏览包只含目录和资产闭包，Git 导出另含版本侧车。插件本地预览只写 `realm.json` 与 `objects/`；不写源工程或样例路点。
- 默认相机位于全部实体世界 AABB 并集中心上方，距离等于并集对角线长度并朝向中心；空世界为 `[0,2,0]` 朝向原点。实体列表、分块地图及米制坐标可随时选点，`--at=x,y,z` 可直接指定起点。
- 已绑定 realm ID 的最近位置保存在本机 Godot 用户目录，`--at` 优先；无版本侧车的本地预览没有 realm ID，因此不保存这类位置。测试用 `--no-recent` 隔离已有位置。
- 房屋样例通过 `--sample=villa-anahid` 启用；生成脚本将 `sample.json` 写在浏览包外，可用 `--sample-data` 选择。通用查看器不读取旧 `entries/tour`。

## 删除后的自动实测

下列回执均在 `build/` 内，是本机产物，不属于版本化源码。前期失败/诊断日志保留，以下列出的回执为验收依据。执行时间以各回执为准：主要回归在 2026-09-17 UTC，最终独立应用及房屋退出复核在 2026-09-20 UTC。

| 检查 | 结果与回执 |
| --- | --- |
| Rust workspace | [49 项测试通过](../../build/rust-validation/final-cargo.log)：核心 30、Git/CLI 16、ABI 3 |
| 格式与静态检查 | [通过](../../build/rust-validation/final-code-checks.log)：cargo fmt、clippy `-D warnings`、Ruff、gdformat |
| C ABI | [C smoke 源码](../../rou/crates/rou-ffi/tests/c_smoke.c) 编译运行通过；复现命令见 [toolchain](../../toolchain/README.md) |
| fuzz | [JSON](../../build/rust-validation/fuzz-json-final.log) 1,879,783 次；[GLB](../../build/rust-validation/fuzz-glb-final.log) 2,992,415 次；[LFS](../../build/rust-validation/fuzz-lfs-final.log) 4,878,191 次；各 21 秒，无崩溃 |
| 空间与 Godot ABI | [77 个夹具、110 项 Godot 断言通过](../../build/s1-2.1/spatial-checks/report.json) |
| Git/Blender/查看器联合流程 | [27 项流程检查、57 条命令通过](../../build/rust-validation/git-final/run-20260917T084328983408Z/report.json)，[83 项 Godot 检查通过](../../build/rust-validation/git-final/run-20260917T084328983408Z/godot-report.json) |
| 插件本地预览 | [9 项通过](../../build/rust-validation/addon-preview-final/report.json) |
| 插件作者会话 | [24 项通过](../../build/rust-validation/addon-authoring-final-2/authoring-report.json)，包括外链库、外部贴图、相对路径与共享模型实例 |
| 插件真实安装/生命周期 | [62 项通过](../../build/rust-validation/addon-install-final/installation-report.json)，隔离用户配置，通过 Preferences 扩展安装/启停；记录了当时交付 ZIP 的 SHA-256 |
| 查看器本地工作流 | [28 项通过](../../build/rust-validation/addon-preview-final/viewer-workflow.json)，覆盖打开、重载、失败保留、空世界与缺失资产 |
| 真实 GLB 导入 | [742 项断言通过](../../build/s1-2.1/checks/godot-checks.log)，覆盖校准街区 50 与房屋 55 个实体 |
| 生命周期 | [通用场景 8 项](../../build/s1-2.1/checks/lifecycle.log)、[房屋样例 15 项](../../build/rust-validation/final-house-lifecycle.log)通过 |
| 房屋建模保留 | [55 实体、7 床、7 沙发、12 门洞检查通过](../../build/rust-validation/villa-final/checks.json) |
| 真实窗口脚本交互 | [作者流程 33 项](../../build/rust-validation/rendered-authoring-final/report.json)、[校准街区 17 项](../../build/rust-validation/rendered-calibration-final/report.json)、[房屋 53 项](../../build/rust-validation/rendered-house-cleanup/report.json)通过；各目录含截图 |
| 独立应用 | [9 项功能检查通过](../../build/rust-validation/standalone-final/acceptance.json)：清空 PATH 启动，固定提交与浏览根一致，4 个模型加载、资产验证、实际帧和截图正常；包内无源工程或 Git 数据 |
| 历史校准回归 | [Python 检查通过](../../build/rust-validation/final-python.log)，保留审阅过的空间/载荷统计，排除协议形状变更影响的目录字节数 |
| 文档链接 | [36 个 Markdown 文件检查通过](../../build/rust-validation/final-links.log) |

独立包绑定提交 `a5b362c61b189332658dd626ecdd08cdf1cdfdcf`。真实窗口中的点击、移动和文件选择由脚本驱动，不能代替用户手工体验确认。此次没有对小场景帧耗时作规模性能外推。

[机器可读汇总](../../build/rust-validation/final-summary.json)记录 11 份 JSON 回执、交付产物 SHA-256、旧路径删除检查与源码 index 为空的检查结果。

## 产物、环境与保留范围

- CLI：[build/bin/rou](../../build/bin/rou)，release Rust 产物。
- Blender：[dist/rou_blender.zip](../../dist/rou_blender.zip)，macOS arm64 原生库随包携带。
- 独立应用：[Realm of Us.app](<../../dist/viewer-2.1/macos/Realm of Us.app>)，macOS arm64，ad-hoc 签名验证通过。
- Rust 由 `rou/rust-toolchain.toml` 固定为 1.94.0，支持 Godot 0.5.5 crate 与 API 4.7；本机 Blender 5.2.1 LTS、Godot 4.7.2。nightly 仅用于 fuzz；未修改用户的全局默认 Rust 工具链。
- 旧源码备份：[tar.gz](../../build/rust-validation/legacy-source-backup.tar.gz) 与 [SHA-256 清单](../../build/rust-validation/legacy-source-backup.json)，125 个文件已逐一校验。旧仓库说明保存在 [历史文档](../old-doc/repository-2.0.md)。
- 未 stage、commit 或 push 源码。用户模型、旧报告和已有构建产物保留；`build/garden` 未转换。房屋样例脚本适配了新 API，建模与美术实验保留。

## 已知边界

1. 加载半径按规范保持 48 m。大世界的默认高空总览可能没有附近实体，界面提示通过地图、实体或坐标选点；没有悄悄扩大加载范围。
2. 独立应用有 440 个实际帧，但焦点计数为 0，`foreground_measurement_valid=false`。激活请求成功不等于有效前台采样，因此仅确认功能通过，不宣称前台性能达标。
3. macOS dylib 的 ad-hoc 校验和普通加载通过。带 quarantine 的测试副本导入超时，手动解除隔离后本机加载与功能检查通过；公开下载的自动放行与公证尚未验收。
4. 房屋真实窗口退出时仍有 `7 RIDs of type "Texture" were leaked` 警告；显式清空场景并等待释放后仍可复现。功能检查通过，但本轮未定位该警告的根因，不能宣称所有 GPU 资源释放检查通过。
5. 仅验收本机 macOS arm64。未验证其他操作系统，未做 Godot Web 导出或 S2 网络功能。

## Rust 目录收拢复验（历史布局）

2026-09-19（本地日期）按用户要求调整源码组织，数据格式与运行行为不变：

- `rou/Cargo.toml` 是主包与 workspace；`rou/src/core/`、`rou/src/git/` 是 `rou_lib` 的内部模块，CLI 入口为 `rou/src/bin/rou.rs`。
- 两个薄清单 `rou/bindings/ffi/` 与 `rou/bindings/godot/` 分别编译 `rou/src/ffi/` 与 `rou/src/godot.rs`，继续生成 `librou` 和 `librou_godot`。独立构建适配器时禁用 CLI/Git feature。
- 测试、夹具、fuzz、C 头文件、Cargo.lock、工具链配置和 target 全部位于 `rou/`；项目根目录不再有 `crates/`、`fuzz/`、`target/` 或 Cargo 配置。脚本会在 `rou/` 执行 Cargo。
- 调整前的 Rust 源码与配置备份在 [before-layout.tar.gz](../../build/rust-layout-validation/before-layout.tar.gz)，旧清单和编辑器恢复文件也保存在该验收目录。未暂存、提交或推送源码。

本轮回执：

| 检查 | 结果 |
| --- | --- |
| Rust workspace 与仅核心构建 | [49 项通过](../../build/rust-layout-validation/cargo-test.log)；[禁用默认 feature 的 30 项核心测试通过](../../build/rust-layout-validation/core-only.log) |
| 静态检查与格式 | [cargo fmt、clippy、Ruff、gdformat 通过](../../build/rust-layout-validation/code-checks.log)；也检查独立 fuzz workspace 的格式 |
| fuzz 与 release | [三个 fuzz target 编译通过](../../build/rust-layout-validation/fuzz-build.log)，[workspace release 构建通过](../../build/rust-layout-validation/release.log)；本轮未重复执行随机 fuzz |
| 空间与 ABI | [110 项 Godot 断言通过](../../build/rust-layout-validation/spatial-report.json)；新路径下 C smoke 编译运行通过；C 头文件和审阅空间夹具与调整前逐字节相同 |
| Git/Blender/Godot | [27 项流程检查、57 条命令通过](../../build/rust-layout-validation/git/run-20260920T045448232407Z/report.json)，[83 项 Godot 检查通过](../../build/rust-layout-validation/git/run-20260920T045448232407Z/godot-report.json)；本轮 Godot 使用 headless |
| 插件预览和真实安装 | [9 项预览检查](../../build/rust-layout-validation/addon-preview/report.json)、[62 项安装/启停检查](../../build/rust-layout-validation/addon-install/installation-report.json)通过；当前 `dist/rou_blender.zip` 为该次测试的包 |
| 查看器打包 | [导出成功](../../build/rust-layout-validation/package-viewer.log)，生成于 `build/rust-layout-validation/viewer/`，macOS 签名检查通过；本轮没有重复真实窗口/性能测量，前次应用保留在 `dist/viewer-2.1/` |

[目录调整检查汇总](../../build/rust-layout-validation/checks.json)记录当前源码布局、测试数量与 index 为空的结果。此前列出的焦点采样、房屋退出纹理警告等限制未因此次目录调整而解决。

## Cargo workspace 重组复验

当前布局采用 `rou/app/` 与 `rou/crates/`：根清单为虚拟 workspace，CLI 包名与可执行文件仍为 `rou`；四个库为 `rou-core`、`rou-git`、`rou-ffi`、`rou-godot`，各有自己的清单与源码。核心/Git 层不再依赖 feature 开关分隔；FFI/Godot 直接依赖核心。`bindings/` 的清单和实现合并进各自 crate；fuzz 仍是独立 nightly workspace。当前布局及命令见 [rou/README](../../rou/README.md)。

构建脚本、Blender 开发环境库发现、Godot 空间夹具路径、规范与中英文架构均已更新。CLI 命令、原生库名、ABI、C 头文件与审阅夹具保持不变。调整前源码备份在 [before.tar.gz](../../build/workspace-refactor/before.tar.gz)，[检查汇总](../../build/workspace-refactor/checks.json)记录本轮结果：

| 检查 | 结果 |
| --- | --- |
| Rust 与 Rustdoc | [49 项测试、8 项文档示例通过](../../build/workspace-refactor/cargo-tests.log)；[严格 Rustdoc 构建通过](../../build/workspace-refactor/rustdoc.log) |
| 静态检查与依赖 | [格式、Clippy、Ruff、gdformat 通过](../../build/workspace-refactor/code-checks.log)；核心、FFI 和 Godot 的普通依赖均不含 CLI/Git |
| 构建 | [Debug](../../build/workspace-refactor/build.log)、[release workspace](../../build/workspace-refactor/release.log) 与 [3 个 nightly fuzz 入口编译](../../build/workspace-refactor/fuzz-build.log)通过；本轮未重跑随机 fuzz |
| CLI 与 C ABI | 54 次 help 调用与调整前输出相同；[C smoke 通过](../../build/workspace-refactor/c-smoke.log)，Python ctypes 从新源码路径自动发现并调用库成功 |
| 空间 | [110 项 Godot 空间断言通过](../../build/workspace-refactor/spatial-report.json) |
| Git/Blender/Godot | [27 项协作流程检查](../../build/workspace-refactor/git/run-20260920T062647305581Z/report.json)、[83 项 Godot 检查](../../build/workspace-refactor/git/run-20260920T062647305581Z/godot-report.json)通过 |
| 插件包 | [release ZIP 生成](../../build/workspace-refactor/package-addon.log)、[9 项 Blender 插件预览检查](../../build/workspace-refactor/addon-preview/report.json)通过 |

`build/bin/rou` 已更新为本轮 release 可执行文件，新的插件包位于 `build/workspace-refactor/rou_blender.zip`。本轮仅验证本机 macOS arm64，Godot 使用 headless；此前列出的实机、性能与退出纹理警告限制不因此次重组消失。源码未暂存、提交或推送。
