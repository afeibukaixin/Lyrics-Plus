use serde::Deserialize;

#[cfg(target_os = "macos")]
use objc2_foundation::NSLocale;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub enum UiLanguage {
    #[serde(rename = "zh-CN")]
    ZhCn,
    #[serde(rename = "en-US")]
    EnUs,
}

#[derive(Clone, Copy)]
pub struct NativeLabels {
    pub toggle_overlay: &'static str,
    pub toggle_status_bar_lyrics: &'static str,
    pub toggle_list_lyrics: &'static str,
    pub toggle_notch_lyrics: &'static str,
    pub switch_lyrics: &'static str,
    pub settings: &'static str,
    pub quit: &'static str,
    pub quick_title: &'static str,
    pub unlock_title: &'static str,
    pub overlay_title: &'static str,
    pub list_title: &'static str,
    pub notch_title: &'static str,
}

#[derive(Clone, Copy)]
pub enum ConfigComment {
    SchemaVersion,
    Theme,
    Language,
    PlayerSelection,
    SystemMediaFilterMode,
    SystemMediaApplications,
    PlayerFollowerApplication,
    HideDockIcon,
    HideMenuBarIcon,
    SilentStartup,
    AutoCheckUpdates,
    Shortcuts,
    AutoApplyThreshold,
    AutoSearchDebounce,
    TitleFilterKeywords,
    AmllBaseUrl,
    ProviderMode,
    Providers,
    ChineseConversion,
    LyricsDisplays,
    OverlayState,
    HideWhenNotPlaying,
    LyricsWindowsSpaceBehavior,
    FontFamily,
    FontSize,
    LineHeight,
    Opacity,
    BackgroundOpacity,
    BackgroundBlur,
    BackgroundGeometry,
    BackgroundMode,
    Background,
    Layout,
    DoubleLineMode,
    Alignment,
    StatusBarAlignment,
    StatusBarVerticalOffset,
    PrimaryLinePosition,
    LineGap,
    SecondaryLineGap,
    LongText,
    SecondaryDisplay,
    AutoCenter,
    KaraokeStyle,
    SecondaryFontScale,
    ActiveLyricsOpacity,
    InactiveLyricsOpacity,
    TextShadow,
    TextStroke,
}

impl UiLanguage {
    pub(crate) fn from_language_tag(language: &str) -> Option<Self> {
        let normalized = language.trim().replace('_', "-").to_ascii_lowercase();
        if normalized == "zh"
            || normalized == "zh-cn"
            || normalized.starts_with("zh-cn-")
            || normalized == "zh-sg"
            || normalized.starts_with("zh-sg-")
            || normalized.starts_with("zh-hans")
        {
            return Some(Self::ZhCn);
        }

        if normalized == "zh-hk"
            || normalized.starts_with("zh-hk-")
            || normalized == "zh-hant-hk"
            || normalized.starts_with("zh-hant-hk-")
            || normalized == "zh-mo"
            || normalized.starts_with("zh-mo-")
            || normalized == "zh-hant-mo"
            || normalized.starts_with("zh-hant-mo-")
            || normalized == "zh-tw"
            || normalized.starts_with("zh-tw-")
            || normalized.starts_with("zh-hant")
            || normalized == "en"
            || normalized.starts_with("en-")
        {
            return Some(Self::EnUs);
        }

        None
    }

    pub(crate) fn system_default() -> Self {
        #[cfg(target_os = "macos")]
        {
            // 系统首选语言可包含当前不支持的语言；继续查找后续可用项。
            let preferred_languages = NSLocale::preferredLanguages();
            for index in 0..preferred_languages.count() {
                let language = preferred_languages.objectAtIndex(index).to_string();
                if let Some(language) = Self::from_language_tag(&language) {
                    return language;
                }
            }
        }

        Self::EnUs
    }

    pub fn native_labels(self) -> NativeLabels {
        match self {
            Self::ZhCn => NativeLabels {
                toggle_overlay: "显示桌面歌词",
                toggle_status_bar_lyrics: "显示菜单栏歌词",
                toggle_list_lyrics: "显示歌词窗口",
                toggle_notch_lyrics: "显示灵动岛歌词",
                switch_lyrics: "切换歌词",
                settings: "设置",
                quit: "退出",
                quick_title: "快速切换歌词",
                unlock_title: "解锁桌面歌词",
                overlay_title: "Lyrics Plus 桌面歌词",
                list_title: "Lyrics Plus 歌词窗口",
                notch_title: "Lyrics Plus 灵动岛歌词",
            },
            Self::EnUs => NativeLabels {
                toggle_overlay: "Show Desktop Lyrics",
                toggle_status_bar_lyrics: "Show Menu Bar Lyrics",
                toggle_list_lyrics: "Show Lyrics Window",
                toggle_notch_lyrics: "Show Dynamic Island Lyrics",
                switch_lyrics: "Switch Lyrics",
                settings: "Settings",
                quit: "Quit",
                quick_title: "Quick Lyrics Switcher",
                unlock_title: "Unlock Desktop Lyrics",
                overlay_title: "Lyrics Plus Desktop Lyrics",
                list_title: "Lyrics Plus Lyrics Window",
                notch_title: "Lyrics Plus Dynamic Island Lyrics",
            },
        }
    }

    pub fn config_comment(self, comment: ConfigComment) -> &'static str {
        match (self, comment) {
            (Self::ZhCn, ConfigComment::SchemaVersion) => "配置结构版本，通常由 Lyrics Plus 管理。",
            (Self::ZhCn, ConfigComment::Theme) => "应用主题：dark、light 或 system。",
            (Self::ZhCn, ConfigComment::Language) => "界面语言：system 或 BCP 47 语言标签，例如 zh-CN、zh-TW、en-US。",
            (Self::ZhCn, ConfigComment::PlayerSelection) => "播放器选择：auto、apple_music、spotify 或 system。",
            (Self::ZhCn, ConfigComment::SystemMediaFilterMode) => "系统媒体第三方应用筛选：allowlist 仅允许列表，blocklist 排除列表。",
            (Self::ZhCn, ConfigComment::SystemMediaApplications) => "系统媒体第三方应用列表；含义由 systemMediaFilterMode 决定。",
            (Self::ZhCn, ConfigComment::PlayerFollowerApplication) => "随其启动和退出的播放器；null 表示关闭跟随。",
            (Self::ZhCn, ConfigComment::HideDockIcon) => "隐藏 macOS 程序坞图标；仍可从菜单栏图标或重新打开应用使用 Lyrics Plus。",
            (Self::ZhCn, ConfigComment::HideMenuBarIcon) => "隐藏固定的 macOS 菜单栏应用图标；菜单栏歌词状态项不受影响。",
            (Self::ZhCn, ConfigComment::SilentStartup) => "启动时不显示设置窗口；可从菜单栏图标或重新打开应用打开 Lyrics Plus。",
            (Self::ZhCn, ConfigComment::AutoCheckUpdates) => "应用启动时自动检查更新；发现新版本后仍需确认安装。",
            (Self::ZhCn, ConfigComment::Shortcuts) => "已设置的全局快捷键必须包含修饰键，并且不能重复。",
            (Self::ZhCn, ConfigComment::AutoApplyThreshold) => "自动采用同步歌词的最低相似度：0–100。",
            (Self::ZhCn, ConfigComment::AutoSearchDebounce) => "自动匹配防抖时间：0–5000 毫秒，必须是 100 毫秒的整数倍；数值越低响应越快，但快速切歌会增加无效请求和歌词源限流风险，0 表示关闭。",
            (Self::ZhCn, ConfigComment::TitleFilterKeywords) => "仅在本地匹配评分前按顺序智能移除标题中的屏蔽内容。",
            (Self::ZhCn, ConfigComment::AmllBaseUrl) => "AMLL Native API 根地址，仅支持 http 或 https。",
            (Self::ZhCn, ConfigComment::ProviderMode) => "strict 按歌词源顺序搜索；smart 可优先选择质量更高的匹配。",
            (Self::ZhCn, ConfigComment::Providers) => "歌词源顺序决定严格模式的搜索顺序；至少启用一个歌词源。",
            (Self::ZhCn, ConfigComment::ChineseConversion) => "中文歌词输出：original 保持原文，simplified 转为简体，traditional 转为通用繁体。",
            (Self::ZhCn, ConfigComment::LyricsDisplays) => "状态栏、列表窗口和灵动岛歌词可独立开启。",
            (Self::ZhCn, ConfigComment::OverlayState) => "桌面歌词的显示与锁定状态。",
            (Self::ZhCn, ConfigComment::HideWhenNotPlaying) => "播放暂停、停止或不可用时隐藏桌面歌词。",
            (Self::ZhCn, ConfigComment::LyricsWindowsSpaceBehavior) => "统一控制桌面歌词、歌词窗口和灵动岛是否显示在所有 macOS Space；开启后会出现在调度中心。",
            (Self::ZhCn, ConfigComment::FontFamily) => "歌词字体族，语法与 CSS font-family 相同，按顺序回退。",
            (Self::ZhCn, ConfigComment::FontSize) => "主歌词字号（16–72px）及颜色。",
            (Self::ZhCn, ConfigComment::LineHeight) => "歌词行高倍数：0.8–2.0。",
            (Self::ZhCn, ConfigComment::Opacity) => "窗口透明度：0.2–1.0。",
            (Self::ZhCn, ConfigComment::BackgroundOpacity) => "背景透明度：0–1.0；不影响歌词文字。",
            (Self::ZhCn, ConfigComment::BackgroundBlur) => "背景模糊：0–40（设置中显示为 0–100%）。",
            (Self::ZhCn, ConfigComment::BackgroundGeometry) => "背景圆角及水平、垂直内边距：0–64px。",
            (Self::ZhCn, ConfigComment::BackgroundMode) => "背景模式：solid 或 transparent。",
            (Self::ZhCn, ConfigComment::Background) => "背景效果：glass 或 solid；transparent 仅为兼容旧配置而保留。",
            (Self::ZhCn, ConfigComment::Layout) => "歌词布局：single 或 double；方向：horizontal 或 vertical。",
            (Self::ZhCn, ConfigComment::DoubleLineMode) => "双行切换方式：rolling 顺序滚动，alternating 交替高亮。",
            (Self::ZhCn, ConfigComment::Alignment) => "对齐方式：start、center、end 或 distributed；start/end 会按横竖方向映射到两端。",
            (Self::ZhCn, ConfigComment::StatusBarAlignment) => "菜单栏歌词对齐方式：left、center 或 right。",
            (Self::ZhCn, ConfigComment::StatusBarVerticalOffset) => "菜单栏歌词垂直偏移：-6–6pt，正值向上，负值向下。",
            (Self::ZhCn, ConfigComment::PrimaryLinePosition) => "主副歌词顺序：first 主歌词横排在上/竖排在右，second 主歌词横排在下/竖排在左。",
            (Self::ZhCn, ConfigComment::LineGap) => "双排主副歌词间距：0–32px。",
            (Self::ZhCn, ConfigComment::SecondaryLineGap) => "列表窗口同一歌词组内主歌词、翻译和音译的间距：0–32px。",
            (Self::ZhCn, ConfigComment::LongText) => "长文本行为：shrink、wrap 或 marquee。",
            (Self::ZhCn, ConfigComment::SecondaryDisplay) => "副内容：next、translation、romanization 或同时显示两者。",
            (Self::ZhCn, ConfigComment::AutoCenter) => "仅在实际显示翻译或音译时居中。",
            (Self::ZhCn, ConfigComment::KaraokeStyle) => "卡拉 OK 效果：sweep、bounce 或 highlight。",
            (Self::ZhCn, ConfigComment::SecondaryFontScale) => "下一句、翻译和音译文字的缩放比例：0.35–1.0。",
            (Self::ZhCn, ConfigComment::ActiveLyricsOpacity) => "列表窗口当前歌词文字透明度：0–1.0。",
            (Self::ZhCn, ConfigComment::InactiveLyricsOpacity) => "列表窗口其他歌词文字透明度：0–1.0。",
            (Self::ZhCn, ConfigComment::TextShadow) => "歌词阴影的水平偏移、垂直偏移、模糊半径和颜色。",
            (Self::ZhCn, ConfigComment::TextStroke) => "歌词描边宽度和颜色；宽度为 0 时关闭描边。",
            (Self::EnUs, ConfigComment::SchemaVersion) => "Configuration schema version. Usually managed by Lyrics Plus.",
            (Self::EnUs, ConfigComment::Theme) => "Application theme: dark, light, or system.",
            (Self::EnUs, ConfigComment::Language) => "Interface language: system or a BCP 47 language tag, such as zh-CN, zh-TW, or en-US.",
            (Self::EnUs, ConfigComment::PlayerSelection) => "Player selection: auto, apple_music, spotify, or system.",
            (Self::EnUs, ConfigComment::SystemMediaFilterMode) => "System Media filtering for third-party apps: allowlist permits listed apps; blocklist excludes them.",
            (Self::EnUs, ConfigComment::SystemMediaApplications) => "Third-party System Media app list interpreted by systemMediaFilterMode.",
            (Self::EnUs, ConfigComment::PlayerFollowerApplication) => "Player whose launch and quit lifecycle Lyrics Plus follows; null disables it.",
            (Self::EnUs, ConfigComment::HideDockIcon) => "Hide the macOS Dock icon; Lyrics Plus remains available from its menu bar icon or by reopening the app.",
            (Self::EnUs, ConfigComment::HideMenuBarIcon) => "Hide the fixed macOS menu bar app icon; the menu bar lyrics status item is unaffected.",
            (Self::EnUs, ConfigComment::SilentStartup) => "Start without showing Settings; open the menu bar icon or reopen the app to access Lyrics Plus.",
            (Self::EnUs, ConfigComment::AutoCheckUpdates) => "Check for updates at startup; installing a new version still requires confirmation.",
            (Self::EnUs, ConfigComment::Shortcuts) => "Configured global shortcuts must include a modifier and must be unique.",
            (Self::EnUs, ConfigComment::AutoApplyThreshold) => "Minimum similarity for automatically applying synchronized lyrics: 0–100.",
            (Self::EnUs, ConfigComment::AutoSearchDebounce) => "Automatic search debounce: 0–5000 ms in 100 ms steps. Lower values respond faster but quick track changes create more wasted requests and rate-limit risk; 0 disables it.",
            (Self::EnUs, ConfigComment::TitleFilterKeywords) => "Keywords intelligently removed in order before local title scoring only.",
            (Self::EnUs, ConfigComment::AmllBaseUrl) => "AMLL Native API root URL using HTTP or HTTPS.",
            (Self::EnUs, ConfigComment::ProviderMode) => "strict follows provider order; smart can prioritize higher-quality matches.",
            (Self::EnUs, ConfigComment::Providers) => "Provider order controls strict search order; at least one must be enabled.",
            (Self::EnUs, ConfigComment::ChineseConversion) => "Chinese lyric output: original preserves source text, simplified converts to Simplified Chinese, and traditional converts to OpenCC standard Traditional Chinese.",
            (Self::EnUs, ConfigComment::LyricsDisplays) => "Menu bar, list-window, and Dynamic Island lyrics can be enabled independently.",
            (Self::EnUs, ConfigComment::OverlayState) => "Desktop lyrics visibility and lock state.",
            (Self::EnUs, ConfigComment::HideWhenNotPlaying) => "Hide while playback is paused, stopped, or unavailable.",
            (Self::EnUs, ConfigComment::LyricsWindowsSpaceBehavior) => "Control whether Desktop Lyrics, the Lyrics Window, and Dynamic Island appear in every macOS Space; enabled windows also appear in Mission Control.",
            (Self::EnUs, ConfigComment::FontFamily) => "Lyric font family using CSS font-family syntax and ordered fallbacks.",
            (Self::EnUs, ConfigComment::FontSize) => "Primary lyric font size (16–72px) and colors.",
            (Self::EnUs, ConfigComment::LineHeight) => "Lyric line-height multiplier: 0.8–2.0.",
            (Self::EnUs, ConfigComment::Opacity) => "Window opacity: 0.2–1.0.",
            (Self::EnUs, ConfigComment::BackgroundOpacity) => "Background opacity: 0–1.0; does not affect lyric text.",
            (Self::EnUs, ConfigComment::BackgroundBlur) => "Background blur: 0–40 (shown as 0–100% in Settings).",
            (Self::EnUs, ConfigComment::BackgroundGeometry) => "Background radius and horizontal/vertical padding: 0–64px.",
            (Self::EnUs, ConfigComment::BackgroundMode) => "Background mode: solid or transparent.",
            (Self::EnUs, ConfigComment::Background) => "Background effect: glass or solid; transparent is retained for legacy compatibility.",
            (Self::EnUs, ConfigComment::Layout) => "Lyrics layout: single or double; orientation: horizontal or vertical.",
            (Self::EnUs, ConfigComment::DoubleLineMode) => "Double-line mode: rolling keeps the current/next order; alternating switches the highlighted slot.",
            (Self::EnUs, ConfigComment::Alignment) => "Alignment: start, center, end, or distributed; start/end follow the horizontal or vertical axis.",
            (Self::EnUs, ConfigComment::StatusBarAlignment) => "Menu bar lyric alignment: left, center, or right.",
            (Self::EnUs, ConfigComment::StatusBarVerticalOffset) => "Menu bar lyric vertical offset: -6 to 6pt; positive values move up and negative values move down.",
            (Self::EnUs, ConfigComment::PrimaryLinePosition) => "Primary/secondary lyric order: first puts the primary lyric top/right; second puts it bottom/left in horizontal/vertical layouts.",
            (Self::EnUs, ConfigComment::LineGap) => "Double-line primary/secondary lyric gap: 0–32px.",
            (Self::EnUs, ConfigComment::SecondaryLineGap) => "List-window gap between primary, translation, and romanization lines in one lyric group: 0–32px.",
            (Self::EnUs, ConfigComment::LongText) => "Long-text behavior: shrink, wrap, or marquee.",
            (Self::EnUs, ConfigComment::SecondaryDisplay) => "Secondary content: next, translation, romanization, or both.",
            (Self::EnUs, ConfigComment::AutoCenter) => "Center only while translation or romanization is actually displayed.",
            (Self::EnUs, ConfigComment::KaraokeStyle) => "Karaoke effect: sweep, bounce, or highlight.",
            (Self::EnUs, ConfigComment::SecondaryFontScale) => "Font scale for next-line, translation, and romanization text: 0.35–1.0.",
            (Self::EnUs, ConfigComment::ActiveLyricsOpacity) => "List-window current lyric text opacity: 0–1.0.",
            (Self::EnUs, ConfigComment::InactiveLyricsOpacity) => "List-window other lyric text opacity: 0–1.0.",
            (Self::EnUs, ConfigComment::TextShadow) => "Lyric shadow horizontal/vertical offsets, blur radius, and color.",
            (Self::EnUs, ConfigComment::TextStroke) => "Lyric stroke width and color; set width to 0 to disable the stroke.",
        }
    }
}

pub fn detect_config_comment_language(raw: &str) -> Option<UiLanguage> {
    if raw.contains("// Configuration schema version") {
        Some(UiLanguage::EnUs)
    } else if raw.contains("// 配置结构版本") {
        Some(UiLanguage::ZhCn)
    } else {
        None
    }
}
