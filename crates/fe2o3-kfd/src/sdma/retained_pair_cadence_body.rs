// Shared with the selector-only Verus leaf, not a scheduler or hardware proof.
macro_rules! retained_pair_cadence_ceiling_body_v1 {
    ($cadence:ident) => {
        match $cadence {
            Gfx942XgmiRetainedWaitCadenceV1::Ordinary1ms => 1_000_000u64,
            Gfx942XgmiRetainedWaitCadenceV1::Ceiling25us => 25_000u64,
        }
    };
}
