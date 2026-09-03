use encoding_rs::GB18030;

/// 严格解码歌词正文，避免使用 UTF-8 有损解码把原始字节替换成 `�`。
///
/// 歌词服务和本地歌词通常使用 UTF-8，但部分历史歌词仍是 GBK/GB2312。
/// GB18030 解码器向下兼容这两种编码，并且在遇到无法解码的字节时返回错误。
pub(crate) fn decode_lyrics_bytes(bytes: &[u8]) -> Result<String, String> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    if let Ok(text) = std::str::from_utf8(bytes) {
        return Ok(text.to_string());
    }

    GB18030
        .decode_without_bom_handling_and_without_replacement(bytes)
        .map(|text| text.into_owned())
        .ok_or_else(|| "歌词文本不是有效的 UTF-8 或 GB18030 编码".into())
}
