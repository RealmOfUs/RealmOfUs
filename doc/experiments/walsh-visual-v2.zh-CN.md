# Villa Anahid 视觉第二版

2026-09-09。在 S1 已走通的 Blender → 实体 GLB → Godot 本地查看链路上，改进建筑展示质量。S1 原版及历史测量保留在 `build/walsh/`、`dist/walsh/`，本次产物独立放在 `build/walsh-v2/`、`dist/walsh-v2/`。

## 参考与目标

用户提供的 [Yunfan Ye 演示视频](https://x.com/realYunfanYe/status/2095612137582526615) 展示了同一房源的暮色外观、暖色灯槽、曲面家具和简洁的镜头画面。本次按这些视觉特征改进实时查看器，没有将视频帧或房源照片作为场景贴图，也不假定视频使用了何种渲染实现。

模型仍是依据公开资料的近似重建。布局、家具、树木和尺寸的准确性边界沿用 [房源重建说明](walsh-study.zh-CN.md)。新增树木、家具和饰面纹理由脚本生成；天空使用 [Poly Haven / Kloppenheim 06 (Pure Sky)](https://polyhaven.com/a/kloppenheim_06_puresky) 的 CC0 HDRI，是通用环境，不代表 Atherton 实景。

## 实际改动

- 石材和木材改用较克制的程序纹理，修正墙面与水平地面的 UV 投影；补充地砖接缝。
- 球状树冠替换为渐细树干、枝条和细叶几何；庭院加入低矮草丛，植物墙改为折叠叶片。没有透明贴片叠加。
- 沙发使用弧形靠背和圆润软包，增加细椅腿、餐桌支撑、酒柜与酒瓶、吊灯、书籍、花器和躺椅织物。
- 加入有实体灯具的檐下照明、台阶灯、灯槽和室内暖灯。默认暮色，保留日光切换。
- Godot 使用 SDFGI、屏幕空间环境遮蔽/间接光、局部反射探针，以及 4× MSAA 和 TAA；水面使用较小的法线扰动呈现环境反射。
- 界面缩减遮挡，增加 Tab 电影视图。H 可隐藏当前界面；8 个导览视点、自由移动、示意图与诊断保留。

“Quality”使用上述完整效果；“Performance”关闭 SDFGI / SSIL，以 75% 的 3D 渲染分辨率及 2× MSAA 显示，界面保留原分辨率。它们是查看器的展示选择，不是 realm 协议的一部分。

## 与运行时加载的衔接

反射探针在资产队列加载完成后生成。静态几何发生加载/卸载后，重新生成 SDFGI 数据，避免间接光仍使用旧场景；这是 [Godot SDFGI 文档](https://docs.godotengine.org/en/stable/tutorials/3d/global_illumination/using_sdfgi.html) 描述的静态遮挡物更新约束。切换光照预设时也更新反射和间接光。

高亮灯带的增强亮度在查看器中设置，GLB 仍保持 S1 支持的核心材质格式。没有启用额外 glTF 扩展，没有放宽单资产 8 MiB、10 万顶点、30 万索引、512 节点等校验限制。新版模型整体比 S1 样例重；这是本次画质实验的成本，不应据此修改协议限制或覆盖 S1 历史性能结论。

打包脚本现在可指定输出目录，并清理该产物的旧预览资产后复制当前目录可达的 GLB。应用只包含当前运行资产、天空和许可证，不包含 `.blend` 或参考视频。

## 重建与体验

```sh
blender --background --python examples/villa_anahid/appearance.py
godot --headless --path client/app --editor --import
godot --path client/app
python3 toolchain/build/package_viewer.py --realm build/walsh-v2/preview/realm.json --output-dir dist/walsh-v2 --platform macos
python3 toolchain/build/benchmark_viewer.py --fixture walsh-v2 --seconds 66
```

- 创作脚本：`examples/villa_anahid/appearance.py`，复用第一版体量并追加细节。
- Blender 完整源场景：`build/walsh-v2/villa-anahid.blend`。
- 独立应用：`dist/walsh-v2/macos/Villa Anahid.app`。
- ZIP：`dist/walsh-v2/Villa-Anahid-Visual-v2-macOS.zip`。
- 截图：`build/walsh-v2/{arrival,living,pool,lounge,primary,atrium,guest,overview,daylight}.png`。
- 同镜位对比：`build/walsh-v2/comparison.jpg`。两边视点位置、朝向和 FOV 相同，显示各自版本的默认界面和光照。

macOS 构建使用本地 ad-hoc 签名。第二版未声称通过 Windows/Linux 运行测试。

## 验证边界

Python 的 8 组格式测试、Godot 的 723 项格式/向量检查以及生命周期回归均通过。生命周期回归实际导入新场景，验证视点按钮、取消过期请求、释放 36 个实体和篡改哈希后的拒绝。资产加载过程中没有拒绝记录；所有 8 个视点及日光模式均检查了实际渲染截图。

独立 macOS 应用在禁止网络访问的进程中完成导览，使用应用内随附资源，不依赖工作目录或 Blender。测试期间仍有 Godot Metal 退出阶段的 7 个 Texture RID 警告，来源未定位；进程正常退出，原始日志保留在测量目录。本轮不是长期耐久测试。

首次独立运行和后续运行的准备时间差异很大，开发期间观察到 Metal 编译进程持续工作。下表的准备时间从 Godot `_ready()` 开始计时，到初始资产队列清空，**不是**双击应用到完整首帧的端到端时间；初次启动应预留约一分钟。帧时间样本跳过初始资产准备及其后 2 秒，但包含导览移动、GI 更新和渲染。

## 资源与实测

环境：MacBook Air M3 / 24 GiB / macOS 26.3，Godot 4.7.2 Metal Forward+。独立进程导览 66 秒，测试时解除 60 FPS 上限；正常浏览默认上限 60 FPS。原始摘要见 [测量数据](data/walsh-visual-v2.json)。

运行 GLB 共 **31.99 MiB**（S1 原版 4.81 MiB），36 个实体、982,504 个顶点。最大 GLB 2.04 MiB，最大单实体 70,043 顶点 / 97,440 索引 / 17 节点，均满足原有限制。另有 4.23 MiB 天空源资源；最终 macOS ZIP 82.7 MiB。整体增加主要来自植被几何和更细的饰面、家具；本次没有实现 LOD 或共享树模型实例。

| 测量 | P50 / P95 / P99 帧时间（ms） | 进程峰值 RSS（MiB） | 初始资产准备（s） |
|---|---:|---:|---:|
| S1 历史原版 | 7.66 / 17.73 / 26.34 | 305.6 | 0.77 |
| V2 Quality / 后续运行 | 20.95 / 29.63 / 33.76 | 320.2 | 1.41 |
| V2 Performance | 8.91 / 17.57 / 30.01 | 329.8 | 1.77 |
| V2 开发期间首次独立运行 | 20.57 / 28.96 / 36.31 | 337.3 | 55.65 |

S1 与新版的空间细节和部分导览镜位不同，以上用于展示各自工作量与画质成本，不是控制变量的渲染器对比。短时截图运行还出现过更高的尾部帧时间，导览切换和间接光重建可能产生停顿，不保证所有视角持续 60 FPS。性能预设减少效果和内部 3D 分辨率，默认仍使用 Quality。
