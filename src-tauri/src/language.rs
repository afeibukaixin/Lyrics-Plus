use serde::Deserialize;

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
    pub switch_lyrics: &'static str,
    pub settings: &'static str,
    pub quit: &'static str,
    pub quick_title: &'static str,
    pub unlock_title: &'static str,
    pub overlay_title: &'static str,
}

#[derive(Clone, Copy)]
pub enum ConfigComment {
    SchemaVersion,
    UiFontScale,
    Language,
    PlayerSelection,
    SystemMediaFilterMode,
    SystemMediaApplications,
    PlayerFollowerApplication,
    HideDockIcon,
    SilentStartup,
    AutoCheckUpdates,
    Shortcuts,
    AutoApplyThreshold,
    TitleFilterKeywords,
    ProviderMode,
    Providers,
    OverlayState,
    HideWhenNotPlaying,
    FontSize,
    Opacity,
    BackgroundOpacity,
    BackgroundBlur,
    BackgroundMode,
    Background,
    Layout,
    Alignment,
    LongText,
    SecondaryDisplay,
    AutoCenter,
    KaraokeStyle,
    SecondaryFontScale,
}

impl UiLanguage {
    pub fn native_labels(self) -> NativeLabels {
        match self {
            Self::ZhCn => NativeLabels {
                toggle_overlay: "显示桌面歌词",
                switch_lyrics: "切换歌词",
                settings: "设置",
                quit: "退出",
                quick_title: "快速切换歌词",
                unlock_title: "解锁桌面歌词",
                overlay_title: "Lyrics Plus 桌面歌词",
            },
            Self::EnUs => NativeLabels {
                toggle_overlay: "Show Desktop Lyrics",
                switch_lyrics: "Switch Lyrics",
                settings: "Settings",
                quit: "Quit",
                quick_title: "Quick Lyrics Switcher",
                unlock_title: "Unlock Desktop Lyrics",
                overlay_title: "Lyrics Plus Desktop Lyrics",
            },
        }
    }

    pub fn config_comment(self, comment: ConfigComment) -> &'static str {
        match (self, comment) {
            (Self::ZhCn, ConfigComment::SchemaVersion) => "配置结构版本，通常由 Lyrics Plus 管理。",
            (Self::ZhCn, ConfigComment::UiFontScale) => "界面文字缩放：80–150，步进为 10%。",
            (Self::ZhCn, ConfigComment::Language) => "界面语言：system 或 BCP 47 语言标签，例如 zh-CN、zh-TW、en-US。",
            (Self::ZhCn, ConfigComment::PlayerSelection) => "播放器选择：auto、apple_music、spotify 或 system。",
            (Self::ZhCn, ConfigComment::SystemMediaFilterMode) => "系统媒体第三方应用筛选：allowlist 仅允许列表，blocklist 排除列表。",
            (Self::ZhCn, ConfigComment::SystemMediaApplications) => "系统媒体第三方应用列表；含义由 systemMediaFilterMode 决定。",
            (Self::ZhCn, ConfigComment::PlayerFollowerApplication) => "随其启动和退出的播放器；null 表示关闭跟随。",
            (Self::ZhCn, ConfigComment::HideDockIcon) => "隐藏 macOS 程序坞图标；仍可从菜单栏使用 Lyrics Plus。",
            (Self::ZhCn, ConfigComment::SilentStartup) => "启动时不显示设置窗口；仍可从菜单栏打开 Lyrics Plus。",
            (Self::ZhCn, ConfigComment::AutoCheckUpdates) => "应用启动时自动检查更新；发现新版本后仍需确认安装。",
            (Self::ZhCn, ConfigComment::Shortcuts) => "全局快捷键必须包含修饰键，并且不能重复。",
            (Self::ZhCn, ConfigComment::AutoApplyThreshold) => "自动采用同步歌词的最低相似度：0–100。",
            (Self::ZhCn, ConfigComment::TitleFilterKeywords) => "仅在本地匹配评分前按顺序智能移除标题中的屏蔽内容。",
            (Self::ZhCn, ConfigComment::ProviderMode) => "strict 按歌词源顺序搜索；smart 可优先选择质量更高的匹配。",
            (Self::ZhCn, ConfigComment::Providers) => "歌词源顺序决定严格模式的搜索顺序；至少启用一个歌词源。",
            (Self::ZhCn, ConfigComment::OverlayState) => "桌面歌词的显示与锁定状态。",
            (Self::ZhCn, ConfigComment::HideWhenNotPlaying) => "播放暂停、停止或不可用时隐藏桌面歌词。",
            (Self::ZhCn, ConfigComment::FontSize) => "主歌词字号（16–72px）及颜色。",
            (Self::ZhCn, ConfigComment::Opacity) => "窗口透明度：0.2–1.0。",
            (Self::ZhCn, ConfigComment::BackgroundOpacity) => "背景透明度：0–1.0；不影响歌词文字。",
            (Self::ZhCn, ConfigComment::BackgroundBlur) => "背景模糊：0–40（设置中显示为 0–100%）。",
            (Self::ZhCn, ConfigComment::BackgroundMode) => "背景模式：solid 或 transparent。",
            (Self::ZhCn, ConfigComment::Background) => "背景效果：glass 或 solid；transparent 仅为兼容旧配置而保留。",
            (Self::ZhCn, ConfigComment::Layout) => "歌词布局：single 或 double；方向：horizontal 或 vertical。",
            (Self::ZhCn, ConfigComment::Alignment) => "对齐方式：center 或 distributed。",
            (Self::ZhCn, ConfigComment::LongText) => "长文本行为：shrink、wrap 或 marquee。",
            (Self::ZhCn, ConfigComment::SecondaryDisplay) => "副内容：next、translation、romanization 或同时显示两者。",
            (Self::ZhCn, ConfigComment::AutoCenter) => "仅在实际显示翻译或音译时居中。",
            (Self::ZhCn, ConfigComment::KaraokeStyle) => "卡拉 OK 效果：sweep、bounce 或 highlight。",
            (Self::ZhCn, ConfigComment::SecondaryFontScale) => "下一句、翻译和音译文字的缩放比例：0.35–1.0。",
            (Self::EnUs, ConfigComment::SchemaVersion) => "Configuration schema version. Usually managed by Lyrics Plus.",
            (Self::EnUs, ConfigComment::UiFontScale) => "Interface text scale: 80–150 in 10% increments.",
            (Self::EnUs, ConfigComment::Language) => "Interface language: system or a BCP 47 language tag, such as zh-CN, zh-TW, or en-US.",
            (Self::EnUs, ConfigComment::PlayerSelection) => "Player selection: auto, apple_music, spotify, or system.",
            (Self::EnUs, ConfigComment::SystemMediaFilterMode) => "System Media filtering for third-party apps: allowlist permits listed apps; blocklist excludes them.",
            (Self::EnUs, ConfigComment::SystemMediaApplications) => "Third-party System Media app list interpreted by systemMediaFilterMode.",
            (Self::EnUs, ConfigComment::PlayerFollowerApplication) => "Player whose launch and quit lifecycle Lyrics Plus follows; null disables it.",
            (Self::EnUs, ConfigComment::HideDockIcon) => "Hide the macOS Dock icon; Lyrics Plus remains available from the menu bar.",
            (Self::EnUs, ConfigComment::SilentStartup) => "Start without showing Settings; Lyrics Plus remains available from the menu bar.",
            (Self::EnUs, ConfigComment::AutoCheckUpdates) => "Check for updates at startup; installing a new version still requires confirmation.",
            (Self::EnUs, ConfigComment::Shortcuts) => "Global shortcuts must include a modifier and must be unique.",
            (Self::EnUs, ConfigComment::AutoApplyThreshold) => "Minimum similarity for automatically applying synchronized lyrics: 0–100.",
            (Self::EnUs, ConfigComment::TitleFilterKeywords) => "Keywords intelligently removed in order before local title scoring only.",
            (Self::EnUs, ConfigComment::ProviderMode) => "strict follows provider order; smart can prioritize higher-quality matches.",
            (Self::EnUs, ConfigComment::Providers) => "Provider order controls strict search order; at least one must be enabled.",
            (Self::EnUs, ConfigComment::OverlayState) => "Desktop lyrics visibility and lock state.",
            (Self::EnUs, ConfigComment::HideWhenNotPlaying) => "Hide while playback is paused, stopped, or unavailable.",
            (Self::EnUs, ConfigComment::FontSize) => "Primary lyric font size (16–72px) and colors.",
            (Self::EnUs, ConfigComment::Opacity) => "Window opacity: 0.2–1.0.",
            (Self::EnUs, ConfigComment::BackgroundOpacity) => "Background opacity: 0–1.0; does not affect lyric text.",
            (Self::EnUs, ConfigComment::BackgroundBlur) => "Background blur: 0–40 (shown as 0–100% in Settings).",
            (Self::EnUs, ConfigComment::BackgroundMode) => "Background mode: solid or transparent.",
            (Self::EnUs, ConfigComment::Background) => "Background effect: glass or solid; transparent is retained for legacy compatibility.",
            (Self::EnUs, ConfigComment::Layout) => "Lyrics layout: single or double; orientation: horizontal or vertical.",
            (Self::EnUs, ConfigComment::Alignment) => "Alignment: center or distributed.",
            (Self::EnUs, ConfigComment::LongText) => "Long-text behavior: shrink, wrap, or marquee.",
            (Self::EnUs, ConfigComment::SecondaryDisplay) => "Secondary content: next, translation, romanization, or both.",
            (Self::EnUs, ConfigComment::AutoCenter) => "Center only while translation or romanization is actually displayed.",
            (Self::EnUs, ConfigComment::KaraokeStyle) => "Karaoke effect: sweep, bounce, or highlight.",
            (Self::EnUs, ConfigComment::SecondaryFontScale) => "Font scale for next-line, translation, and romanization text: 0.35–1.0.",
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
