// Expected-negative R46 mutation: the signal-to-named-fence premise is omitted.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_contract_is_exact_v1(
    admitted: bool,
    system_scope: bool,
    signal_names_fence: bool,
    preceding_visible: bool,
) -> bool {
    admitted && system_scope && preceding_visible
}
pub proof fn mutated_signal_fence_binding_is_rejected_v1()
    ensures !mutated_contract_is_exact_v1(true, true, false, true),
{}
}
