use vstd::prelude::*;

verus! {

pub open spec fn source_trace_length_v3(guard: bool) -> nat {
    if guard { 3 } else { 0 }
}

pub open spec fn hostile_kir_trace_length_v3(_guard: bool) -> nat { 3 }

pub proof fn false_guard_effect_cannot_refine_v3()
    ensures source_trace_length_v3(false) == hostile_kir_trace_length_v3(false),
{
}

}
