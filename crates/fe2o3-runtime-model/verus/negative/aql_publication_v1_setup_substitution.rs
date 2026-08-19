use vstd::prelude::*;

verus! {

pub open spec fn copied_invalid_word_v1() -> Seq<u8> {
    seq![1u8, 0u8, 1u8, 0u8]
}

pub open spec fn mutated_setup_substitution_v1() -> Seq<u8> {
    seq![0x02u8, 0x14u8, 2u8, 0u8]
}

pub proof fn mutated_setup_substitution_preserves_copied_setup_v1()
    ensures
        mutated_setup_substitution_v1()[2] == copied_invalid_word_v1()[2],
        mutated_setup_substitution_v1()[3] == copied_invalid_word_v1()[3],
{
}

} // verus!
