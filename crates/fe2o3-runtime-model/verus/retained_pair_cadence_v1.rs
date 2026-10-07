// Proves only the actual closed ceiling-selection body. It does not model
// Instant, cursor construction, scheduling, custody callbacks or hardware.
use vstd::prelude::*;
include!("../../fe2o3-kfd/src/sdma/retained_pair_cadence_declarations.rs");
include!("../../fe2o3-kfd/src/sdma/retained_pair_cadence_body.rs");
retained_pair_cadence_declarations_v1!(verus);

verus! {

fn retained_pair_cadence_ceiling_v1(cadence: Gfx942XgmiRetainedWaitCadenceV1) -> (ceiling: u64)
    ensures
        0 < ceiling <= 1_000_000,
        match cadence {
            Gfx942XgmiRetainedWaitCadenceV1::Ordinary1ms => ceiling == 1_000_000,
            Gfx942XgmiRetainedWaitCadenceV1::Ceiling25us => ceiling == 25_000,
        },
{
    retained_pair_cadence_ceiling_body_v1!(cadence)
}

}
