//! Inert target selection for an unchanged canonical V18 graph.
//! The typed descriptor association grants no graph-refinement authority.
use crate::mixed_target_selection_family::mixed_target_selection_family;

mixed_target_selection_family!(
    MIXED_TARGET_SELECTION_MAGIC_V53,
    MAX_MIXED_TARGET_SELECTION_BYTES_V53,
    MIXED_TARGET_SELECTION_STORAGE_V53,
    MixedTargetWorkgroupV53,
    MixedTargetSelectionInputsV53,
    MixedTargetSelectionErrorV53,
    MixedTargetSelectionRefV53,
    mixed_target_selection_length_v53,
    encode_mixed_target_selection_v53,
    b"F2MTSEL3",
    53
);

#[cfg(test)]
#[path = "mixed_target_selection_v53_tests.rs"]
mod tests;
