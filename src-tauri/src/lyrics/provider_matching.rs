pub(crate) fn validate_settings(settings: &ProviderSettings) -> Result<(), String> {
    if settings.auto_apply_threshold > 100 {
        return Err("自动匹配相似度必须在 0–100 之间".into());
    }
    if settings.auto_apply_duration_tolerance_seconds > MAX_AUTO_APPLY_DURATION_TOLERANCE_SECONDS {
        return Err("自动匹配歌词时长容差必须在 0–60 秒之间".into());
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
    validate_settings(settings)?;
    complete_settings(settings);
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
pub(crate) enum DurationUnit {
    Seconds,
    SecondsOrMilliseconds,
}

const DURATION_FUZZY_WINDOW_MS: u64 = 12_000;

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
    match (expected, actual) {
        (Some(expected), Some(actual)) => {
            let delta = expected.abs_diff(actual) as f64;
            (1.0 - delta / DURATION_FUZZY_WINDOW_MS as f64).clamp(0.0, 1.0)
        }
        _ => 0.6,
    }
}

pub fn score_candidate(input: &LyricsSearchInput, result: &LyricsSearchResult) -> f64 {
    let scoring = &input.scoring;
    let japanese = metadata_is_japanese(&input.title, &input.artist, input.album.as_deref())
        || metadata_is_japanese(&result.title, &result.artist, result.album.as_deref());
    let title = best_similarity(
        &normalized_title_variants(&input.title, scoring, japanese),
        &normalized_title_variants(&result.title, scoring, japanese),
    );
    let artist = best_similarity(
        &metadata_variants(&input.artist, scoring.normalize_chinese, japanese),
        &metadata_variants(&result.artist, scoring.normalize_chinese, japanese),
    );
    let album = match (&input.album, &result.album) {
        (Some(expected), Some(actual)) => best_similarity(
            &metadata_variants(expected, scoring.normalize_chinese, japanese),
            &metadata_variants(actual, scoring.normalize_chinese, japanese),
        ),
        _ => 0.6,
    };
    let duration = duration_score(input.duration_ms, result.duration_ms);
    let weights = scoring.match_weights;
    let weight_total = f64::from(weights.total());
    (title * f64::from(weights.title) / weight_total
        + artist * f64::from(weights.artist) / weight_total
        + album * f64::from(weights.album) / weight_total
        + duration * f64::from(weights.duration) / weight_total
        + if result.synced { 0.04 } else { 0.0 })
    .clamp(0.0, 1.0)
}

#[cfg(test)]
pub fn can_auto_apply(results: &[LyricsSearchResult], threshold_percent: u8) -> bool {
    let Some(first) = results.first() else {
        return false;
    };
    first.score >= f64::from(threshold_percent) / 100.0 && first.synced
}
