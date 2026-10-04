//! Shared producer/receiver reconstruction of inert V18 target selection.
use crate::mixed_target_selection_family::mixed_target_selection_family;

mixed_target_selection_family!(
    MAX_MIXED_TARGET_SELECTION_BYTES_V53,
    MIXED_TARGET_SELECTION_STORAGE_V53,
    MixedTargetSelectionErrorV53,
    MixedTargetSelectionInputsV53,
    MixedTargetWorkgroupV53,
    encode_mixed_target_selection_v53,
    mixed_target_selection_length_v53,
    MIXED_DESCRIPTOR_READER_STORAGE_V53,
    MixedDescriptorErrorV53,
    MixedDescriptorTableV53,
    decode_mixed_descriptor_v53,
    MixedTargetSelectionValidationErrorV53,
    MixedTargetSelectionSubjectV53,
    mixed_conditional_v26,
    MixedContractErrorV26,
    MixedContractV26,
    with_mixed_target_selection_v53,
    check_mixed_target_selection_v53
);

#[cfg(test)]
#[path = "mixed_target_selection_v53_tests.rs"]
mod tests;
