# Makepad Android 轨迹回放能力 Demo PRD

> 项目目标：为 Project-Robius-China 组织提交一个可由社区 review 的 Makepad Android 示例需求文档。
> 当前交付：仅 PRD 与 `spec.spec.md`，不写实现代码。
> 默认体验：cycling 轨迹回放。
> 长期方向：通用 trajectory replay engine，可承载运动轨迹、旅行轨迹和飞行轨迹等多类数据。

---

## 0. 摘要

本 PRD 定义一个 Makepad Android demo：用真实轨迹数据驱动 GPU-first 的数据艺术回放界面，展示 Makepad 在移动端自定义渲染、显式状态管理、后台数据加载和 agent-spec 约束层上的组合能力。

这不是一个完整骑行 App，也不是商业路演页面。它是一个开源工程 demo，提交目标是 Project-Robius-China 组织，由 community review 评估需求清晰度、边界、实现风险和后续可维护性。文档语气应以工程事实为主，避免投资人腔、夸张竞品攻击和不可验证的效果承诺。

第一版渲染层只实现 cycling 默认体验；数据层从一开始按多类型轨迹设计。也就是说：engine 能通过 GitHub API 从集中数据仓库读取 manifest 和数据文件，归一化运动 / 旅行 / 飞行轨迹，renderer 当前只消费 cycling profile。这个边界能避免把 demo 做成封闭的一次性样品，同时控制首版实现复杂度。

---

## 1. 背景与问题

Makepad 的优势不在于“复刻一个已有的地图 App”，而在于它允许应用把 GPU 当成一等公民：shader、widget、状态机和 Rust 业务逻辑可以在同一个工程模型中协作。移动端常见 UI 框架当然也能画轨迹、做动画、跑网络请求，但它们通常把 GPU 放在渲染流水线末端；复杂视觉往往要绕过默认组件模型，才能接近 shader-first 的表达。

轨迹回放是一个合适的展示载体，因为它同时需要：

- 大量点位数据的稳定解析与归一化；
- 时间轴驱动的状态插值；
- 路径、速度、心率、海拔、踏频等多维信息同屏表达；
- 可拖动 scrubber 与可暂停回放；
- 动态路径绘制、走过段分色、当前点、尾迹、粒子、海拔起伏等视觉层；
- 一个可验证的安全约束示例，即 AI 建议被 spec guard 拦截。

首版选择 cycling，不是因为 engine 只能做骑行，而是因为现有 hero 数据具备丰富字段：lat / lon / ele / time / hr / cad，且存在真实高强度片段，适合验证 contract guard。后续 running / hiking / walking / transit / travel / flight 可复用同一数据模型，但不进入首版渲染范围。

---

## 2. 范围决策

### 2.1 Scope：通用 engine + cycling 默认

本 demo 的实现边界分为两层：

**数据与回放层是通用 engine。**

它负责通过 GitHub Contents API 加载 trajectory manifest，选择默认数据集，解析 GPX / CSV / GeoJSON 或规范化后的轨迹数据，输出统一的 `TrajectoryPoint` 和 `PlaybackState`。字段按能力分层：基础字段必需，运动字段、旅行字段、飞行字段可选，安全字段可选但若声明可用则必须可验证。

**渲染层首版 cycling-only。**

UI、HUD、配色含义、contract guard、cadence 表达和高强度段逻辑都按 cycling 默认 profile 实现。其他轨迹类型可以进入数据 repo 和归一化测试，但不要求在首版渲染为不同 UI。

这个决策的目的是同时满足两个目标：代码不是一次性骑行样品；首版又不会因为多运动 UI 设计而发散。

### 2.2 视觉风格：C+ + huashu 哲学锚定

视觉方向保留 C+：数据艺术为主体，最小地理感作为辅助。C+ 的含义是：

- C：抽象轨迹数据艺术，让曲线、粒子、速度、心率、海拔成为画面主角；
- +：保留少量地理识别线索，例如静态底板、路线名、起终点方向、总距离和时间轴。

huashu 哲学锚定用于避免常见 AI 视觉噪声：

- 工具优先：首屏直接进入可运行的回放工具，不做 landing page。
- 信息密度克制：HUD 是可扫描的工程仪表，不是装饰卡片。
- 真实数据驱动：视觉变化必须来自轨迹字段或回放状态，不用随机“科技感”装饰。
- 不做单色主题：主背景深色，但速度色、状态色、警示色各自承担明确语义。
- 控件稳定：scrubber、HUD、徽章、卡片尺寸稳定，状态变化不导致布局跳动。

首版视觉增强固定 5 个 shader 方向：

1. SDF path stroke：轨迹线宽、边缘和 glow 由 shader 控制。
2. Speed color ramp：速度映射到暖白、暖橙、青蓝三段色。
3. Instanced particle flow：沿轨迹切线方向绘制粒子尾迹。
4. Elevation lift：用 vertex 偏移表现 2.5D 海拔起伏。
5. Guard pulse field：contract guard 激活时由 shader uniform 驱动红边 pulse。

### 2.3 集中数据 repo + GitHub API

数据源目标仓库固定为：

`https://github.com/Project-Robius-China/trajectory-replay-data`

数据集中在独立 GitHub 仓库，App 仓库只负责渲染、解析和 fallback。后续数据仓库需要提供：

- `manifest.json`：列出可用数据集、类型、默认数据集、license、字段能力。
- `cycling/bikeride.gpx`：cycling 默认 hero 数据。
- `travel/*`：旅行时间线样例，例如城市步行、火车、跨城 road trip。
- `flight/*`：飞行轨迹或航线样例，例如机场到机场的大圆弧 polyline、ADS-B 风格采样点。
- 可选的 normalized JSON：用于测试 parser 与 renderer 的分离。
- README：记录数据来源、许可、字段缺失情况和验证方式。

首版 App 通过 GitHub REST Contents API 获取数据：

- manifest：`GET https://api.github.com/repos/Project-Robius-China/trajectory-replay-data/contents/manifest.json?ref=main`
- 小文件：读取 Contents API JSON 响应中的 `content` 或 `download_url`。
- 1-100 MB 文件：使用 `Accept: application/vnd.github.raw+json` 获取 raw 内容，或先取 fresh `download_url` 再下载。
- 公开仓库可无 token 访问；遇到 rate limit / 403 / 404 / timeout 时退回 bundled fallback。

任何网络请求都必须限制在 GitHub API、GitHub Raw 下载 URL 和该数据 repo 范围内。

### 2.4 多类轨迹层级

数据层支持多元，渲染层 cycling-only。

数据层需要识别以下轨迹类型：

- `cycling`
- `running`
- `hiking`
- `walking`
- `transit`
- `travel`
- `flight`

所有类型都归一化到统一结构：

```rust
struct TrajectoryPoint {
    lat: f64,
    lon: f64,
    ele_m: Option<f32>,
    timestamp_ms: i64,
    speed_mps: Option<f32>,
    heart_rate_bpm: Option<u16>,
    cadence_rpm: Option<u16>,
    power_w: Option<u16>,
    heading_deg: Option<f32>,
    altitude_m: Option<f32>,
    transport_mode: Option<String>,
    route_label: Option<String>,
    source_index: usize,
}
```

cycling profile 额外消费 `heart_rate_bpm` 和 `cadence_rpm`；flight profile 额外消费 `altitude_m`、`heading_deg`、`route_label`；travel profile 额外消费 `transport_mode`。除 cycling 外，其他 profile 暂时只进入 parser / manifest / normalization 测试，不进入首版 UI 切换。

### 2.5 开源项目借鉴

本 demo 可以借鉴以下开源项目，但不直接复刻它们的产品形态：

| 项目 | 可借鉴点 | 本项目取舍 |
|---|---|---|
| GPX Animator (`github.com/gpx-animator/gpx-animator`) | GPX 转动画视频、头部光点、预画完整轨迹、多交通图标 | 借轨迹动画语法；不生成视频，改为 Makepad 实时渲染 |
| GPXSee (`github.com/tumic0/GPXSee`) | 多格式 GPS viewer，支持 GPX / TCX / FIT / KML / IGC / NMEA / GeoJSON，含 elevation / speed / heart-rate / cadence / power 图表 | 借多格式字段模型和指标层；首版只实现 GPX + normalized JSON |
| Dawarich / GeoPulse / OwnTracks Recorder | 旅行时间线、location history、trip / stay / transport mode 概念 | 借旅行轨迹数据 taxonomy；不做账号、上传、后台服务 |
| OpenFlights (`github.com/jpatokal/openflights`) | 机场、航线、飞机型号等公开航线数据结构 | 借 flight profile 数据结构；首版只做静态航线/采样轨迹，不接实时航班 |

---

## 3. 用户体验

### 3.1 信息架构

App 是单页回放体验，无导航栈、无 tab、无设置页。屏幕分为四块：

- 顶部：路线名、数据源徽章、当前 profile。
- 主画布：轨迹、走过段、当前点、粒子、海拔起伏、guard pulse。
- HUD：速度、心率、海拔、踏频。
- 底部：scrubber、当前时间、总时长、倍速、暂停。

所有运行期文案中文。代码注释和工程 README 可使用英文或中英混排，但 UI 文案必须中文。

### 3.2 90 秒默认故事

默认 demo 时长约 90 秒，但实现中应由 `PlaybackState.playback_progress` 驱动，不把固定秒数散落到 UI 逻辑中。

**S0 数据同步，0-2 秒。**

启动后显示“同步中...”，后台线程通过 GitHub Contents API 从 trajectory-replay-data 拉取 manifest 和默认 cycling 数据。成功显示“已同步”，失败显示“本地缓存”。即使网络很快，也需要保留一个可见的状态切换过程，避免用户无法确认数据来源。

**S1 path draw，2-5 秒。**

完整轨迹从 0 到 100% 绘制出来，`track_progress` 同时驱动路径可见区域和当前点起始位置。路线名在底部或顶部淡入。

**S2 主回放，5-45 秒。**

scrubber 默认以 4x 速度推进。HUD 每帧从当前点更新。画布显示完整轨迹、已走段、未走段、当前点、粒子尾迹、海拔起伏。用户可拖动 scrubber，拖动时所有派生状态同步更新。

**S3 spec guard，45-65 秒。**

当 scrubber 进入真实高强度窗口，用户触发“询问 AI 建议”。AI mock 生成“建议加速 30% 通过此段，缩短爬坡时间。”contract c1.2 检查到 5 分钟窗口心率超过阈值，拒绝该建议。UI 显示红边 pulse 和拦截卡片，原 AI 建议不在可见区域展示。

**S4 stats 收尾，65-90 秒。**

轨迹淡到背景，中央显示总距离、总时长、累计爬升、平均心率。stats 逐行 fade-up。结束后保持静止，便于录屏和 review。

### 3.3 交互

首版交互只保留必要路径：

- 拖动 scrubber：更新 progress、当前点、HUD、shader uniforms。
- 点击暂停：切换 `is_paused`。
- 切换倍速：`1x / 4x / 16x` 循环。
- 双击画布：触发 AI mock 建议。
- 关闭 guard 卡片：隐藏卡片，guard 状态按规则复位。

不做设置页，不做运动类型切换 UI，不做登录，不做分享，不做地图缩放。

---

## 4. 数据规格

### 4.1 数据源

默认数据仓库：

`Project-Robius-China/trajectory-replay-data`

默认数据集：

`cycling/bikeride.gpx`

bundled fallback：

`assets/cycling-track.gpx`

manifest 示例：

```json
{
  "default_profile": "cycling",
  "default_dataset": "cycling/bikeride.gpx",
  "datasets": [
    {
      "id": "cycling-bikeride",
      "profile": "cycling",
      "format": "gpx",
      "path": "cycling/bikeride.gpx",
      "required_fields": ["lat", "lon", "time", "ele", "hr"],
      "optional_fields": ["cad", "power"],
      "license": "source documented in data repo"
    }
    ,
    {
      "id": "travel-city-walk",
      "profile": "travel",
      "format": "geojson",
      "path": "travel/city-walk.geojson",
      "required_fields": ["lat", "lon", "time"],
      "optional_fields": ["transport_mode", "route_label"],
      "license": "source documented in data repo"
    },
    {
      "id": "flight-sample-route",
      "profile": "flight",
      "format": "csv",
      "path": "flight/sample-route.csv",
      "required_fields": ["lat", "lon"],
      "optional_fields": ["time", "altitude_m", "heading_deg", "route_label"],
      "license": "source documented in data repo"
    }
  ]
}
```

### 4.2 GPX 字段

必需字段：

- `lat`
- `lon`
- `time`
- `ele`
- `hr`，文件级必须存在；逐点可稀疏。

可选字段：

- `cad`
- `power`

TrackPointExtension 按 namespace URI 解析，不按 prefix。prefix 可能是 `ns3:`、`gpxtpx:` 或其他合法形式。解析器必须支持 Garmin TrackPointExtension v1 与 v2。

字段稀疏规则：

- 某个点缺 `hr`：该点 `heart_rate_bpm = None`，cycling HUD 显示破折号。
- 文件级完全没有 `hr`：cycling 默认数据不合格，parse 返回错误。
- 某个点缺 `cad`：该点 `cadence_rpm = None`，HUD 显示破折号。
- 文件级没有 `power`：允许，首版 UI 不显示 power。

### 4.3 派生值

解析后生成以下派生值：

- `distance_m_total`
- `duration_ms_total`
- `elevation_min_m`
- `elevation_max_m`
- `elevation_gain_m`
- `observed_max_hr`
- `avg_hr`
- `speed_min_mps`
- `speed_max_mps`
- `track_bounds`

速度可由相邻点距离和时间差计算；若原始数据提供速度字段，也以统一派生值为准。运行期禁止重新解析 GPX，回放只读归一化后的点数组和派生统计。

---

## 5. PlaybackState

`PlaybackState` 是运行期唯一权威状态。UI、shader uniform、HUD 和 guard 都从它派生。

```rust
struct PlaybackState {
    profile: TrajectoryProfile,
    playback_progress: f32,
    current_trkpt_index: usize,
    current_speed_mps: f32,
    current_hr_bpm: Option<u16>,
    current_ele_m: Option<f32>,
    current_cad_rpm: Option<u16>,
    playback_speed: f32,
    is_paused: bool,
    data_source: DataSource,
    network_state: NetworkState,
    contract_guard_active: bool,
    contract_guard_reason: Option<String>,
    walked_segment_ratio: f32,
    scrubber_echo_phase: f32,
}
```

`walked_segment_ratio` 表示已经走过的路径比例，等价于 `playback_progress`，但单独命名以便 renderer 表达“已走段分色”。`scrubber_echo_phase` 在用户拖动 scrubber 时触发短尾迹，用于显示拖动后的回声反馈。

---

## 6. 视觉契约

### 6.1 色彩

```text
bg_primary:      #0A0A0F
bg_secondary:    #14141C
ink_primary:     #F5F5FA
ink_secondary:   #7A7B8C
speed_low:       #E8E8F0
speed_mid:       #FF8A3D
speed_high:      #00E5FF
walked_segment:  #F5F5FA
unwalked_segment:#3B3B46
accent_success:  #10B981
accent_warning:  #FF3B6E
```

背景以深色为底，但不能把所有 UI 都做成同一蓝紫色系。速度、成功、警示、已走段、未走段要有清晰语义。

### 6.2 字体与布局

- Display / HUD：Inter 或系统 sans，启用 tabular figures。
- Monospace：IBM Plex Mono 或系统 mono，用于数据值。
- 卡片圆角不超过 12dp；徽章圆角 4dp。
- 不做嵌套卡片；主画布和 HUD 是全宽布局，不使用装饰性外框。
- 文本不得覆盖轨迹主信息；小屏上 HUD 可改为 2x2 网格。

### 6.3 12 个轨迹与界面动效

首版固定 12 个动效。实现可降级，但不能随意增删语义。

| # | 动效 | 触发 | 契约 |
|---|---|---|---|
| 1 | 同步 spinner | app 启动 | `network_state = Fetching` 时可见 |
| 2 | 同步成功转场 | fetch 成功 | “已同步”可见不少于 800ms |
| 3 | path draw | 进入主画布 | `track_progress` 从 0 到 1 |
| 4 | 速度色 ramp | 回放全程 | `polyline_color_mix = speed / max_speed` |
| 5 | 粒子流 | 回放全程 | `particle_density` 随速度变化 |
| 6 | 2.5D 海拔 | 回放全程 | `elevation_z` 映射海拔范围 |
| 7 | 当前光点 | 回放全程 | 由 `track_progress` 定位 |
| 8 | AI 卡片 slide-up | 询问 AI | 卡片从底部进入 |
| 9 | guard 红边 pulse | contract reject | `guard_pulse_phase` 驱动 1.5s |
| 10 | guard 卡片 slide-down | contract reject | 拦截原因进入并遮住 AI 文本 |
| 11 | 已走段分色 | 回放全程 | `walked_segment_ratio` 前后分色 |
| 12 | scrubber echo | scrubber 拖动 | thumb 后方出现短暂 echo trail |

新增动效说明：

**已走段分色**让用户明确知道当前位置之前的轨迹已被播放。已走段用暖白或轻微速度色增强，未走段降低 opacity 或转为 `#3B3B46`。这不是第二条路径，而是同一轨迹根据 progress 做前后分段。

**scrubber echo**用于改善拖动反馈。用户快速拖动时，thumb 后方保留 300-500ms 的淡色回声，显示刚刚经过的位置。echo 不改变真实 playback progress，只是视觉反馈。

### 6.4 视觉设计 spec 与真值优先级

§6.1 ~ §6.3 是视觉契约的文字描述层；执行层由独立文件 `visual.spec.md` 承载，以 BDD 场景形式锁定 mockup 中已经达到的视觉效果，并把 mockup 残留的 3 处不一致（顶栏返回箭头 / profile 标签英文 Cycling / 主回放仅暖橙单色）强制纠偏。

视觉真值优先级如下，冲突时以高优先级为准：

1. **`visual.spec.md` BDD 场景**（最高，机械可验证）
2. **`prd.md` §6 视觉契约**（文字描述）
3. **`design/refs/*.png` 参考底板**（视觉氛围，不作为像素真值）

参考底板存放在 `design/refs/`，文件命名与故事板编号对齐：

- `design/refs/storyboard-1.png`：S0 同步 / S1 path-draw / S3 Guard 拦截 / S4 stats 收尾 四宫格
- `design/refs/storyboard-2.png`：S2 主回放 / S3 Guard 拦截 / S4 stats 收尾 三联竖屏

当 mockup 与 `visual.spec.md` 冲突时，mockup 必须重画；当 `visual.spec.md` 与 `prd.md` §6 冲突时，需要先评审是否更新文字契约，再同步至 spec。

---

## 7. Shader 与渲染能力

### 7.1 必须暴露的 uniforms

- `track_progress`
- `polyline_color_mix`
- `particle_density`
- `elevation_z`
- `guard_pulse_phase`
- `walked_segment_ratio`
- `scrubber_echo_phase`

前 5 个是原始 C+ 核心；后 2 个对应本轮新增轨迹动效。

### 7.2 渲染约束

- 轨迹由 Makepad 自定义 widget 自渲染，不依赖在线 map tile。
- 不使用完整地图 SDK，不接 Mapbox / Google Maps / OSM live tile。
- 可以使用一张静态底板作为 C+ 的地理感补充，但底板缺失时必须可运行。
- 粒子使用 instanced quads，不做每粒子 widget。
- 海拔表现是 2.5D vertex offset，不做真实 3D terrain mesh。

### 7.3 降级路径

渲染失败时按以下顺序降级：

1. 关闭粒子，保留路径和 HUD。
2. 关闭 2.5D 海拔，保留平面路径。
3. 关闭 glow，保留基础线宽和颜色。
4. 关闭静态底板，进入纯 C 模式。
5. 若 shader path stroke 有设备兼容问题，退到 polyline quads。

降级不能破坏数据同步、scrubber、HUD 和 contract guard 的可验证行为。

---

## 8. 联网与 fallback

联网只用于通过 GitHub API 加载 Project-Robius-China 数据 repo 中的 manifest 和默认数据，不用于登录、地图、LLM、analytics 或 telemetry。

流程：

1. app 启动，`network_state = Idle`。
2. UI 显示“同步中...”，状态进入 `Fetching`。
3. 后台线程发起 GitHub Contents API HTTP GET，3 秒总超时。
4. 成功并 parse 后进入 `Success`，`data_source = Network`。
5. 失败、超时、非 2xx 或 parse error 时读取 bundled GPX，进入 `Fallback`，`data_source = LocalFallback`。

`Fallback` 是终态，不重试。整个 90 秒默认回放期间最多发起一次 manifest 请求和一次默认数据请求。fetch 到的数据不写入磁盘 cache；每次启动都重新尝试 fetch，以便 review 时能验证联网状态机。

推荐实现仍是 `ureq + std::thread + mpsc`。禁止为一次性 fetch 引入 tokio / reqwest / async-std。

---

## 9. Contract Guard

首版 guard 只实现一个合约：c1.2 高强度持续约束。

定义：

```text
如果当前 5 分钟窗口 avg_hr > effective_max_hr * 0.92，则拒绝“加速 30%”类 AI 建议。
```

`effective_max_hr` 固定为：

```text
max(user_profile.max_hr, observed_max_hr_in_track)
```

用户 profile mock：

```text
max_hr = 195
age = 35
sport = cycling
```

AI mock 文案：

```text
建议加速 30% 通过此段，缩短爬坡时间。
```

拦截 UI 文案：

```text
AI 建议已被 spec 阻止
原因：心率会持续超过 92% 区间
违反契约 c1.2「高强度持续约束」
```

关键要求：

- 原 AI 建议在 reject 后不出现在任何可见 UI 区域。
- guard pulse 持续 1.5s。
- 拦截卡片包含 5 分钟 HR mini-chart。
- 离开高强度窗口并经过 1.5s 后，guard active 复位。

---

## 10. Cargo 与依赖边界

建议依赖：

```toml
makepad-widgets = { git = "https://github.com/kevinaboos/makepad", branch = "cargo_makepad_ndk_fix", features = ["serde"] }
makepad-code-editor = { git = "https://github.com/kevinaboos/makepad", branch = "cargo_makepad_ndk_fix" }

robius-use-makepad = "0.1.1"
robius-open = { git = "https://github.com/Project-Robius-China/robius2.git" }
robius-directories = { git = "https://github.com/Project-Robius-China/robius2.git" }
robius-location = { git = "https://github.com/Project-Robius-China/robius2.git" }
robius-proxy = { git = "https://github.com/Project-Robius-China/robius2.git" }

quick-xml = { version = "0.31", features = ["serde"] }
serde = { version = "1", features = ["derive"] }
ureq = { version = "2", default-features = false, features = ["tls", "rustls-ring"] }
```

禁用：

- `tokio`
- `reqwest`
- `async-std`
- `georust/gpx`
- `serde-xml-rs`
- `aws-lc-rs`
- 真实 LLM API
- Strava / Garmin / Komoot OAuth
- live tile 服务
- analytics / telemetry / crash report 网络上报

`robius-location` 可以作为标准套件引入，但首版运行期不调用 location API，避免权限弹窗和设备差异。

---

## 11. Non-goals

- 不做完整地图产品。
- 不画城市名、街道名、POI、比例尺、路面类型。
- 不做账号、登录、注册、OAuth。
- 不接真实 Strava / Garmin / Komoot。
- 不调用真实 LLM。
- 不录制真实 GPS。
- 不做多运动类型 UI 切换。
- 不接实时航班 API。
- 不做 3D terrain mesh。
- 不做离线 tile cache。
- 不做多语言和主题切换。
- 不在本轮写任何代码。

---

## 12. 验收方式

本轮只验收文档：

- `prd.md` 全文改为通用 engine + cycling 默认的开源工程口吻。
- `spec.spec.md` 保留原 18 个场景结构，并扩展为 22 个场景。
- 新增场景覆盖：数据 repo、通用 engine、多类型数据层、已走段分色、scrubber echo。
- 数据要求覆盖：GitHub API 获取、集中数据仓库、运动 / 旅行 / 飞行 profile。
- spec 可被 `agent-spec parse` 解析，且 lint 分数不低于 0.7。

后续实现验收由 `spec.spec.md` 驱动。PRD 负责解释设计背景和工程边界，spec 负责机械可验证的完成条件。

---

## 13. 提交目标

目标组织：

`Project-Robius-China`

建议提交内容：

- `prd.md`
- `spec.spec.md`
- 后续实现 PR 的 README 链接到上述两份文档。

review 重点：

- scope 是否清晰；
- 数据 repo 是否可维护；
- cycling-only renderer 与 multi-profile engine 的边界是否合理；
- shader 动效是否有降级路径；
- guard 逻辑是否可验证；
- Forbidden 依赖和网络边界是否足够明确。

---

## 附录 A：术语

**Trajectory replay engine**

解析、归一化并按时间轴回放轨迹数据的通用层。它不等于 cycling UI。

**Cycling default**

首版默认 profile。它决定 HUD 字段、guard 逻辑和视觉文案。

**C+**

抽象数据艺术 + 最小地理感。不是地图产品，不依赖 live tiles。

**walked segment**

已经播放过的轨迹段。cycling 场景中也沿用这个名字，因为它表达的是“已走过的路径”，不是运动类型。

**scrubber echo**

拖动 scrubber 后短暂停留的视觉回声，不影响真实 progress。

---

## 附录 B：实现提示

实现时建议先做数据闭环，再做视觉增强：

1. bundled GPX parse 成 `TrajectoryPoint`。
2. `PlaybackState` 驱动 HUD 和 scrubber。
3. 接入数据 repo fetch + fallback。
4. 自渲染平面轨迹。
5. 添加 path draw、速度色、已走段分色。
6. 添加当前点、scrubber echo。
7. 添加粒子与 2.5D 海拔。
8. 添加 contract guard。
9. 最后调视觉常量与降级路径。

不要把所有 shader 效果作为第一步，否则数据和状态 bug 会被视觉复杂度掩盖。
