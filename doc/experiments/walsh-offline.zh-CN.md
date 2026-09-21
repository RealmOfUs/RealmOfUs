# Villa Anahid：客厅离线画质样片

2026-09-09。本轮将工作集中在一个客厅，用 Blender / Cycles 检查模型、材质和光照的上限。场景独立于第三版 Godot 运行包，采用米制、Z 向上的 Blender 坐标；尚未转换成 S1 的 GLB 实体。

## 画面与建模依据

参考[房源官网客厅照片](https://villaanahid.com/main-residence/)中的两张浅色弧形沙发、圆形靠垫、薄椭圆茶几、石材壁炉、可收起的落地玻璃和植物庭院，并参考[用户提供的视频](https://x.com/realYunfanYe/status/2095612137582526615)的暖色室内气氛。房间尺寸、墙线和镜头路线是根据照片推定的独立美术练习，不是完整房屋的测绘重建。

沙发重新构造连续软包靠背、圆润扶手、座垫、细线缝边和有轻微褶皱的靠垫；茶几增加薄桌面、弧形支脚、带封面文字的书和花器。场景包含真实的壁炉凹槽、地板拼缝、灯槽、天窗、泳池深度和植物网格。三个镜头分别查看整体、家具细节、壁炉与庭院。

光照由天空 HDRI、太阳、顶灯和表面发光材质组成，使用 Cycles 路径追踪、间接光、多次反射、折射、景深和 OpenImageDenoise 降噪。试帧中可见的摄影补光板已移除。石材背光是低强度表面发光与次表面散射的近似；火焰和水面造型保持静态，本片的动画来自相机运动。

## 材质来源

**Powered by [Poly Haven](https://polyhaven.com/)**。使用其 [CC0 资产](https://polyhaven.com/license)：`wool_boucle`、`rough_linen` 的织物法线与粗糙度（重新染成纯色），`white_oak_veneer`、`fern_02`、`potted_plant_02`、`ceramic_vase_03`，以及项目已有的 `kloppenheim_06_puresky` 天空。下载器逐个核对提供方 MD5，并记录 SHA-256 和来源 URL。

石材颜色纹理由内置 **imagegen** 工具生成，保存为 `examples/villa_anahid/offline/assets/champagne-onyx-v1.png`，实际尺寸 1254 × 1254；[完整提示词](../../examples/villa_anahid/offline/assets/onyx-prompt.txt)和[来源记录](../../examples/villa_anahid/offline/assets/provenance.json)一并保留。它只用于 3D 材质表面，视频画面由 Blender 渲染。房源照片和参考视频不进入场景或输出贴图。

## 复现

本机验证环境为 MacBook Air M3 / 24 GiB，Blender 5.2.1 LTS，Cycles / Metal。Blender 对 Apple Silicon 的 Metal 支持见[官方说明](https://docs.blender.org/manual/en/latest/render/cycles/gpu_rendering.html)。脚本当前选择 Metal GPU，移到其他系统需更换设备后端。

```sh
python3 examples/villa_anahid/offline/fetch_assets.py
blender --background --python-exit-code 1 \
  --python examples/villa_anahid/offline/create_scene.py

blender --background build/walsh-offline/living-room-cycles.blend \
  --python-exit-code 1 --python examples/villa_anahid/offline/render.py -- \
  --frames 1-288 --stride 2 --width 1920 --samples 64 \
  --min-samples 16 --threshold .035 --label frames

blender --background build/walsh-offline/living-room-cycles.blend \
  --python-exit-code 1 --python examples/villa_anahid/offline/render.py -- \
  --frames 1 --width 3840 --samples 256 --min-samples 64 \
  --threshold .008 --depth 16 --label stills

python3 examples/villa_anahid/offline/package.py
```

`.blend` 内打包外部图片，并保存全部相机关键帧。12 秒源镜头轨道每隔一帧采样，得到 144 个独立渲染画面，以 24 fps 组成 6 秒样片；相机移动速度因此是源轨道的两倍。成片为原生 1920 × 1080，H.264 / CRF 16，首尾短暂淡入淡出，没有插值帧。静帧目标是原生 3840 × 2160、16 位 PNG。

渲染器将每帧的耗时、文件大小和哈希写到 `build/walsh-offline/frames/render-report.json`，可断点续作；更改场景或渲染参数时要求使用新输出目录或显式覆盖。打包器要求 144 帧齐全、尺寸和哈希一致，再用 ffprobe 检查帧数、时长和编码，用 ffmpeg 完整解码检查视频。

## 产物与验证记录

- 视频：`dist/walsh-offline/living-room-cycles-1080p.mp4`。
- 4K 静帧：`dist/walsh-offline/living-room-4k.png`。
- 可编辑场景：`build/walsh-offline/living-room-cycles.blend`。
- 完整输出检查及实测耗时：`dist/walsh-offline/delivery-report.json`。
- 源场景图片打包与相机检查：`build/walsh-offline/source-checks.json`。
- 外部资产来源及哈希：`build/walsh-offline/asset-provenance.json`。

以上渲染结果用于评估客厅美术效果。将这些效果带回 Godot 还需要处理材质转换、光照烘焙与流式加载；本轮没有据此承诺实时帧率，也没有修改 realm 协作设计或第三版查看器。
