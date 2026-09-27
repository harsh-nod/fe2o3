use vstd::prelude::*;

include!("../../fe2o3-kfd/src/initialized_prefix_body.rs");

verus! {

pub open spec fn valid_write(prefix: u64, extent: u64, offset: u64, len: u64) -> bool {
    prefix <= extent && len > 0 && offset as int + len as int <= extent as int
}

pub open spec fn next_prefix(prefix: u64, offset: u64, len: u64, known: bool) -> int {
    if known && offset <= prefix && offset as int + len as int > prefix as int {
        offset as int + len as int
    } else if !known && offset < prefix {
        offset as int
    } else {
        prefix as int
    }
}

pub open spec fn byte_is_known(prefix: u64, offset: u64, len: u64, known: bool, i: int) -> bool {
    if offset as int <= i < offset as int + len as int { known }
    else { i < prefix as int }
}

pub fn covers(prefix: u64, extent: u64, offset: u64, len: u64) -> (out: bool)
    ensures out == (prefix <= extent && len > 0
        && offset as int + len as int <= prefix as int),
{
    initialized_prefix_covers_body!(verus_exec_expr, prefix, extent, offset, len)
}

pub fn after_write(prefix: u64, extent: u64, offset: u64, len: u64, known: bool)
    -> (out: Option<u64>)
    ensures
        out.is_some() == valid_write(prefix, extent, offset, len),
        out.is_some() ==> out.unwrap() as int == next_prefix(prefix, offset, len, known),
        out.is_some() ==> out.unwrap() <= extent,
        out.is_some() && known ==> out.unwrap() >= prefix,
        out.is_some() && !known ==> out.unwrap() <= prefix && out.unwrap() <= offset,
        out.is_some() && offset > prefix ==> out.unwrap() == prefix,
        out.is_some() ==> forall|i: int| 0 <= i < out.unwrap() as int ==>
            #[trigger] byte_is_known(prefix, offset, len, known, i),
{
    initialized_prefix_after_write_body!(verus_exec_expr, prefix, extent, offset, len, known)
}

pub fn sequential_windows_cover_logical_extent(extent: u64, split: u64) -> (out: bool)
    requires 0 < split < extent,
    ensures out,
{
    let first = after_write(0, extent, 0, split, true).unwrap();
    let second = after_write(first, extent, split, extent - split, true).unwrap();
    covers(second, extent, 0, extent)
}

pub fn known_partial_overwrite_preserves_full_coverage(extent: u64, offset: u64, len: u64)
    -> (out: u64)
    requires valid_write(extent, extent, offset, len),
    ensures out == extent,
{
    after_write(extent, extent, offset, len, true).unwrap()
}

pub fn unknown_overwrite_excludes_modified_bytes(prefix: u64, extent: u64, offset: u64, len: u64)
    -> (out: bool)
    requires valid_write(prefix, extent, offset, len),
    ensures !out,
{
    let next = after_write(prefix, extent, offset, len, false).unwrap();
    covers(next, extent, offset, len)
}

pub fn gap_never_becomes_full(prefix: u64, extent: u64, offset: u64, len: u64) -> (out: bool)
    requires valid_write(prefix, extent, offset, len), prefix < offset,
    ensures !out,
{
    let next = after_write(prefix, extent, offset, len, true).unwrap();
    covers(next, extent, 0, extent)
}

pub fn reset_never_certifies_bytes(extent: u64, offset: u64, len: u64) -> (out: bool)
    ensures !out,
{
    covers(0, extent, offset, len)
}

pub fn maximal_ranges_do_not_wrap() {
    let full = after_write(0, u64::MAX, 0, u64::MAX, true);
    assert(full == Some(u64::MAX));
    let last = covers(u64::MAX, u64::MAX, u64::MAX - 1, 1);
    let outside = covers(u64::MAX, u64::MAX, u64::MAX, 1);
    let overflow1 = after_write(u64::MAX, u64::MAX, u64::MAX, 1, true);
    let overflow2 = after_write(u64::MAX, u64::MAX, 1, u64::MAX, false);
    assert(last && !outside);
    assert(overflow1.is_none() && overflow2.is_none());
}

}
