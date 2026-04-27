# Kickoff — 给下一个 cc session 的全自主任务书

> **使用方式**: 新开 cc session, 第一条消息只需说一句:
>
> > 读 `/Users/zhaoyue/workspace/matrix/mobile_example/kickoff.md` 然后按上面执行, 全程不用问我。
>
> 不要复制本文件内容到 prompt 里, 让 cc 自己 Read 这个文件 — 这样以后改任务只动这一个文件即可。

---

## 总则

你接下来全程自主推进, 用户不会中途回应。除了"卡死上报"以外, 不要问任何问题。

**默认决策权**: 二义性裁决遵循 `visual.spec.md` > `prd.md §6` > `design/refs/*.png` 顺序。三件套都决定不了时, 你自己拍板, 把决策写进 commit message body。

---

## 任务

在当前仓库 `/Users/zhaoyue/workspace/matrix/mobile_example` 用 Makepad 2.0 实现一个 Android 端骑行轨迹回放 demo, 交付目标是 **Project-Robius-China 社区 review 时可现场跑通的开源示例**。

最终验收 = `visual.spec.md` 22 条 BDD + `spec.spec.md` 34 条 BDD 全部通过自审 (允许少量 BLOCKED, 见 [完成条件](#完成条件))。

---

## 开工前必读 (顺序固定)

1. `CLAUDE.md` — 你的工作上下文, 真值优先级 / skill 索引 / 截图硬约束都在这
2. `prd.md` — 产品需求, 重点 §6 视觉锚定 / §6.4 真值条款
3. `visual.spec.md` — 22 条视觉 BDD, **这是验收清单**
4. `spec.spec.md` — 34 条行为 BDD
5. `design/refs/storyboard-{1,2}.png` — 4+3 屏拼图底板, 每写完一屏要回来对照

读完先用 `TaskCreate` 把工作拆成可执行 task 列表 (按屏切: S0 同步 / S1 path-draw / S2 主回放 / S3 Guard / S4 stats), 每个 task 用 `TaskUpdate` 切 `in_progress` / `completed`。用户离场后靠这个看进度。

---

## 实现纪律

- 按 `CLAUDE.md` 标 ★★★ 的 skill 顺序读 SKILL.md 再写代码
  - **必读**: `makepad-2.0-design-judgment` / `app-structure` / `dsl` / `shaders` + `xor-shader-techniques`
  - **写到对应模块再读**: `widgets` / `layout` / `events` / `animation` / `vector`
- shader 先抄 `xor-shader-techniques` 现成片段, 别自己从零推导 SDF / glow / bloom
- 写不通过 `cargo check` 的代码不要继续往下加, 先修编译再走
- **不要**碰 `prd.md` / `visual.spec.md` / `spec.spec.md` — 这是契约不是代码

---

## 视觉自审 (这一节最关键, 严格执行)

每完成一屏立刻跑这个闭环:

1. `cargo run` 等价命令把 demo 跑起来
2. 严格按 `CLAUDE.md` "截图工作流模板" 的 5 步走:
   1. **osascript 把 makepad 窗口提到最前** (这一步漏了截到的就是 IDE, 自审作废)
   2. `screencapture -x` 截原图到 `design/auto/raw_<HHMMSS>.png`
   3. `sips -Z 800 -s formatOptions 70` 降采样到 `_800w.png`
   4. `rm` 原图
   5. `Read` 降采样版本
3. 对照 `visual.spec.md` 该屏所有 BDD 逐条写一行:

   ```
   BDD-N: [我观察到 ...] → [通过 / 不通过, 因为 ...]
   ```

4. 不通过的条目, 调 shader uniform / token / layout, 重跑步骤 1-3
5. 同一屏视觉 diff 卡 5 轮还过不了, 把这条 BDD 标 `BLOCKED`, 写理由, 跳到下一屏

### 禁止行为 (踩到就当 task 失败)

- ✗ Read 任何未经 `sips -Z 800` 处理的原始截图 (会爆 context)
- ✗ 没截图就声称 BDD 通过 (无视觉证据 = 不通过)
- ✗ 看到 diff 就说 "差不多", 必须给出具体像素 / 颜色 / 位置差异描述
- ✗ 把代码改动当成视觉结果 (改了 shader 不等于看到效果)
- ✗ 没 osascript 提前不截图 (默认窗口不在最前, 截到的 99% 不是目标)

---

## Git 节奏

- 每完成一屏的 BDD 自审 (含 BLOCKED 标注) 提一个 commit
- commit message 格式:

  ```
  feat(S2): 主回放屏完成, BDD 18/22 通过, 4 条 BLOCKED (见 body)

  通过: BDD-1, BDD-2, ...
  BLOCKED: BDD-7 (理由: ...) ...
  决策: <非契约默认的设计选择写这里>
  ```

- **不要** push, 用户后面自己看
- 中途 context 接近上限时, 先提一个 WIP commit 把当前进度落地, 总结写进 body, 这样下一个 cc session 能从 git log + design/auto 恢复上下文

---

## 卡死上报 (唯一允许停下的条件)

满足任一条立刻停, 在最后一条消息里按下面模板写报告, 然后停止后续 tool 调用:

- **A**. 同一编译错误连续 30 分钟 (≈10 轮 `cargo check`) 没解决
- **B**. 同一屏 BDD 自审连续 8 轮全部不通过且 diff 没收敛
- **C**. 命中 prd / visual.spec / spec.spec 之间硬冲突, 三件套都决定不了
- **D**. macOS 截图权限突然失效 (`screencapture` 报错 `could not create image`)
- **E**. osascript 提前总报 `-1719 invalid index`, 说明 demo 进程根本没起 / 崩了, 排查 `cargo run` 后仍不能恢复

### 报告模板

```
## BLOCKED
- 触发条件: [A/B/C/D/E]
- 已尝试: [3-5 句, 干了什么 / 看到什么]
- 当前状态: [代码到哪了 / 哪些 task 完成 / 哪些没动]
- 我建议下一步: [1-2 个具体方向]
- 截图证据: design/auto/<file>_800w.png
```

---

## 完成条件

满足下列**全部**即可宣布完工, 写一条总结然后停:

- ✓ 22 条视觉 BDD + 34 条行为 BDD 全部 PASS / BLOCKED 标注完整
- ✓ 单屏 BLOCKED 总数 ≤ 6 条 (允许少量妥协, 但每条要有 commit 记录)
- ✓ `design/auto/` 里有完整的最终验收截图 (每屏至少一张 800w PNG)
- ✓ git log 看得出每屏一个 commit 的清晰节奏

---

## 第一步

读 `CLAUDE.md`。不要跳过。读完之后再读三件套, 然后 `TaskCreate` 拆任务, 然后开始第一屏。
