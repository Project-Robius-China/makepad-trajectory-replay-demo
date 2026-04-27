spec: task
name: "Makepad Android 轨迹回放 demo PRD/spec 合约"
tags: [demo, makepad, android, trajectory-replay, project-robius-china]
---

## 意图

为 Project-Robius-China community review 准备一个 Makepad Android 轨迹回放 demo 的实现合约。首版运行体验为 cycling 默认回放，但数据与回放层必须按通用 trajectory replay engine 设计，能够通过 GitHub API 从 `Project-Robius-China/trajectory-replay-data` 读取 manifest 和默认数据，并为运动、旅行、飞行轨迹保留统一归一化模型。本文档只约束后续实现，不要求本轮写代码。

## 决策

### Scope

- 项目定位: 开源工程 demo，不写投资人话术，不做商业 landing page。
- Engine scope: 通用 trajectory replay engine，支持 `cycling`、`running`、`hiking`、`walking`、`transit`、`travel`、`flight` 七类 profile 的 manifest 与归一化。
- Renderer scope: 首版渲染层只实现 `cycling` 默认 profile，不提供运动类型切换 UI。
- 提交目标: `Project-Robius-China` 组织，PR 面向 community review。
- 本轮交付: 只改 `prd.md` 与 `spec.spec`，不写 Cargo 项目、不添加源码、不下载数据文件。

### 数据层

- 数据仓库: `https://github.com/Project-Robius-China/trajectory-replay-data`
- 默认数据集: `cycling/bikeride.gpx`
- bundled fallback: `assets/cycling-track.gpx`
- 联网范围: 只允许访问 GitHub REST Contents API、GitHub Raw download_url 和 trajectory-replay-data 仓库内容；禁止访问 Strava、Garmin、Komoot、Mapbox、OSM live tiles、实时航班 API、LLM API、analytics、telemetry。
- 数据获取: manifest 通过 `GET /repos/Project-Robius-China/trajectory-replay-data/contents/manifest.json?ref=main` 获取；默认数据通过 Contents API 的 `content`、`download_url` 或 `Accept: application/vnd.github.raw+json` 获取。
- GPX 解析: `quick-xml = "0.31"` + `serde` derive；禁止 `georust/gpx` 与 `serde-xml-rs`。
- TrackPointExtension: 按 namespace URI 解析，支持 Garmin v1 与 v2，不按 prefix 解析。
- 字段稀疏: 文件级 `hr` 必须存在；逐点缺 `hr` 或 `cad` 时使用 `None`/破折号显示，不 panic。
- 运行期: GPX 或 normalized 数据只在启动期 parse 一次；回放期禁止重新解析。

### 渲染层

- 视觉风格: C+ 数据艺术 + 最小地理感，遵守 `prd.md` 中 huashu 哲学锚定。
- 主背景: `#0A0A0F`
- 速度色: 低速暖白 `#E8E8F0`，中速暖橙 `#FF8A3D`，高速青蓝 `#00E5FF`
- guard 警示色: `#FF3B6E`，只用于 contract guard。
- 已走段分色: `walked_segment_ratio` 前段用已走段色，后段用未走段色或降低 opacity。
- scrubber echo: 用户拖动 scrubber 时，thumb 后方显示 300-500ms 的 echo trail，不改变真实 playback progress。
- 必须暴露 shader uniforms: `track_progress`、`polyline_color_mix`、`particle_density`、`elevation_z`、`guard_pulse_phase`、`walked_segment_ratio`、`scrubber_echo_phase`
- 粒子流: 使用 instanced quads，不使用 per-particle widget。
- 海拔: 2.5D vertex offset，不做真实 3D terrain mesh。

### 联网层

- 启动时后台线程对 GitHub Contents API 发 HTTP GET，先取 manifest，再取 trajectory-replay-data 默认数据，3 秒总超时。
- HTTP client: `ureq = "2"`，`default-features = false`，启用 rustls ring backend。
- 并发模型: `std::thread::spawn` worker + `std::sync::mpsc` 回主线程。
- 禁止引入 `tokio`、`reqwest`、`async-std`、`aws-lc-rs`。
- `NetworkState`: `Idle -> Fetching -> Success` 或 `Idle -> Fetching -> Fallback`。
- `Fallback` 是终态；fetch 失败后不重试。
- 不持久化 fetch 到的 GPX、CSV、GeoJSON 或 manifest cache。

### Contract Guard

- contract id: `c1.2`
- 名称: `高强度持续约束`
- 条件: 当当前 5 分钟窗口 `avg_hr > effective_max_hr * 0.92` 时，拒绝“加速 30%”类 AI 建议。
- `effective_max_hr = max(user_profile.max_hr, observed_max_hr_in_track)`
- user profile mock: `max_hr = 195`，`age = 35`，`sport = "cycling"`
- AI mock 文案: `建议加速 30% 通过此段，缩短爬坡时间。`
- reject 后原 AI 文案不出现在任何可见 UI 区域。

## 边界

### Allowed Changes

- prd.md
- spec.spec
- assets/cycling-track.gpx
- assets/static-tile.png
- Cargo.toml
- Cargo.lock
- README.md
- src/**/*.rs
- src/shaders/**

### Forbidden

- 本轮不得写实现代码；仅允许改 `prd.md` 与 `spec.spec`
- 不得引入 `tokio`、`reqwest`、`async-std`
- 不得引入 `georust/gpx`、`serde-xml-rs`
- 不得引入或通过依赖树拉入 `aws-lc-rs`
- 不得调用 Anthropic、OpenAI 或任何真实 LLM API
- 不得调用 Strava、Garmin Connect、Komoot、Mapbox、OSM live tile、Google Maps 或实时航班 API
- 不得调用 `robius-location` 的 location API
- 不得实现账号、登录、注册、OAuth、分享、社交、排行榜
- 不得把 fetch 到的数据写入磁盘 cache
- 不得在运行期重新解析 GPX
- 不得在 HTTP fetch 或 GPX parse 路径使用 `unwrap()`
- 不得阻塞 Makepad 主循环执行 HTTP fetch

### Out of Scope

- 完整地图产品
- 多运动类型渲染 UI
- 真实 GPS 录制
- 多骑手并播
- 真实 LLM
- 离线 tile cache
- 3D terrain mesh
- 多语言和主题切换

## 验收标准

场景: 联网 fetch 成功后完整播放 90 秒故事
  测试: test_network_fetch_success_full_playback
  层级: integration
  替身: 真实 Android 设备 + 真实网络
  假设 Android 设备网络畅通且能访问 api.github.com 与 raw.githubusercontent.com 上的 trajectory-replay-data 默认数据
  并且 包内 bundled assets/cycling-track.gpx 文件存在作为 fallback
  当 用户启动 app 并放任 playback_progress 自动从 0 走到 1
  那么 app 在启动后 3 秒内通过 GitHub API 成功 fetch manifest 与默认 cycling 数据并完成 parse
  并且 PlaybackState.data_source 等于 Network
  并且 PlaybackState.network_state 经历 Idle 然后 Fetching 然后 Success 三个状态
  并且 UI 在 Success 期间至少 800 毫秒显示包含 "已同步" 的文字
  并且 app 在整个 90 秒内不访问 api.github.com 与 raw.githubusercontent.com 之外的任何主机
  并且 整个回放过程不出现 panic 或 crash
  并且 平均帧率不低于 50 fps

场景: 联网 fetch 失败时退回 bundled GPX 完成完整播放
  测试: test_network_fetch_failure_local_fallback
  层级: integration
  替身: 真实 Android 设备 + 飞行模式或 hosts 屏蔽 raw.githubusercontent.com
  假设 Android 设备无法访问 raw.githubusercontent.com
  并且 包内 bundled assets/cycling-track.gpx 文件存在
  当 用户启动 app 并放任 playback_progress 自动从 0 走到 1
  那么 app 在 3 秒 timeout 后切换到 local fallback 并完成 parse
  并且 PlaybackState.data_source 等于 LocalFallback
  并且 PlaybackState.network_state 经历 Idle 然后 Fetching 然后 Fallback 三个状态
  并且 UI 在 Fallback 期间持续显示包含 "本地缓存" 的文字
  并且 app 在整个 90 秒内不访问 api.github.com 与 raw.githubusercontent.com 之外的任何主机
  并且 fetch 失败后 app 不发起任何重试请求
  并且 整个回放过程不出现 panic 或 crash
  并且 平均帧率不低于 50 fps

场景: network_state 在三态下都有可见 UI 指示
  测试: test_network_state_visible_in_ui
  层级: widget
  替身: 注入 NetworkState
  假设 app 处于 S0 启动阶段
  当 PlaybackState.network_state 等于 Fetching
  那么 UI 在屏幕可见区域显示包含 "同步" 的文字
  并且 当 network_state 变为 Success 时 UI 显示包含 "已同步" 的文字
  并且 当 network_state 变为 Fallback 时 UI 显示包含 "本地缓存" 的文字
  并且 三种文字均不通过 toast 一闪而过的方式呈现
  并且 主屏右上角持久徽章持续显示当前同步状态文字直到 app 退出

场景: HTTP client 是 ureq 且未引入 tokio 或 reqwest 或 aws-lc-rs
  测试: test_http_client_is_ureq_only
  层级: static
  假设 Cargo.toml 由后续实现生成且包含默认数据 fetch 实现
  并且 HTTP client 决策为 "ureq"
  当 在仓库根执行 cargo tree
  那么 输出文本包含字符串 "ureq"
  并且 输出文本不包含字符串 "reqwest"
  并且 输出文本不包含字符串 "tokio"
  并且 输出文本不包含字符串 "async-std"
  并且 输出文本不包含字符串 "aws-lc-rs"

场景: GitHub API fetch 在后台线程发起且不阻塞主循环
  测试: test_github_api_fetch_off_main_thread
  层级: integration
  替身: fetch worker 入口插桩记录线程 ID
  假设 app 已启动并触发 GitHub API manifest fetch
  当 fetch worker 执行 ureq HTTP GET 调用
  那么 执行 ureq HTTP GET 的线程 ID 不等于 Makepad 主循环线程 ID
  并且 fetch 期间 Makepad 主循环帧率不低于 50 fps
  并且 fetch worker 通过 std::sync::mpsc::channel 把结果发回主线程

场景: 速度染色 uniform 由当前速度归一化驱动
  测试: test_polyline_speed_color_mix
  假设 GPX 派生速度最大值为 12 m/s
  当 渲染处于 S2 主屏回放状态
  那么 shader uniform polyline_color_mix 等于 current_speed_mps 除以 12.0 的值
  并且 当 current_speed_mps 大于 10 m/s 时 polyline_color_mix 大于 0.83
  并且 当 current_speed_mps 小于 2 m/s 时 polyline_color_mix 小于 0.17
  并且 polyline_color_mix 的值始终在 0.0 到 1.0 之间
  并且 低速色包含 #E8E8F0
  并且 中速色包含 #FF8A3D
  并且 高速色包含 #00E5FF

场景: 粒子流密度由当前速度驱动且 instanced 渲染
  测试: test_particle_density_uniform_driven
  假设 渲染处于 S2 主屏回放状态
  并且 当前速度等于 6 m/s
  当 渲染粒子流
  那么 shader uniform particle_density 等于 0.5 误差在 0.01 内
  并且 粒子总数大于等于 1000 且小于等于 2000
  并且 粒子绘制使用单一 instanced draw call 而非 per-particle draw call

场景: 海拔 2.5D 起伏由 vertex shader 偏移实现
  测试: test_elevation_2_5d_vertex_offset
  假设 GPX 海拔范围 -79.8 到 299.4 m
  并且 渲染处于 S2 主屏回放状态
  当 当前海拔 current_ele_m 等于 110.0 m
  那么 shader uniform elevation_z 等于 current_ele_m 减去 -79.8 再除以 379.2 的值
  并且 elevation_z 在 0.0 到 1.0 之间
  并且 vertex shader 把对应轨迹顶点的 y 坐标减去 elevation_z 乘以 50.0 像素

场景: scrubber 拖动驱动 PlaybackState 与 HUD 同步
  测试: test_scrubber_drag_updates_state
  假设 app 处于 S2 主屏回放状态
  并且 轨迹共有 7200 个归一化点
  当 用户拖动时间轴 scrubber 到屏幕宽度的 50% 位置
  那么 PlaybackState.playback_progress 等于 0.5
  并且 PlaybackState.current_trkpt_index 等于 3600
  并且 HUD 显示的速度等于第 3600 个点的速度
  并且 HUD 显示的心率等于第 3600 个点的 heart_rate_bpm 字段
  并且 shader uniform track_progress 等于 0.5

场景: GPX 解析仅在启动时执行一次
  测试: test_gpx_parsed_once
  假设 app 已启动并完成 GPX parse
  当 时间轴回放进行 90 秒并切换 playback_speed 共 3 次
  那么 quick_xml::Reader 的实例化次数等于 1
  并且 PlaybackState.current_trkpt_index 仅通过插值或索引更新

场景: contract-guard 在高强度段触发并阻止 AI 输出
  测试: test_contract_guard_blocks_ai
  假设 AI mock 文案为 "建议加速 30% 通过此段，缩短爬坡时间。"
  并且 cycling 数据中存在连续 5 分钟 avg_hr 大于 effective_max_hr 乘以 0.92 的高强度段
  并且 effective_max_hr 等于 user_profile_max_hr 195 与 observed_max_hr_in_track 中的较大值
  当 scrubber 进入该高强度段
  并且 用户双击轨迹触发 AI 询问
  那么 AI mock 输出文本为 "建议加速 30% 通过此段，缩短爬坡时间。"
  并且 spec 引擎执行 contract c1.2 检查并返回 reject
  并且 PlaybackState.contract_guard_active 在至少 1.5 秒内为 true
  并且 UI 显示边框颜色 #FF3B6E 的 pulse 动效持续 1500 毫秒
  并且 拦截卡片显示的文本包含字符串 "违反契约 c1.2"
  并且 拦截卡片显示的文本包含字符串 "高强度持续约束"
  并且 AI mock 的输出文本 "建议加速 30%" 不出现在 UI 任何可见区域

场景: contract c1.2 在低强度段不误触发
  测试: test_contract_guard_no_false_positive
  假设 cycling 数据中存在连续 5 分钟 avg_hr 小于 effective_max_hr 乘以 0.85 的低强度段
  当 scrubber 进入该低强度段
  并且 用户双击轨迹触发 AI 询问
  那么 PlaybackState.contract_guard_active 保持为 false
  并且 AI mock 输出文本被正常透传到 UI 显示

场景: 时间轴 scrubber 离开高强度段时 contract_guard_active 复位
  测试: test_contract_guard_resets_on_leave
  假设 PlaybackState.contract_guard_active 当前为 true
  当 用户拖动 scrubber 离开高强度段且经过 1.5 秒
  那么 PlaybackState.contract_guard_active 变为 false
  并且 拦截卡片从 UI 移除
  并且 红边 pulse 动效停止

场景: GPX 部分 trkpt 缺 hr 字段时该点 heart_rate_bpm 为空
  测试: test_gpx_per_trkpt_hr_sparse
  假设 GPX 共 8234 个 trkpt 且其中 39 个 trkpt 的 extensions 子树不含 hr 元素
  并且 其余 8195 个 trkpt 的 extensions 子树含 hr 元素与整数文本值
  当 app 启动并完成 GPX parse
  那么 parse 成功完成且不出现 panic
  并且 缺 hr 的 trkpt 对应 TrajectoryPoint.heart_rate_bpm 等于 None
  并且 含 hr 的 trkpt 对应 TrajectoryPoint.heart_rate_bpm 等于该 trkpt hr 文本的整数值

场景: GPX 缺少 cad 字段时正常解析且 HUD 隐藏踏频
  测试: test_gpx_parse_missing_cad
  假设 文件 assets/cycling-track.gpx 中所有 trkpt 不含 cad 字段
  当 app 启动并执行 GPX parse
  那么 parse 成功完成
  并且 TrajectoryPoint.cadence_rpm 等于 None
  并且 HUD 在踏频字段位置显示破折号字符
  并且 不出现 panic 或 parse error

场景: GPX 缺少文件级 hr 字段时拒绝 cycling 默认数据
  测试: test_gpx_parse_rejects_missing_hr
  假设 文件 assets/cycling-track.gpx 中所有 trkpt 不含 hr 字段
  当 app 启动并执行 cycling 默认数据 parse
  那么 parse 返回 Err
  并且 错误信息字符串包含 "cycling 默认数据必须含 hr 字段"
  并且 app 显示错误提示而非静默 fallback 到 0

场景: cargo tree 不含禁用依赖且包含 Robius 全套件
  测试: test_no_forbidden_deps
  层级: static
  假设 Cargo.toml 由后续实现生成
  并且 禁用依赖集合包含 "georust"
  当 在仓库根执行 cargo tree
  那么 输出文本不含字符串 "georust"
  并且 输出文本不含字符串 "serde-xml-rs"
  并且 输出文本不含字符串 "aws-lc-rs"
  并且 输出文本不含字符串 "tokio"
  并且 输出文本不含字符串 "reqwest"
  并且 输出文本包含字符串 "robius-use-makepad"
  并且 输出文本包含字符串 "robius-open"
  并且 输出文本包含字符串 "robius-directories"
  并且 输出文本包含字符串 "robius-location"
  并且 输出文本包含字符串 "robius-proxy"

场景: C+ 视觉契约 shader uniform 命名与背景色
  测试: test_shader_uniforms_and_background
  假设 后续实现已生成 makepad widget 文件位于 src 目录下
  当 在 src 下搜索所有 shader 与 widget 中的 uniform 声明
  那么 存在 uniform 名为 track_progress
  并且 存在 uniform 名为 polyline_color_mix
  并且 存在 uniform 名为 particle_density
  并且 存在 uniform 名为 elevation_z
  并且 存在 uniform 名为 guard_pulse_phase
  并且 存在 uniform 名为 walked_segment_ratio
  并且 存在 uniform 名为 scrubber_echo_phase
  并且 主屏 widget 的背景颜色字面量包含字符串 #0A0A0F

场景: trajectory-replay-data manifest 指定 cycling 默认数据
  测试: test_manifest_selects_cycling_default_dataset
  层级: integration
  替身: mock GitHub Contents API manifest response
  假设 GitHub Contents API 返回 manifest.json
  当 app 启动并读取 manifest
  那么 manifest.default_profile 等于 "cycling"
  并且 manifest.default_dataset 等于 "cycling/bikeride.gpx"
  并且 manifest 来源 URL 以 "https://github.com/Project-Robius-China/trajectory-replay-data" 为仓库源
  并且 默认数据 URL 的仓库路径属于 "Project-Robius-China/trajectory-replay-data"
  并且 GitHub Contents API 响应包含字段 name path sha size content download_url
  并且 数据层从 content 字段或 download_url 取得 manifest 内容
  并且 默认数据请求使用 Contents API raw media type 或 fresh download_url
  并且 任何 403 404 rate limit timeout 均切换到 LocalFallback
  并且 fetch 逻辑不回退到 gpx-animator 原始仓库 URL 作为主路径

场景: 多类型轨迹数据层归一化但渲染层只消费 cycling
  测试: test_multi_profile_data_layer_renderer_cycling_only
  假设 默认 active_profile 为 "cycling"
  并且 manifest 中声明 profile 为 cycling running hiking walking transit travel flight 的 7 个数据集
  当 数据层加载 manifest 并构建 profile registry
  那么 profile registry 包含 cycling running hiking walking transit travel flight
  并且 每个 profile 都映射到统一 TrajectoryPoint 字段集合
  并且 flight profile 允许 altitude_m heading_deg route_label 字段为空或有值
  并且 travel profile 允许 transport_mode route_label 字段为空或有值
  并且 renderer 当前 active_profile 固定为 cycling
  并且 UI 不显示运动类型切换控件

场景: 已走段分色由 walked_segment_ratio 驱动
  测试: test_walked_segment_coloring
  层级: shader
  假设 app 处于 S2 主屏回放状态
  并且 PlaybackState.playback_progress 等于 0.35
  当 渲染轨迹 polyline
  那么 shader uniform walked_segment_ratio 等于 0.35
  并且 路径位置小于等于 0.35 的片段使用已走段颜色或已走段 opacity
  并且 路径位置大于 0.35 的片段使用未走段颜色或降低 opacity
  并且 已走段分色不改变 PlaybackState.current_trkpt_index

场景: scrubber echo 只作为拖动反馈不改变真实进度
  测试: test_scrubber_echo_visual_feedback_only
  假设 app 处于 S2 主屏回放状态
  并且 PlaybackState.playback_progress 等于 0.40
  当 用户把 scrubber 从 40% 快速拖动到 70%
  那么 PlaybackState.playback_progress 更新为 0.70
  并且 shader uniform scrubber_echo_phase 在 300 到 500 毫秒内从 1.0 衰减到 0.0
  并且 echo trail 显示在 scrubber thumb 后方
  并且 echo trail 不会额外修改 PlaybackState.playback_progress
