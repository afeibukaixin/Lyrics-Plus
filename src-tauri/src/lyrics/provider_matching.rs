pub(crate) fn validate_settings(settings: &ProviderSettings) -> Result<(), String> {
    if settings.auto_apply_threshold > 100 {
        return Err("自动匹配相似度必须在 0–100 之间".into());
    }
    if settings.auto_search_debounce_ms > 5_000 {
        return Err("自动匹配防抖时间必须在 0–5000 毫秒之间".into());
    }
    if settings.auto_search_debounce_ms % 100 != 0 {
        return Err("自动匹配防抖时间必须是 100 毫秒的整数倍".into());
    }
    if settings.capability_preference_tolerance > MAX_CAPABILITY_PREFERENCE_TOLERANCE {
        return Err("歌词能力优选范围必须在 0–20 之间".into());
    }
    if [
        settings.match_weights.title,
        settings.match_weights.artist,
        settings.match_weights.album,
        settings.match_weights.duration,
        settings.match_weights.version,
    ]
    .into_iter()
    .any(|weight| weight > 100)
    {
        return Err("歌词匹配重要度必须在 0–100 之间".into());
    }
    if settings.match_weights.total() == 0 {
        return Err("歌词匹配重要度不能全部为 0".into());
    }
    let known = provider_definitions()
        .into_iter()
        .map(|(id, _)| id)
        .collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    for preference in &settings.providers {
        if !known.contains(preference.id.as_str()) {
            return Err(format!("未知歌词源：{}", preference.id));
        }
        if !seen.insert(&preference.id) {
            return Err(format!("歌词源重复：{}", preference.id));
        }
    }
    if !settings.providers.iter().any(|provider| provider.enabled) {
        return Err("请至少启用一个歌词源".into());
    }
    let amll_url = reqwest::Url::parse(settings.amll_base_url.trim())
        .map_err(|_| "AMLL API 根地址必须是有效的绝对 URL".to_string())?;
    if !matches!(amll_url.scheme(), "http" | "https") {
        return Err("AMLL API 根地址只支持 http 或 https".into());
    }
    prepare_title_filter_keywords_with_normalization(
        &settings.title_filter_keywords,
        settings.normalize_chinese,
    )?;
    Ok(())
}
pub(crate) fn normalize_settings(settings: &mut ProviderSettings) -> Result<(), String> {
    for keyword in &mut settings.title_filter_keywords {
        *keyword = keyword.trim().to_string();
    }
    settings.amll_base_url = settings
        .amll_base_url
        .trim()
        .trim_end_matches('/')
        .to_string();
    if LEGACY_AMLL_BASE_URLS.contains(&settings.amll_base_url.as_str()) {
        settings.amll_base_url = DEFAULT_AMLL_BASE_URL.into();
    }
    complete_settings(settings);
    validate_settings(settings)?;
    Ok(())
}

#[cfg(test)]
fn prepare_title_filter_keywords(keywords: &[String]) -> Result<Vec<String>, String> {
    prepare_title_filter_keywords_with_normalization(keywords, true)
}

fn prepare_title_filter_keywords_with_normalization(
    keywords: &[String],
    normalize_chinese: bool,
) -> Result<Vec<String>, String> {
    if keywords.len() > MAX_TITLE_FILTER_KEYWORDS {
        return Err(format!("标题屏蔽内容最多 {MAX_TITLE_FILTER_KEYWORDS} 条"));
    }
    let mut seen = HashSet::new();
    keywords
        .iter()
        .enumerate()
        .map(|(index, keyword)| {
            let keyword = keyword.trim();
            if keyword.is_empty() {
                return Err(format!("第 {} 条标题屏蔽内容不能为空", index + 1));
            }
            if keyword.chars().count() > MAX_TITLE_FILTER_KEYWORD_LENGTH {
                return Err(format!(
                    "第 {} 条标题屏蔽内容不能超过 {MAX_TITLE_FILTER_KEYWORD_LENGTH} 个字符",
                    index + 1
                ));
            }
            let keyword = normalize_case(keyword, normalize_chinese);
            if !seen.insert(keyword.clone()) {
                return Err(format!("第 {} 条标题屏蔽内容重复", index + 1));
            }
            Ok(keyword)
        })
        .collect()
}

fn complete_settings(settings: &mut ProviderSettings) {
    for (id, _) in provider_definitions() {
        if !settings.providers.iter().any(|provider| provider.id == id) {
            settings.providers.push(ProviderPreference {
                id: id.into(),
                enabled: default_provider_enabled(id),
            });
        }
    }
}

#[cfg(test)]
fn deduplicate(results: &mut Vec<LyricsSearchResult>) {
    let mut seen = HashSet::new();
    results.retain(|result| {
        let lyric_key = result
            .lyrics
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        seen.insert(lyric_key)
    });
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn simplify(value: &str) -> String {
    convert_text(value, Config::T2s).to_lowercase()
}

fn normalize_case(value: &str, normalize_chinese: bool) -> String {
    if normalize_chinese {
        simplify(value)
    } else {
        value.to_lowercase()
    }
}

#[cfg(test)]
fn normalise(value: &str) -> String {
    normalise_with_options(value, true)
}

fn normalise_with_options(value: &str, normalize_chinese: bool) -> String {
    normalize_case(value, normalize_chinese)
        .chars()
        .filter(|character| character.is_alphanumeric())
        .collect()
}

fn metadata_is_japanese(title: &str, artist: &str, album: Option<&str>) -> bool {
    let text = [Some(title), Some(artist), album]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("\n");
    is_japanese(&text)
}

fn canonical_metadata_aliases(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            // These characters are context-dependent in Chinese and are not
            // always covered by a generic OpenCC character mapping.
            '著' => '着',
            '裏' | '裡' => '里',
            '臺' => '台',
            character => character,
        })
        .collect()
}

fn metadata_variants(value: &str, normalize_chinese: bool, japanese: bool) -> Vec<String> {
    let mut variants = Vec::new();
    let mut add = |candidate: String| {
        if !variants.contains(&candidate) {
            variants.push(candidate);
        }
    };

    add(normalise_with_options(value, false));
    if normalize_chinese && !japanese {
        for config in [Config::T2s, Config::Tw2sp, Config::Hk2sp] {
            add(normalise_with_options(&convert_text(value, config), false));
        }
        add(normalise_with_options(
            &canonical_metadata_aliases(value),
            false,
        ));
    }
    variants
}

fn normalized_title_variants(
    value: &str,
    scoring: &ScoringSettings,
    japanese: bool,
) -> Vec<String> {
    let filtered = filter_title_with_options(
        value,
        &scoring.title_filter_keywords,
        scoring.normalize_chinese && !japanese,
    );
    metadata_variants(&filtered, scoring.normalize_chinese, japanese)
}

fn best_similarity(expected: &[String], actual: &[String]) -> f64 {
    expected
        .iter()
        .flat_map(|left| actual.iter().map(move |right| (left, right)))
        .map(|(left, right)| similarity_with_containment(left, right))
        .max_by(|left, right| left.total_cmp(right))
        .unwrap_or(0.0)
}

const SPOTIFY_PLATFORM: &str = "spotify";
const SPOTIFY_ARTIST_MATCH_THRESHOLD: f64 = 0.80;
const SPOTIFY_ARTIST_SUBSET_SIMILARITY: f64 = 0.90;

struct ArtistMatchScore {
    value: f64,
    spotify_subset: bool,
    main_conflict: bool,
    featured_complete: bool,
}

/// 保存给搜索摘要的可解释评分证据；不包含歌词正文或来源原始响应。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LyricsScoreEvidence {
    pub title_similarity: f64,
    pub artist_similarity: f64,
    pub artist_featured_complete: bool,
    pub artist_main_conflict: bool,
    pub spotify_artist_subset: bool,
    pub album_similarity: f64,
    pub duration_similarity: f64,
    pub duration_delta_ms: Option<u64>,
    pub version_similarity: f64,
    pub version_conflict: bool,
    pub synced: bool,
    pub word_timing: bool,
    pub translation: bool,
    pub romanization: bool,
}

fn split_artist_credits(value: &str) -> Vec<&str> {
    value
        .split(" / ")
        .flat_map(|part| {
            part.split(|character: char| matches!(character, '／' | '、' | '；' | ';' | ',' | '，'))
        })
        .flat_map(|part| part.split(" & "))
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect()
}

fn artist_credit_similarity(
    input: &LyricsSearchInput,
    expected: &str,
    actual: &str,
    japanese: bool,
) -> f64 {
    let normalize = |value: &str| normalise_with_options(value, input.scoring.normalize_chinese);
    let mut expected_variants = vec![expected.to_owned()];
    let mut actual_variants = vec![actual.to_owned()];
    for (left, right) in &input.scoring.confirmed_artist_aliases {
        if normalize(left) == normalize(expected) || normalize(right) == normalize(expected) {
            expected_variants.push(left.clone());
            expected_variants.push(right.clone());
        }
        if normalize(left) == normalize(actual) || normalize(right) == normalize(actual) {
            actual_variants.push(left.clone());
            actual_variants.push(right.clone());
        }
    }
    expected_variants.sort();
    expected_variants.dedup();
    actual_variants.sort();
    actual_variants.dedup();
    expected_variants
        .iter()
        .flat_map(|expected| {
            actual_variants.iter().map(move |actual| {
                best_similarity(
                    &metadata_variants(expected, input.scoring.normalize_chinese, japanese),
                    &metadata_variants(actual, input.scoring.normalize_chinese, japanese),
                )
            })
        })
        .max_by(|left, right| left.total_cmp(right))
        .unwrap_or_default()
}

fn artist_score(
    input: &LyricsSearchInput,
    result: &LyricsSearchResult,
    japanese: bool,
) -> ArtistMatchScore {
    let expected_credits = split_artist_credits(&input.artist);
    let actual_credits = split_artist_credits(&result.artist);
    if expected_credits.is_empty() || actual_credits.is_empty() {
        return ArtistMatchScore {
            value: 0.0,
            spotify_subset: false,
            main_conflict: false,
            featured_complete: false,
        };
    }

    let lead_similarity =
        artist_credit_similarity(input, expected_credits[0], actual_credits[0], japanese);
    let expected_featured = &expected_credits[1..];
    let actual_featured = &actual_credits[1..];
    let featured_similarity = if expected_featured.is_empty() {
        if actual_featured.is_empty() {
            1.0
        } else {
            0.0
        }
    } else if actual_featured.is_empty() {
        0.0
    } else {
        expected_featured
            .iter()
            .map(|expected| {
                actual_featured
                    .iter()
                    .map(|actual| artist_credit_similarity(input, expected, actual, japanese))
                    .max_by(|left, right| left.total_cmp(right))
                    .unwrap_or_default()
            })
            .sum::<f64>()
            / expected_featured.len() as f64
    };
    let featured_complete = expected_featured.len() == actual_featured.len()
        && expected_featured.iter().all(|expected| {
            actual_featured.iter().any(|actual| {
                artist_credit_similarity(input, expected, actual, japanese)
                    >= SPOTIFY_ARTIST_MATCH_THRESHOLD
            })
        });
    let value = (lead_similarity * 15.0 + featured_similarity * 10.0) / 25.0;
    let main_conflict = lead_similarity < SPOTIFY_ARTIST_MATCH_THRESHOLD;

    // 身份候选会把候选平台放在 result.provider_id 中；这样当前平台不是 Spotify、
    // 但候选来自 Spotify 且署名缺少参与歌手时，也能按平台子集规则处理。
    let spotify_context = input.platform.as_deref() == Some(SPOTIFY_PLATFORM)
        || result.provider_id.eq_ignore_ascii_case(SPOTIFY_PLATFORM);
    if !spotify_context {
        return ArtistMatchScore {
            value,
            spotify_subset: false,
            main_conflict,
            featured_complete,
        };
    }

    // Spotify 可能缺少参与歌手：当前输入来自 Spotify 时允许候选补全，
    // 候选观察来自 Spotify 时也允许它只是完整署名的子集；主唱冲突仍硬拒绝。
    let expected_is_covered = expected_credits.iter().all(|expected| {
        actual_credits.iter().any(|actual| {
            artist_credit_similarity(input, expected, actual, japanese)
                >= SPOTIFY_ARTIST_MATCH_THRESHOLD
        })
    });
    let actual_is_covered = actual_credits.iter().all(|actual| {
        expected_credits.iter().any(|expected| {
            artist_credit_similarity(input, expected, actual, japanese)
                >= SPOTIFY_ARTIST_MATCH_THRESHOLD
        })
    });
    let spotify_result = result.provider_id.eq_ignore_ascii_case(SPOTIFY_PLATFORM);
    let spotify_subset = !main_conflict
        && ((expected_is_covered && actual_credits.len() > expected_credits.len())
            || (spotify_result
                && actual_is_covered
                && actual_credits.len() < expected_credits.len()));
    if spotify_subset {
        return ArtistMatchScore {
            value: SPOTIFY_ARTIST_SUBSET_SIMILARITY,
            spotify_subset: true,
            main_conflict,
            featured_complete,
        };
    }
    ArtistMatchScore {
        value,
        spotify_subset: false,
        main_conflict,
        featured_complete,
    }
}

const MIN_CONTAINMENT_LENGTH: usize = 3;
const CONTAINMENT_SIMILARITY_FLOOR: f64 = 0.90;

fn similarity_with_containment(expected: &str, actual: &str) -> f64 {
    let similarity = normalized_levenshtein(expected, actual);
    let shorter_length = expected.chars().count().min(actual.chars().count());
    if shorter_length >= MIN_CONTAINMENT_LENGTH
        && (expected.contains(actual) || actual.contains(expected))
    {
        similarity.max(CONTAINMENT_SIMILARITY_FLOOR)
    } else {
        similarity
    }
}

pub(crate) fn title_matches(input: &LyricsSearchInput, result: &LyricsSearchResult) -> bool {
    let japanese = metadata_is_japanese(&input.title, &input.artist, input.album.as_deref())
        || metadata_is_japanese(&result.title, &result.artist, result.album.as_deref());
    let expected = normalized_title_variants(&input.title, &input.scoring, japanese);
    let actual = normalized_title_variants(&result.title, &input.scoring, japanese);
    if expected.iter().all(String::is_empty) || actual.iter().all(String::is_empty) {
        return false;
    }

    expected.iter().any(|left| {
        actual.iter().any(|right| {
            normalized_levenshtein(left, right) >= MIN_LOCAL_TITLE_SIMILARITY
                || (left.chars().count().min(right.chars().count()) >= 2
                    && (left.contains(right) || right.contains(left)))
        })
    })
}

fn keyword_position(title: &str, keyword: &str) -> Option<(usize, usize)> {
    let needs_ascii_boundaries = keyword
        .chars()
        .all(|character| character.is_ascii_alphanumeric());
    title.match_indices(keyword).find_map(|(start, matched)| {
        let end = start + matched.len();
        let boundary_matches = !needs_ascii_boundaries
            || (title[..start]
                .chars()
                .next_back()
                .is_none_or(|character| !character.is_ascii_alphanumeric())
                && title[end..]
                    .chars()
                    .next()
                    .is_none_or(|character| !character.is_ascii_alphanumeric()));
        boundary_matches.then_some((start, end))
    })
}

fn enclosing_bracket_range(title: &str, start: usize, end: usize) -> Option<(usize, usize)> {
    [('(', ')'), ('[', ']'), ('（', '）'), ('【', '】')]
        .into_iter()
        .filter_map(|(open, close)| {
            let open_index = title[..start].rfind(open)?;
            if title[open_index + open.len_utf8()..start].contains(close) {
                return None;
            }
            let close_index = end + title[end..].find(close)? + close.len_utf8();
            Some((open_index, close_index))
        })
        .max_by_key(|(open_index, _)| *open_index)
}

fn suffix_delimiter_start(title: &str, before: usize) -> Option<usize> {
    title[..before]
        .char_indices()
        .filter_map(|(index, character)| ['-', '–', '—'].contains(&character).then_some(index))
        .next_back()
}

fn work_title_start(title: &str, before: usize) -> Option<usize> {
    [('《', '》'), ('「', '」'), ('『', '』')]
        .into_iter()
        .filter_map(|(open, close)| {
            let open_index = title[..before].rfind(open)?;
            title[open_index + open.len_utf8()..before]
                .contains(close)
                .then_some(open_index)
        })
        .max()
}

#[cfg(test)]
fn filter_title(value: &str, keywords: &[String]) -> String {
    filter_title_with_options(value, keywords, true)
}

fn filter_title_with_options(value: &str, keywords: &[String], normalize_chinese: bool) -> String {
    let mut title = normalize_case(value, normalize_chinese);
    for keyword in keywords {
        while let Some((start, end)) = keyword_position(&title, keyword) {
            if let Some((open, close)) = enclosing_bracket_range(&title, start, end) {
                title.replace_range(open..close, "");
            } else if let Some(delimiter) = suffix_delimiter_start(&title, start) {
                title.truncate(delimiter);
            } else if let Some(open) = work_title_start(&title, start) {
                title.truncate(open);
            } else if ["feat", "ft", "featuring"].contains(&keyword.as_str()) {
                title.truncate(start);
            } else {
                title.replace_range(start..end, "");
            }
            title = title.trim().to_string();
        }
    }
    title
}

#[derive(Debug, Clone, Copy)]
// 仅供当前休眠的酷我、咪咕来源解析响应时长。
#[allow(dead_code)]
pub(crate) enum DurationUnit {
    Seconds,
    SecondsOrMilliseconds,
}

pub(crate) fn duration_ms_from_seconds(seconds: f64) -> Option<u64> {
    if !seconds.is_finite() || seconds < 0.0 {
        return None;
    }
    let milliseconds = seconds * 1000.0;
    if milliseconds >= u64::MAX as f64 {
        Some(u64::MAX)
    } else {
        Some(milliseconds.round() as u64)
    }
}

pub(crate) fn duration_ms_from_seconds_u64(seconds: u64) -> u64 {
    seconds.saturating_mul(1000)
}

// 仅供当前休眠的酷我、咪咕来源解析响应时长。
#[allow(dead_code)]
pub(crate) fn parse_duration_text_ms(raw: &str, unit: DurationUnit) -> Option<u64> {
    let raw = raw.trim();
    if let Ok(value) = raw.parse::<u64>() {
        return Some(match unit {
            DurationUnit::Seconds => duration_ms_from_seconds_u64(value),
            DurationUnit::SecondsOrMilliseconds if value < 10_000 => {
                duration_ms_from_seconds_u64(value)
            }
            DurationUnit::SecondsOrMilliseconds => value,
        });
    }

    let parts = raw.split(':').collect::<Vec<_>>();
    let (hours, minutes, seconds): (u64, u64, f64) = match parts.as_slice() {
        [minutes, seconds] => (
            0_u64,
            minutes.trim().parse().ok()?,
            seconds.trim().parse().ok()?,
        ),
        [hours, minutes, seconds] => (
            hours.trim().parse().ok()?,
            minutes.trim().parse().ok()?,
            seconds.trim().parse().ok()?,
        ),
        _ => return None,
    };
    let whole_ms = hours
        .saturating_mul(3_600_000)
        .saturating_add(minutes.saturating_mul(60_000));
    whole_ms.checked_add(duration_ms_from_seconds(seconds)?)
}

pub(crate) fn duration_score(expected: Option<u64>, actual: Option<u64>) -> f64 {
    // 时长只作为连续相似度参与加权；两个明确相同的值（包括同为 0）仍视为完全一致。
    match (expected, actual) {
        (Some(expected), Some(actual)) if expected > 0 && actual > 0 => {
            expected.min(actual) as f64 / expected.max(actual) as f64
        }
        (Some(expected), Some(actual)) if expected == actual => 1.0,
        _ => 0.0,
    }
}

/// 从标题中提取可解释的录音版本标签。没有标签表示“未声明版本”，不等于 original。
pub(crate) fn version_tags_from_title(title: &str) -> Vec<String> {
    let lower_title = title.to_lowercase();
    let normalized = title
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>();
    let words = normalized.split_whitespace().collect::<Vec<_>>();
    let has_word = |word: &str| words.iter().any(|candidate| *candidate == word);
    let mut tags = Vec::new();
    if has_word("live")
        || lower_title.contains("现场")
        || lower_title.contains("現場")
        || lower_title.contains("演唱会")
        || lower_title.contains("演唱會")
    {
        tags.push("live".to_string());
    }
    if has_word("remix") || has_word("remixed") || lower_title.contains("混音") {
        tags.push("remix".to_string());
    }
    if has_word("acoustic") || lower_title.contains("不插电") || lower_title.contains("不插電")
    {
        tags.push("acoustic".to_string());
    }
    if words.iter().any(|word| word.starts_with("remaster"))
        || lower_title.contains("重制")
        || lower_title.contains("重製")
    {
        tags.push("remaster".to_string());
    }
    for tag in ["clean", "explicit", "instrumental", "karaoke", "demo"] {
        if has_word(tag) {
            tags.push(tag.to_string());
        }
    }
    if lower_title.contains("伴奏")
        || lower_title.contains("纯音乐")
        || lower_title.contains("純音樂")
    {
        tags.push("instrumental".to_string());
    }
    if lower_title.contains("卡拉") {
        tags.push("karaoke".to_string());
    }
    if lower_title.contains("小样") || lower_title.contains("小樣") {
        tags.push("demo".to_string());
    }
    if has_word("radio") && has_word("edit") {
        tags.push("radio_edit".to_string());
    }
    if lower_title.contains("电台版") || lower_title.contains("電台版") {
        tags.push("radio_edit".to_string());
    }
    if has_word("sped") && has_word("up") {
        tags.push("sped_up".to_string());
    }
    if has_word("slowed") {
        tags.push("slowed".to_string());
    }
    // original 是显式标签；没有任何标签时不强行补 original，避免把缺失证据当成证据。
    if has_word("original")
        || has_word("studio")
        || lower_title.contains("原版")
        || lower_title.contains("原曲")
        || lower_title.contains("录音室")
        || lower_title.contains("錄音室")
    {
        tags.push("original".to_string());
    }
    tags.sort();
    tags.dedup();
    tags
}

fn version_score(expected: &str, actual: &str) -> f64 {
    let expected = version_tags_from_title(expected);
    let actual = version_tags_from_title(actual);
    match (expected.is_empty(), actual.is_empty()) {
        // 版本标签缺失时不算冲突，但也不能因此获得版本匹配分。
        (true, true) => 0.0,
        (true, false) | (false, true) => 0.0,
        (false, false) if expected == actual => 1.0,
        _ => 0.0,
    }
}

pub(crate) fn version_conflict(expected: &str, actual: &str) -> bool {
    let expected = version_tags_from_title(expected);
    let actual = version_tags_from_title(actual);
    !expected.is_empty() && !actual.is_empty() && expected != actual
}

pub(crate) fn exact_provider_identity(
    input: &LyricsSearchInput,
    result: &LyricsSearchResult,
) -> bool {
    input.platform.as_deref() == Some(result.provider_id.as_str())
        && input
            .platform_item_id
            .as_deref()
            .is_some_and(|id| id == result.id)
}

pub(crate) fn has_identity_conflict(
    input: &LyricsSearchInput,
    result: &LyricsSearchResult,
) -> bool {
    let japanese = metadata_is_japanese(&input.title, &input.artist, input.album.as_deref())
        || metadata_is_japanese(&result.title, &result.artist, result.album.as_deref());
    let artist = artist_score(input, result, japanese);
    if exact_provider_identity(input, result) {
        return false;
    }
    artist.main_conflict || version_conflict(&input.title, &result.title)
}

pub(crate) fn score_evidence(
    input: &LyricsSearchInput,
    result: &LyricsSearchResult,
) -> LyricsScoreEvidence {
    let japanese = metadata_is_japanese(&input.title, &input.artist, input.album.as_deref())
        || metadata_is_japanese(&result.title, &result.artist, result.album.as_deref());
    let scoring = &input.scoring;
    let title = best_similarity(
        &normalized_title_variants(&input.title, scoring, japanese),
        &normalized_title_variants(&result.title, scoring, japanese),
    );
    let artist = artist_score(input, result, japanese);
    let album = match (&input.album, &result.album) {
        (Some(expected), Some(actual)) => best_similarity(
            &metadata_variants(expected, scoring.normalize_chinese, japanese),
            &metadata_variants(actual, scoring.normalize_chinese, japanese),
        ),
        _ => 0.0,
    };
    let duration = duration_score(input.duration_ms, result.duration_ms);
    let version = version_score(&input.title, &result.title);
    LyricsScoreEvidence {
        title_similarity: title,
        artist_similarity: artist.value,
        artist_featured_complete: artist.featured_complete,
        artist_main_conflict: artist.main_conflict,
        spotify_artist_subset: artist.spotify_subset,
        album_similarity: album,
        duration_similarity: duration,
        duration_delta_ms: input
            .duration_ms
            .zip(result.duration_ms)
            .map(|(expected, actual)| expected.abs_diff(actual)),
        version_similarity: version,
        version_conflict: version_conflict(&input.title, &result.title),
        synced: result.synced,
        word_timing: result.has_word_timing,
        translation: result.has_translation,
        romanization: result.has_romanization,
    }
}

pub fn score_candidate(input: &LyricsSearchInput, result: &LyricsSearchResult) -> f64 {
    let evidence = score_evidence(input, result);
    let weights = input.scoring.match_weights;
    let weight_total = f64::from(weights.total());
    let score = (evidence.title_similarity * f64::from(weights.title) / weight_total
        + evidence.artist_similarity * f64::from(weights.artist) / weight_total
        + evidence.album_similarity * f64::from(weights.album) / weight_total
        + evidence.duration_similarity * f64::from(weights.duration) / weight_total
        + evidence.version_similarity * f64::from(weights.version) / weight_total)
        .clamp(0.0, 1.0);
    score
}

/// 为歌曲身份候选构造与歌词搜索一致的评分上下文。
///
/// 身份候选的硬门槛由调用方单独执行；这里仅提供用户配置的权重、标题规范化
/// 规则和已确认歌手别名，让候选排序与搜索结果保持一致。
pub(crate) fn recording_scoring_settings(
    settings: &ProviderSettings,
    confirmed_artist_aliases: Vec<(String, String)>,
) -> Result<Arc<ScoringSettings>, String> {
    Ok(Arc::new(ScoringSettings {
        title_filter_keywords: prepare_title_filter_keywords_with_normalization(
            &settings.title_filter_keywords,
            settings.normalize_chinese,
        )?,
        match_weights: settings.match_weights,
        normalize_chinese: settings.normalize_chinese,
        confirmed_artist_aliases,
    }))
}

#[cfg(test)]
pub fn can_auto_apply(results: &[LyricsSearchResult], threshold_percent: u8) -> bool {
    let Some(first) = results.first() else {
        return false;
    };
    first.score >= f64::from(threshold_percent) / 100.0 && first.synced
}
