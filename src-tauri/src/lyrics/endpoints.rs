// 歌词来源固定地址集中维护；请求参数、凭据与协议处理留在各来源模块。

pub(crate) mod qqmusic {
    pub const SEARCH: &str = "https://c.y.qq.com/soso/fcgi-bin/search_for_qq_cp";
    pub const RPC: &str = "https://u.y.qq.com/cgi-bin/musicu.fcg";
    pub const REFERER: &str = "https://y.qq.com/";
}

pub(crate) mod netease {
    pub const SEARCH: &str = "https://music.163.com/api/cloudsearch/pc";
    pub const LYRIC: &str = "https://music.163.com/api/song/lyric/v1";
    pub const REFERER: &str = "https://music.163.com/";
}

pub(crate) mod kugou {
    pub const SEARCH: &str = "https://songsearch.kugou.com/song_search_v2";
    pub const LYRIC_SEARCH: &str = "https://lyrics.kugou.com/search";
    pub const DOWNLOAD: &str = "https://lyrics.kugou.com/download";
    pub const REFERER: &str = "https://www.kugou.com/";
}

pub(crate) mod lrclib {
    pub const GET: &str = "https://lrclib.net/api/get";
    pub const SEARCH: &str = "https://lrclib.net/api/search";
}

pub(crate) mod qishui {
    pub const SEARCH: &str = "https://api.qishui.com/luna/search/track";
    pub const DETAIL: &str = "https://beta-luna.douyin.com/luna/h5/seo_track";
}

pub(crate) mod musixmatch {
    pub const DESKTOP_API_BASE: &str = "https://apic-desktop.musixmatch.com/ws/1.1";
    pub const DEVELOPER_API_BASE: &str = "https://api.musixmatch.com/ws/1.1";
    pub const ORIGIN: &str = "https://www.musixmatch.com";
    pub const REFERER: &str = "https://www.musixmatch.com/";
    pub const SEARCH_PATH: &str = "track.search";
    pub const SUBTITLES_PATH: &str = "track.subtitles.get";
    pub const MACRO_SUBTITLES_PATH: &str = "macro.subtitles.get";
    pub const RICHSYNC_PATH: &str = "track.richsync.get";
    pub const TOKEN_PATH: &str = "token.get";
}

#[allow(dead_code)]
pub(crate) mod kuwo {
    pub const SEARCH: &str = "https://search.kuwo.cn/r.s";
    pub const LYRIC: &str = "https://kuwo.cn/openapi/v1/www/lyric/getlyric";
    pub const SEARCH_REFERER: &str = "https://www.kuwo.cn/";
    pub const LYRIC_REFERER: &str = "https://kuwo.cn/";
}

#[allow(dead_code)]
pub(crate) mod migu {
    pub const SEARCH: &str = "https://c.musicapp.migu.cn/v1.0/content/search_all.do";
    pub const DETAIL: &str = "https://app.c.nf.migu.cn/MIGUM3.0/strategy/pc/listen/v1.0";
}

pub(crate) mod amll_ttml {
    pub const DEFAULT_BASE: &str = "https://api.amll.dev";
    pub const SEARCH_PATH: &str = "/v1/lyrics/search";
    pub const GET_PATH: &str = "/v1/lyrics/get";
    // 仅识别并迁移历史设置，不用于网络回退。
    pub const LEGACY_BASES: [&str; 3] = [
        "https://amlldb.bikonoo.com",
        "https://cdn.jsdelivr.net/gh/Steve-xmh/amll-ttml-db@main",
        "https://github.com/amll-dev/amll-ttml-db/raw/refs/heads/main",
    ];
}
