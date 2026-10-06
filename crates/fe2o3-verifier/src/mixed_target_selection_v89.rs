//! Shared producer/receiver reconstruction of inert V18 target selection.
use crate::mixed_target_selection_family::mixed_target_selection_family;

mixed_target_selection_family!(
    MAX_MIXED_TARGET_SELECTION_BYTES_V89,
    MIXED_TARGET_SELECTION_STORAGE_V89,
    MixedTargetSelectionErrorV89,
    MixedTargetSelectionInputsV89,
    MixedTargetWorkgroupV89,
    encode_mixed_target_selection_v89,
    mixed_target_selection_length_v89,
    MIXED_DESCRIPTOR_READER_STORAGE_V89,
    MixedDescriptorErrorV89,
    MixedDescriptorTableV89,
    decode_mixed_descriptor_v89,
    MixedTargetSelectionValidationErrorV89,
    MixedTargetSelectionSubjectV89,
    mixed_conditional_v86,
    MixedContractErrorV86,
    MixedContractV86,
    with_mixed_target_selection_v89,
    check_mixed_target_selection_v89
);

#[cfg(test)]
#[path = "mixed_target_selection_v89_tests.rs"]
mod tests;
