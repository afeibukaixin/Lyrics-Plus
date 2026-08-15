# shadcn UI 主题与排版规范

## 技术基线

普通应用界面统一使用 shadcn/ui 的 Base UI 实现、Nova 风格和 Tailwind CSS v4。主题入口为 `src/tailwind.css`，组件通过 `background`、`foreground`、`card`、`popover`、`primary`、`secondary`、`muted`、`accent`、`destructive`、`border`、`input`、`ring` 等语义 token 获取颜色。

不得在页面组件中添加独立品牌色、灰阶或手写暗色覆盖。成功与警告状态使用项目注册的 `success`、`warning` 语义 token。

## 明暗主题

应用保留三种主题偏好：

- `light`：使用 `:root` 中的浅色主题。
- `dark`：在根元素附加 `.dark`，使用深色主题。
- `system`：监听系统配色并解析为浅色或深色。

主题偏好仍由应用配置持久化，并同步到设置窗口、快速歌词窗口、更新与法律弹窗、Toast、桌面歌词工具栏和解锁控件。组件不自行判断系统主题。

## 组件优先级

新增或修改普通界面时按以下顺序选择实现：

1. 使用已安装的 shadcn 组件及其现有 variant。
2. 组合 `Sidebar`、`Card`、`Field`、`Item`、`Alert`、`Empty`、`ScrollArea` 等组件表达页面结构。
3. 通过语义 token 调整组件状态。
4. 仅在布局、窗口约束、拖拽、动态颜色或动画无法由组件表达时保留 CSS Modules。

设置项使用横向 `Field`，说明文字使用 `FieldDescription`；布尔值、范围、下拉和少量选项分别使用 `Switch`、`Slider`、`Select`、`ToggleGroup`。输入框内操作使用 `InputGroup`。空状态、状态标签、分隔线和通知分别使用 `Empty`、`Badge`、`Separator`、Sonner。

## 排版与间距

- 页面标题使用 Tailwind `text-2xl font-semibold`。
- 卡片标题、正文、说明和控件文字沿用 shadcn 组件默认排版，不在页面中覆盖组件字号或字重。
- 技术文本使用 `--font-family-mono`，例如配置内容、日志、路径和快捷键。
- 布局间距使用 `gap-*`，不使用 `space-x-*` 或 `space-y-*`。
- 等宽等高尺寸使用 `size-*`，文本截断使用 `truncate`。
- 条件类名使用 `cn()`；按钮中的前后图标使用 `data-icon`。

## 合理例外

桌面歌词正文是用户可配置的展示内容，继续使用运行时字号、缩放、方向、卡拉 OK、跑马灯与换行算法，不套用普通 UI 排版。

配置编辑器的等宽文本、行号同步，调试日志的技术信息，以及桌面歌词的拖拽和缩放柄可以保留专用 CSS。颜色预览块可以使用运行时内联背景色，但其按钮、弹层、输入和校验状态仍须使用 shadcn 组件。
