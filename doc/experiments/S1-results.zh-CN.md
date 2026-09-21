# S1.1 / S1.2 本地实验结果

日期：2026-09-08。已完成空间参数实验和 Blender → GLB → 独立 Godot 的本地验证闭环。S1.3 仓库引擎、签名/P2P、RealmHub 不在这次实现范围内。房源模型是风格化的近似重建，详见 [房源说明](walsh-study.zh-CN.md)。

**更新：2026-09-09，S1.1 收尾完成，本地空间契约已冻结。** 本文原有性能与 723 项断言为 2026-09-08 的历史测量；本次新增验证单独记录如下，没有重新测量渲染性能。

2026-09-10：源码目录与验证入口已整理，本文命令路径同步更新，历史数字和回执保持原样。最新代码结构与回归结果见 工程整理记录。

2026-09-12：S1.2 新增通用打开/重载和可编辑源交接，最新功能结果与性能条件见 [S1.2 收尾记录](S1.2-results.zh-CN.md)。下方性能数字仍属于各自的历史版本。

## S1.1 收尾验收（2026-09-09）

| 收尾项 | 结果与证据 |
| --- | --- |
| 冻结默认值与允许范围 | 默认 256×128×256 m、32 m 块、毫米放置；允许各轴独立块长 1..64000 mm、总块数 ≤ 32768、限定整数/世界端点范围。保留已有格式/profile ID，完整规则见 [空间规范](../protocol/spec/s1-preview.zh-CN.md) |
| Schema 与实现一致性 | 修正 Schema 的零/负数和超大块长；字段约束由标准 JSON Schema 校验器验证，跨字段约束由 Python/Godot 检查；整数值的 JSON 数字表示与规范化一致 |
| 共同向量 | 33 个参数案例、17 个位置案例、13 个包围盒案例、14 个目录案例，共 77 个；覆盖尺寸上下限、偏移原点、长方体分块、负坐标、半开边界、覆盖预算、稳定 ID、共享资产与拒绝规则 |
| 空 / 未知范围 | 完整有效目录内缺项为已知空块，允许全空世界；不完整、缺失完整性标记或伪造空间索引均拒绝。部分目录三态浏览明确留给 S1.3 |
| 冻结预期结果 | 验证脚本只读 `spatial.json`，不再由待测 Python 实现重新生成答案；默认参数 hash 未变 |
| 文档状态 | 中英文产品/架构、RD-03、S1-A02 分阶段范围及实施计划同步；房源美术不作为 S1.1 退出条件 |

本机 Python 3.14.3 / jsonschema 4.26.0 / Godot 4.7.2：**12 个 Python 测试组通过，无跳过；空间契约 110 项 Godot 断言通过；已有资产集成 706 项 Godot 断言通过。** 原集成脚本中的 17 项位置检查已由独立空间套件替代，不能把断言数量减少理解为删掉边界覆盖。模型集成仍检查原 CC0 街区与 S1 房源 GLB；资源生命周期在当前默认 v3 场景验证取消 55 个排队项、释放 55 个实体，以及篡改资产在原生导入前被拒。预期日志仍有一次 `Asset hash mismatch`。

本次重新计算的 16/32/64 m 分块统计与下方历史表一致。没有重建房屋或重新测 GPU 性能；后台 Cycles 渲染不参与 S1.1 性能结论。机器可读的验证版本、范围、案例数与源码 hash 见 [S1.1-closeout.json](data/S1.1-closeout.json)，本地完整日志位于 `build/s1/spatial-checks/`、`build/s1/checks/` 与 `build/s1/closeout-integration.log`。

关闭的是 **S1.1 / S1-A02 的本地完整目录范围**。`profile_hash` 仅校验目录内部参数一致性；签名创世身份、历史不可变性、部分元数据 clone、仓库贡献及网络协议仍由 S1.3/S2 实现。格式允许不同 entity 共享资产引用，不表示导出模板复用、GPU 资源共享和模板更新传播已经实现。

可独立复现空间验收，无需生成 Blender 场景：

```sh
python3 -m venv .cache/s1-validation-venv
.cache/s1-validation-venv/bin/python -m pip install -r toolchain/tests/requirements.txt
.cache/s1-validation-venv/bin/python toolchain/build/verify_spatial.py
```

## 实现与决定

- 空间 schema、毫米放置、负坐标 floor、半开边界、完整跨块索引、稳定 entity ID 和共同向量已实现。
- 默认采用 256 × 128 × 256 m 有界笛卡尔世界、32 m 块、Y-up/右手/米制。16/64 m 保留对比样例。固定的是 S1 空间约定；签名创世对象及 realm/repo 身份仍待仓库/协议阶段实现。
- Blender 插件支持加载参数、显示网格、标记 entity、导出 GLB/源 Collection、追加源工程。几何、纹理内嵌；观看不携带 `.blend`。完整场景和逐实体创作源均已生成。
- Godot 按块查询、按 ID 去重、逐帧导入、过期请求取消、离开后释放，提供导览和自由飞行。房源应用有 8 个视点、日光切换、场地示意和说明面板。
- 本地格式 `rou-preview/0.1` 和 S2 `rou-realm/0.1-draft` 分离；没有加入资产加密、账号、签名或远程服务依赖。

## S1.1 分块比较

同一原创 CC0 街区、50 个实体、相同 48 m 查询半径和相机轨迹。数字按最终目录重新计算；仅空间参数不同。

| 块边长 | 非空块 | entity 引用数 | 入口需加载 GLB 字节 | 东侧视点需加载字节 |
| --- | ---: | ---: | ---: | ---: |
| 16 m | 142 | 251 | 247,776 | 74,684 |
| 32 m | 43 | 126 | 345,688 | 98,028 |
| 64 m | 16 | 94 | 436,776 | 217,152 |

默认选 32 m：相较 16 m，跨块目录引用约减半；相较 64 m，东侧视点取得的模型约少 55%。这只是此样例下的折中，不说明所有世界都应选 32 m。原始计算见 [grid-comparison.json](data/grid-comparison.json)。

## S1.2 实际运行

硬件：MacBook Air / Apple M3（8 CPU、10 GPU）、24 GiB 统一内存、macOS 26.3。Blender 5.2.1 LTS、Godot 4.7.2，Metal Forward+，实际渲染截图尺寸 1920 × 1080。测试在普通桌面会话进行，未隔离其他进程；首个分块测试期间还有后台建模工作。它不是 16 GiB 设备的测试。

相机每 8 s 切换入口，过渡 2.3 s。帧统计排除首次区域加载及其后 2 s，必须结合冷启动数据阅读；RSS 每 0.2 s 采样该进程，不将 Godot 的静态内存计数冒充进程总内存。

| 场景 | 实际运行 | 帧时间 P95 | 峰值 RSS | 导入 / 释放实体次数 |
| --- | --- | ---: | ---: | ---: |
| CC0 街区，16 m | 30 s 轨迹 | 6.90 ms | 368.9 MiB | 63 / 62 |
| CC0 街区，32 m | 30 s 轨迹 | 7.15 ms | 410.7 MiB | 66 / 65 |
| CC0 街区，64 m | 30 s 轨迹 | 7.19 ms | 470.0 MiB | 66 / 59 |
| Villa Anahid | macOS 独立应用，加载后 66 s | **17.73 ms** | **305.6 MiB** | 36 / 0 |

房源应用最终测试 P50 7.66 ms、P99 26.34 ms，首次区域就绪 0.77 s（着色器缓存已热）。此前一个构建的第一次独立运行需要约 **17 s** 才就绪，结合首次 Metal 运行的编译停顿，推测主要受着色器缓存初始化影响。冷启动等待仍是待优化问题，不能把 0.77 s 当作首次安装体验。

房源有 36 个 entity，完整观看闭包 **5,047,324 bytes / 4.81 MiB**，最大单 GLB **1,101,948 bytes / 1.05 MiB**。通过移除树木重复携带的木纹图、缩小木纹贴图，把完整运行资产从约 7.2 MiB 降至 4.81 MiB。满足本机的初始 5 MiB 入口目标、8 MiB 单 GLB 上限、33 ms 帧时间和 2 GiB RSS 目标；尚不能推广为跨硬件性能保证。

汇总与构建 hash 见 [S1-measurements.json](data/S1-measurements.json)。完整运行报告在 `build/s1/measurements/` 和 `build/walsh/measurements/`；视口截图在 `build/walsh/*-final.png`。

## 验证证据

- Python 8 个测试组通过：空间边界、参数、移动保留 ID/资产、目录篡改、JSON 歧义、GLB 反例与真实 Blender 资产。
- Godot **723 项断言**通过：共同向量、profile hash、13 个 GLB 正反例、两个真实目录及 86 个实体原生导入后的世界 AABB（容差 3 mm），覆盖方向/尺寸/坐标只转换一次。
- 资源生命周期测试通过：相机立即离开时取消 **36** 个排队项；加载后移走，**36** 个实体已从场景树实际释放；篡改字节在原生导入前被拒。错误日志中预期出现一次 `Asset hash mismatch`。
- 自动化真实指针事件可选择泳池视点；打包应用内实际观察并验证了键盘视点切换。室外、室内及泳池截图经过检查。
- 插件类注册、网格开关和单实体源 Collection 追加回读通过 headless Blender 操作验证。未把这项写成插件安装界面已经人工逐项验收。
- 使用 macOS `sandbox-exec` 对应用进程施加 `(deny network*)`，在 `/tmp` 工作目录打开随应用携带的目录；36 个 GLB 全部导入，拒绝数为零。没有修改用户系统的网络设置。
- macOS universal、Windows x86_64、Linux x86_64 已导出。**仅 macOS 实际运行验证**；另外两个平台仍需真机运行、驱动与输入测试。macOS 为本地 ad-hoc 签名，未做商店发行或公证。
- `doc/old-doc` 的 5 个文件校验和保持不变。

对应验收：S1-A01/A05 和 S1-A02 的本地完整目录范围已验证；S1-A02 的部分目录浏览与 S1-A03/A04/A06 属于后续仓库阶段。S2 网络及不可信资产隔离仍未实现。

## 复现与边界

从项目根目录运行：

```sh
python3 -m venv .cache/s1-validation-venv
. .cache/s1-validation-venv/bin/activate
python -m pip install -r toolchain/tests/requirements.txt
blender --background --factory-startup --python examples/calibration/create_scene.py
blender --background --factory-startup --python examples/villa_anahid/architecture.py
blender --background --factory-startup --python toolchain/tests/blender/verify_addon.py
python3 -m unittest discover -s toolchain/tests -v
python3 toolchain/build/verify_s1.py
python3 toolchain/build/fetch_templates.py
python3 toolchain/build/package_viewer.py --platform all --realm build/walsh/preview/realm.json --output-dir dist/walsh --name "Villa Anahid"
python3 toolchain/build/benchmark_viewer.py --fixture s1 --seconds 30
python3 toolchain/build/benchmark_viewer.py --fixture walsh --seconds 66
```

模型生成每次需要几分钟，导出报告的 `export_seconds` 仅衡量导出，不包含建模。模板来自 Godot 官方 release；GLB 预检查不是原生解析器沙箱。当前没有碰撞/重力、照片级材质、LOD、后台解码、无限世界、历史版本或分布式同步。空间 profile、格式边界与资源限制见 [S1 preview 规范](../protocol/spec/s1-preview.zh-CN.md)。
