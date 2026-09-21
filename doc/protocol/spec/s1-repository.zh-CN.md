# S1 Git realm · 本地创作契约

版本：2026-09-16。此规范是 2.1 Rust 实现的目标契约，替代自研 rou repository/commit/state/ref/workspace 格式；无转换器、兼容 reader 或双后端。Git 是唯一历史和暂存权威。公开签名、P2P 和节点授权由 S2 另行定义。与 2.0 参考实现的差异：目录与字段改名（authoring→models、binding→model、derived/builds→runtime/<model>.json）、去掉兼容层、哈希改用 RFC 8785；2.0 参考实现已删除，迁移过程见 [架构](../../Realm_of_Us_Architecture.zh-CN.md) §10。

## 1. 数据目录与格式

| 路径 | 格式/作用 |
| --- | --- |
| realm.json | rou-realm/0.1；固定 realm ID 与 profile，init 后不变 |
| realm/models/<id>.json | rou-model/0.1；模型定义：源 .blend 路径、入口对象或 Collection、依赖 |
| realm/entities/<id>.json | rou-entity/0.1；放置：稳定 ID、model、整数毫米位置 |
| runtime/<model>.glb | 标准 LFS 跟踪的导出模型 |
| runtime/<model>.json | rou-build/0.1；该 GLB 的构建记录：输入哈希、input_digest、GLB 哈希与长度、局部包围盒、工具版本 |
| sources/ 或其他安全作者目录 | 完整原工程、共享库、贴图；可读真实路径 |
| .git/ | 原生 tree/index/commit/refs/reflog、LFS 缓存 |
| .rou/ | 被 .gitignore 忽略的 selection/session/build/view/backups/cache；无权威提交历史 |

sources 是作者的完整工程；realm/models 说明从哪个文件导出什么；realm/entities 说明放在哪里；runtime 是 build 的全部输出，每个模型恰好一个 GLB 加一份同名记录。作者只写 realm/ 与 sources/，rou 只写 runtime/。每个 ID 一个文件，是并行贡献能由 Git 直接合并的前提；realm.json 在 init 之后不再改变，仓库里没有任何多人共同修改的单个文件。

版本化 JSON 在磁盘上按固定排版写出：键排序、两空格缩进、数组元素各占一行、UTF-8 无 BOM、末尾一个换行。rou 与插件都用这一排版，让同一文件不同字段的修改可以由 Git 按行合并，diff 可读。哈希与摘要对解析后的值做 RFC 8785 编码，与排版无关。

工程目录统一 realm/。根无 work/staging/profile.json。普通文本、README、脚本按原生 Git 文件树管理。原工程与导出模型均用标准 LFS pointer，载荷以 SHA-256 和实际长度标识；Git OID 单独记录算法，不限定为 64 字符。

完整浏览根和 chunk 索引从上述版本化记录确定性推导，不写入或提交任何共享索引文件。需要落盘的浏览包位于 `.rou/view/` 或显式输出目录。

### 格式标识

每个 JSON 文档的 `format` 字段是它的格式标识，形如 `名称/主版本.次版本`。读取方只接受自己认识的标识，遇到其他值一律拒绝，不做猜测式兼容；预研阶段任何不兼容改动都递增版本并重新生成数据。全部标识只在核心库的 `formats` 常量模块定义一次，各宿主引用同一处。2.1 的完整清单：

| 标识 | 文档 | 写入方 | 读取方 |
| --- | --- | --- | --- |
| `rou-realm/0.1` | `realm.json` | rou init、作者 | rou、插件 |
| `rou-model/0.1` | `realm/models/<id>.json` | 作者、插件 | rou |
| `rou-entity/0.1` | `realm/entities/<id>.json` | 作者、插件 | rou |
| `rou-build/0.1` | `runtime/<model>.json` | rou build | rou |
| `rou-blender-export/0.1` | `.rou/build/` 内 Blender 导出 helper 的 response.json；同时是记录里 `tool_versions.exporter` 的值，参与 input_digest | 导出 helper | rou build |
| `rou-view/0.1` | 内存中的完整 view，即 `projections` 的输出，浏览根哈希的对象；不落盘 | rou | rou |
| `rou-preview/0.1` | 浏览包 `realm.json` | rou view、插件本地导出 | Godot 扩展 |
| `rou-git-view/0.1` | 浏览包 `version.json` | rou view | Godot 扩展 |

`.rou/` 内的 selection、session、缓存等本地文件不是版本化文档，不需要格式标识。2.0 的 `rou-authoring/0.1`、`rou-chunk/0.1`、`rou-blender-build/0.1`、`rou-blender-git/0.1` 在 2.1 不存在。

## 2. 作者输入

```json
{
  "format": "rou-model/0.1",
  "id": "house",
  "source": "sources/houses/courtyard.blend",
  "entry": {"kind": "object", "name": "house"},
  "dependencies": ["sources/shared/ornament.blend", "sources/shared/checker.png"],
  "settings": {}
}
```

```json
{
  "format": "rou-entity/0.1",
  "id": "house",
  "model": "house",
  "position_mm": [8000, 0, 8000],
  "label": "Courtyard house"
}
```

模型定义字段，不允许其他字段：

| 字段 | 约束 |
| --- | --- |
| `format` | 常量 `rou-model/0.1` |
| `id` | 稳定 ID，等于文件名，见下节 |
| `source` | 仓库内安全相对路径，以 `.blend` 结尾，不得位于 `runtime/` |
| `entry.kind` | `object` 或 `collection` |
| `entry.name` | 1–256 字符 |
| `dependencies` | 字符串数组，≤ 4096 项，无重复，每项为安全相对路径，不含 `source` 自身，不得位于 `runtime/` |
| `settings` | 必须是空对象 |

实体字段，不允许其他字段：

| 字段 | 约束 |
| --- | --- |
| `format` | 常量 `rou-entity/0.1` |
| `id` | 稳定 ID，等于文件名 |
| `model` | 模型 ID，必须存在 |
| `position_mm` | 3 个整数，毫米，须在世界范围内 |
| `label` | 可选，≤ 256 字符 |

### 标识符与文件名

稳定 ID 的语法是 `[a-z0-9_-]{1,80}`：只允许小写 ASCII 字母、数字、下划线、连字符，避免大小写不敏感文件系统上的冲突。三个命名空间互相独立：

| 标识符 | 出现位置 | 规则 |
| --- | --- | --- |
| realm ID | `realm.json` 的 `id` | `rou init` 生成 `realm_` 加 32 位十六进制；clone 与 fork 保持不变，属于规则谱系 |
| 模型 ID | `realm/models/<id>.json` 的文件名与内容 `id`；实体文件的 `model` 字段；`runtime/<id>.glb`、`runtime/<id>.json` 的文件名与记录的 `model` 字段 | 模型命名空间内唯一；每个模型恰好一个 GLB 与一份记录，文件名等于模型 ID |
| 实体 ID | `realm/entities/<id>.json` 的文件名与内容 `id`；view 与浏览包中实体的 `id` | 实体命名空间内唯一；与模型 ID 可以同名，例如模型 `house` 与放置它的实体 `house` |

文件名必须等于内容里的 `id`，不一致拒绝；同一命名空间内两个文件同一 ID 拒绝，即使 Git 合并没有冲突。ID 不可变：改 ID 等于删除旧记录并新建，移动实体只改 `position_mm` 并保持 ID。实体的 `model` 必须指向存在的模型；删除模型前必须先删除或改指引用它的实体，validate 拒绝悬空引用。`label` 是可选显示名，最长 256 字符，不参与任何引用。

### 入口与依赖

入口 kind 为 object 或 collection。object 是无父、单位旋转/缩放的根，导出时根平移归零；Collection 无独立变换，其内部坐标直接定义模型坐标系，不臆造统一 pivot。

模型定义是源路径、入口、依赖的唯一权威。同一源内的 `(entry.kind, entry.name)` 必须唯一；多个实体应引用同一模型实现实例化，不重复定义相同入口。Blender 登记按持久化的作者实体 ID 找到当前根对象并同步改名，实例对象的独立名字不改变共享模型入口；绑定保存在源 .blend 中，重建会话和撤销后不依靠旧名字重绑。dependencies 明确列出静态导出闭包（含间接库/图片），路径保持仓库内相对关系；缺失、逃逸、符号链接和未声明外部依赖拒绝。Blender 以 `//` 开头的路径是相对于所属 .blend 的相对路径，不是绝对路径。settings 当前只接受空对象，不静默忽略未知设置。未支持的动画/缓存/动态依赖不能被当作有效静态资产，检查覆盖入口下全部对象而不只是根。

realm.json 字段只有 format/id/profile，init 之后不再改变；profile 规则见空间规范。realm 的 format/id/profile 与全部可达 realm 祖先保持一致；不接受通过先提交非法父版本再提交不变子版本绕过规则。S1 需要完整 Git 元数据历史，拒绝 shallow 历史，忽略本地 replace 对象。实体当前只支持平移。最多 4096 entity/model；一个模型可以被多个 entity 引用，插件写摆放时按实体 ID 而非对象名定位。

已登记作者根的名字必须与模型入口一致，改名后先登记再保存。插件完整保存及“仅保存源”在写文件前检查；构建导出器也根据冻结快照的实体关联检查源内持久化作者身份，拒绝原生保存后遗留的名字漂移或作者根缺失，不按旧名导出其他对象。

## 3. 构建记录与空间结果

build 默认基于已保存工作树；--staged 以 index 为输入。逻辑树只对明确 skip-worktree 的缺席路径读取 index，真正删除要报错或作为删除处理。合并使用已解决 index，绝不从旧 HEAD 补回旧实体。

构建在 .rou/build 的新目录冻结相关字节，在该副本运行 headless Blender；共用同一 .blend 的模型一次导出。每个模型写 `runtime/<model>.glb` 与 `runtime/<model>.json`：

```json
{
  "format": "rou-build/0.1",
  "model": "house",
  "inputs": {
    "sources/houses/courtyard.blend": {"sha256": "fa71…4a88", "bytes": 91180},
    "sources/shared/checker.png": {"sha256": "ce30…401d", "bytes": 300},
    "sources/shared/ornament.blend": {"sha256": "e5e8…591d", "bytes": 88969}
  },
  "input_digest": "31e6…ba56",
  "asset": {"path": "runtime/house.glb", "sha256": "ce42…2ec7", "bytes": 5620},
  "local_bounds_mm": {"min": [-4500, 0, -3500], "max": [4500, 4400, 3500]},
  "tool_versions": {"blender": "5.2.1 LTS", "exporter": "rou-blender-export/0.1"}
}
```

构建记录字段，不允许其他字段：

| 字段 | 约束 |
| --- | --- |
| `format` | 常量 `rou-build/0.1` |
| `model` | 模型 ID，等于文件名 |
| `inputs` | 对象，键为 `source` 与每个依赖的相对路径，键集合必须恰好等于模型定义的 `source` 加 `dependencies`；值为 `{sha256, bytes}`，sha256 为 64 位小写十六进制，bytes 为非负整数 |
| `input_digest` | 64 位十六进制：对 `{exporter, profile, model, inputs}` 组成的对象做 RFC 8785 编码后的 SHA-256，其中 exporter 为 `rou-blender-export/0.1`，model 为完整模型定义 |
| `asset.path` | 固定 `runtime/<model>.glb` |
| `asset.sha256`、`asset.bytes` | GLB 实际字节的 SHA-256 与长度；长度 ≥ 20 且不超过浏览包规范的单 GLB 预算 8 MiB |
| `local_bounds_mm` | `min`、`max` 各 3 个整数，每轴 `min < max` |
| `tool_versions` | 可选，字符串到字符串的映射，含 `exporter` 与 Blender 版本；不参与 `input_digest` |

摘要包含规则、完整模型定义和输入字节描述；世界位置另纳入派生实体与完整索引，不反向进入几何构建循环。

GLB 是本地模型。派生 entity 根据 position_mm 加局部 AABB 得世界 AABB，计算 owner/offset 与所有相交 chunk。`projections` 在内存生成完整 view（rou-view/0.1）：实体的世界位置、包围盒、owner/offset、资产描述，以及 chunk 到实体 ID 的完整索引；没有独立的 chunk 页对象。空块由完整索引缺少该 key 表示，不用“下载失败”冒充空块。局部 AABB 允许某轴退化，导出时对退化轴外扩 1 mm，保证每轴正厚度。

view 不包含当前 Git commit。`view_root_sha256 = SHA-256(JCS(view))`，JCS 为 RFC 8785 规范 JSON。导出的浏览目录 catalog_sha256 和 GLB SHA-256 仍以**实际文件字节**为准。构建内部 input_digest 同样对 JCS 编码计算；构建输入不能包含该次输出摘要。

build 只写 runtime/，不自动 add。新输出未暂存时是 untracked，已跟踪输出改变是 modified。模型定义被删除后，runtime 内作废的 GLB 与记录由 build 直接删除并报告，无论是否已跟踪。同一模型的记录/GLB 冲突可以从已解决输入重建，不要求先对整个 unmerged index write-tree。源冲突必须先解决。构建失败不推进 Git HEAD。

sparse 工作区中，未选中 GLB 被删改、已跟踪构建记录被普通删除时，build 报 `stale_runtime`，应先选入实体再修复；匹配的冷 GLB 指针仍可不下载直接复用。缺失记录与 runtime 符号链接在拉取载荷和调用 Blender 前检查；按 chunk 修复缺失记录需要快照中仍有新鲜包围盒，否则需明确选入实体。索引基线取自构建快照实际解析的同一份数据；捕获或发布期间发生索引变化报 `snapshot_changed`。

## 4. validate 与 commit

validate --staged 检查 index；validate COMMIT 检查固定原生提交。检查规则/身份/引用、依赖指针、逐模型构建新鲜度、GLB hash/长度/预检预算，并从当前输入推导完整空间结果。runtime/ 内缺失、过期或多余的记录与 GLB 拒绝。错误不能通过旧输出被静默补全。

严格 JSON：拒绝重复键、非有限数与溢出、非 UTF-8 编码、超出深度与大小预算；预算与 GLB 顶层数组/图片预算在核心库统一定义，所有宿主共用同一实现。

新或改变的 GLB 字节须取得并验证。未改变的父版本 GLB 可仍未下载，结果明确 missing_assets 与 byte_validation_complete:false，不把元数据有效混称全部资产已检查。固定版本浏览还需验证实际提供的 GLB 字节。

commit 先固定 index tree/HEAD 并校验，调用 Git commit，再读实际 commit 校验。hook/外部写入改变结果时，报告已创建提交 ID 及校验错误，不假装提交未发生或自动回退。合法 staged v1 不受 unstaged v2 干扰，README-only 无需 Blender。Git 原生命令可能创建未经领域校验的草稿，存在 commit 不等于发布有效。

## 5. 选择与传输

.rou/selection.json 为 {all:boolean,chunks:[string],entities:[string]}，数组使用 []，不是 null。set 替换，add 追加，list 查看；sparse-checkout 为 sparse 别名。--all 与明确选择互斥；set 不带任何选择器视为错误。

chunk 接受 x,y,z 或两对角点 x,y,z..x,y,z。每轴独立 min/max，含两端，支持负数及反向，重叠去重；每次最多 32768 块，范围须在 realm 内，不自动裁切。范围解析只有一份实现，命令行原样传递选择器字符串。选择 chunk 包含全部相交 entity，跨块/共享路径去重。目录交 Git cone，LFS 下载按实际依赖限制；cone 带入的指针不代表载荷已准备。

Git cone 永远包含 realm/、runtime/ 与 docs/；runtime/ 内的 GLB 在未选中时只是指针。选区决定哪些 sources 目录进入 cone、哪些源文件与 GLB 载荷被物化。clone no-checkout → 局部 skip-smudge → 轻量 cone → checkout。fetch 只更新 Git 对象/远端跟踪引用。LFS fetch 只缓存，checkout 只从缓存物化，物化后必须验证真实 bytes/hash；已验证的载荷按路径、大小、修改时间、inode 缓存哈希。全世界选择不等于下载全历史。

工作版本切换遵循原生 Git 的修改保护，并预检选区收缩里的作者文件/ignored 备份：将被 cone 删除的 ignored 备份先迁移到 `.rou/backups/` 并报告，作者修改则拒绝。多阶段操作不是统一事务，失败显示真实状态，不隐藏 reset/删除。外部 Git 可改变状态，.rou 锁不能替代 Git 锁。

## 6. 合并与推送

switch -c/checkout -b 创建分支；checkout 分支附着，commit ID detached。普通文本交给 Git；rou mv/rm 保留原生文件改名/删除语义。默认 diff 为 Git 文件差异，diff --realm [--staged] [A [B]] 额外提供无需载荷下载的实体/放置/模型摘要。跨 entity 并行修改合并后验证；重复 ID、同实体不一致、删除/修改或二进制冲突显式处理。

独立 entity/模型的并行贡献（包括同 chunk 新增不同实体）可由原生 Git 直接合并，无需在合并后 build 公共索引；托管平台的原生合并也不依赖本地 merge driver。Git 合并成功仍须校验领域结果。

rou 的分叉 merge --no-commit --no-ff 保留现场，快进单独验证并推进。同一模型的记录/GLB 冲突从已解决输入重建，不自动 ours。merge --abort 调用 Git，不承诺恢复额外未追踪文件。

push 固定验证 OID/refspec。标准 LFS 推送覆盖新可达历史，不受 sparse 限制；必要时从明确来源恢复缺少的历史载荷。Git ref 和 LFS 上传不构成原子分布式 release。正式权限/签名/pin 留 S2。

## 7. 独立浏览包

rou view [REVISION] 默认固定 HEAD；版本参数解析为完整 commit OID；--worktree 显式导出已构建草稿。输出含 realm.json（rou-preview/0.1）、version.json（rou-git-view/0.1）、objects/<sha256>.glb。--chunk 可只准备选区 GLB，但保留完整元数据，未取模型仍显示缺失。浏览包不含观看入口，起点由查看器根据内容推导或由玩家选择。

version 字段包含 realm_id、git_object_format、commit_oid、view_root_sha256、catalog_sha256、validation:local-trusted。草稿 commit 为空。Godot 通过 Rust 扩展核验目录 bytes 与版本侧车绑定、GLB bytes 和预算，不读 .git 或运行 Git/LFS/Blender。无 version.json 的浏览包显示为未绑定版本，不静默当作固定版本。一个本地 JSON 不是公开身份签名；不可信提供者验证由 S2 定义。

这保留独立 preview 的浏览格式，不保留旧自研仓库、旧根 staging 或旧 .rou HEAD 的读取兼容。查看器是只读应用，Godot Editor 放置另行实现。

## 8. 实现与验收

实现结构、op 表与 FFI 契约、迁移顺序见[架构](../../Realm_of_Us_Architecture.zh-CN.md)；2.0 参考实现的验收见[S1 Git 结果](../../experiments/S1-Git-results.zh-CN.md)。

校验分两层。形状层：字段、类型、常量、范围、必填与禁止额外字段，由核心库里每种文档对应的类型实现，本规范各节的字段约束表是其人读定义，没有 JSON Schema。语义层：跨文件引用、ID 唯一与文件名一致、谱系、hash、AABB、输入输出关系，由核心库代码实现，本规范文本是其定义。两层都通过才算有效。审阅过的空间与目录测试用例在 `rou/crates/rou-core/tests/fixtures/spatial.json`。S2 公开网络格式尚未冻结。
