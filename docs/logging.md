# 内部日志规范

## 适用范围

本规范适用于 Rust `log` 宏、Tauri 日志插件、前端 `reportFrontendError`、调试日志流以及辅助脚本的正式诊断输出。用户界面中的错误提示不属于内部日志，必须通过 i18n 资源和稳定错误码输出。

## 语言与格式

- 日志上下文统一使用英文，并以可搜索的动作和对象开头。
- 失败使用 `Failed to <action> <object>`；无法继续定位资源时可使用 `Unable to <action>`。
- 原始错误放在上下文之后，使用英文半角冒号分隔：`Failed to ...: {error}`。
- 日志应说明失败的操作和对象，不写面向用户的建议、安抚文字或翻译键。
- 不记录歌曲名、歌词、访问凭据或第三方响应正文等非必要内容。

推荐示例：

```text
Failed to restore the main window focus: {error}
Failed to sync the native overlay vibrancy effect: {error}
Failed to scan the lyrics library: {error}
```

## 统一术语

| 对象 | 英文术语 |
| --- | --- |
| 主窗口 | `main window` |
| 桌面歌词窗口 | `overlay` / `overlay window` |
| 快速歌词窗口 | `quick lyrics window` |
| 托盘菜单 | `tray` |
| 歌词库扫描 | `lyrics library scan` |
| 鼠标跟踪 | `mouse tracking` |
| 原生磨砂效果 | `native overlay vibrancy effect` |

同一操作应复用相同动词：状态校正使用 `reconcile`，同步使用 `sync`，窗口恢复使用 `restore` 或具体的 `unminimize`，任务调度使用 `schedule`。

## 前端错误详情

`reportFrontendError(context, error)` 负责把英文上下文和原始详情写入 Tauri 日志：

- `Error` 优先保留 `stack`，没有堆栈时保留 `message`；存在 `cause` 时递归追加 `Caused by` 链，并防止循环引用。
- 字符串保持原样。
- 其他值优先使用 JSON 序列化，失败时回退到 `String(value)`。
- 日志写入本身失败时不得递归上报。

启用实时调试日志后，`DebugLogProvider` 会注册全局 `error` 与 `unhandledrejection` 监听器，并在关闭时移除。Tauri 命令边界和桌面歌词窗口操作同样通过 `reportFrontendError` 记录。调用方不得只记录本地化后的用户提示，因为那会丢失底层原因。

## Rust 业务错误边界

Rust 命令返回的 `Result<_, String>`、歌词源诊断和配置校验详情不是日志语句，不进行机械翻译。它们可能包含系统、第三方服务或历史数据产生的原始文本，前端只把这些详情保存在 `AppOperationError.cause` 并写入内部日志；用户界面通过稳定错误码和 i18n 显示提示。

因此，日志的稳定英文上下文必须独立于原始错误语言。即使操作系统返回本地化错误，仍可通过开头的英文动作和对象搜索相关事件，同时保留完整底层原因。

## 调试日志

调试设置页显示的是开发者日志流，日志内容保持英文，不进入 i18n 资源。日志流仅在用户启用后附加，保存当前会话最近 300 条记录；切换启用状态会清空已有记录。`DEBUG`、`INFO`、`WARN`、`ERROR` 等级名保持固定写法；启用、筛选、清空等用户界面控件仍通过 i18n 输出。

## 辅助脚本

辅助脚本输出错误时必须先给出英文操作上下文，再附带原始 `Error` 对象或退出信息。不得只调用 `console.error(error)`，否则日志缺少稳定、可搜索的失败动作。
