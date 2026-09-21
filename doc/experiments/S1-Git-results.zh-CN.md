# S1 Git · 实现与当前 Mac 验收

2026-09-14。本文记录在既有 S1.1 空间与 S1.2/1.3 实验之后，按 Git + LFS 方案实现 G1–G4 的结果。G1–G4 的本地实现和下述回归已通过；旧自有仓库不再是生产后端。

## 实现交付

| 阶段 | 代码与行为 |
| --- | --- |
| G1 | 原生 Git/LFS 唯一历史/index，统一 rou；完整原工程/可读子目录/普通文本；metadata clone、分支、改名删除、ignore |
| G2 | realm/authoring/entities/derived 新契约；真实依赖选择、反向范围；工作树/--staged 冻结构建；源输出校验与显式放置会话 |
| G3 | 原生三方合并、派生冲突重建、文本和领域 diff；固定 Git 版本或显式草稿浏览包；无 Git/Blender 的 Godot |
| G4 | 移除旧 Go CAS/refs/commit/workspace 与 Godot reader、旧集成入口；重建真实原工程样例、更新中英文设计/规范/指南及本机测试 |

world/ 改为 realm/。新目录里保存作者输入和可重建输出；.rou/ 只保存被 ignore 的本地状态。Git index 是唯一暂存区，没有转换器、双后端或旧格式兼容。

## 真实协作样例

examples/calibration/create_git_scene.py 生成四个原创 CC0 entity。house/plaque 共用完整多实体 courtyard.blend，bench 使用独立工程，far_tree 在远处；前三者引用共享 linked .blend 网格和外部 PNG。所有文件以可读路径保留。

verify_git_realm.py 在 build/s1-git/verification 下创建每次独立运行目录和 bare Git/LFS 远端。Alice/Bob 克隆时无载荷，选择不同实体后恢复原工程及共享依赖；真实 Blender 打开/编辑并保存，两边分别 build/add/commit。Alice merge 后重建，得到两个父提交，并在初始/合并版本间切换 Godot。

此样例是标准本地 Git/LFS 远端，不是公网 P2P。未改模型美术质量，也不把小校准样例帧率外推到整屋或大型世界。

## 验证回执

[精简机器回执](../reviews/data/s1-git-acceptance.json) 保存在源码内；完整日志/模型/浏览包位于被忽略的 build/。

| 验证 | 实际结果 |
| --- | --- |
| Python 全套 | 45/45，通过；其中新 Git 领域回归 29 项 |
| Go | 全套 go test -race ./... 与 go vet ./... 通过 |
| Python/Go/GDScript 格式 | Ruff、gofmt、gdformat 检查通过 |
| 空间共同向量 | Python 33 profile/17 position/13 bounds/14 catalog；Godot 110 断言通过 |
| 新 JSON Schema | 4 个 Schema 合法，真实新样例的 project/binding/entity 验证通过 |
| 真实 Blender 原工程与双人 Git/LFS | 24 项检查通过；包括完整 Scene/共享依赖恢复、无载荷 clone、干净 hydration、staged 语义、并行源修改、双父 merge、领域 diff、全世界与反向范围 |
| Godot 窗口 | 34 断言通过；固定版本/重载/GLB/AABB、篡改拒绝、缺失/未知区分、失败保留与恢复 |
| Blender 创作插件 | ZIP 解压后执行的 17 项领域检查通过；绑定/显式放置/依赖更新/相对路径/会话 stale/根变换；0.3.1 已重跑通过 |
| Blender 实际安装/启用 | 0.3.1 通过 61 项安装与生命周期检查；Preferences 操作符、3 次真实 RestrictBlend、重复启停、脏工程拒绝与重开恢复 |
| 独立 macOS app | 屏幕开启后，空 PATH 下真实 Metal/Apple M3 运行 12.18 s；固定 commit/root、3 GLB、无拒绝、实际帧/截图等 9 项功能检查通过；1031 个统计帧全部保持焦点 |
| 当前样例前台性能 | 两轮 36 s，3502/3488 个统计帧均无失焦；P95 11.328/11.356 ms，峰值 RSS 340.78/368.66 MiB，达到 P95 ≤ 33 ms、RSS ≤ 2 GiB 的目标 |

本机工具版本：Git 2.50.1（Apple Git-155）、Git LFS 3.6.1、Blender 5.2.1 LTS、Godot 4.7.2，macOS arm64。记录实际安装版本，不据此推荐版本组合或宣称其他 OS 已验证。

最终集成运行：`build/s1-git/verification/run-20260914T075233760959Z/`。其 report.json 记录每条 argv/cwd/退出码/日志；versions.json 记录 C0、两条贡献和 merge 的真实 Git OID；initial-view 与 merged-view 可直接在当前 Godot 打开。采用新增样例重现，不用旧 hash 源库转换。

## 本机交付产物

- CLI：`build/bin/rou`。
- Blender 插件：`dist/rou_authoring.zip`，当前版本 0.3.1；ZIP 内容已执行 17 项创作和 7 项独立导出检查。
- 独立应用：`dist/viewer-git/macos/Realm of Us.app`，内置最终合并版本的目录/版本侧车与 GLB，无作者工程或 Git 数据。

最新独立应用回执位于 `build/s1-git/foreground-20260914T165303Z/standalone/acceptance.json`，截图为同目录 `viewer.png`。应用记录的固定提交为 `bd52c15f3ceb0ae73a5b9bb260b0b7d2c5759547`。此前屏幕锁定时的回执保留在 `build/s1-git/mac-acceptance-foreground/acceptance.json`：其 0 focused / 336 unfocused frames 只证明独立运行与渲染功能，不计入性能结论。下述屏幕开启复测补齐了当前 Git 样例的前台性能验收。

这些产物位于被忽略的 build/dist；代码和回执可重新生成，不将模型或应用二进制混入源码提交。

独立应用可重复验证（每次选择新 output 目录）：

```bash
.cache/s1-validation-venv/bin/python toolchain/build/verify_git_viewer.py \
  --app "dist/viewer-git/macos/Realm of Us.app" \
  --output build/s1-git/mac-viewer-check --seconds 12 --activate
```

## Blender 0.3.1 安装修复

用户通过 Preferences 安装 0.3.0 时发现 `'_RestrictData' object has no attribute 'is_dirty'`。此前 17 项创作检查虽然使用 ZIP 内的代码，但通过直接调用 `register()` 启用，未覆盖 Blender 的真实插件启用入口；不能将它等同于 Preferences 安装验收。

在本机 Blender 5.2.1 LTS 的隔离用户目录中，通过 `bpy.ops.preferences.addon_install` / `addon_enable` 已复现同一错误，日志位于 `build/addon-install-regression/baseline/install.log`。Blender 在启用时使用 RestrictBlend 限制 `bpy.data`，0.3.0 的注册函数读取 `is_dirty` 因而失败。

0.3.1 移除注册阶段的场景访问，编辑会话只由工程加载或插件首次保存回调建立；首次启用和重新启用后需要重新打开作者工程。旧版失败可能留下部分注册状态，升级前应重启 Blender。新版 ZIP 的 17 项创作检查与 7 项独立导出检查均通过，原始回执位于 `build/addon-0.3.1/authoring/authoring-report.json` 和 `build/addon-0.3.1/export/report.json`。

新增 `verify_addon_install.py` 通过 Preferences 的真实安装/启用操作符测试 ZIP。相同脚本在 0.3.0 上重现错误，在 0.3.1 上完成 61 项检查：观察到 3 次 Blender 自身的 RestrictBlend，验证面板与操作符注册、三轮启停清理、脏工程不被采纳，以及重新加载后的会话/权威放置恢复和实际保存。测试使用独立的 Blender 用户目录；这是 headless 下的真实安装操作符测试，不是手工点击 UI 的记录。

[安装修复机器回执](../reviews/data/blender-addon-installation.json) 保存版本、ZIP SHA-256、检查结果与原始报告路径；复现命令见[工具链说明](../../toolchain/README.md#blender-插件)。

## 前台性能复测（2026-09-14，屏幕开启）

直接运行上述已导出的应用，没有重建应用或改动模型。Apple M3、接通电源、Metal Forward+、1920×1080，Quality 档（render scale 1.0、4× MSAA、SDFGI 与 SSIL 开启），daylight。两轮串行运行，性能采样期间不截图、没有并行 Blender 渲染；benchmark 解除常规的 60 FPS 上限。

| 测量 | 首轮 | 重复一轮 |
| --- | ---: | ---: |
| P50 帧耗时 | 9.424 ms | 9.423 ms |
| P95 帧耗时 | 11.328 ms | 11.356 ms |
| P99 帧耗时 | 11.982 ms | 12.124 ms |
| 峰值进程 RSS | 340.78 MiB | 368.66 MiB |
| 进程启动至区域首帧（外部观察） | 1.489 s | 0.845 s |
| 有焦点 / 失焦统计帧 | 3502 / 0 | 3488 / 0 |

两轮均满足严格前台条件（所有统计帧有焦点）、P95 ≤ 33 ms 和 RSS ≤ 2 GiB；各加载 3 个 GLB，无资产拒绝或脚本错误。额外的 12 s 空 PATH 独立运行复核也通过，截图和功能检查单独保存，不混入上表。验证脚本的前台有效条件已统一为失焦帧必须为 0。

[精简性能回执](../reviews/data/s1-git-foreground-performance.json) 保存原始数值、条件与门槛；完整报告和日志位于 `build/s1-git/foreground-20260914T165303Z/`，分别为 `tour-first.json`、`tour-repeat.json` 及同名 `.log`。两轮固定 commit 和 view root 均与应用内置版本一致。

测量边界：

- 帧统计从初始加载队列完成后再等待 2 s 开始；每轮运行约 36 s。默认导览每 8 s 在两个书签之间切换并淡入淡出，不是连续自由飞行。
- 两个书签复用 house/plaque/bench 三个 GLB，远处的 far_tree 未进入 48 m 加载范围；两轮 unload/cancel 均为 0。因此这里只验当前小型协作样例，不代表 V3 房屋、大型世界或持续流式装卸压力。
- 每轮是新进程，但没有清空操作系统文件缓存或驱动缓存，不称为严格冷启动。区域首帧指加载队列完成后的实际绘制；外部观察以约 0.2 s 轮询回执，包含进程启动时间，也不代表全局光照已经完全收敛。
- RSS 以约 0.2 s 间隔采样，仅表示进程驻留内存，不覆盖所有 GPU/驱动内存。既有房屋性能报告仍按各自原版本解释。

复测命令（在屏幕开启、应用保持前台时串行运行；每次更换报告文件名）：

```bash
.cache/s1-validation-venv/bin/python toolchain/build/benchmark_viewer.py \
  --executable "dist/viewer-git/macos/Realm of Us.app/Contents/MacOS/Realm of Us" \
  --report build/s1-git/foreground-20260914T165303Z/tour-first.json \
  --seconds 36

.cache/s1-validation-venv/bin/python toolchain/build/benchmark_viewer.py \
  --executable "dist/viewer-git/macos/Realm of Us.app/Contents/MacOS/Realm of Us" \
  --report build/s1-git/foreground-20260914T165303Z/tour-repeat.json \
  --seconds 36
```

## 审查 §3.1 修复回归（2026-09-14）

新增原生 Git 回归在修复前复现：不同 chunk 的实体移动仅在 view.json 冲突；同 chunk 新增不同实体同时在 view.json 和 chunk 页冲突。修复后完整 view/chunk 索引在读取时推导，Git 只保存逐 binding 构建记录和 runtime，两项合并均可直接校验和提交，无需合并后 build。

本次检查全部通过：Python 49 项（含旧 Git commit 可读、旧共享索引清理不改 index、浏览根摘要一致和 staged/worktree 隔离）；go vet、go test -race；Ruff/gofmt/gdformat。新增临时仓库测试还覆盖首次 add 前跨 chunk 连续 build。

真实 Blender/Git/LFS/Godot 运行完成 27 项流程检查、34 项 headless Godot 检查。Alice/Bob 分别编辑模型、构建并提交后，merge 返回成功，直接 rou commit 保留双亲；合并流程中无 build/add。固定版本浏览通过，Godot 无错误。完整回执为 [report.json](../../build/s1-git/verification/run-20260915T060838113440Z/report.json) 和 [godot-report.json](../../build/s1-git/verification/run-20260915T060838113440Z/godot-report.json)（目录名使用 UTC）。

本次运行使用隔离的本地 bare Git/LFS 远端，未操作托管平台网页；未重新打包插件或独立应用。前文 24 项流程及交付产物为修复前历史回执，保留原记录。

## 重现

```bash
.cache/s1-validation-venv/bin/python toolchain/build/check_code.py
go test -race ./...
go vet ./...
.cache/s1-validation-venv/bin/python -m unittest discover -s toolchain/tests -v
.cache/s1-validation-venv/bin/python toolchain/build/verify_spatial.py
.cache/s1-validation-venv/bin/python toolchain/build/verify_git_realm.py --windowed
```

插件 ZIP 与独立应用重现见[工具链](../../toolchain/README.md)，数据字段见[规范](../protocol/spec/s1-repository.zh-CN.md)。

## 实际边界

- Git 作者配置和标准远端凭据复用，尚无 S2 release 签名、P2P、节点身份/管理或 pin。
- Git clean、元数据一致、真实载荷已检查分别报告。未变化父版本的 runtime 可未下载，validate 明确 byte_validation_complete:false；新输出和实际浏览的 GLB 必须验字节。
- LFS 是整文件去重。共享文件可被多绑定引用，但完整模型/实例编辑、跨格式贴图去重、视觉 diff 和二进制自动合并未实现。
- 当前使用完整轻量 Git 历史/元数据与 LFS 按需载荷；为验证不可变规则谱系，拒绝 shallow 历史，并忽略本地 replace 对象。尚未做大型元数据 partial clone 基准。
- instance 根只支持平移；object/Collection 入口坐标规则明确。不支持的动态数据/外部依赖应拒绝。
- 插件守卫它自己的写入，不能拦截所有外部程序和 Blender 原生保存。最终 build/index/commit 校验仍是磁盘一致性门槛。
- Git、LFS 网络和工作树操作不是一个事务；错误保留现场，禁止隐藏强制回退。
- 独立浏览包为本地可信桥接，不是可从任意公网来源信任的签名。完整 metadata 可带部分模型载荷，缺模型不当空块。
- 本轮只验当前 Mac。其他 OS、Godot Editor 放置、公开资产隔离和网络压力测试后续推进。

原作者工程、房屋/离线渲染与历史报告保留；旧验收数字不并入本轮结果。
