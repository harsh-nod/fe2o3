use vstd::prelude::*;

verus! {

pub struct InventoryIdentityV1 {
    pub gpu_unique: nat,
    pub pci: nat,
    pub device: nat,
    pub pci_revision: nat,
    pub target: nat,
}

pub open spec fn mutated_selected_inventory_without_target_v1(
    selected: InventoryIdentityV1,
    expected: InventoryIdentityV1,
) -> bool {
    &&& selected.gpu_unique == expected.gpu_unique
    &&& selected.pci == expected.pci
    &&& selected.device == expected.device
    &&& selected.pci_revision == expected.pci_revision
}

pub proof fn mutated_projection_inventory_target_substitution_is_rejected_v1(
    selected: InventoryIdentityV1,
    expected: InventoryIdentityV1,
)
    requires
        selected.gpu_unique == expected.gpu_unique,
        selected.pci == expected.pci,
        selected.device == expected.device,
        selected.pci_revision == expected.pci_revision,
        selected.target == 942 || selected.target == 950,
        expected.target == 942 || expected.target == 950,
        selected.target != expected.target,
    ensures
        !mutated_selected_inventory_without_target_v1(selected, expected),
{
}

} // verus!
