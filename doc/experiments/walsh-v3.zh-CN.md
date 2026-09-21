# Villa Anahid 第三版：房间布局与家具

2026-09-09。本轮针对卧室分布、厨房缺失、家具粗糙和沙发朝向进行重建。代码在原有 Blender → GLB → Godot 链路上运行，产物放在 `build/walsh-v3/` 和 `dist/walsh-v3/`，保留前两版产物。

## 房源依据与准确性边界

主要依据是[房源官网的楼层说明](https://villaanahid.com/details-main-residence/)、[主楼照片](https://villaanahid.com/main-residence/)和[独立 ADU 说明](https://villaanahid.com/details-adu/)。重点核对了厨房、家庭厅、下层酒墙、影音室、主卧与浴室的照片。

主楼是 **5 间卧室**：二层 1 间可兼作办公室的套房，三层主卧加 3 间套房；独立 ADU 另有 **2 间卧室**。上一版总共 7 张床并非数量本身错误，错误在于把主楼四间次卧都排列在三层，并将 ADU 做成单纯的卧室长条，其他生活空间很难辨认。

[2021 年 Atherton 审批资料](https://www.athertonca.gov/DocumentCenter/View/9241/349-Walsh-Final-Packet-Materials)最后两页提供场地图和外观透视，**没有可用于复原每个房间的完整室内平面图**。官网可以确认房间用途、楼层和视觉特征，不能确认本文模型中的尺寸和具体墙线。因此本版仍是依据照片的近似重建，不是测绘或竣工模型。入口、服务空间、庭院、楼层桥接及部分隔墙属于建模推定。

用户提供的[演示视频](https://x.com/realYunfanYe/status/2095612137582526615)作为暖色室内、弧形家具和简洁导览画面的视觉参考。没有复制视频或房源照片作为应用贴图，也没有推断其渲染实现。

## 本版实现

- 二层主厨房改为两面浅橡木柜体、双大理石瀑布岛台，含烤箱、灶具与炉架、抽油烟设备、冰箱、酒柜、水槽龙头和四张吧椅。另建独立备餐厨房；厨房与家庭厅连通，各有专门视点。
- 客厅重做两张弧形软包沙发、围合靠背、圆形靠垫、座垫滚边、椭圆矮茶几、花器和书籍。沙发按茶几位置确定朝向；餐椅和室外柚木沙发也面向使用中心。
- 主楼卧室按官网楼层重新分布，增加门洞、套房卫浴、主卧浴室和衣帽间。ADU 有独立厨房、起居区、两间卧室及各自淋浴间。模型未完整细化房源列出的所有服务设施和卫浴数量。
- 下层恢复植物墙、酒墙与迎宾厅，新增三排共九座影音室；上层增加公共休息区和三张工作台。健身房有跑步机与玻璃隔开的桑拿区。
- 楼梯改为连续的两段旋转楼梯，每层 28 级、每级 0.15 米；楼板给楼梯留洞并连接平台。修正地形穿入影音室的问题。
- 床增加床垫、曲面软包床头、垂落被褥、枕头、床尾织物、床头柜和灯具。饰面使用打包的原创纹理，软包增加细微法线纹理。

材质和家具仍有程序建模的简化痕迹，植被、真实织物褶皱、反射和柔和间接光也未达到照片级重建。不能将这次改进表述为已经完全复现参考视频的效果。

## 查看器

导览由 8 个增加到 **16 个视点**，保持 S1 格式的 16 项上限。底部按钮按两行排列，可按 Grounds、L1、L2、L3、ADU 筛选；Map 显示按楼层区分的房间示意，点击房间可访问，包括不在主导览中的三层次卧。

`[` / `]` 遍历全部视点，数字 1–8 选择前八个。房间之间采用短暂淡入淡出，避免直线镜头穿过隔墙；自由探索仍是无碰撞的飞行相机。保留日光/暮色、Quality/Performance、电影视图和隐藏界面。

房间图作为房源实验的可选 `listing` 展示信息存放，不是 realm 协作协议或权限模型。没有引入加密、服务器、登录、P2P 或新的 glTF 扩展。原有资产大小、顶点、索引、节点与图片限制不变。

## 重建与检查

```sh
blender --background --python-exit-code 1 --python examples/villa_anahid/create_scene.py
python3 examples/villa_anahid/verify.py
python3 -m unittest discover -s toolchain/tests -v
python3 toolchain/build/verify_s1.py
godot --path client/app --script ../tests/capture_walsh.gd
python3 toolchain/build/package_viewer.py --realm build/walsh-v3/preview/realm.json --output-dir dist/walsh-v3 --platform macos
python3 toolchain/build/benchmark_viewer.py --fixture walsh-v3 --seconds 132
```

几何和房间生成入口是 `examples/villa_anahid/create_scene.py`，家具库是 `examples/villa_anahid/furniture.py`。重建会复用前版的生成代码，但不依赖前版 `.blend`、下载照片或缓存 PDF。Blender 5.2.1 LTS、Godot 4.7.2；Python 构建脚本只使用标准库，Blender 内使用自带 NumPy。

`room-audit.json` 同时记录房间计划和合并前真实几何的检查结果。验证器从实际床组件确定数量和位置，并以沙发座面、靠背网格的几何中心验证其是否朝向茶几；不是只检查记录的旋转角。Godot 回归另验证厨房按钮、楼层筛选、16 个按钮是否在窗口内、地图是否进入正确楼层的次卧，以及原有加载、卸载和篡改哈希拒绝行为。

## 产物与实测

完整源场景：`build/walsh-v3/villa-anahid.blend`。独立应用：`dist/walsh-v3/macos/Villa Anahid.app`。压缩包：`dist/walsh-v3/Villa-Anahid-v3-macOS.zip`。

真实 Godot 截图：`build/walsh-v3/view-00.png` 至 `view-15.png`；其中 `view-02.png` 为客厅，`view-04.png` 为厨房，`room-map.png` 为可点击的房间图。第三方天空的许可说明随应用附带；参考照片、视频、PDF 和 Blender 源文件不进入运行包。

实际床组件检查为主楼二层 1、三层 4、ADU 2；7 组室内沙发的几何朝向均通过。55 个实体通过完整 bundle 校验，Python 8 组格式测试、Godot 723 项检查及生命周期回归通过。所有 16 个视点检查了真实渲染截图，另检查了房间图的标签与按钮布局。

运行 GLB 共 **75.05 MiB**，1,335,674 个顶点；最大单实体 3.07 MiB / 54,016 顶点，满足原有预算。macOS ZIP **114.6 MiB**。应用包仅含目录可达的 55 个 GLB，签名、ZIP CRC 和资源哈希均通过检查。

压缩包 SHA-256：`0c9fb2d526d1068f07c068046bbecb3687bfb89202c35a2255cb5a712feff06b`。

测试环境：MacBook Air M3 / 24 GiB / macOS 26.3，Godot 4.7.2 Metal Forward+，1920 × 1080。独立应用从 `/tmp` 启动，通过 `sandbox-exec` 禁止网络，两种模式分别导览 132 秒。正常浏览上限 60 FPS，测量时解除上限。原始日志在 `build/walsh-v3/measurements/`，摘要见 [测量数据](data/walsh-v3.json)。

| 模式 | P50 / P95 / P99 帧时间（ms） | 进程峰值 RSS（MiB） | 初始资产准备（s） |
| --- | ---: | ---: | ---: |
| Quality | 51.72 / 69.54 / 133.91 | 348.1 | 2.34 |
| Performance | 20.43 / 27.33 / 33.45 | 460.9 | 2.69 |

Quality 使用原分辨率、4× MSAA、SDFGI 和 SSIL；Performance 使用 75% 的 3D 分辨率、2× MSAA，并关闭 SDFGI/SSIL，界面仍为原分辨率。关键起居空间保留实时阴影，其他房间使用环境遮蔽和补光。上述数字包含完整导览和间接光更新，不保证每个视角持续 60 FPS；想优先保证流畅度，可选择 Performance。

初始资产准备从 Godot `_ready()` 到初始队列清空，不等于双击应用到可用首帧。首次着色器准备可能更慢，前版实验出现过约一分钟的准备时间。增加房间、家具和纹理后，整体资源比第二版更重；未实现共享家具实例、LOD 或长期驻留优化。

两种离线运行都加载 55 个实体且无拒绝记录，进程正常退出。Metal 退出阶段仍记录 7 个 Texture RID 警告，来源未定位；日志保留。本次验证 macOS ad-hoc 签名构建，没有声称完成 Windows/Linux 运行、公证或长期耐久测试。

