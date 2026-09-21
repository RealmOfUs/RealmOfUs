# 统一 rou 入口的多人协作工作流提案

> 2026-09-14：本文保留方案形成时的论证。用户已授权 G1–G4 实现，工程目录统一为 `realm/`；当前可执行命令、格式和实测边界以 [新规范](../protocol/spec/s1-repository.zh-CN.md) 与 [验收](../experiments/S1-Git-results.zh-CN.md) 为准。下文“拟议/待实现”描述研究时点，不是当前实现状态。

本文细化 [Git/LFS 复用方案](git-reuse-assessment.zh-CN.md)。以下 rou 命令、目录布局与校验规则是新后端的目标行为，尚未实现；底层 Git/LFS 命令用于说明委托关系，不表示整条 Realm 流程已经验收。项目处于预研阶段，不实现旧仓库转换器、旧提交映射或读取兼容。

## 1. 日常入口与唯一历史

日常操作统一使用 rou。常见的 add、commit、branch、switch、log、restore、remote、fetch、push 委托原生 Git；rou 增加空间依赖、LFS 下载控制、编辑器基线和构建验证。用户不需要在一次建模贡献中交替使用两种 CLI。

`.git` 仍是唯一 index、HEAD、refs、commit/tree/blob 和 LFS 指针的权威。`.rou` 没有另一套提交图。原生 Git/IDE 仍能识别仓库，rou 每次重新读取实际状态，不能假定外部工具不会修改它。原生 Git 创建的未构建草稿也不会自动获得“有效世界”状态。

| rou 入口 | 原生 Git/LFS 操作 | rou 的扩展 |
| --- | --- | --- |
| `rou init` | `git init -b main`、局部 LFS 安装 | 世界配置、ignore/attributes 模板；不隐式创建首个提交 |
| `rou clone URL DIR` | `git clone --no-checkout`、局部 skip-smudge、sparse、checkout | 默认只准备世界元数据工作区 |
| `rou switch -c NAME` | `git switch -c NAME` | 同提交新分支保留选区，无需重新下载 |
| `rou switch NAME` / `rou checkout COMMIT` | `git switch` / `git checkout --detach` | 固定目标、协调新版本依赖、标记旧编辑器会话 |
| `rou sparse set/add/list` | Git sparse 规则与 LFS fetch/checkout | chunk/entity → 工程目录及真实依赖 |
| `rou build` | 读取 Git 状态，必要时取 LFS 输入 | 固定工作树输入快照，导出 GLB、重建派生目录；不自动 add |
| `rou add PATH...` | `git add -- PATH...` | 指针/缺失文件检查；Git index 是唯一暂存区 |
| `rou status` / `rou diff --staged` | `git status --porcelain=v2 -z` / `git diff --cached` | 世界就绪状态及实体变更摘要 |
| `rou commit -m MESSAGE` | 校验 index 后 `git commit`，再核验实际提交 | 源/输出/索引一致性；README-only 不运行 Blender |
| `rou fetch origin` | `git fetch origin` | 默认只更新历史与远端引用，不取模型、不改 HEAD/工作区 |
| `rou pull` | fetch、固定目标、`git merge --ff-only` | 默认只快进，随后准备当前选区；分叉明确转 merge |
| `rou merge REF` | 分叉时 `git merge --no-commit REF`；快进单独处理 | 输入冲突、派生重建与完整世界校验 |
| `rou push origin BRANCH` | Git push 与标准 LFS pre-push | 校验固定推送对象、远端载荷可用性；不按 sparse 截断 |

`rou sparse` 是拟议的 `rou sparse-checkout` 简写；两者使用同一选区状态及 set/add/list 语义，正文使用短形式。它不改变底层 Git sparse-checkout 的名称，也不要求作者输入 --cone。当前 CLI 尚未增加此别名。

这里的命令表定义常用入口，不承诺首轮覆盖所有 Git 参数。保留参数语义，未支持组合应明确报错。`rou build --staged` 可作为高级入口从 index 构建；日常使用工作树构建，随后 rou add 和 rou commit，避免每次先学习 index 构建细节。

默认工作树构建的完整逻辑输入由实际工作文件与明确未检出路径的 Git index 记录组成。合并时未检出文件必须读取合并后的 index，不能回退到尚未推进的 HEAD；应当存在却被作者删除的选区内文件也不能被当作 sparse 缺席而自动恢复。尚未暂存的新创作文件可以构建并记录摘要，commit 再检查它们是否连同所需依赖进入 index。

`rou commit` 默认要求暂存的世界输入与派生结果一致；缺构建时提示先 build/add，不自动启动 Blender，也不替用户暂存别的文件。检查范围是整个逻辑世界及受影响依赖，不受本地空间选区截断。数据没有改变时复用已有构建记录与输出，无需下载所有历史模型。检查和后续 Git 操作之间仍需检测并发变化；若 Git commit 已成功而后验失败，明确报告提交 ID 和无效状态，不伪装成未提交或自动回滚。

## 2. clone 后三个位置分别有什么

本例的轻量文件均放在根、`realm/` 和 `docs/`；重资产位于 `sources/`、`runtime/`。默认 clone 获取完整 Git 元数据历史和 LFS 指针，工作区只检出轻量目录，不启用全历史 LFS 下载。大规模 Git 元数据的 partial clone 属于后续优化。

```text
alice-realm/
├── README.md                         # 真实文本
├── LICENSE
├── realm.json                        # 世界固定规则
├── .gitignore                        # 包含 /.rou/
├── .gitattributes                    # LFS 跟踪规则
├── docs/                             # 真实说明文件
├── realm/                            # 真实 JSON，描述完整世界
│   ├── entities/house.json
│   ├── entities/bench.json
│   ├── authoring/house.json
│   ├── authoring/bench.json
│   └── derived/                      # 构建记录、chunk 页、view 根
├── .rou/                             # 被 ignore，本地状态；名称尚为草案
│   ├── selection.json                # 当前没有创作选区
│   └── state.json                    # 已校验版本/就绪状态缓存，不是 HEAD
└── .git/
    ├── HEAD、index、config、refs/      # 版本、完整逻辑索引、远端信息
    ├── objects/                      # Git 历史/文件树、普通 blob、LFS 指针
    └── lfs/                          # 新 clone 尚无模型载荷
```

此时 `sources/`、`runtime/` 通常不存在；不创建空文件或空目录冒充模型。它们的路径、版本和指针在 Git tree 中，因此 rou 能读取世界目录并确定需要取哪些字节。选择后才出现对应源工程及依赖。

这是推荐布局的结果。cone 模式还会包含根文件及所选目录祖先的直属文件；若作者把大型 LFS 文件放在这些位置，它可能以未展开的指针文本出现。rou 必须识别它，不能因为文件名以 `.blend` 结尾就认为可以打开。单独复制一个指针不等于取得模型。[Git sparse-checkout](https://git-scm.com/docs/git-sparse-checkout)

`.git` 的物理目录展示只对应普通单工作树仓库。实现通过 Git/LFS 查询实际目录，兼容 gitfile/worktree 和 LFS 存储配置；不把上述路径硬编码进公开协议。

clone 的底层顺序示意如下，工作目录切换在实际 Go 代码中用进程 cwd 完成：

```bash
git clone --no-checkout "$REALM_URL" alice-world
pushd alice-world
git lfs install --local --skip-smudge
git sparse-checkout set --cone realm docs
git checkout main
popd
```

示例默认分支为 main，实际读取远端默认分支。设置 sparse 本身也可能更新工作区，所以 skip-smudge 必须在它之前配置。LFS 规则安装为局部配置，每次 clone 都需要初始化。[Git clone](https://git-scm.com/docs/git-clone)、[LFS install](https://github.com/git-lfs/git-lfs/blob/main/docs/man/git-lfs-install.adoc)

## 3. 具体场景与起始世界

Alice 是维护者，Bob 是贡献者。两人使用同一支持 Git 与 LFS 的远端：Alice 可更新 main，Bob 可推自己的 feature 分支。服务器提供账号/SSH 凭据，rou 复用 Git 的认证。例子不依赖 RealmHub；未来个人 fork 可使用不同 remote，公开 P2P 与发布签名仍属于 S2。

起始提交记为 C0；C0/A1/B1/M1 是讲解用标签，不是 CLI 内置引用。

| 实体 | 所在 chunk | 作者源 | 共享依赖 | 运行时输出 |
| --- | --- | --- | --- | --- |
| house | `0,0,0` | `sources/house/house.blend` | `sources/shared/wood.png` | `runtime/house.glb` |
| bench | `1,0,0` | `sources/bench/bench.blend` | 同一 wood.png | `runtime/bench.glb` |

两者使用独立 `.blend`，都引用共享纹理。每个 entity 在 `realm/entities` 保存稳定 ID 和放置，在 `realm/authoring` 的 binding 保存工程入口与依赖。其他远处实体可以存在，本次无需下载其模型。

如从零准备 C0，维护者先取得一个空的 Git/LFS 远端，再执行：

```bash
mkdir seed-world
pushd seed-world
rou init

# Blender 创建/保存 house、bench 工程；插件写出实体与 authoring 记录。
# 保存共享 wood.png；本例两者分别位于表中 chunk。
rou build
rou add .
rou commit -m "Initial village"          # 得到 C0
rou remote add origin "$REALM_URL"
rou push -u origin main
popd
```

`rou remote add` 委托 `git remote add`。init 生成世界配置与 Git/LFS 设置；build 生成 runtime 与派生 JSON；add 将普通字节/LFS 指针写入 Git index；commit 才创建 C0；push 将 C0 和必要载荷提供给远端。这里第一次 `rou add .` 包含全部创作文件，但 `.rou/` 已被 ignore。

## 4. Alice 选择房屋并贡献 A1

```bash
rou clone "$REALM_URL" alice-world
pushd alice-world
rou switch -c feature/house
rou sparse set --chunk '0,0,0'

# 在 Blender 打开 sources/house/house.blend，修改屋顶并保存。
# 若修改放置，插件还更新 realm/entities/house.json。
rou build
rou add sources/house realm/authoring/house.json realm/entities/house.json \
  runtime/house.glb realm/derived
rou status
rou diff --staged
rou commit -m "Improve the house roof"   # 得到 A1
rou push -u origin feature/house
popd
```

空间选择需要先读 C0 的有效索引，得到 house 和 wood.png。底层大致为：

```bash
git switch -c feature/house
git sparse-checkout set --cone realm docs sources/house sources/shared
git -c lfs.fetchrecentalways=false lfs fetch \
  --include='sources/house/house.blend,sources/shared/wood.png' \
  --exclude='' origin HEAD
git lfs checkout sources/house/house.blend sources/shared/wood.png
```

实际执行先固定 HEAD 的完整 commit OID，按该 OID 取指针和载荷，并核对执行期间基线。路径集合由 rou 生成，不要求用户手写依赖。`runtime/` 尚未选择，浏览载荷不因创作检出默认下载。[LFS fetch](https://github.com/git-lfs/git-lfs/blob/main/docs/man/git-lfs-fetch.adoc)、[LFS checkout](https://github.com/git-lfs/git-lfs/blob/main/docs/man/git-lfs-checkout.adoc)

| 操作完成后 | 工作区可见文件 | `.rou` | `.git` |
| --- | --- | --- | --- |
| clone | 根文件、realm/docs 文本；无 sources/runtime | 无创作选区，版本校验缓存 | C0 历史、树和指针；main/origin/main、index；无重载荷 |
| switch -c | 文件不变 | 选区不变 | 新 feature/house 指向 C0，HEAD 改指分支 |
| 选 chunk | 新增 house.blend、wood.png 真文件；bench 源仍未检出 | 保存空间选择、解析出的依赖与就绪状态 | sparse 规则与索引呈现状态更新；LFS 缓存新增两个 OID；HEAD 不动 |
| Blender 保存 | house.blend 变化，可能有实体放置 JSON 变化 | 编辑器基线/输入摘要用于检测过期 | Git index 仍是旧版，文件尚未暂存 |
| build | 新屋顶 GLB、构建记录、相关空间页与 view 根 | `.rou/build/<run>/` 的固定输入、临时输出与报告 | 不更改提交和用户暂存内容；必要输入获取可增加载荷缓存 |
| add | 真模型仍在原路径 | 构建缓存可保留 | LFS clean 保存新模型对象，index 指向新 pointer/JSON blob |
| commit | 文件内容不因提交改写 | 缓存实际提交的验证结果；旧内存会话仍需基线协调 | 新 Git tree/commit A1，feature/house 与 reflog 推进 |
| push | 不变 | 推送结果可缓存 | 远端 feature/house → A1，LFS 上传目标缺失对象；本地跟踪信息更新 |

build 创建 `runtime/house.glb` 时，还要把必要输出目录纳入准备范围，使后续普通 Git add 可以工作。初期 cone 可能因此带入 runtime 同目录的其他 LFS 指针；它们不是自动下载的模型。只把本次真实输出加入提交，不把未展开指针误判为新资产。

共享木纹没有修改，所以不产生新的 wood 对象。若修改的是共享纹理，受影响的 bench 等实体也必须参与失效分析，选区不能阻止必要构建；需要额外源文件时 rou 应明确列出并获取依赖。

## 5. Bob 从同一 C0 并行贡献 B1

Bob 在 Alice 合入 main 之前 clone，因此同样从 C0 开始；Alice 提交 A1 只更新 feature 分支，不改变此时的 main。

```bash
rou clone "$REALM_URL" bob-world
pushd bob-world
rou switch -c feature/bench
rou sparse set --chunk '1,0,0'

# Blender 修改 sources/bench/bench.blend 中的长椅并保存。
rou build
rou add sources/bench realm/authoring/bench.json realm/entities/bench.json \
  runtime/bench.glb realm/derived
rou commit -m "Improve the bench"        # 得到 B1
rou push -u origin feature/bench
popd
```

底层与 Alice 相同，选择的是 bench 及 wood.png。Bob 不需要 Alice 的屋顶模型；自己的 `.git/lfs` 有 bench 和 wood，Alice 的缓存是另一份本地存储，不承诺跨机器/仓库物理去重。

此时远端 main 仍在 C0，两个分支分别是 C0→A1 与 C0→B1。Alice 的贡献和 Bob 的贡献没有互相覆盖。

## 6. Alice 获取、审阅并合并

```bash
pushd alice-world
rou fetch origin
rou diff main origin/feature/bench
rou switch main
rou merge --ff-only feature/house       # main 快进到 A1
rou merge origin/feature/bench          # A1 与 B1 分叉，进入合并状态
rou status

# 本例作者输入可自动合并；共同派生 view 根等需要重新生成。
rou build
rou add runtime realm/derived
rou commit -m "Merge the bench contribution"  # 得到 M1，父提交为 A1/B1
rou push origin main
popd
```

底层与扩展如下：

| rou 操作 | Git 操作 | 文件与状态变化 |
| --- | --- | --- |
| fetch origin | `git fetch origin` | 新 Git 对象、origin/feature/bench 与 FETCH_HEAD；本地 HEAD/index/工作文件不变，LFS 未自动下载 |
| diff | `git diff` 加领域记录比较 | 只读：显示 bench 模型版本或放置更新，不需要源模型载荷 |
| switch main | `git switch main` | 切回 C0；按既有 house 空间选区还原 C0 文件，已有源可从缓存物化；旧编辑器会话标为 stale |
| merge --ff-only feature/house | 固定并校验 A1，`git merge --ff-only` | main/index/工作树切到 A1，再协调选区；无新 merge commit |
| merge origin/feature/bench | 固定 B1，`git merge --no-commit` | main 仍在 A1；工作树/index 是待合并结果，Git 写入 MERGE_HEAD 等状态 |
| build | rou 构建/派生重建 | 已合并的 house、bench 输入产生统一目录；无需把源模型都下载来重建未变几何 |
| add + commit | `git add`、校验、`git commit` | 解决派生冲突，生成两父提交 M1，清除 Git 合并状态 |
| push main | `git push origin main` 与 LFS pre-push | 远端 main 推进到 M1；不按 Alice 的 house 选区删除或漏传 bench 的历史 |

示例区分了快进和分叉。`git merge --no-commit` 不能阻止快进，因此不能靠该参数保证所有合并都停在提交前；rou 应预先识别目标并选择对应路径。分叉默认留下待合并结果，让作者确认输入和重建后显式 rou commit。[Git merge](https://git-scm.com/docs/git-merge)

派生 JSON 的冲突不是要求作者手工编辑哈希。`rou build` 必须能够在作者输入已解决、旧派生输出仍有 Git 冲突的状态下工作：它不解析冲突 view-root，而是从作者输入及可验证构建记录重新生成。此时不能先对整个 index 执行 git write-tree，也不能对所有派生结果直接选 ours。

如果 house 与 bench 的构建记录各自仍匹配合并后的输入，重建索引可复用它们的 GLB OID、局部边界等结果，不需要重新渲染两份模型。若共享依赖改变或记录失效，则必须取得相应源并重新构建。对实际运行时内容的浏览/发布校验需要从本地或提供者取得所需载荷，不能只相信记录中的摘要。

本例 main 和双方 feature 分支都在同一个 origin，Bob 的载荷此前已经上传。Git/LFS 可以复用远端已有对象；如果所需指针对象尚未在本机且工具要求上传验证，rou 要按缺口补取，不能忽略失败。推到全新 fork 时，必须确保拟推送历史所需载荷也在那里，不能只上传当前工作区选中的 house。[LFS pre-push](https://github.com/git-lfs/git-lfs/blob/main/docs/man/git-lfs-pre-push.adoc)、[LFS push](https://github.com/git-lfs/git-lfs/blob/main/docs/man/git-lfs-push.adoc)

## 7. Bob 更新主线并按需取得合并后的版本

```bash
pushd bob-world
rou switch main
rou pull --ff-only                      # 本地 main 从 C0 快进到 M1
rou sparse add --chunk '0,0,0'
rou status
popd
```

pull 先 fetch，再固定并验证 origin/main，调用 git merge --ff-only，最后按原有 bench 选区准备 M1 的源。此时 world JSON 已包含 Alice 的新屋顶，但 house.blend 可以仍不在工作区。再 add house 区域后，才取得屋顶源和必要依赖；已有木纹复用本地缓存，bench 仍保留。

如果 Bob 的 main 已有独立本地提交，ff-only 应明确拒绝自动整合，让他选择 rou merge；不偷偷 rebase。源工作区和浏览资源获取分开：查看 M1 时，独立 Godot 通过本地版本桥接/提供器按位置取得 runtime GLB，不需要这些 `.blend`。正式发布签名与 P2P 节点留在 S2。

## 8. 同 chunk、同 entity 和同 .blend 的变体

同 chunk 的两个不同 entity 可以按 `rou sparse set --entity house`、`--entity bench` 分别取得源；若使用 `--chunk` 选择该块，则会选择其中所有相交实体及其源依赖。不同 entity 的独立源通常能自动合并，共同 chunk 页仍由 rou 重建。

同一 entity 的 JSON 有冲突时，先决定最终位置、绑定和属性；Git 文本合并无冲突也仍需检查 entity ID、依赖和世界规则。不同 entity 若共存在同一 `.blend`，仍共享一个二进制文件，不能承诺自动合并其中对象。

同 `.blend` 两端修改时，rou 从 Git index 冲突阶段取得 base/ours/theirs 的 LFS 指针，按需取三份真文件放在 `.rou/conflicts/<operation>/`。作者用 Blender 比较/合成，最终文件保存回正常工程路径：

```bash
rou status
# 在 Blender 合成两个版本，将结果保存到 sources/house/house.blend。
# 如实体输入也冲突，先解决对应 JSON。
rou add sources/house realm/authoring/house.json realm/entities/house.json
rou build
rou add runtime realm/derived
rou commit -m "Resolve both house edits"
```

最终源、输出与实体记录进入同一 Git 提交；`.rou/conflicts` 中的辅助副本不提交。停止合并可用 `rou merge --abort` 委托 Git，但必须保护合并期间的新编辑，不能声称任何状态都能无损自动回退。

## 9. 文件状态与失败处理的共同规则

- sparse 改变的是文件呈现，不是提交或合并范围；未选路径仍属于完整 Git tree。Git 合并可以临时呈现选区外冲突文件，解决前不重新收缩这些路径。
- 收缩选区先检查本地修改与作者备份，不能因 ignored 就授权自动清理。缓存回收是独立操作。
- build 冻结工作树输入副本；编辑器未保存内存不算构建输入。执行期间输入变化时，输出保留为过期结果，不静默覆盖新状态。
- rou add 不隐式添加其他模型或派生文件；rou commit 校验实际 index，保留 Git 的部分暂存语义。index 已有源 v1 和输出 v1，工作树另存未暂存 v2 时，允许提交 v1 并提示剩余修改；只有把源 v2 暂存却仍搭配输出 v1 时才拒绝不一致提交。
- push 校验实际要推送的固定 Git 提交，使用明确 refspec；不能校验 A 后因本地分支并发推进而实际推送 B。原生 Git push 与 LFS 上传分别有失败状态，不是分布式原子发布。
- `.rou` 的锁只串行 rou 自己，不能锁住另一个 Git/IDE；多个 Git/LFS 操作失败后报告实际已完成步骤，不自动 hard reset 用户修改。
- 本例普通 push 是向明确远端共享版本，正式 release 的不可变签名、权限和资产 pin 另由 S2 publish 完成。

本提案只修改协作接口与实施计划。G0 的 35 项隔离检查仍是底层机制证据，以上统一 rou 工作流需要 G1–G4 重新实现并在当前 Mac 验收。

## 10. 数据、就绪状态与提交校验的补充

`sources/` 保存作者工程内容，包括网格、材质、贴图和创作场景；`realm/entities/` 保存世界实例的轻量记录，如稳定 ID、authoring binding ID 和世界放置。两处不各存一份相同模型。`realm/authoring/` 将 binding 连接到 `.blend` 文件及内部入口；一份工程可以包含多个实体入口。`.blend` 也可能保存位置和实体标记，但跨工具的放置以 entity 记录为权威，编辑器须同步基线，不能让旧 Scene 位置覆盖新的世界记录。

cone 是 Git 按目录子树选择的模式，不是世界空间锥体。例如选 `sources/house` 会包含该目录及子目录，也包含根和祖先目录直属文件。rou 根据空间依赖生成这些目录，再独立限制实际 LFS 载荷下载，作者不必手写 cone 参数。

| 状态/操作 | Git 工作区是否因此 dirty | 世界或文件是否就绪 |
| --- | --- | --- |
| 只执行 LFS fetch | 否，工作文件不变 | 对象已进缓存，文件不一定已展开 |
| 将当前指针展开为匹配的真实载荷 | 正常 LFS filter 配置下不会因此 dirty | 指定文件可用，其他模型可能仍缺失 |
| 调整 sparse 呈现而不改文件内容 | 不等于暂存删除 | 选区外文件可能不存在 |
| 修改源工程或生成不同 GLB | 已跟踪路径显示修改，新路径显示 untracked | 还需暂存及一致性检查 |
| Git clean，但 GLB 对应旧源 | 可以 clean | 仍不是通过验证的完整世界 |

LFS 用 clean/smudge 将工作树字节映射到同一份指针，不能按指针文本与 `.blend` 的文件长度差异推断 dirty。下载了另一个版本只影响缓存；若把别的版本字节写进当前路径，则会产生真实变更。错误的 attributes/filter 配置须单独报错。[LFS 存储规范](https://github.com/git-lfs/git-lfs/blob/main/docs/spec.md)

build 的正式输出计划进入 Git/LFS，但生成文件不等于自动跟踪：已有 GLB 改变会显示 modified，新的 GLB 在 rou add 前是 untracked；临时渲染与构建文件仍在被 ignore 的 `.rou/build/`。

rou commit 的默认检查针对准备提交的完整逻辑树，包括：未解决冲突、格式及不可变世界规则、entity ID 与绑定/依赖引用、位置/AABB/chunk 索引的一致性、源/依赖/导出设置与构建记录对应关系，以及新产物的哈希、长度和支持格式。未改变且已有可信验证结果的对象可以复用检查，不要求每次取全世界所有源。还未取得实体字节时只能报告对应层级未验证，不能把记录摘要匹配说成已经打开并审查了模型。

有效中间作品可以提交，例如只完成建筑框架、但其源和 GLB 一致；不允许默认提交的是断裂引用、冲突标记、旧输出配新源等内部不一致状态。检查不评价美术完成度，也不要求普通说明文件的提交运行 Blender。远端是否持有所有待共享载荷属于 push/publish 的额外检查；本地提交成功不是远端可用性证明。

`switch -c NAME` 与 `checkout -b NAME` 都可以从同一指定起点创建并切换分支；switch 专注版本切换，checkout 还承担路径恢复等其他语义。从当前提交新建分支通常不改模型；换到其他提交时是否下载由所选路径、缓存和 LFS 配置决定。rou 保持 skip-smudge 后显式准备选区，而不是依赖两种命令名字决定下载。[Git switch](https://git-scm.com/docs/git-switch)、[Git checkout](https://git-scm.com/docs/git-checkout)

Alice 合入 Bob 的版本后，B 的元数据和 LFS 指针必须进入完整结果；本地是否需要取得 B 的字节取决于后续动作。只重建可验证的索引时可复用既有记录；若 B 进入选区、需要重建几何、视觉审阅、字节校验，或目标远端缺载荷，就必须补取。不能把“Git 合并完成”描述成所有模型已下载，也不能宣称 merge 永远无需下载。

尤其新引入的 GLB 若没有可复用的可信验证结果，rou commit 应取得它进行必要的字节和格式检查，再认定该层级通过；不会只因 Git 已合并指针就信任模型。取得 GLB 不要求同时取得作者 `.blend`，源工程可编辑性与发布载荷可用性仍分别检查。

## 11. 视觉 diff 的后续方向

`rou diff A B` 先比较稳定 entity ID、绑定、位置及模型版本。可进一步提供拟议的 `rou diff A B --visual`：从 Git/LFS 准备两版资产与依赖到 `.rou/diff/`，让 Blender 在只读比较场景中切换、并排或用不同颜色/线框叠加。它不切换作者当前分支，也不自动将对比副本提交。

Blender 的 Render Slots 支持保存多次渲染并用 J/Alt-J 切换；对象 Wireframe、颜色与 viewport overlays 可帮助观察几何差异。这些是可组合的原生工具，读取两个 Git 版本、绑定同一实体和搭建对比场景仍需 rou 集成。[Render Slots](https://docs.blender.org/manual/en/4.4/editors/image/introduction.html)、[Viewport Display](https://docs.blender.org/manual/en/dev/scene_layout/object/properties/display.html)

第三方 [BlenDiff](https://github.com/Vishrut2403/blendiff) 可作为结构提取/差异比较的研究候选；它依赖抽取的 sidecar 快照，对 mesh 几何等仍有手工协调限制，本项目未安装或验证。若借用比较能力，版本来源仍应为 Git，不能再引入另一套权威历史。

画面对比需固定相机、灯光、帧、颜色管理和采样条件，减少无关变化。画面相同不等于工程完全相同，视觉 diff 也不等于 `.blend` 自动合并；此功能不作为 Git 后端基础接入的前提。
