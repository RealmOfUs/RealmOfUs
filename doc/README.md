# Realm of Us · 共境

设计基线 **2.1 · 2026-09-16**。Git + 标准 LFS 管完整 3D 创作工程，rou 提供统一协作命令与空间/构建校验，独立 Godot 浏览确定版本。**核心工具代码统一用 Rust 实现**：`rou` 是单一二进制；同一份核心库以 C ABI 供 Blender 插件通过 ctypes 调用，以 GDExtension 供 Godot 查看器使用。2.1 迁移已实现，旧 Go/Python 规则层已删除；自动验收和实机限制见[迁移记录](experiments/rust-migration-status.zh-CN.md)。

| 文档 | 中文 | English |
| --- | --- | --- |
| 产品设计 | [设计](Realm_of_Us_Design.zh-CN.md) | [Design](Realm_of_Us_Design.en.md) |
| 架构与迁移 | [架构](Realm_of_Us_Architecture.zh-CN.md) | [Architecture](Realm_of_Us_Architecture.en.md) |
| 数据契约总览 | [protocol/README](protocol/README.md) | What each spec covers |
| 仓库契约 | [Git realm 规范](protocol/spec/s1-repository.zh-CN.md) | Repository contract |
| 空间与浏览包 | [空间 profile 与浏览包](protocol/spec/s1-preview.zh-CN.md) | Spatial profile and viewer bundle |
| 2.0 参考实现验收 | [S1 Git 结果](experiments/S1-Git-results.zh-CN.md) | Evidence and limits |
| 选型论证 | [Git 复用研究](reviews/git-reuse-assessment.zh-CN.md) · [双人流程](reviews/git-rou-collaboration-workflow.zh-CN.md) | Why Git + LFS; historical rationale |
| 原始设想 | [initial-envision](initial-envision.md) | Vision statement |
| 历史设计备份 | [old-doc](old-doc/README.md) | Baseline 0.5, historical only |

阶段划分：S1 本地闭环（空间契约、原工程、Git/LFS、分支/合并、按需载荷、构建校验、独立浏览）已由 2.0 参考实现验证；2.1 用 Rust 重写核心并接入 Blender 与 Godot，不保留任何兼容层；S2 接公开 P2P、发布签名/维护权、信令/TURN、区域 pin 与故障恢复。资产不加密，公开读取不要求平台账号。RealmHub、Supabase、Steam、Godot Editor 放置、模型/实例优化和视觉 diff 独立规划。

## 历史实验与素材

[S1.1 空间验收](experiments/S1-results.zh-CN.md)、[S1.2 实现记录](experiments/S1.2-results.zh-CN.md)、[S1.2 Mac 实机](experiments/S1.2-mac-acceptance.zh-CN.md)、[旧 S1.3 验收](experiments/S1.3-results.zh-CN.md) 保留当时结果与机器回执（`experiments/data/`、`reviews/data/`）。它们描述的旧 CLI 与自研后端已移除，对应操作指南与计划文档已删除，不能从这些文件推断当前实现。

[房源重建](experiments/walsh-study.zh-CN.md)、[视觉 V2](experiments/walsh-visual-v2.zh-CN.md)、[V3](experiments/walsh-v3.zh-CN.md)、[Cycles 离线](experiments/walsh-offline.zh-CN.md) 是独立美术实验，原始模型、资产与报告保留。
