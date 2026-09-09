use super::models::LibraryPage;

pub(super) const DEFAULT_LIBRARY_PAGE_SIZE: usize = 20;
pub(super) const LIBRARY_PAGE_SIZES: [usize; 3] = [20, 50, 100];

pub(super) fn library_page<T: Clone>(items: Vec<T>, page: u64, page_size: u64) -> LibraryPage<T> {
    let total = items.len() as u64;
    let page = page.max(1);
    let page_size = usize::try_from(page_size)
        .ok()
        .filter(|value| LIBRARY_PAGE_SIZES.contains(value))
        .unwrap_or(DEFAULT_LIBRARY_PAGE_SIZE);
    let start = ((page - 1) as usize).saturating_mul(page_size);
    let items = items
        .get(start..start.saturating_add(page_size).min(items.len()))
        .unwrap_or_default()
        .to_vec();
    LibraryPage {
        items,
        total,
        page,
        page_size: page_size as u64,
    }
}

pub(super) fn library_page_parameters(page: u64, page_size: u64) -> (u64, usize, i64) {
    let page = page.max(1);
    let page_size = usize::try_from(page_size)
        .ok()
        .filter(|value| LIBRARY_PAGE_SIZES.contains(value))
        .unwrap_or(DEFAULT_LIBRARY_PAGE_SIZE);
    let offset = page
        .saturating_sub(1)
        .saturating_mul(page_size as u64)
        .min(i64::MAX as u64) as i64;
    (page, page_size, offset)
}
