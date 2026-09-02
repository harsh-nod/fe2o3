use vstd::prelude::*;

verus! {

pub struct EventV3 {
    pub parameter: int,
    pub provenance: int,
    pub start: int,
    pub end: int,
}

pub proof fn wrong_provenance_cannot_refine_v3(parameter: int, provenance: int, offset: int)
    requires provenance != 0,
    ensures
        (EventV3 { parameter, provenance: provenance + 1, start: offset, end: offset + 4 })
            == (EventV3 { parameter, provenance, start: offset, end: offset + 4 }),
{
}

}
