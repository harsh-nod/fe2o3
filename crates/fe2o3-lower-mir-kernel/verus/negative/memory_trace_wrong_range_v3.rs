use vstd::prelude::*;

verus! {

pub struct RangeV3 {
    pub start: int,
    pub end: int,
    pub absolute: int,
}

pub proof fn wrong_byte_range_cannot_refine_v3(base: int, offset: int)
    requires base >= 0, offset >= 0,
    ensures
        (RangeV3 { start: offset, end: offset + 8, absolute: base + offset })
            == (RangeV3 { start: offset, end: offset + 4, absolute: base + offset }),
{
}

}
