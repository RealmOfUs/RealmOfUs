# 349 Walsh Road / Villa Anahid 重建实验

此实验优先验证用户提出的“headless Blender 重建房源 → Godot 漫游应用”。当前环境没有可调用的 Blender MCP，建模通过 Blender 自带 Python API 在后台完成。

## 参考资料与模型范围

查阅时间：2026-09-08。用户给出的 [Zillow 页面](https://zillow.com/homedetails/349-Walsh-Rd-Atherton-CA-94027/15598337_zpid/) 无法由抓取器直接读取，改用同地址、同一现有建筑的经纪方资料。

- [官方房源网站 Villa Anahid](https://villaanahid.com/)：入口、主层客厅、下层休息区实景照片。
- [Compass 房源](https://www.compass.com/homedetails/349-Walsh-Rd-Atherton-CA-94027/1OZIPK_pid/)：外观照片、三层建筑、11,968 sq ft、5 卧室、6 全卫/2 半卫及独立 ADU 的描述。应用不展示可能变化的售价。
- [Town of Atherton 建筑资料](https://www.athertonca.gov/DocumentCenter/View/9241/349-Walsh-Final-Packet-Materials)：2021 年场地图与透视图（PDF 第 16、17 页），用于核对阶梯体量、后侧 ADU、右侧泳池；图中泳池 26 ft × 55 ft 10 in，模型采用约 7.92 × 17.02 m。

模型还原可辨识的建筑语言与空间关系：通高玻璃入口、三层螺旋楼梯、白色悬挑框架、石材/木饰面、上层露台、泳池/水幕/植物墙和客房。房间尺寸、室内隔墙、家具、花园树种位置与地形为估计或简化；不是精确竣工复原。2021 方案也不代表全部建成细节。

材质纹理由脚本程序生成，几何由脚本建模；没有把房源照片贴在平面上假装三维场景，也没有将照片/PDF 复制到交付应用。参考资料只缓存在忽略的 `.cache/walsh/`。房源重建内容的许可没有推定为 CC0；格式中使用 `NOASSERTION`。

## 应用

原生 Godot 桌面应用，8 个导览视点、自由飞行、Golden hour/Daylight 切换、空间示意、说明面板和诊断。GLB 运行时从应用随附目录读取，可断网观看。它不提供房产交易、实时房源同步、测量或碰撞漫游功能。

源脚本：`examples/villa_anahid/architecture.py`；完整源场景：`build/walsh/villa-anahid.blend`；预览：`build/walsh/preview/realm.json`；macOS 应用：`dist/walsh/macos/Villa Anahid.app`。S1 的格式和引擎代码独立于这套房源，原创街区仍用于分块与坐标验证。

## 视觉第二版（2026-09-09）

在上述近似重建基础上追加材质、植被、家具、灯光和浏览界面的改进。S1 原版产物与测量保留；新版脚本 `examples/villa_anahid/appearance.py`、产物 `build/walsh-v2/`。详见 [第二版实验报告](walsh-visual-v2.zh-CN.md)。
