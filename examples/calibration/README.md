# 原创校准街区

50 个实体，包含道路、房屋、树木、跨块展廊与不等长彩色坐标轴。原创场景内容声明为 CC0-1.0，用于比较分块目录和验证 Blender → GLB → Godot 的尺寸、方向及边界。

```bash
blender --background --factory-startup --python-exit-code 1 --python examples/calibration/create_scene.py
```

默认输出 `build/s1-2.1/`；使用 `-- --output build/my-calibration` 可以另存。完整工程为 `neighborhood.blend`，预览目录为 `preview/`，同一几何分别索引到 16/32/64 米分块。

运行前先在项目的 `rou/` 目录执行 `cargo build -p rou-ffi`。

S1.1 的独立统计使用 `toolchain/fixtures/calibration.json` 中已经审阅的元数据；重新生成 Blender 场景会产生新的源库文件和相应哈希，不会自动替换历史实验输入。
