//! Read-only bounded cache DATA for B2 components; cfg(test) parent only.
//! No owner, source, meter, reverse conversion, mutable getter or proof API.
use super::*;
type Entry = ((usize, usize), bool);
pub(in crate::production_ranked_projection_v1) const CACHE_CONTENT_CAP: usize = 128;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production_ranked_projection_v1) enum ContentRefusal {
    LegacyCache,
    CacheLimit,
    CheckedRowsLimit,
    CheckedEntriesLimit,
    OwnedSide,
    OriginalConstructionUnavailable,
}
pub(in crate::production_ranked_projection_v1) type ContentResult<T> =
    std::result::Result<T, ContentRefusal>;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production_ranked_projection_v1) struct CacheContent {
    pub len: usize,
    pub entries: [Option<Entry>; CACHE_CONTENT_CAP],
}
fn storage_content(storage: &CacheStorage) -> ContentResult<CacheContent> {
    let CacheStorage::Strict(rows) = storage else {
        return Err(ContentRefusal::LegacyCache);
    };
    if rows.len() > CACHE_CONTENT_CAP {
        return Err(ContentRefusal::CacheLimit);
    }
    let mut result = CacheContent {
        len: rows.len(),
        entries: [None; CACHE_CONTENT_CAP],
    };
    for (ordinal, row) in rows.iter().enumerate() {
        result.entries[ordinal] = Some(*row);
    }
    Ok(result)
}
pub(in crate::production_ranked_projection_v1) fn live_cache_content(
    cache: &AssertionCacheV1<'_>,
) -> ContentResult<CacheContent> {
    storage_content(&cache.storage)
}
pub(in crate::production_ranked_projection_v1) fn retired_cache_content(
    cache: &RetiredAssertionCacheV1,
) -> ContentResult<CacheContent> {
    storage_content(&cache.storage)
}

// Source-level logical policy only, not machine-stack or allocator/RSS data.
// The B2 caller prepays six complete invocations: two live caches, two retired
// caches, and two final postflight retired-cache observations. No state branch
// changes that amount. Legacy/oversize refusals never invoke a proof operation.
const ROWS: usize = 9;
fn frame<T>(locals: usize) -> Result<usize> {
    locals
        .checked_add(
            size_of::<T>()
                .checked_mul(2)
                .ok_or_else(|| resource(Resource::Arithmetic))?,
        )
        .and_then(|n| {
            size_of::<Result<T>>()
                .checked_mul(2)
                .and_then(|r| n.checked_add(r))
        })
        .and_then(|n| {
            size_of::<ContentResult<T>>()
                .checked_mul(2)
                .and_then(|r| n.checked_add(r))
        })
        .ok_or_else(|| resource(Resource::Arithmetic))
}
fn rows() -> Result<[usize; ROWS]> {
    Ok([
        frame::<CacheContent>(size_of::<(
            &AssertionCacheV1<'static>,
            &CacheStorage,
            ContentResult<CacheContent>,
        )>())?,
        frame::<CacheContent>(size_of::<(
            &RetiredAssertionCacheV1,
            &CacheStorage,
            ContentResult<CacheContent>,
        )>())?,
        frame::<CacheContent>(size_of::<(
            &CacheStorage,
            &Vec<Entry>,
            usize,
            bool,
            ContentRefusal,
            CacheContent,
        )>())?,
        frame::<CacheContent>(size_of::<(
            usize,
            [Option<Entry>; CACHE_CONTENT_CAP],
            CacheContent,
        )>())?,
        frame::<()>(size_of::<(
            std::iter::Enumerate<std::slice::Iter<'static, Entry>>,
            Option<(usize, &Entry)>,
            usize,
            &Entry,
            Entry,
            Option<Entry>,
            &mut Option<Entry>,
        )>())?,
        frame::<ContentResult<CacheContent>>(size_of::<(
            ContentResult<CacheContent>,
            CacheContent,
            ContentRefusal,
        )>())?,
        frame::<usize>(size_of::<(
            usize,
            usize,
            Option<usize>,
            Result<usize>,
            Error,
            Resource,
        )>())?,
        frame::<[usize; ROWS]>(size_of::<(
            [usize; ROWS],
            Result<[usize; ROWS]>,
            std::slice::Iter<'static, usize>,
            &usize,
            usize,
            Option<usize>,
            Result<usize>,
        )>())?,
        frame::<usize>(size_of::<(
            usize,
            Option<usize>,
            Result<usize>,
            Error,
            Resource,
        )>())?,
    ])
}
pub(in crate::production_ranked_projection_v1) fn cache_content_frame() -> Result<usize> {
    rows()?.iter().try_fold(0usize, |total, row| {
        total
            .checked_add(*row)
            .ok_or_else(|| resource(Resource::Arithmetic))
    })
}

#[test]
fn original_cache_content_is_exact_insertion_order_and_same_retired_allocation() {
    // Inert storage-only control. This is not a query or authenticated owner.
    let cache = AssertionCacheV1 {
        storage: CacheStorage::Strict(vec![((3, 5), true), ((7, 11), false)]),
        owner: None,
        lifetime: PhantomData,
    };
    let raw = cache.snapshot_for_retirement_test();
    let content = live_cache_content(&cache).unwrap();
    assert_eq!(content.len, 2);
    assert_eq!(content.entries[0], Some(((3, 5), true)));
    assert_eq!(content.entries[1], Some(((7, 11), false)));
    assert!(content.entries[2..].iter().all(Option::is_none));
    let retired = cache.retire_payload_v1();
    assert_eq!(retired.snapshot_for_test(), raw);
    assert_eq!(retired_cache_content(&retired).unwrap(), content);
}
#[test]
fn original_cache_content_refuses_legacy_and_oversize_without_truncation() {
    let legacy = CacheStorage::Legacy(HashMap::new());
    assert_eq!(storage_content(&legacy), Err(ContentRefusal::LegacyCache));
    let oversized = CacheStorage::Strict(vec![((0, 0), false); CACHE_CONTENT_CAP + 1]);
    assert_eq!(storage_content(&oversized), Err(ContentRefusal::CacheLimit));
    let exact = CacheStorage::Strict(vec![((0, 0), false); CACHE_CONTENT_CAP]);
    assert_eq!(storage_content(&exact).unwrap().len, CACHE_CONTENT_CAP);
}
#[test]
fn original_cache_content_equal_counts_do_not_hide_key_value_or_order_changes() {
    let expected = storage_content(&CacheStorage::Strict(vec![
        ((3, 5), true),
        ((7, 11), false),
    ]))
    .unwrap();
    for rows in [
        vec![((3, 6), true), ((7, 11), false)],
        vec![((3, 5), false), ((7, 11), false)],
        vec![((7, 11), false), ((3, 5), true)],
    ] {
        let other = storage_content(&CacheStorage::Strict(rows)).unwrap();
        assert_eq!(other.len, expected.len);
        assert_ne!(other, expected);
    }
}
#[test]
fn original_cache_content_header_is_explicit_checked_and_state_independent() {
    let actual = rows().unwrap();
    assert_eq!(actual.len(), ROWS);
    assert_eq!(cache_content_frame().unwrap(), actual.iter().sum::<usize>());
    assert!(frame::<CacheContent>(usize::MAX).is_err());
}
