// NSFontManager uses an approximate 0–15 scale. Keep the panel and native
// status item on the same CSS-to-AppKit mapping.
const WEIGHT_HINTS: [(u16, isize); 9] = [
    (100, 1),
    (200, 2),
    (300, 3),
    (400, 5),
    (500, 6),
    (600, 8),
    (700, 9),
    (800, 11),
    (900, 13),
];

pub(crate) fn font_manager_weight(weight: u16) -> isize {
    WEIGHT_HINTS
        .iter()
        .min_by_key(|(css_weight, _)| css_weight.abs_diff(weight))
        .map_or(5, |(_, hint)| *hint)
}

pub(crate) fn css_weight_from_font_manager(weight: isize) -> u16 {
    WEIGHT_HINTS
        .iter()
        .min_by_key(|(_, hint)| hint.abs_diff(weight))
        .map_or(400, |(css_weight, _)| *css_weight)
}

pub(crate) fn system_font_weight(weight: u16) -> f64 {
    match weight {
        0..=149 => -0.80,
        150..=249 => -0.60,
        250..=349 => -0.40,
        350..=449 => 0.0,
        450..=549 => 0.23,
        550..=649 => 0.30,
        650..=749 => 0.40,
        750..=849 => 0.56,
        _ => 0.62,
    }
}
