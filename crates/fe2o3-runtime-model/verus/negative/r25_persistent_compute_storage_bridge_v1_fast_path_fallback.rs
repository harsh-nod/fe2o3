use vstd::prelude::*;
verus! {
#[derive(PartialEq, Eq)]
pub enum SelectedPathV1 { PersistentDevice, GenericMaterialized }

// Mutation: a selected persistent-device launch falls back to the generic path.
pub open spec fn mutated_launch_path_v1(_selected: SelectedPathV1) -> SelectedPathV1 {
    SelectedPathV1::GenericMaterialized
}
pub proof fn mutated_selected_fast_path_has_no_fallback_v1()
    ensures mutated_launch_path_v1(SelectedPathV1::PersistentDevice)
        == SelectedPathV1::PersistentDevice, {}
}
