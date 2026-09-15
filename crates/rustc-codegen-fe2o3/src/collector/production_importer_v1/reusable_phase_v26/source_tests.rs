//! Actual registered two-phase callback assertions. No source or KIR success
//! is inferred from the dormant in-memory lifecycle unit tests.
use super::*;
use crate::rustc_semantic_plan_v1::{
    DebugSourceCaptureRequestV2, build_production_semantic_preflight_plan_v1,
};
use fe2o3_mir_model::semantic_mir_v1::SemanticDefinedReusablePhaseRecipeV1 as R;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBasicBlockV1, SemanticStatementKindV1, SemanticStatementV1,
};
use fe2o3_mir_model::{SemanticCallExpansionLimitsV1, SemanticCallExpansionV1};

#[path = "unique_carrier_tests.rs"]
mod unique_carrier_tests;

#[path = "emission_source_tests.rs"]
mod emission_source_tests;

#[path = "three_phase_source_tests.rs"]
pub(in crate::collector::production_importer_v1) mod three_phase_source_tests;

#[path = "duplicate_bind_receipt.rs"]
mod duplicate_bind_receipt;

pub(in crate::collector::production_importer_v1) const SOURCE: &str = r#"
#![no_std]
use fe2o3_device::{KernelContext, kernel};
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1]))]
pub fn two_phases(mut context: KernelContext<'_>, value: u32) {
    context.with_workgroup(|workgroup| {
        let mut storage = workgroup.allocate_lds::<f32, 64>().into_reusable();
        let mut phases = workgroup.into_reusable();
        let first = phases.with_phase(|phase| {
            let _lease = phase.bind_reusable_lds(&mut storage);
            (phase.finish_reusable_phase(), value)
        });
        let second = phases.with_phase(|phase| {
            let _lease = phase.bind_reusable_lds(&mut storage);
            (phase.finish_reusable_phase(), first)
        });
        if second == 0 { fe2o3_device::trap(); }
    });
}
"#;

pub(in crate::collector::production_importer_v1) fn check(tcx: TyCtxt<'_>, require_ssa: bool, expected_rejection: Option<&'static str>) {
    check_inner(tcx, require_ssa, expected_rejection, false);
}

pub(in crate::collector::production_importer_v1) fn check_emission(tcx: TyCtxt<'_>) {
    check_inner(tcx, true, None, true);
}

fn check_inner(tcx: TyCtxt<'_>, require_ssa: bool, expected_rejection: Option<&'static str>, require_emission: bool) {
    use crate::production_target_v1::RetainedProductionTargetV1;
    let target = RetainedProductionTargetV1::authenticate_live_before_collection(tcx).unwrap();
    let partitions = tcx.collect_and_partition_mono_items(());
    let closure = crate::collector::collect_authenticated_kernel_closure_v1(
        tcx,
        partitions.codegen_units,
        false,
        target,
    )
    .expect("actual registered two-phase closure");
    let typed_roots = closure.rederive_typed_descriptor_roots(tcx).unwrap();
    let observed = RetainedProductionTargetV1::authenticate_live_before_collection(tcx)
        .unwrap()
        .authenticate_import_session(tcx)
        .unwrap();
    let inventory =
        build_identity_inventory_v1(tcx, &observed, &closure.collection, &closure.roots).unwrap();
    let closure_types = closure
        .collection
        .functions
        .iter()
        .filter_map(|function| function.closure_plan.as_ref())
        .flat_map(|plan| plan.authenticated_closure_type_identities())
        .map(SemanticTypeIdentityV1::from_sha256)
        .collect::<BTreeSet<_>>();
    let plan = build_production_semantic_preflight_plan_v1(
        tcx,
        canonical_target_layout_v1(observed.rustc_layout()),
        inventory.functions,
        inventory.roots,
        inventory.sha256,
        &closure_types,
        DebugSourceCaptureRequestV2::Disabled,
    )
    .expect("actual phase source plan including nested logical types");
    let mut imported =
        construct_production_semantic_mir_v1(tcx, closure, DebugSourceCaptureRequestV2::Disabled)
            .expect("all five original phase recipes must complete canonical import");
    assert_eq!(imported.rustc_identity_inventory.sha256(), inventory.sha256);
    assert_eq!(
        imported.rustc_preflight_plan.canonical_transcript(),
        plan.canonical_transcript()
    );
    let mir = &imported.semantic_mir;
    assert_eq!(mir.wire_version(), SemanticMirWireVersionV1::V26);
    assert!(mir.transpose_owned_flows().is_empty());
    let typed_roots =
        crate::compiler_descriptor::order_typed_descriptor_roots_by_semantic_v1(typed_roots, mir)
            .unwrap();
    crate::compiler_descriptor::validate_production_v1_semantic_ownership_evidence(
        &typed_roots,
        mir,
    )
    .unwrap();
    mir.require_complete_external_entries().unwrap();
    let current = AdmittedInertSemanticMirV1::decode_current_production_canonical(
        mir.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    let exact = AdmittedInertSemanticMirV1::decode_exact_v26_canonical(
        mir.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    for decoded in [&current, &exact] {
        assert_eq!(decoded.canonical_encoding(), mir.canonical_encoding());
        validate_execution_terminal_carriage_v1(tcx, &plan, &imported.kernel_contexts, decoded)
            .unwrap();
        validate_carriage(tcx, &plan, &imported.kernel_contexts, decoded).unwrap();
    }
    assert!(
        AdmittedInertSemanticMirV1::decode_exact_v25_canonical(
            mir.canonical_encoding(),
            SemanticMirLimitsV1::default()
        )
        .is_err()
    );
    if let Some(detail)=expected_rejection {
        assert!(matches!(&imported.kernel_contexts.reusable_phase_source,
            Some(production::SourceStatus::Unsupported(actual)) if *actual==detail));
        assert!(matches!(execution_source::CheckedSource::observe(tcx,&plan,&imported.kernel_contexts,mir),
            Err(ProductionSemanticImportErrorV1::KernelContextBinding(actual)) if actual==detail));
        return;
    }
    let records = mir
        .functions()
        .iter()
        .filter_map(|function| match function.defined_capability_contract() {
            Some(SemanticDefinedCapabilityContractV1::ReusablePhase(record)) => Some(*record),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut roles = [0usize; 5];
    for record in &records {
        let original_body = &mir.functions()[record.function().index() as usize];
        let (body_commitment, _) = fe2o3_mir_model::semantic_mir_v1::canonical_semantic_source_body_sha256_v25(
            original_body,
            SemanticMirLimitsV1::default().limit(SemanticMirResourceV1::CanonicalBytes),
        ).expect("V26 attachment must keep an exactly representable V25 ordinary source body");
        assert_eq!(record.body_identity(), &body_commitment);
        let role = match record.recipe() {
            R::OwnerConvert { .. } => 0,
            R::Issue { .. } => 1,
            R::WithPhase { .. } => 2,
            R::Bind { .. } => 3,
            R::Finish { .. } => 4,
        };
        roles[role] += record.incoming().count() as usize;
        assert_eq!(
            record.source_identity(),
            mir.functions()[record.function().index() as usize].identity()
        );
        assert_ne!(record.source_binding(), &[0; 32]);
        substituted_source_binding(tcx, &plan, &imported.kernel_contexts, mir, *record);
        substituted_original_body_and_abi(mir,*record);
    }
    assert_eq!(
        roles,
        [1, 2, 2, 2, 2],
        "complete original role/incoming roster, not just distinct callees"
    );
    let expansion =
        SemanticCallExpansionV1::try_new(mir, SemanticCallExpansionLimitsV1::default()).unwrap();
    let [root] = mir.roots() else {
        panic!("one actual registered phase root")
    };
    let view = expansion.root(*root).unwrap();
    let source =
        execution_source::CheckedSource::observe(tcx, &plan, &imported.kernel_contexts, mir)
            .expect("actual HIR owner/storage binding and complete acyclic phase source protocol");
    assert_eq!(source.protocols.len(), 2);
    let bound = source
        .bind_expansion(&expansion, view)
        .expect("all expanded occurrences bound to the same live source owner");
    assert_eq!(bound.phases.len(), 2);
    let [first, second] = bound.phases.as_slice() else {
        unreachable!()
    };
    assert_eq!(first.owner, second.owner);
    assert_ne!(first.wrapper, second.wrapper);
    assert_ne!(first.issue, second.issue);
    assert_ne!(first.finish, second.finish);
    assert_ne!(first.closure, second.closure);
    assert_eq!(first.binds.len(), 1);
    assert_eq!(second.binds.len(), 1);
    assert_eq!(
        first.binds[0].storage_conversion,
        second.binds[0].storage_conversion
    );
    assert_ne!(first.binds[0].call, second.binds[0].call);
    drop(bound);
    if require_ssa {
        // This required positive is deliberately not converted to a negative
        // assertion while the production ordered-loan/event bridge is pending.
        let owner = fe2o3_pliron::ProductionSemanticMirOwnerV1::try_new(
            exact,
            fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap();
        let ssa = fe2o3_pliron::ProductionSemanticSsaOwnerV1::try_new(
            owner,
            fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
        )
        .expect("actual two-phase source requires complete consuming SSA transport");
        ssa.verify_replay().unwrap();
        AuthenticatedProductionKernelContextsV1::validate_reusable_phase_source_ssa(Some(&imported.kernel_contexts),&ssa)
            .expect("the actual production transition must consume the live source seal");
        let source=execution_source::CheckedSource::observe(tcx,&plan,&imported.kernel_contexts,ssa.source_semantic()).unwrap();
        let root=ssa.source_semantic().roots()[0];
        let bound=source.bind_expansion(ssa.execution_expansion(),ssa.execution_view_for_root(root).unwrap()).unwrap();
        let checked=bound.bind_ssa(&ssa).expect("complete live source, allocation capture, ordered references and linear relay SSA");
        assert!(std::ptr::eq(checked.owner,&ssa));
        assert_eq!(checked.phases.len(),2);
        let [first,second]=checked.phases.as_slice() else {unreachable!()};
        assert_eq!(first.leases.len(),1);
        assert_eq!(second.leases.len(),1);
        assert_eq!(first.leases[0].allocation,second.leases[0].allocation);
        assert_eq!(first.owner_workgroup,second.owner_workgroup);
        assert_eq!(first.converted_owner,second.converted_owner);
        assert_ne!(first.owner_reference,second.owner_reference);
        assert_ne!(first.issued_phase,second.issued_phase);
        assert_ne!(first.closure_phase,second.closure_phase);
        assert_ne!(first.issued_phase,first.closure_phase);
        assert_ne!(second.issued_phase,second.closure_phase);
        assert_ne!(first.leases[0].root_reference,second.leases[0].root_reference);
        assert_ne!(first.leases[0].phase_reference,second.leases[0].phase_reference);
        assert_ne!(first.leases[0].storage_reference,second.leases[0].storage_reference);
        assert_ne!(first.leases[0].result,second.leases[0].result);
        assert_ne!(first.leases[0].close,second.leases[0].close);
        assert_ne!(first.leases[0].borrowed_at,second.leases[0].borrowed_at);
        assert_ne!(first.begin,second.begin);
        assert_ne!(first.end,second.end);
        assert_ne!(first.completion,second.completion);
        assert_ne!(first.relay,second.relay);
        let borrow_points = [first.leases[0].borrowed_at, second.leases[0].borrowed_at];
        drop(checked);
        production::tests::substitutions(&mut imported.kernel_contexts,&ssa);
        duplicate_bind_receipt::duplicate_bind_receipt(tcx,&plan,&imported.kernel_contexts,&ssa);
        unique_carrier_tests::check(&ssa, borrow_points);
        if require_emission {
            emission_source_tests::lower(imported, ssa, &typed_roots);
        }
    }
}

fn substituted_original_body_and_abi(mir:&AdmittedInertSemanticMirV1,record:SemanticDefinedReusablePhaseV1) {
    let f=&mir.functions()[record.function().index() as usize];
    let rebuild=|abi,blocks| SemanticFunctionDeclV1::new(f.identity(),f.role(),f.item_definition_identity(),
        f.monomorphization_identity(),f.generic_type_arguments_identity(),f.const_generic_arguments_identity(),
        f.source(),abi,f.locals().to_vec(),f.entry(),blocks).unwrap();
    let mut blocks=f.blocks().to_vec();
    let block=&blocks[f.entry().index() as usize];
    let mut statements=block.statements().to_vec();
    statements.push(SemanticStatementV1::new(block.source(),SemanticStatementKindV1::Nop));
    blocks[f.entry().index() as usize]=SemanticBasicBlockV1::new(block.identity(),block.source(),statements,block.terminator().clone()).unwrap();
    assert!(matches!(rebuild(f.abi().clone(),blocks).with_defined_capability_contract(
        SemanticDefinedCapabilityContractV1::ReusablePhase(record)),Err(SemanticMirErrorV1::InvalidFunctionAbi)),
        "every defined recipe must reject a changed retained body");
    let mut ownership=f.abi().source_argument_ownership().to_vec();
    ownership[0]=match ownership[0] {
        SemanticSourceArgumentOwnershipV1::ByValue=>SemanticSourceArgumentOwnershipV1::SharedBorrow,
        SemanticSourceArgumentOwnershipV1::SharedBorrow|SemanticSourceArgumentOwnershipV1::UniqueBorrow=>SemanticSourceArgumentOwnershipV1::ByValue,
        SemanticSourceArgumentOwnershipV1::Unspecified|SemanticSourceArgumentOwnershipV1::ExclusiveOwner
            |SemanticSourceArgumentOwnershipV1::RawPointer=>panic!("closed phase recipe has unexpected first-argument ownership"),
    };
    let abi=f.abi().clone().with_source_argument_ownership(ownership).unwrap();
    assert!(matches!(rebuild(abi,f.blocks().to_vec()).with_defined_capability_contract(
        SemanticDefinedCapabilityContractV1::ReusablePhase(record)),Err(SemanticMirErrorV1::InvalidFunctionAbi)),
        "every defined recipe must reject substituted source ownership/ABI");
}

fn substituted_source_binding<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    contexts: &AuthenticatedProductionKernelContextsV1,
    mir: &AdmittedInertSemanticMirV1,
    record: SemanticDefinedReusablePhaseV1,
) {
    let mut binding = *record.source_binding();
    binding[0] ^= 1;
    let mut work = SemanticMirLimitsV1::default().limit(SemanticMirResourceV1::ValidationWork);
    let changed = SemanticDefinedReusablePhaseV1::for_defined_function(
        record.function(),
        mir.functions(),
        mir.callables(),
        mir.types(),
        record.provenance(),
        binding,
        record.recipe(),
        &mut work,
    )
    .unwrap();
    let mut functions = mir.functions().to_vec();
    let f = &functions[record.function().index() as usize];
    let replacement = SemanticFunctionDeclV1::new(
        f.identity(),
        f.role(),
        f.item_definition_identity(),
        f.monomorphization_identity(),
        f.generic_type_arguments_identity(),
        f.const_generic_arguments_identity(),
        f.source(),
        f.abi().clone(),
        f.locals().to_vec(),
        f.entry(),
        f.blocks().to_vec(),
    )
    .unwrap()
    .with_defined_capability_contract(SemanticDefinedCapabilityContractV1::ReusablePhase(changed))
    .unwrap();
    functions[record.function().index() as usize] = replacement;
    let changed = InertSemanticMirRequestV1::new_with_callables(
        mir.target(),
        mir.types().to_vec(),
        mir.allocations().to_vec(),
        mir.statics().to_vec(),
        mir.vtables().to_vec(),
        functions,
        mir.callables().to_vec(),
        mir.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let result = validate_carriage(tcx, plan, contexts, &changed);
    assert!(matches!(
        &result,
        Err(ProductionSemanticImportErrorV1::KernelContextBinding(
            "phase source carriage was forged, substituted or omitted"
        ))
    ), "substituted {:?}: {result:?}", record.recipe());
}
