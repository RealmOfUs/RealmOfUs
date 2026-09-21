# Realm of Us · 共境

**Design documents / 设计文档 · Baseline 0.5 · 2026-09-07**

用户在本地重建真实地点，通过用户持有的副本分享，并在自己的设备上自由飞行观光。官方提供可选的目录、社区和客户端入口；自有代码采用 GPL-3.0-or-later。

Reconstruct real places locally, share user-held replicas, and explore through locally rendered free flight. The official project provides optional catalog, community, and client entry points. Project-owned code uses GPL-3.0-or-later.

## 文档入口 / Documents

| 文档 / Document | 中文 | English |
| --- | --- | --- |
| 代码架构设计 / Code Architecture | [中文版](Realm_of_Us_Architecture.zh-CN.md) | [English](Realm_of_Us_Architecture.en.md) |
| 产品与系统设计 / Product and System Design | [中文版](Realm_of_Us_Design.zh-CN.md) | [English](Realm_of_Us_Design.en.md) |

本次扩展平台 Supabase 复用规划，并提出节点内嵌 Web 管理方案。先读架构第 1–3 节了解模块和进程边界；第 6.5 节说明统一管理入口，第 8 节说明 Supabase 分工、平台身份、各模块鉴权/存储及部署。产品设计保留内容、隐私、预算与验收依据。

This revision expands Supabase reuse and proposes embedded node Web administration. Architecture sections 1–3 define module/process boundaries; section 6.5 explains the unified management entry, and section 8 covers Supabase responsibilities, platform identity, per-module authentication/storage, and deployment. Product design retains content, privacy, budget, and acceptance requirements.

## 五个核心目录 / Five core directories

| 目录 / Directory | 责任 / Responsibility | 主要语言或依赖 / Language or dependency |
| --- | --- | --- |
| `protocol/` | 规范、API 契约与公共验证 / Specifications, contracts, shared validation | Markdown, JSON Schema, OpenAPI, Go |
| `client/` | 观光、缓存与可选节点入口 / Sightseeing, caching, optional node entry | Godot, typed GDScript; native adapters |
| `node/` | 地图提供、完整镜像与拟议 Web 管理 / Map serving, mirrors, proposed Web administration | Go, embedded HTML/CSS/JavaScript |
| `toolchain/` | Blender、地图发布与软件构建 / Blender, map publication, software builds | Python, Go |
| `server/` | 平台业务、Supabase 适配与连接服务 / Platform business, Supabase adapters, connection services | Go, HTML/CSS/JavaScript; Supabase |

## 本版决定与提案 / Decisions and proposal

- 保持五目录 monorepo，初期一个 Go module，程序分别构建发布。
- 平台优先复用 Supabase Auth、PostgreSQL、Storage、Queues/pgmq 和 Cron/pg_cron；Realtime 按需启用，Studio 仅供运维。
- Go 保留签名登记、版本历史、业务权限、图片处理等项目逻辑；具体 Supabase 组合在 M3 前验证锁定。
- 拟采用节点自带 Web 页统一完整管理；Godot 仅保留启动/连接、打开页面和副本移交入口，`rou-nodectl` 后置。
- 客户端缓存与节点存储独立，本机管理和地图权限不依赖平台 Auth；观光无需本机节点或官方账号。
- 保留独立信令与 coturn；Realtime 信令适配单独验证。普通 Web 管理页不改变浏览器三维观光后置的决定。
- Windows/macOS/Linux 为正式目标，Android/iOS 在 M0 实机验证；GPL 和默认不集成 Steamworks 运行时 SDK 的决定保持。

- Preserve five core directories, one initial Go module, and independently built/released programs.
- Prefer Supabase Auth, PostgreSQL, Storage, Queues/pgmq, and Cron/pg_cron; enable Realtime by use case and restrict Studio to operations.
- Go retains signed registration, history, business authorization, and image processing; validate/pin Supabase combinations before M3.
- Propose embedded node Web administration, minimal Godot launch/connection/page/handoff entry points, and deferred `rou-nodectl`.
- Client/node storage remains independent; local administration and map authority need no platform Auth. Viewing needs no local node or official account.
- Retain independent signaling/coturn; validate Realtime signaling separately. Conventional Web administration does not bring browser 3D into scope.
- Windows/macOS/Linux remain formal targets with M0 Android/iOS device probes; preserve GPL and no default Steamworks runtime SDK.

## 状态与版本 / Status and versions

设计基线为 `0.5`，地图协议仍为 `0.1-draft`。所有目录、接口和命令是实施规划，尚未初始化业务代码。文档、协议、官方 API、本地节点管理 API 和软件版本分别管理。本版新增 ADR-012（Supabase 复用）、ADR-013（Web 管理提案）及 A13，更新 A12；中英文编号、字段与引用同步。节点 Web 入口仍标记为拟采用，尚未冻结。

The design baseline is `0.5`; the map protocol remains `0.1-draft`. Directories, interfaces, and commands are implementation plans, not initialized application code. Documentation, protocol, official API, local administration API, and software versions remain independent. This revision adds ADR-012 (Supabase reuse), ADR-013 (Web administration proposal), and A13, and updates A12. IDs, fields, and references match across languages. The Web entry remains proposed, not frozen.

本版承接并修订 0.4；更早的技术方案和架构只作历史资料。五份 Markdown 文件保持同目录以保证相对链接可用。本次仅更新设计，未验证软件或部署服务。

This revision builds on and amends 0.4; earlier proposals are historical. Keep all five Markdown files together for relative links. This update changes design only, without validating software or deploying services.
