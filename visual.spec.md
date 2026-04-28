spec: task
name: "Makepad Android 轨迹回放 视觉设计 spec"
tags: [visual, design, mockup, makepad, android, trajectory-replay]
---

## 意图

锁定 mockup (design/refs/storyboard-1.png 与 design/refs/storyboard-2.png) 已经达到的视觉效果作为视觉设计真值, 让后续 ChatGPT Image 2 出图、Figma 标注、cc 代码实现稳定收敛到同一视觉锚点, 避免视觉漂移。

mockup 已把整体氛围 (深色底 + 暖橙速度色 + guard 红边 pulse + 收尾 stats 网格) 立起来, 但 `design/refs/storyboard-{1,2}.png` 残留 4 处与 prd.md §6 契约不一致 (顶栏出现返回箭头 / profile 标签英文 "Cycling" / 主回放仅暖橙单色 / 主画布右上角第三个按钮字符显示 "3D" 应为 "2D")。本 spec 用 BDD 场景把这些不一致强制纠偏, 同时把 mockup 中正确的部分锁成可被机械检查的契约。

**P11.3 进度**: 上述 4 处残留**已通过 P10/P11 代码实施修正**, 单屏 demo ground truth 见 `design/refs/v2/s2-main-replay.png` (S2 主回放) 与 `design/refs/v2/s4-stats.png` (S4 收尾)。原 storyboard-{1,2}.png 4-grid/3-grid 拼图重画归 P12 设计师 follow-up (cc 无 image montage 工具)。

本 spec 是 prd.md §6 视觉契约的执行层, 由设计、产品、cc 三方共同消费。

## 已定决策

视觉设计决策按 5 组组织。

### Scope 与真值优先级

- 真值优先级: 本 spec 的 BDD 场景 > prd.md §6 视觉契约 > design/refs/*.png 参考底板
- 当 mockup 与本 spec 冲突时, 以本 spec 为准, mockup 必须重画
- 本 spec 由设计 / 产品 / cc 共同消费, 任何一方修改均需同步更新本文件
- 视觉参考底板必须存放在 design/refs/ 并加 README 索引到故事板编号

### 屏幕拓扑

- 屏幕分 4 块: 顶栏 / 主画布 / HUD / 底栏
- 顶栏组成固定 3 件: 路线名 (左) / 数据源徽章 (右, 与主画布右上角控件组共享水平区域但归属顶栏图层) / profile 标签 (中)
- 顶栏不出现导航返回控件 / 设置入口 / 登录入口 / 分享入口
- 主画布是单层渲染目标, 不嵌套卡片
- HUD 与主画布是兄弟节点, 无父子层级
- 主画布右上角垂直堆叠 3 件控件, 从上到下依次为: 速度图例 → 圆形 compass 按钮 → 圆形 2D 锁定按钮
- 底栏从左到右固定 4 件: 当前时间 / scrubber / 总时长 / 倍速按钮组 + 暂停按钮
- 系统状态栏 (信号 / wifi / 电量 / 时间) 由 Android 系统层绘制, app 不参与渲染

### 触摸热区 (mobile a11y, 新增)

所有可点击控件的可触发命中区 (hit slop) ≥ 48 × 48dp, 与视觉尺寸独立解耦. mockup 中保留设计视觉尺寸, hit slop 通过 Makepad widget 的扩展命中区实现, **不**改变视觉布局.

| 控件 | 视觉尺寸 | 最小 hit slop |
|---|---|---|
| compass 按钮 | 32dp 圆 | 48 × 48dp |
| 2D 锁定按钮 | 32dp 圆 | 48 × 48dp |
| 暂停 / 重播按钮 | 36dp 圆 | 48 × 48dp |
| 倍速按钮 1x / 4x / 16x | 视设计 | 各 48 × 48dp, 相邻按钮间最小 8dp 间距 |
| scrubber thumb | 16dp / 24dp 圆 | 48 × 48dp (沿 scrubber 全宽延伸) |
| guard 卡片 "知道了" 按钮 | 视设计 | 48 × 48dp |
| 速度图例渐变条 | 96 × 8dp | 不可点击 (装饰) |
| 起点 / 终点 marker | 8dp 圆点 | 不可点击 (装饰) |
| 主画布双击 (触发 AI 询问) | 全画布 | 全画布, 但避开 z-30 / z-40 / z-50 浮层区域 |

约束:
- 多个 hit slop 像素重叠时, 命中优先级按 z-index 高者 (z-50 > z-40 > z-30 > z-20 > z-10)
- hit slop 区域不绘制视觉反馈, 仅影响 FingerDown 命中判定
- 触摸命中后控件视觉状态变化 (例如 thumb 16→24dp) 仍按各控件独立 BDD 规约

### Z-order 层叠 (新增)

显式定义 6 个层的叠加顺序, 防止实现期 stacking 决策飘移. 在 Makepad 中以 widget 声明顺序兑现; 编号语义对齐 web z-index scale 习惯.

| z 等级 | 层 | 内容 |
|---|---|---|
| z-10 | 主画布 | 轨迹 polyline / 已走段 / 未走段 / 水域层 / 地名标签 / 起终点 marker / 当前位置 dot+halo / 粒子流 |
| z-20 | 顶栏 + HUD | 顶栏 (路线名 / profile 标签) 与 HUD 4 单元同 z 层, 位置不重叠 |
| z-30 | 主画布右上角控件组 | 速度图例 / compass 按钮 / 2D 锁定按钮 (垂直堆叠) |
| z-40 | 同步徽章 | 顶栏右侧持久徽章 (Fetching / Success / Fallback 三态) |
| z-50 | 浮层 (单次) | contract guard 拦截卡片 / scrubber echo trail (拖动期间) |
| z-system | Android 系统状态栏 | 由系统绘制, 不参与 app z-order |

约束:
- 同 z 等级的控件不允许像素重叠
- contract guard 卡片激活期间允许遮挡 z-10 ~ z-40 任意区域
- scrubber echo trail 仅在 scrubber thumb 后方衰减, 不进入主画布中心区域

### S2 主屏元素细化

- 主画布渲染只用 2D shader, 不存在 3D 模式 / 3D 切换 / 3D 渲染管线
- 速度图例为横向渐变条, 约 96dp × 8dp, 渐变色顺序 speed_low → speed_mid → speed_high
- 速度图例上方挂 "速度 (m/s)" 标签, 两端挂 "0" 与轨迹归一化最大速度刻度
- 速度图例最大刻度值取启动期计算的 max_speed_mps 向上取整, 启动后不再变
- compass 按钮圆形 32dp, 内嵌罗盘 "N" 字形, 位于速度图例下方
- 2D 锁定按钮圆形 32dp, 内嵌 "2D" 文本, 位于 compass 按钮下方
- 2D 按钮是装饰性锁定指示, 点击无副作用 (demo 不存在 3D 模式)
- 主画布渲染地名标签层 4-10 条, 字号 11sp, 颜色 ink_secondary @ 40% opacity
- 地名标签层不随 playback_progress 变化, 启动期一次性布局
- 起点 marker: accent_success #10B981 圆点直径 8dp, 旁标签文本 "起点"
- 终点 marker: ink_secondary #7A7B8C 圆点直径 8dp, 旁标签文本 "终点"
- 起点 / 终点 marker 始终可见, 不被已走段 / 未走段染色覆盖
- 当前位置 dot 直径 6dp, 外包 halo 直径 16-24dp 之间随 HR pulse 呼吸
- 当前位置 halo 颜色固定 speed_high #00E5FF, 不随当前速度区间变化
- 主画布存在水域装饰层 (河流 / 湖泊), 颜色 speed_high #00E5FF @ 8% opacity
- 水域层为静态 SDF, 不参与任何 shader uniform 驱动, 帧间几何不变
- HUD 4 单元每个均带迷你进度条 (mini sparkline), 高度 4dp, 宽度填满单元宽度
- 迷你进度条显示该字段当前值在全轨迹该字段 (min, max) 区间的归一化位置
- 迷你进度条已填充段颜色 = 该字段对应主色, 未填充段 unwalked_segment #3B3B46
- 倍速按钮活跃态: 圆角矩形 8dp, 底色 ink_primary @ 8% alpha, 文字色 ink_primary
- 倍速按钮非活跃态: 透明底色, 文字色 ink_secondary
- 任意时刻 1x / 4x / 16x 中恰有 1 个为活跃态, 默认 4x
- 暂停按钮位于倍速按钮组右侧, 圆形 36dp, 底色 ink_primary @ 8% alpha
- 暂停按钮播放态显示三角 ▶ 字形, 暂停态显示双竖线 ⏸ 字形
- 暂停按钮与三个倍速按钮在同一水平基线上对齐

### 文案与语种

- profile 标签中文化: 骑行 / 跑步 / 徒步 / 步行 / 通勤 / 旅行 / 飞行
- 同步状态文案三态: "同步中..." / "已同步" / "本地缓存", 分别对应 NetworkState 的 Fetching / Success / Fallback (与 prd.md §3.2 S0 一致)
- contract guard 拦截卡片标题文案为 "违反契约 c1.2"
- guard 卡片关闭按钮文案为 "知道了"
- stats 收尾 4 项命名: "总距离" / "总时长" / "累计爬升" / "平均心率"
- HUD 单位标识可见: km/h / bpm / m / rpm
- 倍速文案: "1x" / "4x" / "16x"

### 色彩契约

- 色 token 完全沿用 prd.md §6.1 的 11 个 token, 不引入新色
- 主回放静态画面必须同时可见 #FF8A3D / #00E5FF / #E8E8F0 三段速度色
- 任意单一颜色像素占主画布比例不超过 60%
- 三段速度色合计像素占比不少于主画布的 40%
- guard 卡片底色固定 accent_warning #FF3B6E (描边 + 主按钮); 卡片正文背景使用 bg_secondary #14141C @ 90% alpha
- 数据源徽章三态颜色:
  - "已同步" 底色 accent_success #10B981 (Success 态)
  - "同步中..." 底色 ink_secondary #7A7B8C @ 60% alpha + 左侧 ⏵ 图标 (Fetching 态; 注: mockup 用橙色为占位, 当前 prd.md §6.1 token 集合无 amber, 待评审引入 amber token 后再校准)
  - "本地缓存" 底色 ink_secondary #7A7B8C @ 60% alpha + 左侧 ⊘ 图标 (Fallback 态)
- HUD 4 字段 mini bar 已填充段主色 (本 spec 暂行约定, 待 prd.md §6.1 评审定稿):
  - 速度 = speed_mid #FF8A3D
  - 心率 = accent_warning #FF3B6E (复用警示色作视觉强度暗示, 与 contract guard 触发无语义关联)
  - 海拔 = ink_secondary #7A7B8C
  - 踏频 = speed_low #E8E8F0
- mini bar 未填充段统一使用 unwalked_segment #3B3B46

### 度量与字体

- HUD 4 指标顺序固定: 速度 → 心率 → 海拔 → 踏频
- 卡片圆角不超过 12dp
- 徽章圆角等于 4dp
- Display / HUD 字体: Inter sans, 启用 tabular figures (font-feature-settings: "tnum")
- 数据值字体: IBM Plex Mono (西文数字与单位)
- 中文字体回退链 (两类字体共用): "Source Han Sans CN" / "Noto Sans CJK SC" / system-ui (Android 默认 PingFang / 思源黑体)
- 中文 + 数字混排时 (例如 "8.6 m/s" / "1时 18分" / "回放已完成"): 数字走 Inter / IBM Plex Mono 的 tabular figures, 中文字符回退到中文字体; 基线对齐通过 Makepad 字体 metric 调整, 视觉基线偏移容忍 ≤ 1px
- 不允许装饰性外框 / 嵌套阴影 / 与轨迹无关的渐变背景图层

## 边界

### 允许修改

- design/refs/ 下的视觉参考图可重画
- prd.md §6 文字描述可补充, 但不得与本 spec 冲突
- spec.spec.md 可引用本 spec 的视觉相关 BDD 场景
- 本 spec 的场景列表可增不可减
- 新色 token 必须先入 prd.md §6.1, 才能进入本 spec

### 禁止做

- 不做嵌套卡片
- 不在顶栏放导航返回控件
- 不让任意单一颜色占据主画布像素超过 60%
- 不修改 HUD 4 指标顺序与命名
- 不让 profile 标签 / 同步徽章 / guard 卡片标题 / stats 4 项以纯英文呈现
- 不修改 contract guard 卡片标题文案 "违反契约 c1.2"
- 不修改 stats 收尾 4 项的命名
- 不引入横屏布局
- 不引入浅色主题
- 不引入用户定制配色
- 不引入与 PlaybackState 字段或 shader uniform 无关的视觉变化

## 完成条件

场景: 顶栏 3 件套布局
  测试:
    包: visual-review
    过滤: top_bar_three_elements
  假设 用户进入主回放屏幕
  当 视线扫描顶栏区域
  那么 看到路线名 / 数据源徽章 / profile 标签 共 3 个元素
  且 不出现导航返回控件
  且 不出现设置入口 / 登录入口 / 分享入口

场景: profile 标签中文化
  测试:
    包: visual-review
    过滤: profile_label_zh
  假设 manifest profile 字段为 cycling
  当 profile 标签渲染
  那么 标签显示文本包含 "骑行"
  且 不出现纯英文 "Cycling"

场景: HUD 4 指标顺序固定
  测试:
    包: visual-review
    过滤: hud_field_order
  假设 主回放屏幕已渲染
  当 从左到右扫描 HUD 4 单元
  那么 第 1 单元数据为速度
  且 第 2 单元数据为心率
  且 第 3 单元数据为海拔
  且 第 4 单元数据为踏频

场景: 数据源徽章三态切换
  测试:
    包: visual-review
    过滤: sync_badge_states
  假设 app 刚启动且未拉取 manifest
  当 manifest 拉取进行中
  那么 徽章显示 "同步中..."
  当 manifest 拉取成功
  那么 徽章显示 "已同步"
  且 "已同步" 文案至少可见 800ms
  当 manifest 拉取失败或超时进入 Fallback 态
  那么 徽章显示 "本地缓存"
  且 "本地缓存" 文案持续显示直到 app 退出 (不通过 toast 一闪而过)

场景: 主回放三段速度色齐现
  测试:
    包: visual-review
    过滤: speed_ramp_visible
  假设 主回放屏幕已渲染当前轨迹
  当 主画布被像素采样
  那么 至少存在 1 段轨迹颜色匹配 #FF8A3D
  且 至少存在 1 段轨迹颜色匹配 #00E5FF
  且 至少存在 1 段轨迹颜色匹配 #E8E8F0

场景: 单一颜色占比克制
  测试:
    包: visual-review
    过滤: no_monochrome
  假设 主回放屏幕已渲染当前轨迹
  当 主画布像素被分桶统计
  那么 任意单一速度色像素占比不超过 60%
  且 三段速度色合计占比不少于 40% 主画布像素

场景: contract guard 卡片文案与底色
  测试:
    包: visual-review
    过滤: guard_card_text_color
  假设 用户触发 AI 建议
  当 contract c1.2 拒绝该建议
  那么 拦截卡片标题文案为 "违反契约 c1.2"
  且 卡片底色匹配 accent_warning #FF3B6E
  且 卡片提供 "知道了" 关闭按钮

场景: stats 收尾 4 项命名固定
  测试:
    包: visual-review
    过滤: stats_field_names
  假设 进入 S4 stats 收尾屏幕
  当 4 项数据淡入完成
  那么 显示 "总距离" 标签
  且 显示 "总时长" 标签
  且 显示 "累计爬升" 标签
  且 显示 "平均心率" 标签

场景: 卡片与徽章圆角约束
  测试:
    包: visual-review
    过滤: corner_radius
  假设 任意卡片或徽章渲染
  当 测量卡片圆角
  那么 圆角不超过 12dp
  当 测量徽章圆角
  那么 圆角等于 4dp

场景: 卡片不嵌套
  测试:
    包: visual-review
    过滤: no_nested_cards
  假设 任意屏幕已渲染
  当 检查视图层级
  那么 不存在卡片包含卡片的层级
  且 主画布与 HUD 是兄弟节点

场景: 底栏控件齐全
  测试:
    包: visual-review
    过滤: bottom_controls_complete
  假设 主回放屏幕已渲染
  当 用户观察底栏
  那么 看到 scrubber 控件
  且 看到当前时间显示
  且 看到总时长显示
  且 看到 "1x" / "4x" / "16x" 三个倍速标签
  且 看到暂停按钮

场景: huashu 哲学装饰约束
  测试:
    包: visual-review
    过滤: huashu_no_decoration
  假设 任意屏幕已渲染
  当 检查屏幕装饰元素
  那么 不存在与轨迹数据无关的装饰图层
  且 不存在装饰性外框 / 嵌套阴影
  且 所有视觉变化均由 PlaybackState 字段或 shader uniform 驱动

场景: 速度图例 3 件套与 3 色渐变
  测试:
    包: visual-review
    过滤: speed_legend_visible
  假设 主回放屏幕已渲染
  当 视线扫描主画布右上角
  那么 看到速度图例横向渐变条
  且 渐变条由左到右依次出现 speed_low / speed_mid / speed_high 三段速度色
  且 渐变条上方显示 "速度 (m/s)" 标签
  且 渐变条左端显示数字 "0"
  且 渐变条右端显示轨迹归一化最大速度的整数刻度

场景: compass 与 2D 按钮垂直堆叠
  测试:
    包: visual-review
    过滤: compass_and_2d_buttons
  假设 主回放屏幕已渲染
  当 视线扫描速度图例下方
  那么 看到圆形 compass 按钮直径 32dp
  且 compass 按钮下方看到圆形 2D 锁定按钮直径 32dp
  且 2D 按钮内显示 "2D" 文本
  且 不出现 "3D" 文本或 3D 切换控件

场景: 地名标签层数量与配色
  测试:
    包: visual-review
    过滤: geo_labels_visible
  假设 主回放屏幕已渲染当前轨迹
  当 视线扫描主画布
  那么 至少看到 4 条地名标签
  且 至多看到 10 条地名标签
  且 标签颜色匹配 ink_secondary #7A7B8C @ 40% opacity
  且 标签不遮挡当前位置 dot 与轨迹主线

场景: 起终点 marker 颜色与文案
  测试:
    包: visual-review
    过滤: start_end_markers
  假设 主回放屏幕已渲染当前轨迹
  当 视线扫描轨迹两端
  那么 起点位置看到 accent_success #10B981 圆点
  且 起点圆点旁出现 "起点" 文字标签
  且 终点位置看到 ink_secondary #7A7B8C 圆点
  且 终点圆点旁出现 "终点" 文字标签
  且 两个 marker 始终不被已走段 / 未走段染色覆盖

场景: 当前位置 halo 可见性
  测试:
    包: visual-review
    过滤: current_position_halo
  假设 主回放屏幕已渲染当前轨迹
  当 检查当前位置 dot
  那么 dot 直径等于 6dp
  且 dot 外包 halo 直径在 16dp 到 24dp 之间
  且 halo 颜色匹配 speed_high #00E5FF
  且 halo 直径不依赖当前速度区间

场景: 水域层低 opacity 静态
  测试:
    包: visual-review
    过滤: water_layer_subtle
  假设 主回放屏幕已渲染
  当 检查主画布水域装饰层
  那么 水域层颜色匹配 speed_high #00E5FF
  且 水域层 opacity 不超过 10%
  且 水域层几何在帧间不变化
  且 水域层不消费任何 shader uniform

场景: HUD 单元迷你进度条数量
  测试:
    包: visual-review
    过滤: hud_mini_bars_count
  假设 主回放屏幕已渲染
  当 视线扫描 HUD 4 单元
  那么 每个单元下方均出现迷你进度条
  且 共看到 4 条迷你进度条
  且 每条进度条高度等于 4dp
  且 已填充段颜色对应该字段主色
  且 未填充段颜色匹配 unwalked_segment #3B3B46

场景: 倍速按钮活跃态视觉差异
  测试:
    包: visual-review
    过滤: speed_button_active_state
  假设 主回放屏幕已渲染且默认倍速为 4x
  当 视线扫描倍速按钮组
  那么 "1x" / "4x" / "16x" 三个按钮均可见
  且 "4x" 按钮底色为圆角矩形 ink_primary @ 8% alpha
  且 "4x" 按钮文字色匹配 ink_primary #F5F5FA
  且 "1x" 与 "16x" 按钮底色透明
  且 "1x" 与 "16x" 按钮文字色匹配 ink_secondary #7A7B8C
  且 同一时刻 3 个按钮中恰有 1 个为活跃态

场景: 暂停按钮形态切换
  测试:
    包: visual-review
    过滤: pause_button_glyph_toggle
  假设 主回放屏幕已渲染
  当 PlaybackState.is_paused 为 false
  那么 暂停按钮显示双竖线 ⏸ 字形
  当 PlaybackState.is_paused 为 true
  那么 暂停按钮显示三角 ▶ 字形
  且 按钮直径在两态下均为 36dp
  且 按钮位置始终在倍速按钮组右侧

场景: app 不绘制系统状态栏
  测试:
    包: visual-review
    过滤: status_bar_system_drawn
  假设 主回放屏幕已渲染
  当 视线扫描屏幕最顶部 24dp 区域
  那么 该区域由 Android 系统绘制信号 / wifi / 电量 / 时间
  且 app 渲染目标 height 等于 屏幕高 减去 status_bar_height
  且 顶栏路线名不与系统状态栏重叠

场景: S0 阶段主画布占位渲染
  测试:
    包: visual-review
    过滤: s0_canvas_placeholder
  假设 app 处于 S0 阶段且 NetworkState 等于 Fetching
  当 主画布渲染
  那么 主画布显示 bundled fallback 轨迹的浅灰色虚线占位 (opacity 不超过 30%)
  且 主画布中央显示圆形 spinner (直径 48dp, 颜色 ink_primary)
  且 spinner 下方显示状态说明文字 "正在同步轨迹数据" (字号 14sp, 颜色 ink_secondary)
  且 path-draw 动效不启动
  且 HUD 4 单元数值全部显示破折号
  且 暂停按钮显示 ▶ 字形 (默认未自动播放)
  当 NetworkState 转为 Success
  那么 占位虚线被实际轨迹替换并启动 S1 path-draw 动效
  当 NetworkState 转为 Fallback
  那么 占位虚线升级为最终轨迹, 直接进入 S1 path-draw, 不重新解析 GPX

场景: scrubber 轨道与 thumb 视觉规约
  测试:
    包: visual-review
    过滤: scrubber_visual
  假设 主回放屏幕已渲染
  当 测量 scrubber 视觉
  那么 scrubber 轨道高度 4dp, 颜色 unwalked_segment #3B3B46
  且 已播放段填充色对应当前速度区间主色 (随时间渐变), 高度 4dp
  且 thumb 直径 16dp 圆形, 颜色 ink_primary #F5F5FA
  且 thumb 边缘 1dp speed_high #00E5FF outline
  且 thumb hit slop 扩展到至少 48 × 48dp
  当 用户 FingerDown thumb
  那么 thumb 直径过渡为 24dp, 200ms ease-out
  当 用户 FingerUp
  那么 thumb 直径回到 16dp, 200ms ease-out

场景: S4 暂停按钮显示重播圆环字形
  测试:
    包: visual-review
    过滤: pause_button_replay_glyph
  假设 app 进入 S4 stats 收尾屏幕且 PlaybackState.playback_progress 等于 1.0
  当 视线扫描底栏右侧暂停按钮位置
  那么 按钮显示圆环箭头 ↻ 字形 (重播 glyph)
  且 字形不显示 ⏸ / ▶
  且 按钮直径保持 36dp, 与 ⏸/▶ 双态尺寸一致
  当 用户点击该按钮
  那么 PlaybackState.playback_progress 重置为 0.0
  且 视图切回 S2 主回放屏幕
  且 按钮字形切回 ⏸

场景: S4 stats 标题与 checkmark
  测试:
    包: visual-review
    过滤: stats_title_checkmark
  假设 进入 S4 stats 收尾屏幕
  当 视线扫描 stats 卡片顶部
  那么 显示 ✓ 圆形 checkmark 图标 (直径 32dp, 颜色 accent_success #10B981)
  且 checkmark 下方显示标题 "回放已完成" (字号 18sp, 颜色 ink_primary)
  且 checkmark 与标题水平居中

场景: S4 stats 4 项单列 fade-up 与 leading 图标
  测试:
    包: visual-review
    过滤: stats_layout_icons
  假设 进入 S4 stats 收尾屏幕
  当 4 项数据淡入完成
  那么 4 项呈单列垂直布局, 水平居中
  且 每项行间距 24dp
  且 每项左侧有 leading 图标 (16dp): 总距离 = 定位针图标, 总时长 = 时钟图标, 累计爬升 = 上升箭头图标, 平均心率 = 心形图标
  且 4 项依次 fade-up, 每项之间间隔 200ms ease-out
  且 数值字体使用 IBM Plex Mono, 字号 28sp
  且 标签字体使用 Inter sans, 字号 14sp, 颜色 ink_secondary

场景: S4 stats 卡片半透明 frosted glass
  测试:
    包: visual-review
    过滤: stats_card_frosted
  假设 进入 S4 stats 收尾屏幕
  当 检查 stats 卡片层
  那么 卡片底色为 bg_secondary #14141C @ 70% alpha
  且 卡片背后主画布轨迹 dim 至 20% opacity (不完全消失)
  且 卡片圆角 12dp, 内边距 24dp
  且 卡片宽度占屏幕宽度的 70% 至 80%

## 排除范围

- 不做动效像素级回归审计, 留待动效录屏审稿
- 不做横屏适配
- 不做浅色主题切换
- 不做用户定制配色
- 不做无障碍颜色对比度审计 (W3C AA / AAA), 留待 P1
- 不做颜色盲模式适配
- 不做 i18n 多语种, 中文固定
- 不做 RTL 布局
- 不做手势冲突审计
