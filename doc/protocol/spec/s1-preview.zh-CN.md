# 空间 profile 与浏览包格式

版本：2026-09-16（2.1）。本文定义两件事：所有 realm 共用的**空间 profile**，以及 `rou view` 或 Blender 本地导出产生、Godot 查看器消费的**浏览包**（`rou-preview/0.1` 与 `rou-git-view/0.1`）。空间规则自 2026-09-09 冻结以来未变，`bounded-cartesian-v1-experiment` 标识与既有整数参数继续使用，后缀 `experiment` 只是历史命名。仓库布局、模型/实体/构建记录、Git 语义见 [Git realm 规范](s1-repository.zh-CN.md)。本文不定义 S2 的签名、公钥、网络鉴权或 P2P；资产不加密；本地 SHA-256 校验不是作者身份验证。

## 1. 空间约定

默认 `bounded-cartesian-v1-experiment`：256 × 128 × 256 m 有界长方体，原点位于中心，右手坐标、Y 向上、米制显示、整数毫米放置，默认块边长 32 m。参数在一个 realm 中不可修改；不同参数得到不同 `profile_hash`，不是同一世界的普通更新。冻结的是默认值及允许范围，不要求所有 realm 大小相同：

| 参数 | 默认值 / 允许范围 |
| --- | --- |
| `id`、`units`、`up`、`handedness` | 固定为 `bounded-cartesian-v1-experiment`、`meter`、`Y`、`right` |
| `placement_unit_mm` | 固定为 `1`；仅约束场景放置，不要求模型顶点吸附毫米网格 |
| `origin_mm` | 默认 `[0,0,0]`；三轴整数，各分量在 `[-1000000000,1000000000]` mm 内 |
| `chunk_size_mm` | 默认 `[32000,32000,32000]`；各轴独立取整数 `1..64000` mm，同一 realm 内固定；块沿世界轴对齐，可为长方体 |
| `min_chunk` / `max_chunk` | 默认 `[-4,-2,-4]` / `[4,2,4]`；各分量在 `[-1000000000,1000000000]` 内，每轴 `min < max` |
| 总块数 | 三轴 `(max_chunk - min_chunk)` 的乘积不超过 `32768`，含空块 |
| 世界范围 | 每轴两端 `origin_mm + bound_chunk * chunk_size_mm` 均在 `[-1000000000,1000000000]` mm 内 |

形状层通过核心库的 serde 类型检查字段、类型、常量和单字段范围；语义层还必须检查 `min < max`、总块数、计算后的世界范围等跨字段约束。仅能解析进类型不代表参数有效。JSON 整数按数值判断：`1`、`1.0`、`1e0` 等价，布尔值、数字字符串、非整数和非有限数值不能用作整数参数；空间整数的规范编码使用十进制整数，`-0.0` 归一为 `0`。

各轴：`chunk = floor((position_mm - origin_mm) / chunk_size_mm)`，`offset = position_mm - origin_mm - chunk * chunk_size_mm`。块与世界都是 `[min,max)`；例如 X = -1 mm 的归属为块 -1、偏移 31999 mm；X = 32000 mm 属于块 1。世界最大边界不属于世界。AABB 的最大值不包含在内，覆盖块范围使用 `max_mm - 1`。边界为整数毫米；导出时对退化轴外扩 1 mm，保证每轴正厚度。

entity 是一个有稳定 ID 和唯一放置位置的场景对象，它引用一个模型的导出 GLB。归属 chunk 由实体放置原点决定，覆盖 chunk 由世界坐标 AABB 决定；后者是保守包围盒查询，不是逐三角形求交。跨块 AABB 在所有相交块内记录同一 ID；查看器按 ID 去重。移动保留 ID、重新计算 position/owner/offset/AABB 和索引。分块不裁剪模型，也不等于 P2P byte block。同一模型摆放多次使用不同 entity ID，引用同一 GLB 哈希；一条 entity 记录不能表示多个位置。实体根只有平移，旋转和缩放放在子对象上。

首个空间 profile 不支持球面、环面、环绕、无界扩展、非规则/逐块不同网格、运行中调整世界参数和超预算覆盖。允许的参数数值范围不等于已验证相应极端坐标下的渲染画质。变更冻结约束须同时更新规范、测试用例与核心库，不可只改单侧。

## 2. 完整索引与空块

`index_complete` 必须是布尔值 `true`。通过完整索引校验后，世界范围内没有索引项的块是已知空块；整个世界允许 `entities: []`、`chunks: {}`。已有实体的覆盖块不得漏记，索引必须与实体 AABB 完全一致，不能通过删掉索引项把有内容的块变成空块。`index_complete: false`、字段缺失或非布尔值一律拒绝，不能把缺失索引显示为已知空块。

浏览包总是携带完整元数据；`rou view --chunk` 只影响哪些 GLB 被包含，未包含的模型显示为缺失，不当作空块。

`profile_hash` 是 profile 的 RFC 8785 规范 JSON 的 SHA-256，绑定当前目录里的参数；参数改动却保留旧 hash 会被拒绝。它不表示世界身份；世界谱系由 [Git realm 规范](s1-repository.zh-CN.md) §2 的祖先检查落实。

## 3. 浏览包

浏览包内容只允许以下三项；根目录与 `objects/` 内允许忽略 Finder 生成的普通 `.DS_Store` 文件，该文件不属于浏览包内容，不参与校验或摘要。其他文件、目录及包根或包内的符号链接拒绝；包根之外的祖先目录可以是符号链接，例如 macOS 的 `/tmp`、`/var`。

| 路径 | 格式 | 内容 |
| --- | --- | --- |
| `realm.json` | `rou-preview/0.1` | 运行时目录：profile 与 hash、全部实体的世界位置/包围盒/资产描述、完整 chunk 索引 |
| `version.json` | `rou-git-view/0.1` | 固定版本侧车：realm ID、Git 对象算法与 commit、浏览根哈希、目录字节哈希、校验等级 |
| `objects/<sha256>.glb` | glTF 2.0 二进制 | 不可变的观看资产，文件名是实际字节的 SHA-256 |

`realm.json` 由核心库的 `projections` 生成并按实体 ID、chunk 键排序，确定性输出：

```json
{
  "format": "rou-preview/0.1",
  "profile": {"id": "bounded-cartesian-v1-experiment", "units": "meter", "up": "Y", "handedness": "right",
              "placement_unit_mm": 1, "origin_mm": [0, 0, 0], "chunk_size_mm": [32000, 32000, 32000],
              "min_chunk": [-4, -2, -4], "max_chunk": [4, 2, 4]},
  "profile_hash": "c5ea37e2ce31d6894d505e45475b8791c530767b8bd7d81637bc16e5df4a1f97",
  "index_complete": true,
  "chunks": {"0,0,0": ["house", "plaque"], "1,0,0": ["bench"]},
  "entities": [
    {
      "id": "house",
      "label": "Courtyard house",
      "position_mm": [8000, 0, 8000],
      "owner_chunk": [0, 0, 0],
      "offset_mm": [8000, 0, 8000],
      "bounds_mm": {"min": [3500, 0, 4500], "max": [12500, 4400, 11500]},
      "asset": {"path": "objects/ce42…2ec7.glb", "sha256": "ce42…2ec7", "bytes": 5620}
    }
  ]
}
```

浏览包不含观看入口。`asset.path` 固定为 `objects/<sha256>.glb`，`sha256` 与文件名相同，`bytes` 为实际长度。GLB 保存模型局部坐标，`position_mm` 提供世界平移；Blender 源使用 Z-up，映射到 Godot 为 `(x,z,-y)`，只在导出时转换一次。

`version.json` 由 `rou view` 写入：

```json
{
  "format": "rou-git-view/0.1",
  "realm_id": "realm_45cde0e62fc8431ca8d627d9dc73bce6",
  "git_object_format": "sha1",
  "commit_oid": "bd52c15f3ceb0ae73a5b9bb260b0b7d2c5759547",
  "view_root_sha256": "054f40ae945efde072983368431293b37eb6e3608692a63e6bc0cac5115fa061",
  "catalog_sha256": "…",
  "validation": "local-trusted"
}
```

`view_root_sha256` 是内存 view（`rou-view/0.1`）的 RFC 8785 规范 JSON 的 SHA-256，供 S2 发布记录绑定；`catalog_sha256` 是 `realm.json` **实际文件字节**的 SHA-256，查看器据此确认目录未被改写；`--worktree` 导出的草稿 `commit_oid` 为空字符串；`validation` 只有 `local-trusted` 一个值，表示本地可信桥接，不是公开签名。Blender 插件的本地导出只产生 `realm.json` 与 `objects/`，没有 `version.json`，查看器必须显示为未绑定版本，不能静默当作固定版本。

查看器通过 Rust 扩展做全部校验：目录形状与预算、`profile_hash`、`version.json` 与目录字节的绑定、每个 GLB 的长度与 SHA-256、GLB 预检；不读 .git，不运行 Git/LFS/Blender。规范 JSON 编码统一采用 RFC 8785，核心库是唯一实现。

## 4. 预算

| 对象 | 上限 |
| --- | --- |
| `realm.json` | ≤ 4 MiB、≤ 4096 实体、≤ 32768 空间块；单实体覆盖 ≤ 2048 块 |
| 单个 GLB | ≤ 8 MiB、≤ 100000 顶点、≤ 300000 索引、≤ 512 节点、顶层数组 ≤ 8192 项；静态三角网格、glTF 2.0；内嵌 PNG/JPEG ≤ 2048²，解码像素预算 64 MiB |
| GLB 拒绝项 | 外部/data URI、扩展、动画、蒙皮、相机、sparse accessor |
| 严格 JSON | 拒绝重复键、非有限数与溢出、非 UTF-8 编码或 BOM、嵌套深度超过 40、超出大小预算 |

所有预算只在核心库定义一次，`rou validate` 与查看器执行同一实现，不存在能提交却不能浏览的数据。加载先检查目录与索引，再检查 GLB 长度和 SHA-256、JSON/缓冲区边界、几何及图片预算，然后交给 Godot 原生 glTF 导入器。预检查不等于完整 glTF 规范验证，也不构成对原生解析器的安全沙箱；S2 接入不可信网络资产前还需隔离与模糊测试。

## 5. 查看器起点与加载行为

起点不来自 realm 数据：默认起点是全部实体世界包围盒并集的中心上方，距离取并集对角线长度，朝向中心；世界为空时在原点上方 2 m 朝向原点。玩家可在进入时或运行中通过分块地图、实体列表或坐标输入选择地点；最近位置按 realm ID 记在本机用户目录，不写入浏览包；命令行 `--at=x,y,z`（米）可直接指定起点。

以相机为中心、48 m 距离查询块 AABB；块内 entity 去重，每帧导入一个 GLB。每 0.35 s 更新所需集合，取消尚未导入的过期队列项，释放离开的实体。同 sha 的 GLB 只读取、校验、导入一次。当前没有 LOD、磁盘缓存淘汰、后台解码或淡入；同步导入可能带来帧尖峰。`F1` 显示诊断，`F2` 显示地面分块网格。浏览包可重复生成；需要保留某次浏览时复制整个目录。

## 6. 测试用例与验证

`rou/crates/rou-core/tests/fixtures/spatial.json` 保存经过审阅的空间与目录测试用例，只读，不能用待测实现重新生成预期值。`profiles` 用例的 `shape_accepted` 指能否解析进 profile 类型，`accepted` 指两层都通过；`positions` 覆盖归属/偏移；`bounds` 覆盖半开 AABB；`catalogs` 覆盖跨块、移动、共享资产、空目录和未知目录拒绝。目录用例只验证元数据，占位资产描述不是真实 GLB。默认 profile 与 `profile_hash` 在 RFC 8785 下不变，因为 profile 只含整数和 ASCII 字符串。`catalogs` 已按本文 §3 迁移为 2.1 浏览包形状，不含观看入口；审阅预期值保持原样。

2.1 验证：

```sh
python3 -m venv .cache/s1-validation-venv
.cache/s1-validation-venv/bin/python -m pip install -r requirements-dev.txt
.cache/s1-validation-venv/bin/python toolchain/build/verify_spatial.py
```

它构建本机扩展，运行 Rust 测试与 headless Godot ABI 检查，输出 `build/s1-2.1/spatial-checks/report.json`。空间冻结的历史测量见 [S1 实验报告](../../experiments/S1-results.zh-CN.md)。
