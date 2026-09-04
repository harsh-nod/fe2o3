use vstd::prelude::*;
verus! {
pub struct NativeV1 { pub allocation: nat, pub generation: nat }
pub open spec fn mutated_same_native_v1(left: NativeV1, right: NativeV1) -> bool {
    left.allocation == right.allocation
}
pub proof fn mutated_completion_native_generation_is_exact_v1()
    ensures !mutated_same_native_v1(
        NativeV1 { allocation: 4, generation: 1 },
        NativeV1 { allocation: 4, generation: 2 },
    ),
{}
}
