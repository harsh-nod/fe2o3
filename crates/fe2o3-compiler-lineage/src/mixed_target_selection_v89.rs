//! Inert target selection for an unchanged canonical V18 graph.
//! The typed descriptor association grants no graph-refinement authority.
use crate::mixed_target_selection_family::mixed_target_selection_family;

mixed_target_selection_family!(
    MIXED_TARGET_SELECTION_MAGIC_V89,
    MAX_MIXED_TARGET_SELECTION_BYTES_V89,
    MIXED_TARGET_SELECTION_STORAGE_V89,
    MixedTargetWorkgroupV89,
    MixedTargetSelectionInputsV89,
    MixedTargetSelectionErrorV89,
    MixedTargetSelectionRefV89,
    mixed_target_selection_length_v89,
    encode_mixed_target_selection_v89,
    b"F2MTSE89",
    89
);

#[cfg(test)]
#[path = "mixed_target_selection_v89_tests.rs"]
mod tests;
