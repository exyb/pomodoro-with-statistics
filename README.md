# 番茄钟 Pomodoro · with statistics

Linux 桌面番茄工作法计时器（Manjaro KDE / X11 为主要目标环境）。Tauri 2 + Vue 3 + 嵌入式 SQLite（rusqlite bundled，无需安装数据库）。

## 功能

- **计时器**：专注 / 短休息 / 长休息时长均可自定义（1–180 分钟），支持快捷预设与自定义分钟数；每 N 个番茄后自动进入长休息。
- **任务描述**：每个专注计时可填写"本次做什么"，或直接关联一条待办（关联后完成的番茄自动计入该任务）。
- **待办侧栏**：可折叠（Ctrl+B），支持增删改、状态流转（待办/进行中/已完成）、拖拽排序；**拖到计时器上即完成关联**。
- **统计视图**：专注时长、完成番茄、中断次数与中断时长、休息时长、完成率、任务明细、小时分布热力图；支持按天（7/14/30 天）与按周（4/8/12 周）切换；计时记录可逐条删除。
- **个性化**：浅色 / 深色 / 跟随系统主题、强调色、界面平滑缩放（70%–180%）、紧凑布局、倒计时是否显示秒；声音开关与音量、自定义提示音文件、KDE 桌面通知、托盘常驻。
- **极简悬浮窗**：独立置顶小窗（Ctrl+F），可拖动、可调大小与不透明度、可点击穿透，不占任务栏。
- **扩展接口**：Rust 侧插件协议（见 [docs/PLUGIN-API.md](docs/PLUGIN-API.md)），内置"英语单词卡"示例插件——在计时过程中按间隔推送单词到主界面和悬浮窗。

## 快捷键

| 按键 | 功能 |
| --- | --- |
| 空格 | 开始 / 暂停 / 继续 |
| Ctrl+S | 停止计时 |
| Ctrl+K | 跳过当前阶段 |
| Ctrl+B | 折叠 / 展开待办侧栏 |
| Ctrl+F | 开关极简悬浮窗 |
| Ctrl+1/2/3 | 切换 计时 / 统计 / 设置 |

## 数据与目录

- 数据库：`~/.local/share/io.github.yuanbo7.pomodoro/pomodoro.db`（SQLite，WAL 模式；崩溃后会自动把遗留的 running 会话标记为 aborted）
- 设置：同一目录内以 JSON 存于 `kv` 表，新字段向后兼容

## 构建与运行

依赖：Node.js + npm、Rust (1.85+)、Linux 下的 webkit2gtk-4.1 / gtk3 开发库、`squashfs-tools`。

```bash
npm install
npm run tauri dev          # 开发调试
scripts/build-appimage.sh  # 产出 AppImage（见下）
cargo test                 # 后端单元测试（在 src-tauri/ 下）
```

### 打包说明（重要）

`scripts/build-appimage.sh` 会在 `npm run tauri build` 之后，跳过 tauri 自带的 AppImage 合成步骤，改为手动 `mksquashfs` + 官方 type2-runtime 拼接。原因是当前网络环境会把 tauri 打包器下载的 `linuxdeploy` 替换成无关文件，导致 `tauri build` 报告成功却不出产物（脚本会对 AppRun / runtime 做魔数与体积校验，防止打进被替换的文件）。

## 安装为桌面应用

```bash
cp src-tauri/target/release/bundle/appimage/Pomodoro_*_amd64.AppImage ~/Applications/Pomodoro.AppImage
cp src-tauri/target/release/bundle/appimage/Pomodoro.AppDir/Pomodoro.png \
   ~/.local/share/icons/hicolor/512x512/apps/pomodoro.png
# .desktop 示例见 ~/.local/share/applications/pomodoro.desktop
update-desktop-database ~/.local/share/applications
```

之后即可从 KDE 应用菜单（搜索"番茄钟"）或快捷方式启动；设置里的"开机自启"会写入 KDE autostart。

## 目录结构

```
src/                  Vue 前端（主窗口 + 悬浮窗两个入口）
  components/         计时面板 / 待办侧栏 / 统计图表 / 设置页 / 提示卡
  stores/app.ts       Pinia 全局状态，invoke 后端命令
  lib/                api 封装、主题与缩放、类型定义
src-tauri/            Rust 后端
  src/engine.rs       计时编排：阶段切换、持久化、提醒、插件分发
  src/timer.rs        权威计时核心（单调时钟，双窗口共享一份状态）
  src/db.rs           SQLite 迁移与访问
  src/stats.rs        按天/周聚合统计
  src/plugins/        插件协议与内置单词卡插件
docs/PLUGIN-API.md    插件扩展接口文档
```
# pomodoro-with-statistics
