# protocol · 数据契约

本目录只有规范文本。它定义 realm 仓库与浏览包里每个文件和对象的形状、规则和哈希，不含实现。2.1 起唯一实现是 Rust 核心库 `rou_core`，Blender 插件与 Godot 查看器经它使用同一规则；见 [架构](../Realm_of_Us_Architecture.zh-CN.md)。

| 文件 | 内容 |
| --- | --- |
| [spec/s1-repository.zh-CN.md](spec/s1-repository.zh-CN.md) | realm 仓库布局；模型、实体、构建记录的字段约束表；标识符规则；格式标识清单；磁盘排版；构建、校验、选择、合并、推送与浏览包导出的语义 |
| [spec/s1-preview.zh-CN.md](spec/s1-preview.zh-CN.md) | 空间 profile 与 chunk 数学（2026-09-09 冻结）；浏览包 `realm.json`、`version.json`、`objects/`；预算；查看器加载行为；测试用例说明 |

文件名里的 `s1-` 前缀是历史命名，指本地闭环阶段；内容已是 2.1 目标契约。

## 校验分两层，没有 Schema

形状层：字段、类型、常量、范围、必填、禁止额外字段，由 `rou_core` 里每种文档对应的类型实现，解析进类型即通过；人读定义是规范里各文档的字段约束表。语义层：跨文件引用、ID 唯一与文件名一致、谱系、哈希、AABB、输入输出关系，由核心库代码实现，规范文本是其定义。两层都通过才算有效。

只有一份实现，因此不维护 JSON Schema，也没有跨实现的共同向量。审阅过的空间与目录测试用例放在 `rou/crates/rou-core/tests/fixtures/spatial.json`，已按 2.1 浏览包形状迁入核心测试；它不是协议的一部分。

## 格式标识

每个 JSON 文档的 `format` 字段形如 `名称/主版本.次版本`，完整清单与含义在 [s1-repository §1「格式标识」](spec/s1-repository.zh-CN.md#格式标识)。读取方只接受认识的值。2.1 的标识：`rou-realm/0.1`、`rou-model/0.1`、`rou-entity/0.1`、`rou-build/0.1`、`rou-blender-export/0.1`、`rou-view/0.1`、`rou-preview/0.1`、`rou-git-view/0.1`。2.1 是一次清零：这些标识全部从 0.1 起算，与 2.0 同名标识不兼容；此后不兼容改动递增次版本号并重新生成数据。

## 变更规则

- 改一条规则要同时改规范文本、核心库和测试用例，不可只改单侧；测试用例只能人工审阅后修改，不能用待测实现重新生成预期值。
- 预研阶段不保留兼容层：2.1 不读取 2.0 的目录布局（`realm/authoring`、`realm/derived`、`binding` 字段、`realm.json` 里的 `entries`）与共享索引文件，不保持旧浏览根哈希。
- 规范 JSON 编码统一为 RFC 8785；所有哈希对 JCS 编码的字节或实际文件字节计算，规范里逐处写明用的是哪一种。磁盘排版固定为多行，与哈希无关。
- S2 的公开网络格式、签名与发布记录尚未冻结，不在本目录。
