# 可复现样例

| 目录 | 内容 | 所属范围 |
| --- | --- | --- |
| `common/` | 样例共用的 Blender 几何函数 | 示例内部复用，不属于世界协议 |
| [calibration/](calibration/README.md) | 原创 CC0 街区、坐标和跨块探针 | S1.1 测量来源与 S1.2 集成样例 |
| [villa_anahid/](villa_anahid/README.md) | Villa Anahid 房屋及离线客厅实验 | 独立展示样例 |

目录使用下划线以支持 Python 包导入。框架插件可以由任意创作者使用，样例只调用其公开导出接口；插件测试不依赖具体房屋。

建模脚本和必要原创纹理属于可复现源码。生成的 `.blend`、GLB、应用和视频放在根目录 `build/`、`dist/`；下载素材放在 `.cache/`。各样例单独声明来源和内容许可。
