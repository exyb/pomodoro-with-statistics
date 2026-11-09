# 扩展接口（Plugin API）

番茄钟把「计时过程中往外推提示」这件事抽象成一个插件协议。内置示例是 `wordcard`
（英语单词卡），后续加听力练习、站立提醒、喝水提醒等都按同一套接口写。

## 运行模型

```
计时引擎 (engine.rs)                Rust 插件                    WebView
 ── PluginEvent ─────────────▶  on_event() -> Vec<Prompt>
                                          └──▶ pomodoro://prompt ──▶ PromptSlot 渲染
```

- 插件是**编译进主程序**的 Rust 组件，不是动态加载的脚本；注册点只有一处：
  `Registry::builtin()`（[src-tauri/src/plugins/mod.rs](../src-tauri/src/plugins/mod.rs)）。
- 插件只能「产出提示」，不能改动计时状态机，所以第三方提示不会打断统计口径。
- 提示通过 `pomodoro://prompt` 事件广播，主窗口和悬浮窗共用同一个 `PromptSlot`。

## 后端：实现 `PomodoroPlugin`

```rust
pub trait PomodoroPlugin: Send {
    fn id(&self) -> &'static str;              // 稳定标识，配置与状态都以它为键
    fn name(&self) -> &'static str;            // 设置页标题
    fn description(&self) -> &'static str;     // 设置页说明
    fn default_config(&self) -> Value;         // JSON 对象：设置页据此渲染表单
    fn on_event(&mut self, event: &PluginEvent, config: &Value) -> Vec<Prompt>;
    fn dump(&self) -> Value { Value::Null }    // 可选：持久化
    fn restore(&mut self, _state: Value) {}
}
```

事件（`PluginEvent`）：

| 事件 | 触发时机 | 附带信息 |
| --- | --- | --- |
| `PhaseStarted` | 一个阶段开始计时 | `phase`、`planned_ms` |
| `Tick` | 运行中每 200ms 一圈（暂停时不发） | 剩余/已用/计划时长、描述、任务 id、当前毫秒 |
| `PhaseCompleted` | 阶段自然走完 | `phase`、`focus_ms` |
| `PhaseAbandoned` | 用户停止或跳过 | `phase`、`focus_ms` |
| `Idle` | 计时器停回待命（点停止，或阶段结束后没有自动开始下一阶段） | — |

提示（`Prompt`）字段：

| 字段 | 含义 |
| --- | --- |
| `id` | 由 `Registry` 分配的单调剂，前端据此关闭/替换 |
| `plugin_id` | 来源插件 |
| `slot` | `timer` / `floating` / `banner`，决定投到哪个位置 |
| `kind` | 渲染器 key，对应前端 `promptRenderers` 的键 |
| `title` / `body` | 没有专用渲染器时的兜底文案 |
| `ttl_ms` | 自动消失延时；`0` 表示保留直到被同 slot 的新提示替换 |
| `data` | 任意 JSON，交给专用渲染器使用 |

约束：
- `dispatch` 对每个启用的插件调用一次；同一个 slot 一次只保留一条提示。
- 只有产出了提示才会调用 `save_states`，所以插件状态写库的频率由提示频率决定。
- 配置合并规则：`default_config()` 打底，用户在设置页存的键覆盖同名默认值，
  未知键保留（见 `merge_config`）。新增配置项不需要迁移数据库。
- 插件状态存在 `plugin_state(plugin_id)` 一行 JSON 里，进程退出（`RunEvent::Exit`）
  和每次派发后都会落盘。

## 前端：登记渲染器

`kind` 到组件的映射在 [src/components/prompts/registry.ts](../src/components/prompts/registry.ts)：

```ts
export const promptRenderers: Record<string, Component> = {
  word: markRaw(WordPrompt),
}
```

组件契约：`defineProps<{ prompt: Prompt }>()`，样式自己负责；关闭按钮由
`PromptRenderer.vue` 统一提供，组件不需要处理 `dismiss`。
未登记的 `kind` 会退化成 `title` + `body` 的通用卡片，所以**只加后端插件、不动前端
也能出提示**，只是排版朴素。

## 新增一个插件的步骤

1. `src-tauri/src/plugins/your_plugin.rs`：实现 `PomodoroPlugin`，`on_event` 里自己
   做节流（参考 `wordcard` 用 `now_ms - last_emit_ms >= interval`）。
2. `src-tauri/src/plugins/mod.rs`：`pub mod your_plugin;` 并把它加进 `Registry::builtin()`。
3. 需要专属排版时，在 `src/components/prompts/` 写一个组件，并在 `promptRenderers`
   用你的 `kind` 注册；否则跳过这步。
4. 设置页无需改动：`PluginMeta.config.default` 会自动生成开关和字段。

## 内置示例：`wordcard`

- 词库 `src-tauri/src/plugins/words.json`（`en` / `zh` / `example`），`include_str!` 编进二进制。
- 选取策略：按「见过次数最少」轮转，状态记在 `plugin_state`，重启后继续。
- 配置键：`intervalSeconds`、`ttlSeconds`、`showDuringWork`、`showDuringBreak`、
  `showInFloating`、`wordsPerPrompt`。
- 一次派发同时产出 `timer` 与（开关打开时）`floating` 两条提示。
