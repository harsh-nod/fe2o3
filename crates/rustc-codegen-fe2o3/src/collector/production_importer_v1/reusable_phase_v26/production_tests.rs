//! Called by both actual registered two-phase SSA callbacks after the positive.
use super::*;
pub(in crate::collector::production_importer_v1::reusable_phase_v26) fn substitutions(
    contexts:&mut AuthenticatedProductionKernelContextsV1,owner:&ProductionSemanticSsaOwnerV1,
) {
    assert!(matches!(AuthenticatedProductionKernelContextsV1::validate_reusable_phase_source_ssa(None,owner),
        Err(ProductionSemanticImportErrorV1::KernelContextBinding("phase SSA requires its complete live source seal"))));
    let Some(SourceStatus::Checked(seal))=contexts.reusable_phase_source.as_mut() else {panic!("actual live two-phase source must have the checked final seal");};
    let hash=seal.semantic;
    seal.semantic[0]^=1;
    assert!(matches!(seal.rebind(owner),Err(ProductionSemanticImportErrorV1::KernelContextBinding(
        "phase execution lost its exact live final-source seal"))));
    seal.semantic=hash;
    let root=seal.root;
    seal.root=SemanticFunctionIdV1::from_index(root.index()+1);
    assert!(matches!(seal.rebind(owner),Err(ProductionSemanticImportErrorV1::KernelContextBinding(
        "phase execution lost its exact live final-source seal"))));
    seal.root=root;
    let protocols=std::mem::take(&mut seal.protocols);
    assert!(matches!(seal.rebind(owner),Err(ProductionSemanticImportErrorV1::KernelContextBinding(
        "phase execution lost its exact live final-source seal"))));
    seal.protocols=protocols;
    assert_eq!(seal.protocols.len(),2);
    let second=seal.protocols.pop().unwrap();
    assert!(matches!(seal.rebind(owner),Err(ProductionSemanticImportErrorV1::KernelContextBinding(
        "phase execution has no complete source protocol"))));
    seal.protocols.push(second);
    seal.rebind(owner).expect("restored complete source owner must still pass");
    ssa_protocol::emission_sites::tests::positive(seal.rebind(owner).unwrap());
    for case in 0..8 {
        ssa_protocol::emission_sites::tests::substitution(seal.rebind(owner).unwrap(),case);
    }
    for case in 0..5 {
        ssa_protocol::emission_sites::tests::completion_boundary(seal.rebind(owner).unwrap(),case);
    }
    let old=contexts.reusable_phase_source.take();
    contexts.reusable_phase_source=Some(SourceStatus::Unsupported("test unsupported phase memory protocol"));
    assert!(matches!(AuthenticatedProductionKernelContextsV1::validate_reusable_phase_source_ssa(Some(contexts),owner),
        Err(ProductionSemanticImportErrorV1::KernelContextBinding("test unsupported phase memory protocol"))));
    contexts.reusable_phase_source=old;
    AuthenticatedProductionKernelContextsV1::validate_reusable_phase_source_ssa(Some(contexts),owner).unwrap();
}
