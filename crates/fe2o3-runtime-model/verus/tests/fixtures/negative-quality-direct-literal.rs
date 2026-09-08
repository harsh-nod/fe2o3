use vstd::prelude::*;

verus! {
pub open spec fn literal_true_direct_v1() -> bool { true }
pub open spec fn literal_false_braced_v1() -> bool { false }
pub open spec fn literal_true_parenthesized_v1() -> bool { (true) }
pub open spec fn literal_false_equality_v1() -> bool { { false } }
pub open spec fn literal_true_assert_v1() -> bool { ((true)) }
pub open spec fn literal_false_wrapper_v1() -> bool { ({ false }) }
pub open spec fn r#literal_raw_identifier_v1() -> bool { true }

pub open spec fn wrapper_v1(value: bool) -> bool { value }

pub proof fn literal_consumers_are_irrelevant_v1()
    ensures
        !literal_true_direct_v1(),
        { literal_false_braced_v1() },
        !(literal_true_parenthesized_v1()),
        literal_false_equality_v1() == false,
        wrapper_v1(literal_false_wrapper_v1()),
        r#literal_raw_identifier_v1(),
{
    assert(literal_true_assert_v1());
}
}
