# Realm of Us · 代码架构

设计基线：2.1 · 2026-09-16。范围见[产品设计](Realm_of_Us_Design.zh-CN.md)，数据契约见[protocol/README](protocol/README.md)、[Git realm 规范](protocol/spec/s1-repository.zh-CN.md)与[空间 profile 与浏览包](protocol/spec/s1-preview.zh-CN.md)。2.0 的 Go/Python 参考实现及其验收见[S1 Git 验收](experiments/S1-Git-results.zh-CN.md)；本文描述 2.1 的目标结构与迁移，参考实现已在 Rust 黑盒验收后删除。实测记录见[迁移记录](experiments/rust-migration-status.zh-CN.md)。

## 1. 模块与进程

所有 Rust 文件集中在 `rou/` 的 Cargo workspace。根 `Cargo.toml` 只管理成员和共享配置；`app/` 是 CLI 包 `rou`，`crates/` 下分别为 `rou-core`、`rou-git`、`rou-ffi` 和 `rou-godot`。各包拥有自己的 `Cargo.toml` 与 `src/`；核心及 CLI 测试随包放置，共享测试辅助代码在 `rou/tests/common/`。fuzz、生成头文件和统一 target 仍位于 `rou/`；见[模块说明](../rou/README.md)。

一个 Cargo workspace，所有规则只在 `rou_core` 实现一次，所有宿主通过同一张操作表访问：

| 模块 | 产物 | 职责 | 依赖 |
| --- | --- | --- | --- |
| `rou/crates/rou-core/src` | `rou_core` | 空间规则、严格 JSON 与 RFC 8785 规范编码、SHA-256、GLB 预检与预算、浏览目录与版本侧车校验、稳定 ID/安全路径/LFS 指针规则、`projections`、构建记录新鲜度、领域 diff；`ops` 模块把这些能力组织成操作表 `dispatch(op, input) -> Result<Vec<u8>>`；每种文档一个 serde 类型（拒绝未知字段）就是形状层，不提交也不加载 JSON Schema | 无 IO，无全局状态；`serde`/`serde_json`、`sha2`、`thiserror` |
| `rou/crates/rou-git/src` | `rou_git` | git 与 git-lfs 子进程封装、快照读取（固定 commit/index/工作树）、sparse 与载荷物化、写锁与信号清理、谱系与源哈希缓存、构建编排与 headless Blender 调用 | `rou_core`、`std::process`、`bstr`、`ctrlc`；只读路径可选 `gix` |
| `rou/app/src/main.rs` | 二进制 `rou` | 现有全部命令与帮助；直接使用类型化 API | `clap`、`rou_git` |
| `rou/crates/rou-ffi/src` | cdylib 与 staticlib `librou` | C ABI：`rou_call`、`rou_free`、`rou_abi_version`，只转发到 `rou_core::ops`；`cbindgen` 生成 `rou.h` | 仅 `rou_core` |
| `rou/crates/rou-godot/src/lib.rs` | cdylib 与 `.gdextension` | GDExtension 类 `Rou`，方法 `invoke(op, bytes) -> Dictionary` 与 `abi_version()`，只转发到 `rou_core::ops` | `godot`（godot-rust）、`rou_core` |

宿主与核心的关系：

| 宿主 | 语言 | 接法 |
| --- | --- | --- |
| `rou` 命令行 | Rust | 直接链接 `rou_git`/`rou_core`，无 FFI |
| Blender 插件 `toolchain/blender/rou_blender/` | Python，仅 bpy 逻辑 | `ctypes` 加载插件内按平台附带的 `librou`，所有规则调用经 `rou_call` |
| Blender 导出 helper `toolchain/blender/export_git_realm.py` | Python | 由 `rou build` 在 headless Blender 中运行，只导出 GLB 并报告包围盒与依赖；GLB 校验由 rou 在导出后完成 |
| Godot 查看器 `client/app/` | GDScript 界面与加载调度 | 经 `Rou.invoke` 完成全部校验与空间换算，不复制规则 |
| 验收驱动 `toolchain/build/*.py` | Python | 黑盒驱动 `rou`、Blender、Godot；不含领域规则 |

rou 调用本机 git/git-lfs，只有构建几何时调用 headless Blender。进程使用参数数组与明确 cwd，无 shell 拼接。不假设 realm 位于源码目录。所有 Git 输出使用 `-z` 并按字节处理路径，只在展示时解码。

## 2. 数据与唯一权威

Git tree/index/commit/refs/reflog 是唯一版本状态，LFS pointer 固定重载荷 SHA-256/长度。支持 .git 目录或 gitfile，不自行解析 pack 写入。公开 CLI 从 cwd 发现仓库，无 --repo；内部组件可接受明确路径。

realm 仓库分三层：`realm/models/<id>.json` 定义模型（源 .blend、入口、依赖），`realm/entities/<id>.json` 定义放置（模型 ID、毫米坐标），`sources/` 是作者的完整工程；`realm.json` 只有 format/id/profile，init 后不变；`runtime/<model>.glb` 与同名 `.json` 记录是 build 的全部输出。作者只写前三者，rou 只写 runtime/。根 .gitignore 忽略 /.rou/，此处仅有选区、会话、构建临时目录、浏览包、备份与哈希缓存。完整 view/chunk 索引由 `projections` 在内存重算，不写入 Git 工作树；浏览根摘要为 SHA-256(JCS(view))。所有文档的 `format` 标识与文件路径常量只在 `rou_core` 的 `formats` 与 `paths` 模块定义一次，清单见规范 §1。版本化 JSON 落盘用固定的多行排版（键排序、两空格缩进、数组元素逐行），哈希用 JCS 与排版无关；`rou_core` 提供唯一的写出函数。

Git OID 与资产 SHA-256 区分，显式算法。输入摘要不得含自己的输出，浏览根不得反向包含当前 commit。安全相对路径保留真实目录，拒绝逃逸与符号链接，不能隐式依赖作者机器绝对路径。

空间复用毫米、Y-up、右手、半开 AABB；Blender 坐标只转换一次。GLB 为局部几何，JSON 应用世界平移。跨块按稳定 ID 去重。

严格 JSON 的全部预算在 `rou_core` 统一定义：拒绝重复键、非有限数与溢出、非 UTF-8 编码、超出深度与大小预算。GLB 顶层数组、图片尺寸与解码像素预算同样只定义一次。三个宿主用同一实现，不存在"可提交但不可看"的差异。

## 3. Git、LFS 与 sparse

init 创建标准 Git、局部 skip-smudge、模板/空领域，不自动 commit。clone no-checkout 后配置 LFS，再设置轻量 cone 和 checkout。cone 永远包含 realm/、runtime/、docs/，runtime/ 内未选中的 GLB 只是指针；选区决定哪些 sources 目录进入 cone、哪些源文件与 GLB 被物化。build 不再需要动态扩展 cone；复用的匹配 LFS 指针不下载、不物化。稀疏收缩只保护含已跟踪文件且将离开 cone 的目录；无关缓存目录保持原位。

resolve 固定 commit，读取实体/模型/构建记录，推导有效空间索引，展开选择为目录与精确依赖路径。Git sparse 管理工作路径，LFS fetch/checkout 准备载荷，必须校验实际字节；checkout 退出 0 不保证缺失载荷已取得。[LFS checkout](https://github.com/git-lfs/git-lfs/blob/main/docs/man/git-lfs-checkout.adoc)

rou --all 是当前世界，不是 LFS 全历史。收缩选区不 GC。预检切换覆盖与收缩目录中的本地修改/ignored 作者文件，失败保留现场，不 reset --hard。

批量读取 Git 对象用 `git cat-file --batch` 或 `gix`，不为每个文件起子进程。已物化载荷的 SHA-256 按路径、大小、修改时间、inode 缓存在 .rou 内，只在 stat 变化时重算。谱系检查结果按 commit OID 缓存在 .rou 内，只验证新祖先。

Git、网络和工作树更新无跨命令原子性。rou 写锁记录 PID，在 Ctrl+C/SIGTERM 时清理，冲突时检查记录的进程是否存活；它不能阻止外部 Git/IDE，每次重读真实状态，编辑器基线另验。

Git LFS 存储目录按 snapshot 缓存一次，新 snapshot 重新读取配置。源哈希缓存按 inode/size/mtime/ctime 失效，最多保存 100,000 个条目并按最近使用顺序淘汰；冻结文件与 LFS 对象哈希仅在本次 snapshot 内缓存，谱系缓存按不可变 commit OID 索引；本机缓存独立采用 128 MiB 上限，损坏、超限或写入失败不阻断领域操作。`rou-git` 公开 API 有英文 Rustdoc 并开启 `deny(missing_docs)`；`error::code` 保留核心错误码并提供 Git/IO/缺失载荷等稳定类别。`validate`、`view` 与只读 LFS 命令无需作者写锁。未选中 runtime 文件的真实删除或改写要求先选入再修复；物化先重置旧指针 stat，再校验并刷新载荷 stat，不暂存内容。临时索引在成功、失败和中断时清理。

## 4. 构建与完整性

读取来源为固定 commit、index 或工作树。工作树以实际文件优先，仅明确 skip-worktree 缺席项读取 index；合并读取已解决 index，不能退回 HEAD。新文件可参与构建，提交时必须进入 index。

build 在 .rou/build 冻结源/依赖，按源文件分组、每个 .blend 只打开一次 headless Blender，导出该文件内全部模型的 GLB，计算局部包围盒并写同名记录。一次 build 只加载一次世界快照。成功后仅将 GLB 和记录写回 runtime/，绝不 add；冻结副本目录在成功后可回收，保留策略由命令行参数控制。输入摘要绑定规则、模型定义、入口/依赖字节与导出器设置，共享依赖改变使关联输出过期；entity 放置单独用于空间投影，不使几何构建过期。

validate 检查文档形状/规则谱系/重复 ID/引用、源输出摘要、LFS pointer、GLB 字节和空间闭包，并从当前输入推导完整空间结果。staged v1 不受 unstaged v2 干扰；commit 前检查 index，后验实际 immutable commit。hook 改变提交造成后验失败应报告已创建 ID，不偷偷撤销。

README-only 复用未变化的构建记录。validate/view/resolve 使用同一投影函数。摘要证明版本一致性，不是对任意导出器诚实执行的密码学证明。

## 5. 合并与推送

快进验证固定目标；分叉 merge --no-commit 保留检查/构建现场，--no-commit 本身不能阻止快进。[Git merge](https://git-scm.com/docs/git-merge)

独立 entity/模型的合并不需要重建，完整空间结果在校验和浏览时推导，同 chunk 新增不同实体也无共享索引冲突。创作冲突先解，同一模型的记录/GLB 冲突可重建。不能在全 index unmerged 时先 write-tree，不能批量选 ours 代替重建。二进制保留 base/ours/theirs 供人工解决。

push 固定被验证 OID 与 refspec，防止验证 A 推送 B。LFS pre-push 覆盖新可达历史而非当前 sparse。Git/LFS 上传非原子 release，长期可用性留 S2。

## 6. Blender 与查看器

Blender 保存完整 .blend，可读路径及共享库/贴图随版本恢复。object/Collection 入口确定导出范围，选择一个入口仍需整个源文件。放置以 entity JSON 为交换权威，编辑会话固定版本/源/模型/放置摘要。显式写放置先查基线，旧 Scene 不覆盖新版本。

插件内只保留 bpy 逻辑：依赖闭包、打包、导出、会话界面。坐标换算、包围盒取整、JSON 规范编码与摘要、模型/实体记录校验、安全路径与相对路径规则全部经 `rou_call` 调用 `librou`；`//` 前缀的 Blender 相对路径在核心库中作为相对路径处理。会话基线只覆盖与当前源相关的模型/实体与依赖，过期时提供"仅保存源"与"把当前场景摆放重应用到最新 JSON"两条路径；建立会话失败的原因保存在会话状态并显示在面板。一个模型被多个实体引用时，登记与写摆放都按实体 ID 定位对象。会话只校验当前源记录，按文件 identity/size/mtime/ctime 缓存读取；无关的畸形记录不使本源会话失效。构建与预览共用隐藏网格显式包含和状态恢复逻辑；helper 的 `bounds_blender_m` 明确为 Z-up 米制包围盒，由 Rust `blender::bounds_native` 统一换算。

独立样例可继续使用 rou-preview/0.1 浏览包，这是运行时格式，不是旧仓库兼容。rou view 校验固定 commit，准备 GLB 并导出独立浏览目录与版本描述。Godot 不调用 shell/Git，不读 .git/LFS，浏览包无 .blend。

查看器由 `rou-godot` 扩展提供目录、版本侧车、GLB 字节与预算校验、毫米与 chunk 换算，GDScript 通过 `Rou.invoke` 使用它们，只负责相机、加载调度、界面。起点不来自 realm 数据：默认起点是全部实体世界包围盒并集的中心上方，距离取并集对角线长度，朝向中心；世界为空时在原点上方 2 m 朝向原点。玩家可在进入时或运行中通过分块地图、实体列表或坐标输入选择地点；最近位置按 realm ID 记在本机用户目录，不写入浏览包；命令行 `--at=x,y,z`（米）可直接指定起点。样例导览的路点属于样例自己的本地数据。打开时先检查浏览包三项布局、对象名及包根/包内符号链接；包外祖先可为符号链接，根与 objects 内仅忽略普通 `.DS_Store` 文件，再校验目录与侧车；失败保留原世界。无版本侧车的浏览包要明确显示为未绑定版本，不静默降级。样例导览下沉到 `client/app/samples/`，通用查看器不硬依赖样例。

版本描述明确本地可信桥接；S2 不可信来源需要签名/授权或完整验证材料，JSON 的 verified 字段不是公开信任。Git tree 路径证明仅证明归属，不证明构建新鲜度。

## 7. 操作表与 FFI 契约

操作表只在 `rou_core::ops` 实现一次：`dispatch(op: &str, input: &[u8]) -> Result<Vec<u8>, Error>`，输入是 JSON 或裸字节（GLB），输出是 JSON 值。它是所有宿主的唯一规范表面，用同一批 JSON 夹具测试。

`librou` 的 C ABI 只有三个函数，全部转发到操作表：

```c
uint32_t rou_abi_version(void);
int32_t  rou_call(const char *op, const uint8_t *input, size_t input_len, RouBuf *out);
void     rou_free(RouBuf buf);
typedef struct { uint8_t *ptr; size_t len; } RouBuf;
```

- `op` 是 NUL 结尾的操作名；`input` 是 JSON 或裸字节；`out` 由库分配，宿主必须用 `rou_free` 释放。返回 0 为成功信封，1 为错误信封；2 为无效参数，3 为捕获的 panic，后二者的 `out` 为空（无效 `out` 指针无法写入）。
- 输出固定为 `{"ok":true,"value":...}` 或 `{"ok":false,"error":{"code":"...","message":"..."}}`；错误码是稳定字符串，宿主据此决定提示。
- 库内用 `catch_unwind` 拦截所有 panic 并转为错误码，panic 绝不穿过边界。无全局可变状态，可多线程并发调用。库不做文件与网络 IO，字节由宿主读好传入。
- 新增操作只加 op 名，不改函数签名；不兼容变化递增 `rou_abi_version`，宿主启动时自检。

`rou-godot` 不经过 C ABI，直接链接 `rou_core` 并暴露 GDExtension 类 `Rou`：`invoke(op: String, input: PackedByteArray) -> Dictionary` 与 `abi_version() -> int`。返回值由 Rust 把 JSON 值转成 Godot Dictionary，GDScript 不解析 JSON；`PackedByteArray` 以切片零拷贝传入。这样 Python 与 Godot 使用同一份 op 名与语义。目录打开时另用 `load_catalog(bytes)` 建立只读 Rust `viewer::Catalog`，0.35 s 加载查询用 `catalog_query(position_bytes)` 复用它，不重复序列化和验证完整目录；规则与操作表的 `viewer.query` 共用。`core.gd` 缓存通用适配器和 ABI 自检结果。

首批 op：`profile.validate`、`spatial.owner`、`spatial.intersections`、`spatial.range`、`spatial.project`、`json.strict`、`json.canonical`、`digest.sha256`、`glb.precheck`、`manifest.validate`、`version.validate`、`path.safe`、`id.validate`、`lfs.pointer`、`model.validate`、`entity.validate`、`build.validate`、`blender.placement`（Blender 坐标到毫米与逆变换）。

Python 侧用 `ctypes.CDLL` 加载，`bytes` 零拷贝传指针，一个几十行的包装模块把返回 JSON 转成 Python 异常。以后若需要细粒度高频调用或零拷贝大缓冲，再加 PyO3 `abi3` 模块，`rou_core` 不变。

## 8. S2 接口

| 组件 | 边界 |
| --- | --- |
| rou-node | 对象服务、区域 pin、配额、复制、恢复、本地 Web 管理 |
| LFS transfer adapter | 输入 OID/size，经节点取得字节，整文件 SHA-256 最终校验 |
| 元数据提供 | Git 或桥接；局部资产节点不必承担全历史 promisor |
| 发布记录 | commit 后绑定 realm/fork 身份、Git 算法/OID、SHA-256 浏览根 |
| 信令/TURN | 可替换连接设施，不授予维护权 |
| RealmHub | 可选业务与目录，不成为版本权威 |

资产不加密。贡献者、维护者、节点、管理凭据分责；公钥传播不等于初始信任。签名、传输会话绑定、重放拒绝及跨端向量留 S2。hash 不验证作者，节点身份不授予版本维护权。

pin 固定 commit 与 runtime/sources 范围，preparing/complete/failed 与共享开关独立。草稿/缓存不自动公开，所有来源离线报告不可用。Web 管理回环监听仍需认证、Origin/CSRF；远程初版 SSH 隧道。Godot 只连接/打开页面，不重复后台。

分片、背压、续传、强制 TURN 与多节点覆盖必须实测，G0 本地 adapter 不代表 P2P。Git/LFS 缓存策略不替代长期 pin。S2 的节点与传输层同样用 Rust 实现。

## 9. 可选服务

本地核心无 server 依赖。未来独立信令服务与 coturn 可单独部署。RealmHub 优先独立 API/worker，复用 Supabase Auth/PostgreSQL/Storage/按需通知；Edge Functions 适合轻量入口。

平台 auth 只处理平台会话，公钥控制权另验。私钥/service-role 不进客户端。Storage 默认管理头像/截图/附件，重资产另设 provider/保留/费用。平台停机不影响已知可达核心链路。

## 10. 迁移与验证

术语与路径对照（2.0 → 2.1）：`realm/authoring/<id>.json` → `realm/models/<id>.json`，格式 `rou-authoring/0.1` → `rou-model/0.1`；entity 字段 `binding` → `model`；`realm/derived/builds/<binding>.json` → `runtime/<model>.json`，记录字段 `binding` → `model`；`realm/derived/` 目录消失；代码中的 binding 一律称 model，加载的世界快照不再叫 model。路径与格式串集中在 `rou_core` 一处常量。

顺序：

1. `rou_core`：移植 `toolchain/realm_format` 与 `toolchain/git_realm/model.py`、`diff.py` 的纯逻辑，按新路径与字段实现；建立 `ops` 操作表；把 `toolchain/fixtures/spatial.json` 的审阅用例复制进 crate 测试（`catalogs` 部分按 2.1 浏览包形状改写）并与移植的 Python 用例一起测实；对 JSON 与 GLB 入口接入 `cargo-fuzz`；profile 用例的 `shape_accepted` 对应“能否解析进 profile 类型”，`accepted` 对应两层都通过。
2. `rou_git` 与 `rou/app/src/main.rs`：移植 `repository/` 与 `toolchain/git_realm/gitio.py`、`workflow.py` 的编排；`examples/calibration/create_git_scene.py` 改为生成新布局；用 `toolchain/build/verify_git_realm.py` 跑双人协作黑盒验收，Go 集成测试的场景逐条移植为 Rust 集成测试。
3. `rou-ffi` 与插件绑定：插件包改名 `rou_blender`（目录 `toolchain/blender/rou_blender/`，安装包 `dist/rou_blender.zip`，显示名 Realm of Us），把它对 `realm_format` 的调用换成 `rou_call`，写入路径改为 `realm/models/`；`toolchain/tests/blender/verify_git_authoring.py` 增加 linked 库工程用例；`package_addon.py` 按平台打包 `librou`。
4. `rou-godot` 与查看器：GDScript 校验代码换成 `Rou.invoke`；`client/tests/*.gd` 改为经扩展验证；`package_viewer.py` 携带扩展。
5. 删除 `repository/`、`toolchain/cmd/rou/`、`go.mod`、`toolchain/git_realm/`、`toolchain/realm_format/`、对应 Python 单元测试、`toolchain/fixtures/spatial.json`（已复制进 crate）与 `check_code.py` 的 Go 部分；更新 README。

不做兼容：不读取 2.0 的目录布局与共享索引文件，不保持旧浏览根哈希，不检测更早的自研 `.rou/repository.json`。预研阶段的 realm 用 `examples/calibration/create_git_scene.py` 重建。

迁移同时修复审查已确认的缺陷：Blender `//` 路径误判、零厚度实体被拒（退化轴外扩 1 mm）、首次 add 前重建失败、Ctrl+C 死锁文件、ignored 备份阻塞收缩、每文件一个子进程、源文件重复哈希、冻结副本不回收、共享模型阻断写摆放、会话基线过宽与异常吞没、Python 与 Godot 预算不对称、`view --chunk` 参数溢出、无侧车浏览包静默降级、查看器半清状态。各项要有回归用例。

验收基准沿用现有黑盒驱动：`verify_git_realm.py --windowed`、空间用例、`client/tests/git_view_workflow.gd`、插件安装与创作回归、`verify_git_viewer.py`。Go/Python 目录删除后全部检查仍须通过，只验当前 Mac；其他 OS、移动/Web 另行实机验证。
