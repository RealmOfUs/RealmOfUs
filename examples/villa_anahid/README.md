# Villa Anahid 展示样例

依据 349 Walsh Road 房源公开资料进行的近似重建。它用于验证建模和查看流程，尺寸、墙线和家具不构成测绘结果。房屋内容许可沿用 `NOASSERTION`，参考照片和建筑 PDF 不进入交付应用；通用天空与下载素材分别保留来源声明。

## 建模模块

| 模块 | 职责 |
| --- | --- |
| `architecture.py` | 建筑体量、基础建模与组装流程；保留第一版重建入口 |
| `appearance.py` | 材质、植被与第二版视觉样式；保留第二版入口 |
| `furniture.py` | 家具几何、朝向与审计数据 |
| `create_scene.py` | 当前第三版楼层布局、设备、家具配置与场景入口 |
| `verify.py` | 根据实际网格和房间审计检查床位、厨房、沙发朝向及资产预算 |
| `offline/` | 独立的 Cycles 客厅建模、素材取得、渲染、编码与后台监控 |

这些是单次 Blender 进程中的程序化场景构建模块；版本样式共用基础函数，当前入口不会导入名为 `v2` / `v3` 的历史脚本。各轮实测报告仍保留原有版本含义。

## 房屋与查看器

```bash
blender --background --factory-startup --python-exit-code 1 --python examples/villa_anahid/create_scene.py
python examples/villa_anahid/verify.py
python toolchain/build/verify_s1.py --realm build/walsh-v3-2.1/preview/realm.json --viewer-realm build/walsh-v3-2.1/preview/realm.json
godot --path client/app -- --realm="$PWD/build/walsh-v3-2.1/preview/realm.json" \
  --sample=villa-anahid --sample-data="$PWD/build/walsh-v3-2.1/sample.json" --station=0
```

建模可传 `-- --output build/my-villa`，检查可传 `--scene build/my-villa`。默认输出 `build/walsh-v3-2.1/`，旧应用与旧输出保持原样。运行前先在项目的 `rou/` 目录执行 `cargo build --workspace`，查看器原生库准备方式见根目录 README。桌面导出沿用 `toolchain/build/package_viewer.py`，详见根目录 README。

样例的 `sample.json` 位于源工程旁边，包含房间说明与导览路点；它不属于 `preview/` 浏览包。通用查看器默认仍使用包围盒起点，只有显式选样例路点才跳转。

## 离线客厅

```bash
python examples/villa_anahid/offline/fetch_assets.py
blender --background --factory-startup --python-exit-code 1 --python examples/villa_anahid/offline/create_scene.py
blender --background build/walsh-offline/living-room-cycles.blend --python-exit-code 1 --python examples/villa_anahid/offline/render.py -- --frames 1-288 --stride 2 --width 1920 --samples 64 --threshold .035 --label frames
python examples/villa_anahid/offline/watch.py --renderer-pid PROCESS_ID
```

最后一条适用于渲染进程已在后台运行的情况，需填入其实际 PID；监控器接着渲染 4K 静帧、编码并检查视频。`package.py` 也可独立编码已有完整序列。默认产物仍在 `build/walsh-offline/` 和 `dist/walsh-offline/`。

`offline/create_scene.py -- --output build/my-offline --preview 1 --width 640 --samples 8` 可用于隔离的快速入口检查。原创石材纹理、生成提示词和出处保留在 [offline/assets/](offline/assets/README.md)；视频由 Blender 渲染。历史交付与参数见 [离线实验报告](../../doc/experiments/walsh-offline.zh-CN.md)。

单独渲染可给 `offline/render.py` 传 `--output-dir`；编码可给 `offline/package.py` 传 `--build-dir` 和 `--output-dir`，用于隔离检查而不覆盖已有交付。
