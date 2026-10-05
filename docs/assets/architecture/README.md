# Oxide 架构面板素材

这组素材用于根目录 [README](../../../README.md)，使用 [live-panel-skill](https://github.com/ythx-101/live-panel-skill) 的 `terminal-dark` 模板，重新编排 Oxide 的实际架构。布局固定，数据包、任务状态与日志随同一时间轴变化。节点说明、任务状态和运行日志均使用中文，技术产品名保留原名；字体包含中文回退。

| 文件 | 用途 |
| --- | --- |
| [oxide.gif](oxide.gif) | GitHub README 内联播放，960px 宽、12fps |
| [oxide.png](oxide.png) | 1440 × 1500 高清静态图，方便放大阅读 |
| [oxide.mp4](oxide.mp4) | 16 秒、20fps 的 H.264 动态视频 |
| [oxide.html](oxide.html) | 自包含动态页面，下载后在浏览器直接打开 |
| [oxide.config.json](oxide.config.json) | 节点、连线、配色与演示状态机的源配置 |
| [LICENSE.live-panel.txt](LICENSE.live-panel.txt) | 生成页面所用模板引擎的 MIT 许可 |

## 内容依据

- 请求入口、Web/API 分工与部署连接：[系统架构](../../architecture.md)、[前端 Vite/Nitro 配置](../../../frontend/vite.config.ts)、[OpenResty 配置](../../../deploy/openresty.conf)。
- API、后台任务与 Agent：[启动入口](../../../src/main.rs)、[模型解析](../../../src/infrastructure/agent.rs)、[评论自动审核](../../../src/infrastructure/comment_moderation.rs)。
- PostgreSQL、Redis 与对象存储：[Compose](../../../deploy/compose.yaml)、[SeaORM 实体](../../../src/entity)、[存储实现](../../../src/infrastructure/storage.rs)。
- 搜索索引：[Tantivy / Jieba 实现](../../../src/infrastructure/search.rs)。
- 72 小时加密备份、下载校验与成功后清理：[备份实现](../../../src/infrastructure/backup.rs)、[备份与恢复](../../../deploy/BACKUP.md)。

节点与固定技术版本来自代码；**所有移动数据包、计数、状态轮换和日志均为模拟**。面板不请求 API、模型或生产遥测，也不运行仓库里的 Agent。图中的任务分支表示能力边界，不表示项目实现了 Codex 的多 Agent 调度。

## 重新生成

需要 Python 3、Chrome/Chromium 与 ffmpeg。使用上游版本 `8a70aa2c4e3fac68b40e2472407e32e2637a7a36`，在项目根目录执行；`/path/to/chrome` 替换为实际可执行文件路径。

```sh
git clone https://github.com/ythx-101/live-panel-skill /tmp/oxide-live-panel-skill
git -C /tmp/oxide-live-panel-skill checkout 8a70aa2c4e3fac68b40e2472407e32e2637a7a36

python3 /tmp/oxide-live-panel-skill/scripts/check_frames.py \
  --config docs/assets/architecture/oxide.config.json \
  --out-dir /tmp/oxide-architecture-frames --repeat \
  --chrome /path/to/chrome

python3 /tmp/oxide-live-panel-skill/scripts/render.py \
  --config docs/assets/architecture/oxide.config.json \
  --out docs/assets/architecture/oxide.mp4 \
  --html-out docs/assets/architecture/oxide.html \
  --chrome /path/to/chrome --audio none

ffmpeg -y -i docs/assets/architecture/oxide.mp4 \
  -vf 'fps=12,scale=960:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=96[p];[b][p]paletteuse=dither=bayer:bayer_scale=3' \
  -loop 0 docs/assets/architecture/oxide.gif

ffmpeg -y -ss 6 -i docs/assets/architecture/oxide.mp4 \
  -frames:v 1 docs/assets/architecture/oxide.png
```

生成后的 HTML 随附引擎许可；重新生成后执行下面的命令保留页面的独立分发许可：

```sh
python3 - <<'PY'
from pathlib import Path
page = Path("docs/assets/architecture/oxide.html")
license_text = Path("docs/assets/architecture/LICENSE.live-panel.txt").read_text()
html = page.read_text()
if "Copyright (c) 2026 live-panel contributors" not in html:
    html = html.replace("<html", "<!--\n" + license_text + "\n-->\n<html", 1)
page.write_text(html)
PY
```

帧检查覆盖 120 个采样时间点与 4 张截图，检查文字溢出、重叠和重复 seek 的确定性。动画通过 `window.seek(t)` 重放，页面常规打开时由 `requestAnimationFrame` 驱动；当前模拟不保证首尾状态无缝衔接。

## 风格来源与许可

终端面板风格与动效思路来源于 [@thedelost 的 Codex Agent-tree 设计](https://x.com/thedelost/status/2105398038026195279)，经 [@slashui 引用](https://x.com/slashui/status/2105850132365443528)；模板引擎由 [ythx-101/live-panel-skill](https://github.com/ythx-101/live-panel-skill) 提供。Oxide 的节点内容与布局重新设计，不复制原图中的模型角色、指标或组织结构。

引擎代码遵循随附 MIT 许可。原示例视觉设计属于原作者；这里保留屏内、README 与本说明的来源署名。
