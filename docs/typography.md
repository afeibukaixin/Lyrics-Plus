# UI 排版规范

## 适用范围

本规范用于主窗口、设置窗口、歌词库、快速歌词窗口和共享 React 组件中的普通 UI 文本。Token 定义在 `src/styles.scss` 的 `:root` 中，字号统一使用 `rem`，并继续受现有 `uiFontScale` 配置控制。

桌面歌词、图标字形、按容器动态计算的展示字号和调试日志等例外不强制套用普通 UI Token，具体边界见“合理例外”。

## 语义化 Token

| 语义 | 字号 Token | 默认字号 | 行高 Token | 推荐字重 Token | 使用场景 |
| --- | --- | ---: | --- | --- | --- |
| `caption` | `--typography-caption-size` | `0.5rem` | `--typography-caption-line-height`（`1.5`） | `--typography-caption-weight`（`500`） | 时间戳、计数、次要元数据、极弱提示 |
| `label` | `--typography-label-size` | `0.5625rem` | `--typography-label-line-height`（`1.4`） | `--typography-label-weight`（`600`） | 表单标签、按钮文字、徽标、导航辅助文字 |
| `body-sm` | `--typography-body-sm-size` | `0.625rem` | `--typography-body-sm-line-height`（`1.5`） | `--typography-body-sm-weight`（`400`） | 辅助说明、设置描述、空状态说明、Toast |
| `body` | `--typography-body-size` | `0.6875rem` | `--typography-body-line-height`（`1.5`） | `--typography-body-weight`（`400`） | 常规正文、输入内容、列表正文 |
| `title-sm` | `--typography-title-sm-size` | `0.8125rem` | `--typography-title-sm-line-height`（`1.35`） | `--typography-title-sm-weight`（`600`） | 卡片标题、面板标题、强调列表文字 |
| `title` | `--typography-title-size` | `1.1875rem` | `--typography-title-line-height`（`1.25`） | `--typography-title-weight`（`700`） | 页面或设置分区标题 |
| `display` | `--typography-display-size` | `1.5rem` | `--typography-display-line-height`（`1.15`） | `--typography-display-weight`（`700`） | 页面主标题、核心内容标题、强展示文本 |

组件应按语义同时引用字号、行高和字重，例如：

```scss
.panelTitle {
  font-size: var(--typography-title-sm-size);
  line-height: var(--typography-title-sm-line-height);
  font-weight: var(--typography-title-sm-weight);
}
```

不应以某个页面的当前数值命名 Token，也不应为单个组件直接新增仅供自身使用的全局字号。现有层级确实无法表达新语义时，应先记录使用场景和复用范围，再扩展体系。

## 迁移映射

阶段 1 盘点覆盖 `src` 下全部 SCSS 及 React 内联字号，阶段 4 已按以下映射完成普通 UI 迁移：

| 现有用法 | 主要语义 | 迁移方向 |
| --- | --- | --- |
| `.4375rem`、`.46875rem`、`.5rem` | 极弱说明、时间、计数 | `caption`；过小用法在迁移时统一校正可读性 |
| `.5625rem` | 标签、按钮、辅助文字 | `label`；长段说明可改用 `body-sm` |
| `.625rem` | 说明文字、状态、Toast | `body-sm` |
| `.6875rem`、`.75rem` | 正文、列表主要文字 | `body`；需要强调时使用 `title-sm` |
| `.8125rem`、`.875rem`、`1rem` | 卡片、面板和内容标题 | `title-sm` |
| `1.1875rem`、`1.3125rem` | 页面或分区标题 | `title` |
| `1.4375rem`、`1.5rem`、`1.75rem` | 页面主标题、核心展示内容 | `display` |

当前显式行高主要为 `1`、`1.2`、`1.45`、`1.5`、`1.55`、`1.6`，字重主要为 `550`、`600`、`750`、`800`。普通 UI 迁移时优先采用 Token 推荐值；单行截断或紧凑控件可保留 `line-height: 1`，品牌标签及关键状态可保留较高字重，但需要在使用处说明语义。

## 阶段 4 迁移结果

设置页、主页面、歌词库、快速歌词窗口、调试日志和配置编辑器中的普通 UI 固定字号均已引用语义化 Token，并同时引用相应行高与推荐字重。`body` 提供 `body` 默认层级，`button`、`input`、`select` 和 `textarea` 统一继承字体属性；技术文本统一引用等宽字体 Token。

中英文共用同一布局规则。设置卡片、快捷键行和颜色面板允许换行；侧栏、标题区和状态说明允许长单词断行；主窗口导航及操作区允许弹性换行；Toast 宽度受视口约束。歌曲名、歌手、路径和外部内容仍可按既有信息密度使用省略号，这属于内容展示策略，不通过缩小英文字号适配。

静态审计后保留的非 Token 字号均属于以下已记录例外，不需要扩展 Token 体系：

- `Overlay.module.scss` 的桌面歌词内容和紧凑工具栏由用户配置或浮层尺寸约束。
- 首页当前歌词激活态及曲名 `clamp()` 是核心动态展示；图标按钮、封面占位符和空状态图形是图标字形。
- 设置侧栏、搜索框及空状态中的固定字号字符仅用于图标字形。
- 搜索图标与圆形清除按钮保留 `line-height: 1`，用于图形居中而非文本层级。

## 字体回退

普通 UI 使用 `--font-family-sans`。顺序优先覆盖 Inter 和 macOS 系统字体，再覆盖 Windows 与常见简体中文字体，最后回退到 Arial 和通用无衬线字体。等宽内容使用 `--font-family-mono`，适用于配置编辑、快捷键、路径和日志等技术文本。

字体族 Token 不改变歌词、歌曲名或外部内容本身。缺少首选字体时应允许系统自然回退，不依赖字体合成模拟字形。

## 合理例外

- 桌面歌词：`Overlay.tsx` 中由用户配置的 `style.fontSize`、辅助歌词缩放比例、适配缩放和最小字号继续使用动态 `px`；不得改成普通 UI Token。
- 桌面歌词工具栏：工具栏是浮层内的紧凑操作区，目前 `8px`、`9px` 属于受浮层尺寸约束的例外，后续迁移时单独复核可读性。
- 动态展示：依赖容器宽度的 `clamp()`、歌词激活态以及运行时计算的字号可保留，但静态回退值应尽量引用语义 Token。
- 图标字形：仅用于视觉图标且由 `font-size` 控制尺寸的声明可以保留 `px`；它们不是文本排版。
- 调试日志、配置源码、路径和快捷键：可使用等宽字体 Token；字号仍优先映射到 `caption`、`label` 或 `body-sm`。
- 外部内容：歌曲名、歌手名、歌词和第三方服务原始响应不因普通 UI 规范被截短、翻译或机械改写。

## 后续开发规则

- 新增普通 UI 文本时先选择已有语义，不直接写任意 `font-size`。
- 同一语义同时统一字号、行高和推荐字重，局部差异必须有明确的交互或内容原因。
- 图标尺寸和动态计算等例外可以使用 `px`，普通 UI 固定字号使用 `rem`。
- 中英文共用同一语义层级；通过换行、弹性布局和合理宽度适配英文，不用缩小英文文本规避布局问题。
