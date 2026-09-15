//! Inert owner/coordinate checks, not frontend authentication or machine authority.

use super::*;
use fe2o3_lower_mir_kernel::ProductionKernelContextEntryTransferV1;

fn transfer(
    owner: &ProductionSemanticMirOwnerV1,
    mutation: u8,
) -> ProductionKernelContextEntryTransferV1 {
    let mir = owner.semantic();
    let identity = |tag| SemanticFunctionIdentityV1::from_sha256(bytes(tag));
    let mut semantic = *mir.semantic_sha256().as_bytes();
    if mutation == 1 {
        semantic[0] ^= 1;
    }
    ProductionKernelContextEntryTransferV1::new(
        semantic,
        if mutation == 2 {
            identity(222)
        } else {
            mir.functions()[0].identity()
        },
        if mutation == 3 {
            identity(223)
        } else {
            mir.functions()[1].identity()
        },
        identity(if mutation == 4 { 224 } else { 187 }),
        SemanticTypeIdentityV1::from_sha256(bytes(if mutation == 5 { 225 } else { 160 })),
        SemanticBlockIdV1::from_index(if mutation == 6 { 2 } else { 0 }),
        SemanticLocalIdV1::from_index(if mutation == 7 { 2 } else { 1 }),
        SemanticBlockIdV1::from_index(if mutation == 8 { 2 } else { 1 }),
        u32::from(mutation == 9),
        if mutation == 10 { [0; 32] } else { bytes(226) },
    )
}

#[test]
fn exact_context_entry_transfer_replays_without_reissuing_context() {
    let owner = kernel_context_owner_v15(KernelContextFixtureV1::ErasedEntry);
    let input = kernel_context_input_v1().with_entry_transfer(transfer(&owner, 0));
    let lowered = ProductionSemanticKirOwnerV1::try_lower_with_kernel_contexts(
        owner,
        ProductionSemanticKirLimitsV1::default(),
        vec![input],
    )
    .unwrap();
    lowered.verify_equivalence().unwrap();
    assert_eq!(
        lowered
            .module()
            .functions
            .iter()
            .filter_map(|f| f.body.as_ref())
            .flat_map(|body| &body.blocks)
            .flat_map(|block| &block.operations)
            .filter(|op| matches!(op.kind, OperationKind::KernelContextIssue(_)))
            .count(),
        1
    );
}

#[test]
fn context_entry_transfer_rejects_changed_owner_identities_and_coordinates() {
    let mut commitments = std::collections::BTreeSet::new();
    for mutation in 0..=10 {
        let owner = kernel_context_owner_v15(KernelContextFixtureV1::ErasedEntry);
        let record = transfer(&owner, mutation);
        assert!(commitments.insert(record.commitment_bytes()));
        if mutation == 0 {
            continue;
        }
        let input = kernel_context_input_v1().with_entry_transfer(record);
        let error = ProductionSemanticKirOwnerV1::try_lower_with_kernel_contexts(
            owner,
            ProductionSemanticKirLimitsV1::default(),
            vec![input],
        )
        .err()
        .expect("changed Context entry commitment must fail closed");
        assert!(
            format!("{error:?}").contains("Context entry source transfer changed"),
            "mutation {mutation}: {error:?}"
        );
    }
}

#[test]
fn context_entry_receipt_cannot_replace_an_existing_source_move() {
    let owner = kernel_context_owner_v15(KernelContextFixtureV1::Valid);
    let input = kernel_context_input_v1().with_entry_transfer(transfer(&owner, 0));
    assert!(
        ProductionSemanticKirOwnerV1::try_lower_with_kernel_contexts(
            owner,
            ProductionSemanticKirLimitsV1::default(),
            vec![input],
        )
        .is_err()
    );
}
