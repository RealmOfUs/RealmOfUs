# Realm of Us · 产品与系统设计

设计基线：2.1 · 2026-09-16。2.0 确定 Git + 标准 LFS 方案并完成 Go/Python 参考实现；2.1 决定核心工具统一用 Rust 实现并接入 Blender 与 Godot，重新组织数据目录，不保留任何兼容层。参考实现的验收见 [S1 Git 验收](experiments/S1-Git-results.zh-CN.md)，数据契约见 [Git realm 规范](protocol/spec/s1-repository.zh-CN.md)，实现结构与迁移见 [架构](Realm_of_Us_Architecture.zh-CN.md)。

## 1. 目标与阶段

为多人分布式开发开放 3D 世界提供协作机制。一个 realm 是有固定空间规则的创作项目。参与者克隆目录、创建分支、选择区域、用 Blender 创作、提交和合并，用独立 Godot 客户端浏览确定版本。

Git 管理完整工程树和唯一历史，Git LFS 保存重资产，rou 统一日常命令并提供空间选取、构建、领域校验及浏览交接。平台账号与商业发行不是本地创作的前提。

核心工具代码用 Rust 实现，规则只有一份实现：rou 是单一二进制；同一核心库以 C ABI 供 Blender 插件通过 ctypes 调用，以 GDExtension 供 Godot 查看器使用，两者调用同一张操作表。Python 只保留 Blender 内依赖 bpy 的逻辑，GDScript 只保留查看器界面与加载调度。

| 阶段 | 范围 |
| --- | --- |
| S1（2.0 参考实现，G1–G4） | 空间规则、完整原工程、Git/LFS、分支/标准远端协作、按区域取源、构建校验、合并、独立浏览、当前 Mac 验收 |
| 2.1 Rust 迁移 | Rust 核心库与 CLI、Blender ctypes 绑定、Godot Rust 扩展、新目录布局、审查已确认缺陷的修复、删除 Go/Python 参考实现 |
| S2 | 公开 P2P、发布签名与维护权、节点身份、信令/TURN、区域 pin、续传与故障接替 |
| 后续扩展 | Godot Editor 放置、模型/实例优化、视觉 diff、RealmHub、Supabase、Steam、其他 OS 实机、移动/Web |

普通 Git 分支、fork 和三方合并纳入 S1。S2 不再实现自有 commit DAG。预研阶段不做兼容：2.1 不读取 2.0 的目录布局与共享索引文件，不保持旧浏览根哈希，现有 realm 用生成脚本重建。

## 2. 世界与空间

根 realm.json 固定 realm 身份及空间规则：右手、Y 向上、米制显示、整数毫米放置，有界笛卡尔分块默认 32 m。相同谱系不能任意更改规则；改变规则需创建新 realm。

chunk 是长方体索引单元，不是模型文件。边界采用半开区间，负坐标使用 floor。entity 保存稳定 ID、所用模型和世界位置；跨块实体定义一次，在相交块中引用，移动保持 ID。S1 根实例仅支持平移，旋转/缩放在模型内部子对象处理。

## 3. 工程树与文件关系

```text
my-realm/
├── .git/                    # Git 历史、唯一 index、LFS 缓存
├── .rou/                    # 被忽略的选区、会话、构建临时目录、浏览包、备份
├── .gitignore
├── .gitattributes
├── README.md
├── realm.json               # 世界规则，init 后不变
├── realm/
│   ├── models/<id>.json     # 模型定义：从哪个 .blend 的哪个入口导出，依赖哪些文件
│   └── entities/<id>.json   # 放置：哪个模型，放在世界的哪个毫米坐标
├── sources/                 # 作者的完整 .blend、库、贴图，自由组织，走 LFS
├── runtime/
│   ├── <model>.glb          # rou build 导出的模型，走 LFS
│   └── <model>.json         # 该 GLB 的构建记录：输入哈希、GLB 哈希、局部包围盒
└── docs/                    # 普通文本
```

三条规则：作者只写 realm/ 与 sources/；rou build 只写 runtime/；每个模型在 runtime/ 里恰好一个 GLB 加一份同名记录。每个 ID 一个文件，realm.json 在 init 之后不再改变，仓库里没有任何多人共同修改的单个文件。sources 保存模型与创作环境，models 说明从哪个文件导出什么，entities 说明放在哪里，没有两份模型。工程目录不由 chunk 坐标决定；不同 entity 可使用独立工程或同一工程不同入口，共享文件按依赖引用。作者安排的子目录进入 Git 历史，README 等普通文件可以单独提交。根目录无额外 work/staging 层，Git index 是唯一暂存区。GLB 包含运行时网格/材质/嵌入贴图，原 .blend 不依赖 GLB。

完整空间目录与浏览根由版本化输入、逐模型构建记录在读取时推导，不作为共享文件提交。浏览根 SHA-256 对 RFC 8785 规范 JSON 计算，供 S2 发布记录绑定；独立浏览包按需导出到 .rou/view 或显式目录。

## 4. 统一命令与选择

日常入口为 rou init/clone、switch/checkout、sparse、build、add、diff、commit、fetch/pull、merge、push。原生 Git/IDE 可访问标准仓库，rou 重读实际状态，不保留第二个 HEAD。

clone 取得轻量 Git 历史与 LFS 指针，默认不下载重载荷。sparse set 替换选区，add 追加，list 查看；--chunk 与 --entity 可组合，--all 选择当前版本整个世界。反向范围 1,-1,0..-1,1,0 各轴取 min/max，包含两端。跨块实体及共享依赖去重。

realm/、runtime/、docs/ 永远检出；runtime/ 里未选中的 GLB 只是指针。选区决定哪些 sources 目录进入 Git cone、哪些源文件与 GLB 被物化。cone 带入的指针不等于下载其模型；单实体位于大工程时仍须下载完整工程。收缩选择不删除历史或自动回收缓存，涉及本地修改的覆盖拒绝，ignored 作者备份先迁移到 .rou 内再执行。

新建分支不天然下载模型；切换版本按新版本重算依赖。Git clean、当前 sparse 范围、载荷是否物化、世界是否有效分别报告。fetch 不改变工作文件；pull 默认仅快进，分叉显式 merge。

## 5. 创作、构建与提交

Blender 保存完整原工程。放置由显式操作更新 entity JSON；建模导出不能用旧 Scene 位置覆盖最新 JSON。会话固定版本及源/放置基线，外部切换后须重新加载，或由作者通过明确操作把当前场景摆放重应用到最新 JSON。

rou build 默认冻结已保存的工作树输入，在 .rou/build 构建，输出写回 runtime/，不自动 add。仅明确 sparse 缺席的路径从 index 补齐；合并以合并后的 index 为准。真实删除不能被当作 sparse 缺席恢复。高级 --staged 从 index 构建，输出仍写工作树。

构建记录绑定源/依赖字节、入口、模型定义、规则、导出器设置及输出 hash/长度。共享依赖变化使所有引用它的模型过期。rou add 显式暂存；rou commit 校验 index，创建原生 commit 后校验实际提交。暂存 v1/输出 v1、工作树保留 v2 合法；暂存 v2/输出 v1 拒绝。允许艺术上未完成但数据自洽的版本，README-only 不运行 Blender。

Git/LFS 按整文件字节去重；不承诺跨 .blend/GLB 贴图语义去重或相似二进制增量存储。

## 6. 多人合并

| 并行改动 | 处理 |
| --- | --- |
| 独立 entity/模型 | Git 文件合并后校验领域结果 |
| 同 chunk 不同 entity | Git 文件合并，读取时推导完整索引，无需额外 build |
| 同 entity 不同字段 | 可文本合并，随后检查整体身份/放置/引用 |
| 不同文件重复 ID | 即使 Git 无冲突仍拒绝 |
| 同 .blend/贴图 | 保留 Git base/ours/theirs，作者用建模工具解决 |
| 同一模型的记录/GLB 冲突 | 解决创作输入后 build，不手选旧 hash |

分叉 merge 保留未提交现场，供检查、build、add、commit；快进单独处理，不能用 --no-commit 声称阻止快进。GLB 冲突应通过源和构建解决，不声称自动合并模型几何。

独立贡献无需合并后重建公共索引，可直接检查并提交。托管平台原生 Git 合并也可使用这一契约；领域校验仍须检查重复 ID、引用和构建新鲜度。

标准远端负责 Git 账号/SSH 写权限；Git 作者配置不等于已认证签名。push 的 LFS 需求覆盖新可达历史，与当前选区不同。向新 fork 远端推送可能需要补齐历史载荷。普通 push 不等于正式 release 或长期保留。

## 7. 独立浏览

Godot 提供自由飞行、区域/实体信息和固定版本展示。起点由玩家选择或从内容推导，realm 数据与浏览包都不含观看入口。房屋导览只是可选样例，其路点是样例自己的本地数据。客户端无放置编辑；后续放置属于 Godot Editor 插件。

rou 从固定 commit 校验并导出独立浏览目录；Godot 只读目录和 GLB，不启动 Git/LFS/Blender，不读取 .git 内部。浏览包带版本信息和 hash，本地可信桥接不是公开签名证明。查看器的目录、版本侧车、GLB 预检与空间换算由 Rust 扩展提供，与 rou 是同一份实现；GDScript 不再复制这些规则。

按块 AABB 距离选取、跨块去重、校验 GLB 字节/预算后导入。目录未知、已知为空、模型缺失分开；不能混入其他提交资源。保留同步导入等 S1 限制，画质和启动性能另测。

## 8. S2 与可选平台

只做公开 realm：取消资产内容加密、世界密码和内容密钥。hash 验证字节，签名验证发布者，标准传输仍安全加密。公开读无需平台 auth；写入、维护权、节点身份与本机管理分别验证。

Git OID 显式记录算法，LFS/浏览根用 SHA-256。发布记录在 commit 之后绑定 realm、fork 治理身份、commit 与浏览根，避免循环。clone 不授予上游写权，签署浏览根不改变 Git 历史的对象算法。

元数据用 Git/桥接交换，重资产在 LFS transfer/节点接口后接 P2P。信令交换连接信息，TURN 无法直连时中继。来源发现、会话身份绑定、续传、配额、pin、换源另行验收。节点只提供明确开启共享的范围，草稿/缓存不自动公开。

管理统一为节点本地 Web 页面，Godot 只做连接/打开入口，不重复完整 rou-nodectl UI。RealmHub 可复用 Supabase Auth/PostgreSQL/Storage/通知，业务优先独立 API/worker 服务，Edge Functions 用于轻量 webhook。节点、CLI、Blender、独立客户端无 Supabase 依赖。平台停机不应破坏本地/已缓存/已知节点链路。

## 9. 验收

| 目标 | 要求 |
| --- | --- |
| G1（2.0 已完成） | 原工程/自定义路径/文本复原；metadata clone；Git dirty、改名、删除、分支 |
| G2（2.0 已完成） | 依赖闭包、范围/跨块、冻结输入、staged 语义；拒绝过期输出/重复 ID/缺依赖/非法规则 |
| G3（2.0 已完成） | 双人分叉合并；不丢冲突源；固定版本 Godot；Blender 原工程再打开 |
| G4（2.0 已完成） | 移除旧后端/测试入口；无转换器/兼容/双写；新样例、当前 Mac 回归、文档 |
| 2.1 | Rust 版按新目录布局通过现有黑盒验收：双人 Git/LFS 流程、空间测试用例、固定版本 Godot 工作流；Blender 插件经 ctypes 完成注册与放置，含 linked 库工程；审查确认的缺陷各有回归用例；Go/Python 目录删除后全部检查仍通过；当前 Mac 实机 |
| S2 | 共同签名/权限向量、双节点直连与中继/恢复、区域副本管理、无平台分发演示 |

以验收报告的命令和结果为准。旧报告与旧房屋产物保留为历史资料；2.1 不兼容 2.0 的目录布局、共享索引文件与浏览根哈希，预研阶段的 realm 用生成脚本重建。
