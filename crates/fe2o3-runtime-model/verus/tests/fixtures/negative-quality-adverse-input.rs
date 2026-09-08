use vstd::prelude::*;

// spec fn line_comment_decoy() -> bool { true }
/* outer decoy: spec fn block_decoy() -> bool { false }
   /* nested decoy: spec fn nested_decoy() -> bool { true } */
*/
const NORMAL_STRING_DECOY: &str = "spec fn normal_decoy() -> bool { true }";
const RAW_STRING_DECOY: &str = r###"spec fn raw_decoy() -> bool { false }"###;
const BYTE_STRING_DECOY: &[u8] = b"spec fn byte_decoy() -> bool { true }";
const RAW_BYTE_STRING_DECOY: &[u8] = br##"spec fn raw_byte_decoy() -> bool { false }"##;
const CHAR_DECOY: char = 't';
const BYTE_CHAR_DECOY: u8 = b'f';

verus! {
pub struct BorrowedCoordinate<'a> {
    pub coordinate: &'a nat,
}

pub open spec fn mutated_matches_v1(expected: nat, observed: nat) -> bool {
    expected == observed
}

pub proof fn coordinate_omission_is_rejected_v1(expected: nat, observed: nat)
    requires expected != observed,
    ensures !mutated_matches_v1(expected, observed),
{}
}
