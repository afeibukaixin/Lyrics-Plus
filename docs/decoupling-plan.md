# Lyrics Plus 全量解耦路线

## 总体目标

按职责边界逐阶段拆分 Lyrics Plus 前后端职责混杂的热点模块。执行从第一个未勾选阶段开始；每次完成一个阶段后立即更新本文件，并记录边界与检查结果，便于恢复上下文后继续。

本路线仅处理职责混杂的热点模块，不按行数拆分纯数据、样式、生成组件或单一算法文件。保持现有行为、配置、事件、Tauri 命令和前端 Hook 接口不变。

## 执行清单

### 后端基础设施

- [x] D01 收尾 `macos_status_item`：确认当前五模块拆分、补齐 import，并根据已有 dev watcher 输出解决编译错误。
- [x] D02 拆分 `app_runtime.rs`：分离全局快捷键、托盘状态同步、Dock/菜单栏图标和语言应用；原函数由 `mod.rs` 转发。
- [x] D03 拆分配置校验：将 draft 解析、JSONC 清理、字段结构、类型选项、数值范围拆入 `config/validation/`，保留 `validate_config_draft`。
- [x] D04 瘦身设置与应用命令：从 `commands/settings.rs`、`commands/application.rs` 抽出应用发现、配置应用副作用和显示配置同步，命令层仅负责参数与返回值。
- [x] D05 拆分 `overlay_placement.rs`：分离主窗口定位、浮窗位置持久化和工具栏方位计算。
- [x] D06 拆分 `overlay_pointer.rs`：分离桌面歌词、列表窗口解锁柄监控及运行时启动编排。
- [x] D07 拆分 `commands/overlay.rs`：把尺寸重置、边缘缩放、内容适配等纯几何算法移出命令层，再分离持久化与窗口副作用。
- [x] D08 拆分 `player/mod.rs`：分离播放模型、播放器选择路由和控制调用，通过 façade 维持现有 re-export。
- [x] D09 拆分 `player/system/mod.rs`：分离 System Media 适配器、元数据标准化、封面缓存和调色算法。
- [x] D10 拆分 `player/spectrum.rs`：分离采样缓冲、DSP、macOS Audio Tap、worker 生命周期和 Tauri 订阅发布。
- [x] D11 拆分歌词解析器：按基础 LRC、LyricsFile、平台逐字歌词、TTML、标准化与质量评估建立 `lyrics/parser/`。
- [x] D12 拆分歌词库：将数据库 schema、扫描协调、文件发现、增量索引和元数据解析拆入 `storage/library/`。
- [x] D13 拆分歌词源注册表：分离 provider catalog、并发执行、缓存/请求合并、冷却与健康状态；保留 `ProviderRegistry` 外观。
- [x] D14 拆分歌词运行时：把排序、搜索会话、自动采用、运行时发布和样式同步移到 `lyrics/runtime/`；`commands` 仅保留兼容转发。

### 前端状态与界面

- [x] D15 拆分 `AppConfigProvider`：提取默认配置、样式继承物化、配置订阅、主题副作用和配置 actions，保持 `useAppConfig` 返回结构不变。
- [x] D16 拆分 `usePlayback`：分离播放事件、封面加载、位置时钟和控制操作，主 Hook 只组合状态。
- [x] D17 拆分 `useLyrics`：分离文档加载、搜索/采用、偏移写队列和展示选择器，保持现有 Hook 接口。
- [x] D18 拆分设置壳：从 `settings.tsx` 提取 provider 操作、重置/导入操作及导航展示，继续使用现有 `SettingsContext`。
- [x] D19 拆分桌面歌词组件：将状态/API 操作集中到 controller Hook，`Overlay.tsx` 只负责组合已有布局、适配和拖动 Hook。
- [x] D20 拆分灵动岛组件集合：封面、频谱、快捷控制、展开播放器、跑马灯和卡拉 OK 各自独立，通过 barrel 保持原 import。
- [x] D21 拆分灵动岛窗口：提取事件订阅、频谱绘制和窗口状态协调，保留现有 motion/geometry Hook。
- [x] D22 拆分快速歌词窗口：分离搜索表单、候选选择/应用状态和纯展示组件。
- [x] D23 拆分列表歌词窗口：提取跟随、工具栏、偏移写入和窗口交互，歌词列表保持纯展示。
- [x] D24 拆分更新 Provider：分离更新服务、状态控制器和更新弹窗；保持 `useUpdates` 接口不变。

## 接口与依赖规则

- 所有现有 Tauri 命令名、事件名、序列化类型、配置结构和前端公开 Hook 均不改变。
- 原模块路径由 façade、wrapper 或 re-export 兼容；调用方只在消除错误依赖时调整 import。
- 依赖方向固定为：纯模型/算法 → domain service → runtime/controller → command/provider/view。
- 子模块默认私有，仅使用满足调用范围的 `pub(super)` 或 `pub(crate)`，不扩大 API。
- 不在解耦过程中顺带改算法、错误文案、刷新频率、交互或新增功能。

## 勾选与验收规则

每次只执行一个编号阶段。完成代码迁移、静态检查和错误修复后，立即在本清单中把该项改为 `[x]`，记录日期、模块边界和检查结果，再进入下一项。

- 不新增测试用例，不主动执行构建。
- 每阶段执行目标文件的格式检查、`git diff --check`、旧符号残留和调用路径检查。
- 若已有 Vite/Tauri dev watcher，读取其现有输出作为编译反馈；出现错误时当前阶段保持未勾选。
- 不自动提交 Git；保留用户已有改动。
- D01–D24 全部勾选后复查公开接口和工作区状态，再把 Codex 长期目标标记完成。

## 明确排除

`config/model.rs`、`config/migration.rs`、测试文件、SCSS、shadcn 通用组件、单个歌词源实现、`bootstrap.rs` 和已内聚的窗口生命周期模块不因体积单独拆分；后续只有发现新的职责交叉证据时才补充计划，不能私自扩展范围。

## 执行记录

| 阶段 | 日期 | 模块边界与检查结果 |
| --- | --- | --- |
| D01 | 2026-09-04 | `macos_status_item` 拆为 `mod.rs`、`icon.rs`、`payload.rs`、`renderer.rs`、`display_driver.rs`；保持五个 crate 内入口及外部调用路径不变，注入 `FrameCallback` 隔离刷新驱动。目标文件 `rustfmt --check` 通过；旧单文件、旧 `sync` 反向依赖和缺失 `tauri::Manager` 导入检查通过；工作区 `git diff --check` 通过；未执行构建和测试。 |
| D02 | 2026-09-04 | `app_runtime` façade 保留根级入口；快捷键注册/回滚移至 `shortcuts.rs`，托盘状态与加速键移至 `tray.rs`，Dock/菜单栏图标移至 `icons.rs`，原生语言应用移至 `language.rs`。保留现有函数签名、调用路径、错误文案与平台分支；目标 Rust 文件 `rustfmt --check`、旧 `include!`/旧文件残留、入口引用和工作区 `git diff --check` 均通过；未执行构建和测试。 |
| D03 | 2026-09-04 | `config/validation` façade 保留 `validate_config_draft` 与 `parse_config_draft` 的 crate 内路径；draft/JSONC 清理、字段结构、类型选项和数值范围分别移至 `draft.rs`、`structure.rs`、`fields.rs`、`ranges.rs`，错误定位/JSON 合并与模型归一化 helper 保持可见。迁移调用链、存储/配置内部调用和现有签名未变；目标 Rust 文件 `rustfmt --check`、旧单文件/符号检查和工作区 `git diff --check` 均通过；未执行构建和测试。 |
| D04 | 2026-09-04 | 应用发现与 Bundle/图标解析移至 `commands/application_discovery.rs`；快捷键、Dock/菜单栏图标、完整配置应用和显示同步移至 `commands/config_runtime.rs`，原命令文件保留参数/返回 façade。Tauri command 声明、内部测试 helper 名称、错误文本及回滚顺序未变；目标 Rust 文件 `rustfmt --check`、命令边界/调用路径和工作区 `git diff --check` 均通过；未执行构建和测试。 |
| D05 | 2026-09-04 | `overlay_placement` 迁移为目录 façade：`main_window.rs` 负责主窗口居中/浮窗坐标设置，`persistence.rs` 负责保存载荷类型与几何偏好读取，`geometry.rs` 负责纯居中与工具栏方位计算，`state.rs` 负责显示器拓扑和拖动状态，`toolbar.rs` 负责方位同步、拖动状态与收尾编排。`overlay_runtime.rs` 改为通过根级 re-export 保留原调用路径，现有 `overlay_persistence.rs` 的存储副作用与格式未变；目标文件 `rustfmt --check`、旧单文件/入口符号/调用路径检查和工作区 `git diff --check` 均通过；未执行构建和测试。 |
| D06 | 2026-09-04 | `overlay_pointer` 迁移为目录 façade：`geometry.rs` 保留边界/解锁柄纯计算，`overlay.rs` 负责桌面歌词解锁柄定位与 hover 采样，`list.rs` 负责列表解锁柄同步与监控，`runtime.rs` 负责编排运行时启动；唤醒通知集中在 `mod.rs`，根级 re-export 保留 `activate_runtime`、两个同步入口、唤醒入口和事件常量。采样间隔、隐藏延迟、事件名及窗口副作用顺序未变，并补回快捷键注册 façade 以保持启动调用链；目标文件 `rustfmt --check`、旧单文件/入口符号/调用路径检查和工作区 `git diff --check` 均通过；未执行构建和测试。 |
| D07 | 2026-09-04 | `commands/overlay` 的纯尺寸与边缘几何算法移至 `overlay_geometry.rs`，浮窗样式/几何偏好持久化移至 `overlay_persistence.rs`，命令文件仅保留 Tauri 参数解析、窗口副作用和结果返回；通过 commands 模块私有导入兼容现有测试与调用符号。保留所有命令签名、错误文案、锚点与缩放算法；目标文件 `rustfmt --check`、纯算法/持久化边界与调用路径检查和工作区 `git diff --check` 均通过；未执行构建和测试。 |
| D08 | 2026-09-04 | `player/mod.rs` façade 保留模型、控制、路由、频谱和 System Media 的现有 re-export；模型/快照归 `model.rs`，进程控制与播放/跳转调用归 `control.rs`，自动选择与系统来源过滤归 `routing.rs`，原有播放器自动化、频谱和系统模块路径不变。保留公开类型/方法与 `query_selected_player`、`control_playback`、`seek_playback` 入口，既有单元测试仅随模块迁移未新增；目标 Rust 文件 `rustfmt --check`、公开入口/调用路径与旧混合符号检查和工作区 `git diff --check` 均通过；未执行构建和测试。 |
| D09 | 2026-09-04 | `player/system` façade 保留 `SystemMediaService` 及其 `snapshot`、`control`、`seek`、`artwork` 方法；适配器初始化/订阅/精确进度刷新移至 `adapter.rs`，元数据规范化、轨道标识和快照投影移至 `metadata.rs`，封面指纹、缓存与 PNG 编码移至 `artwork.rs`，主色/频谱调色算法移至 `palette.rs`，既有兼容适配路径和单元测试保持不变。目标 Rust 文件 `rustfmt --check`、旧混合符号/公开入口/调用路径检查、未跟踪文件空白检查和工作区 `git diff --check` 均通过；未执行构建和测试。 |
| D10 | 2026-09-04 | `player/spectrum` façade 保留频谱服务、状态/帧类型及两个事件常量；采样缓冲与采样率同步移至 `input.rs`，FFT/频段合并/平滑移至 `dsp.rs`，macOS Audio Tap 与错误码移至 `audio_tap.rs`，worker 线程与运行态移至 `worker.rs`，订阅、状态协调和 Tauri 事件发布移至 `service.rs`，播放器模块原 re-export 与命令调用路径未变。目标 Rust 文件 `rustfmt --check`、旧单文件/职责边界/公开入口/调用路径检查、未跟踪文件空白检查和工作区 `git diff --check` 均通过；未执行构建和测试。 |
| D11 | 2026-09-04 | `lyrics/parser` façade 保留 `parse_lrc`、`parse_lrc_with_options`、`LyricsQualityReport`、质量报告和语义指纹的原级别入口；基础 LRC 与辅助轨选择移至 `basic_lrc.rs`，LyricsFile YAML 解析移至 `lyricsfile.rs`，平台逐字格式及语言轨移至 `platform.rs`，TTML 解析移至 `ttml.rs`，时间轴标准化与质量评估移至 `normalize.rs`。`lyrics` 根级 re-export 保持 providers、storage、commands 和既有歌词测试的调用路径；目标 Rust 文件 `rustfmt --check`、旧单文件/公开入口/调用路径检查、未跟踪文件空白检查和工作区 `git diff --check` 均通过；未执行构建和测试。 |
| D12 | 2026-09-04 | `storage/library` façade 保留 `LibraryScanPhase`、`LibraryScanStatus`、`LibraryScanCoordinator`、目录偏好常量和全部 `Storage` 扫描/目录方法；schema 初始化与碰撞元数据修复移至 `schema.rs`，扫描 generation 与 `Storage` façade 移至 `scan.rs`，文件发现和扩展名/目录规范化移至 `discovery.rs`，增量事务、缺失清理和时间戳计算移至 `index.rs`，文件名碰撞与歌词元数据解析移至 `metadata.rs`。数据库初始化、命令层调用和既有测试路径保持不变；目标 Rust 文件 `rustfmt --check`、旧单文件/职责边界/公开入口/调用路径检查、新文件空白检查和工作区 `git diff --check` 均通过；未执行构建和测试。 |
| D13 | 2026-09-04 | `provider_registry` façade 保留 `ProviderRegistry` 及其构造、设置、凭据、搜索、缓存、provider 测试和运行时辅助入口；provider catalog/实例构建与初始状态移至 `catalog.rs`，并发请求、结果校验与排序移至 `search.rs`，缓存生命周期与请求合并移至 `cache.rs`，健康状态、冷却退避、状态投影和 provider 测试状态移至 `health.rs`。`provider.rs` 通过模块声明及 re-export 保持原模块路径，`ProviderSettings`、provider matching、命令调用和既有 provider 测试路径不变；目标 Rust 文件 `rustfmt --check`、旧单文件/职责边界/公开入口/调用路径检查、新文件空白检查和工作区 `git diff --check` 均通过；未执行构建和测试。 |
| D14 | 2026-09-04 | `lyrics/runtime` façade 将运行时模型移至 `model.rs`，候选分析/排序/自动采用判定移至 `ranking.rs`，搜索会话、防抖、缓存请求合并与自动保存移至 `search.rs`，运行时事件发布移至 `publication.rs`，播放轨道键、活动歌词加载与运行时同步移至 `controller.rs`，设置分区/样式模式与桌面样式同步移至 `style.rs`；`commands/lyrics_runtime.rs` 保留原名称的 crate 内兼容转发，命令、状态和歌词模块的调用路径未变。目标 Rust 文件 `rustfmt --check`、旧单文件/旧实现符号、职责归属、公开入口与兼容调用路径检查、新文件空白检查和工作区 `git diff --check` 均通过；未执行构建和测试。 |
| D15 | 2026-09-04 | `AppConfigProvider.tsx` 保留 Provider 与 `useAppConfig` 入口及原返回字段顺序；默认配置移至 `provider/defaults.ts`，样式继承物化与灵动岛待写入覆盖移至 `inheritance.ts`，Tauri 配置订阅移至 `subscription.ts`，主题副作用与解析移至 `theme.ts`，全部配置 actions 与兼容 `syncConfig` 移至 `actions.ts`，上下文类型与实例移至 `context.ts`。保留 Web/Tauri 双路径、主题监听、配置事件和灵动岛偏好写队列；调用方 import 未变。目标文件 TypeScript 语法解析、旧实现残留/公开入口/调用方检查、新文件空白检查和工作区 `git diff --check` 均通过（仓库未配置 Prettier/Biome）；未执行构建和测试。 |
| D16 | 2026-09-04 | `usePlayback.ts` 保留原 Hook 入口、参数默认值和 23 个返回字段；播放快照/播放器选择加载与事件监听移至 `playback/events.ts`，封面 Blob URL、来源切换保护、竞态版本和清理移至 `artwork.ts`，100ms 位置时钟与插值计算移至 `position.ts`，播放器选择写入、控制串行队列、播放/暂停/切歌/跳转操作移至 `controls.ts`。保留 Web/Tauri 分支、事件名、封面确认延迟、进度上限、错误状态和调用方 import；主 Hook 仅组合各内部 Hook。目标文件 TypeScript 语法解析、返回字段顺序、职责符号/调用路径、空白检查和工作区 `git diff --check` 均通过；未执行构建和测试。 |
| D17 | 2026-09-04 | `useLyrics.ts` 保留原 Hook 入口、参数和 20 个返回字段；状态与共享引用移至 `useLyrics/state.ts`，文档加载、缓存读取、导入/解除关联移至 `document.ts`，搜索会话、已完成搜索恢复和候选采用移至 `search.ts`，歌词/库事件与切歌生命周期移至 `lifecycle.ts`，偏移写入合并队列移至 `offset.ts`，活动行、下一行及翻译/音译对齐选择移至 `display.ts`；`findAlignedAuxiliaryLine` 继续从原路径 re-export，所有调用方 import 未变。保留 track key 算法、事件名、错误处理、偏移重载保护与返回字段顺序；目标 TypeScript 语法解析、返回字段比对、职责/调用路径、旧实现残留、新文件空白检查和工作区 `git diff --check` 均通过；未执行构建和测试。 |
| D18 | 2026-09-04 | `settings.tsx` 保留设置页入口、`SettingsContext` 导出、状态组合和兼容 context 字段；provider 保存/凭据、拖拽排序、单源/全量测试移至 `settings/providerActions.ts`，样式与窗口副作用、文件导入、分区/位置重置和配置应用移至 `settings/operations.ts`，导航、主题切换与更新指示器派生移至 `settings/navigation.ts`，侧栏/工具栏/Outlet/重置弹窗渲染移至 `SettingsShell.tsx`。保留路由入口、context 54 个字段及其调用方 import、provider/重置/导入行为和更新状态展示；目标 TypeScript 语法解析、context 字段顺序、职责符号/调用路径、新文件空白检查和工作区 `git diff --check` 均通过；未执行构建和测试。 |
| D19 | 2026-09-04 | `Overlay.tsx` 保留桌面歌词内容派生、现有窗口布局/内容适配/resize Hook 组合与渲染；`useOverlayController.ts` 集中播放与歌词 presentation、样式/窗口状态、偏移写入、样式切换、窗口拖动及工具栏锁定/隐藏/设置入口；`OverlayToolbar.tsx` 改为接收 controller 回调，不再直接依赖 API。保留布局、跑马灯、卡拉 OK、拖动/缩放和窗口 API 行为，主组件调用路径与 `main.tsx` import 未变；目标 TypeScript 语法与 `tsc --noEmit`、工具栏既有 props/新增回调、API 引用边界、调用路径、新文件空白检查和工作区 `git diff --check` 均通过；未执行构建和测试。 |
| D20 | 2026-09-04 | `NotchLyricsComponents.tsx` 收缩为兼容 barrel；封面翻转图层移至 `NotchArtwork.tsx`，频谱柱图移至 `NotchSpectrum.tsx`，快捷控制移至 `NotchQuickControls.tsx`，展开播放器移至 `NotchExpandedPlayer.tsx`，跑马灯与时长常量移至 `NotchMarquee.tsx`，卡拉 OK 行移至 `NotchKaraokeLine.tsx`。保留 `NotchLyricsWindow.tsx` 的原 import、六个组件导出、歌词/滚动/播放控制/动画实现与样式路径未变；目标 TypeScript 语法与 `tsc --noEmit`、旧符号残留和调用路径检查、新文件空白检查、工作区 `git diff --check` 均通过；未执行构建和测试。 |
| D21 | 2026-09-04 | `NotchLyricsWindow.tsx` 保留歌词派生、渲染和现有 `useNotchIslandMotion`/`useNotchWindowGeometry` 组合；窗口 state/ref 初始化与宿主适配状态移至 `useNotchWindowState.ts`，频谱柱注册、Bezier 高度映射和播放频谱订阅移至 `useNotchSpectrum.ts`，布局/显隐/原生指针/宽度预览事件及其清理移至 `useNotchWindowEvents.ts`。事件名、RAF 入口、预览提交与 hover 重协调顺序未变；目标 TypeScript 语法与 `tsc --noEmit`、事件符号/调用路径/旧实现残留检查、新文件空白检查和工作区 `git diff --check` 均通过；未执行构建和测试。 |
| D22 | 2026-09-04 | `QuickLyricsWindow.tsx` 收缩为窗口编排与默认入口；搜索表单状态、时长解析、自动/手动搜索及刷新事件移至 `quickLyrics/useSearch.ts`，候选默认选中、当前候选判断、应用状态和提示移至 `quickLyrics/useSelection.ts`，时长/键值纯函数移至 `quickLyrics/utils.ts`，表单、候选列表和原文预览分别移至 `SearchForm.tsx`、`Results.tsx`、`Preview.tsx`。保留 `main.tsx` 原默认 import、Tauri 刷新事件、搜索/应用参数、提示与展示结构；目标 TypeScript 语法与 `tsc --noEmit`、旧符号残留和调用路径检查、新目录空白检查和工作区 `git diff --check` 均通过；未执行构建和测试。 |
| D23 | 2026-09-04 | `LyricsListWindow.tsx` 收缩为列表窗口编排；跟随状态与自动滚动移至 `useListLyricsFollowing.ts`，偏移写入队列移至 `useListLyricsOffset.ts`，工具栏状态/配置副作用移至 `useListLyricsToolbar.ts`，拖动、缩放、窗口尺寸重置及歌词选择入口移至 `useListLyricsWindow.ts`，工具栏和歌词/空状态展示分别移至 `LyricsListToolbar.tsx`、`LyricsListContent.tsx`。保留原窗口入口、歌词行/辅助歌词对齐、跟随交互、偏移步长、Tauri 调用与样式变量；目标 TypeScript 语法与 `tsc --noEmit`、旧职责符号/调用路径检查、新文件空白检查和工作区 `git diff --check` 均通过；未执行构建和测试。 |
| D24 | 2026-09-04 | `UpdateProvider.tsx` 收缩为 Context façade；Tauri 版本读取、更新检查、下载/安装、资源关闭、重启和开发预览数据移至 `updateService.ts`，状态、并发保护、自动检查、下载进度、重试与生命周期清理移至 `useUpdateController.ts`，更新详情/版本、发布说明、进度与操作按钮展示移至 `UpdateDialog.tsx`。保留 `UpdateProvider`、`useUpdates`、`UpdateStatus` 的原 import 路径与返回字段，更新状态转换、错误文案、预览模式和 updater 调用参数未变；目标 TypeScript 语法与 `tsc --noEmit`、facade 依赖/公开调用路径检查、新文件空白检查和工作区 `git diff --check` 均通过；未执行构建和测试。 |
