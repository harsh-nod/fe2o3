//! Genuine shared-body vecadd observations; pending replay is not source refinement.
use super::*;
use fe2o3_kernel_ir::{
    AccessMode, BinaryOp, ComparePredicate, Function, MemoryAccess, Terminator, Type, ValueId,
};

const CHILD_PREFIX: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::pending_source_tests::vecadd_tests::";
const SOURCE: &str =
    include_str!("../tests/fixtures/production-ranked-bounds-device/context_vecadd.rs");

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct RootObservation {
    source: [u8; 32],
    physical_arguments: usize,
    logical_arguments: usize,
    workgroup: [u32; 3],
}

fn inspect_root(
    root: ProductionCheckedContextRootV29<'_>,
    budget: &mut Budget<'_>,
) -> Result<RootObservation, ProductionContextRootErrorV29> {
    budget.charge_work(64)?;
    budget.reserve_storage(std::mem::size_of::<RootObservation>())?;
    let source = root.semantic_ssa().source_semantic();
    assert_eq!(source.wire_version(), SemanticMirWireVersionV1::V29);
    let physical = root.root().abi();
    let logical = root.helper().abi();
    assert_eq!(physical.source_input_types().len(), 3);
    assert_eq!(physical.arguments().len(), 3);
    assert!(physical.hidden_arguments().is_empty());
    assert_eq!(physical.adjusted_arguments().len(), 3);
    assert_eq!(logical.source_input_types().len(), 4);
    assert_eq!(logical.arguments().len(), 4);
    assert!(logical.hidden_arguments().is_empty());
    assert_eq!(logical.adjusted_arguments().len(), 4);
    assert_eq!(logical.source_input_types()[0], root.context_type());
    assert_eq!(
        source.types()[root.context_type().index() as usize]
            .layout()
            .size_bytes(),
        Some(0)
    );
    assert!(matches!(
        logical.adjusted_arguments()[0].mode(),
        SemanticAbiPassModeV1::Ignore
    ));
    assert_eq!(
        &logical.source_input_types()[1..],
        physical.source_input_types()
    );
    assert_eq!(
        &logical.adjusted_arguments()[1..],
        physical.adjusted_arguments()
    );
    assert_eq!(
        physical.source_argument_ownership(),
        &[
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
            SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
        ]
    );
    assert_eq!(
        &logical.source_argument_ownership()[1..],
        physical.source_argument_ownership()
    );
    assert!(root.issuance().arguments().is_empty());
    assert_eq!(root.helper_call().arguments().len(), 4);
    assert_eq!(
        root.launch().source_launch().exact_workgroup(),
        Some([256, 1, 1])
    );
    Ok(RootObservation {
        source: *source.semantic_sha256().as_bytes(),
        physical_arguments: physical.source_input_types().len(),
        logical_arguments: logical.source_input_types().len(),
        workgroup: root.launch().source_launch().exact_workgroup().unwrap(),
    })
}

// Function/block/operation ordinals refer only to this pending graph identity.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
struct OperationSite {
    function: usize,
    block: usize,
    operation: usize,
    result: Option<u32>,
    operands: [Option<u32>; 3],
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
struct BranchSite {
    function: usize,
    block: usize,
    condition: u32,
    then_target: u32,
    else_target: u32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
enum AccessRole {
    ReadOnly,
    WriteOnly,
    ReadWrite,
}

fn global_f32_access(ty: &Type) -> AccessRole {
    let (element, space, access) = match ty {
        Type::Pointer(pointer) => (
            pointer.pointee.as_ref(),
            pointer.address_space,
            pointer.access,
        ),
        Type::Slice(slice) => (slice.element.as_ref(), slice.address_space, slice.access),
        _ => panic!("expected a typed vecadd address, found {ty:?}"),
    };
    assert_eq!(element, &Type::F32);
    assert_eq!(space, AddressSpace::Global);
    match access {
        AccessMode::ReadOnly => AccessRole::ReadOnly,
        AccessMode::WriteOnly => AccessRole::WriteOnly,
        AccessMode::ReadWrite => AccessRole::ReadWrite,
    }
}

fn resource(error: ResourceError) -> PipelineError {
    PipelineError::ContextHandoff(error.into())
}

fn value_type<'a>(
    function: &'a Function,
    value: ValueId,
    budget: &mut Budget<'_>,
) -> Result<&'a Type, PipelineError> {
    let body = function.body.as_ref().expect("observed function body");
    for (id, ty) in body
        .parameters
        .iter()
        .copied()
        .zip(&function.signature.parameters)
    {
        budget.charge_work(1).map_err(resource)?;
        if id == value {
            return Ok(ty);
        }
    }
    for block in &body.blocks {
        budget.charge_work(1).map_err(resource)?;
        for definition in &block.parameters {
            budget.charge_work(1).map_err(resource)?;
            if definition.id == value {
                return Ok(&definition.ty);
            }
        }
        for operation in &block.operations {
            budget.charge_work(1).map_err(resource)?;
            for definition in &operation.results {
                budget.charge_work(1).map_err(resource)?;
                if definition.id == value {
                    return Ok(&definition.ty);
                }
            }
        }
    }
    panic!("observed SSA value {value:?} has no type");
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
struct SliceParameter {
    function: usize,
    position: usize,
    value: u32,
    access: AccessRole,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
struct MemorySite {
    site: OperationSite,
    pointer_access: AccessRole,
    alignment: u32,
    volatile: bool,
}

fn memory_site(
    site: OperationSite,
    definition: &Function,
    pointer: ValueId,
    access: &MemoryAccess,
    budget: &mut Budget<'_>,
) -> Result<MemorySite, PipelineError> {
    let ty = value_type(definition, pointer, budget)?;
    assert!(matches!(ty, Type::Pointer(_)));
    assert_eq!(access.address_space, AddressSpace::Global);
    Ok(MemorySite {
        site,
        pointer_access: global_f32_access(ty),
        alignment: access.alignment,
        volatile: access.volatile,
    })
}

// These are symbolic SSA addresses/extents, not runtime addresses or guard proofs.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
enum AddressOrBound {
    SliceLength(AccessRole),
    SliceData(AccessRole),
    ElementPointer(AccessRole),
    LessThan,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
struct AddressOrBoundSite {
    site: OperationSite,
    kind: AddressOrBound,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct VecaddPendingObservation {
    pending: PendingObservation,
    slices: [Option<SliceParameter>; 32],
    slice_count: usize,
    loads: [Option<MemorySite>; 2],
    add: Option<OperationSite>,
    store: Option<MemorySite>,
    addresses_and_bounds: [Option<AddressOrBoundSite>; 32],
    address_or_bound_count: usize,
    branches: [Option<BranchSite>; 32],
    branch_count: usize,
}

fn inspect_vecadd(
    owner: &Pending,
    budget: &mut Budget<'_>,
) -> Result<VecaddPendingObservation, PipelineError> {
    budget
        .reserve_storage(std::mem::size_of::<VecaddPendingObservation>())
        .map_err(resource)?;
    let mut observation = VecaddPendingObservation {
        pending: inspect(owner, budget)?,
        slices: [None; 32],
        slice_count: 0,
        loads: [None; 2],
        add: None,
        store: None,
        addresses_and_bounds: [None; 32],
        address_or_bound_count: 0,
        branches: [None; 32],
        branch_count: 0,
    };
    let mut loads = 0;
    let mut lengths = 0;
    for (function, definition) in owner.pending_module().functions.iter().enumerate() {
        budget.charge_work(1).map_err(resource)?;
        let Some(body) = &definition.body else {
            continue;
        };
        for (position, (id, ty)) in body
            .parameters
            .iter()
            .zip(&definition.signature.parameters)
            .enumerate()
        {
            budget.charge_work(1).map_err(resource)?;
            if matches!(ty, Type::Slice(_)) {
                let slot = observation
                    .slices
                    .get_mut(observation.slice_count)
                    .ok_or_else(|| resource(ResourceError::Arithmetic))?;
                *slot = Some(SliceParameter {
                    function,
                    position,
                    value: id.0,
                    access: global_f32_access(ty),
                });
                observation.slice_count += 1;
            }
        }
        for (block, basic) in body.blocks.iter().enumerate() {
            budget.charge_work(1).map_err(resource)?;
            if let Some(Terminator::ConditionalBranch {
                condition,
                then_target,
                else_target,
                ..
            }) = &basic.terminator
            {
                let slot = observation
                    .branches
                    .get_mut(observation.branch_count)
                    .ok_or_else(|| resource(ResourceError::Arithmetic))?;
                *slot = Some(BranchSite {
                    function,
                    block,
                    condition: condition.0,
                    then_target: then_target.0,
                    else_target: else_target.0,
                });
                observation.branch_count += 1;
            }
            for (operation, value) in basic.operations.iter().enumerate() {
                budget.charge_work(8).map_err(resource)?;
                let result = value.results.first().map(|result| result.id.0);
                let f32_result = value.results.len() == 1 && value.results[0].ty == Type::F32;
                let site = |operands| OperationSite {
                    function,
                    block,
                    operation,
                    result,
                    operands,
                };
                let address_or_bound = match &value.kind {
                    OperationKind::SliceLength { slice } => {
                        assert_eq!(value.results.len(), 1);
                        assert_eq!(value.results[0].ty, Type::INDEX);
                        lengths += 1;
                        Some(AddressOrBoundSite {
                            site: site([Some(slice.0), None, None]),
                            kind: AddressOrBound::SliceLength(global_f32_access(value_type(
                                definition, *slice, budget,
                            )?)),
                        })
                    }
                    OperationKind::SliceData { slice } => Some(AddressOrBoundSite {
                        site: site([Some(slice.0), None, None]),
                        kind: AddressOrBound::SliceData(global_f32_access(value_type(
                            definition, *slice, budget,
                        )?)),
                    }),
                    OperationKind::GetElementPointer { base, offset } => {
                        assert_eq!(value.results.len(), 1);
                        Some(AddressOrBoundSite {
                            site: site([Some(base.0), Some(offset.0), None]),
                            kind: AddressOrBound::ElementPointer(global_f32_access(
                                &value.results[0].ty,
                            )),
                        })
                    }
                    OperationKind::Compare {
                        predicate: ComparePredicate::LessThan,
                        lhs,
                        rhs,
                    } => Some(AddressOrBoundSite {
                        site: site([Some(lhs.0), Some(rhs.0), None]),
                        kind: AddressOrBound::LessThan,
                    }),
                    _ => None,
                };
                if let Some(address_or_bound) = address_or_bound {
                    let slot = observation
                        .addresses_and_bounds
                        .get_mut(observation.address_or_bound_count)
                        .ok_or_else(|| resource(ResourceError::Arithmetic))?;
                    *slot = Some(address_or_bound);
                    observation.address_or_bound_count += 1;
                }
                match &value.kind {
                    OperationKind::Load { pointer, access }
                        if access.address_space == AddressSpace::Global =>
                    {
                        assert!(f32_result, "vecadd global load has the wrong element type");
                        assert!(loads < 2, "extra vecadd global load");
                        observation.loads[loads] = Some(memory_site(
                            site([Some(pointer.0), None, None]),
                            definition,
                            *pointer,
                            access,
                            budget,
                        )?);
                        loads += 1;
                    }
                    OperationKind::GuardedLoad {
                        pointer,
                        predicate,
                        fallback,
                        access,
                    } if access.address_space == AddressSpace::Global => {
                        assert!(f32_result, "vecadd guarded load has the wrong element type");
                        assert!(loads < 2, "extra vecadd global load");
                        observation.loads[loads] = Some(memory_site(
                            site([Some(pointer.0), Some(predicate.0), Some(fallback.0)]),
                            definition,
                            *pointer,
                            access,
                            budget,
                        )?);
                        loads += 1;
                    }
                    OperationKind::Binary {
                        op: BinaryOp::Add,
                        lhs,
                        rhs,
                    } if f32_result => {
                        assert!(
                            observation
                                .add
                                .replace(site([Some(lhs.0), Some(rhs.0), None]))
                                .is_none(),
                            "extra f32 add"
                        );
                    }
                    OperationKind::Store {
                        pointer,
                        value,
                        access,
                    } if access.address_space == AddressSpace::Global => {
                        assert_eq!(value_type(definition, *value, budget)?, &Type::F32);
                        assert!(
                            observation
                                .store
                                .replace(memory_site(
                                    site([Some(pointer.0), Some(value.0), None]),
                                    definition,
                                    *pointer,
                                    access,
                                    budget,
                                )?)
                                .is_none(),
                            "extra global store"
                        );
                    }
                    OperationKind::GuardedStore {
                        pointer,
                        predicate,
                        value,
                        access,
                    } if access.address_space == AddressSpace::Global => {
                        assert_eq!(value_type(definition, *value, budget)?, &Type::F32);
                        assert!(
                            observation
                                .store
                                .replace(memory_site(
                                    site([Some(pointer.0), Some(value.0), Some(predicate.0)]),
                                    definition,
                                    *pointer,
                                    access,
                                    budget,
                                )?)
                                .is_none(),
                            "extra global store"
                        );
                    }
                    _ => {}
                }
            }
        }
    }
    assert_eq!(loads, 2);
    assert!(observation.add.is_some() && observation.store.is_some());
    for load in observation.loads.iter().flatten() {
        assert_eq!(load.pointer_access, AccessRole::ReadOnly);
    }
    assert_eq!(
        observation.store.unwrap().pointer_access,
        AccessRole::ReadWrite
    );
    assert!(observation.slice_count >= 3);
    assert!(
        lengths >= 3,
        "symbolic input/output extents must be observed"
    );
    assert!(
        observation.branch_count > 0,
        "expected conditional control in the shared body"
    );
    assert!(
        observation.pending.assertion_attachments >= 2,
        "expected at least two retained source assertion attachments"
    );
    Ok(observation)
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
enum Observation {
    Root(RootObservation),
    Pending(Box<VecaddPendingObservation>),
}

struct VecaddCallbacks {
    pending: bool,
    result: Option<Result<Observation, String>>,
}

impl Callbacks for VecaddCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            let mut observation = None;
            if self.pending {
                let result = transaction.observe_pending_scoped_source_v29(|owner, budget| {
                    assert!(observation.is_none());
                    observation = Some(Observation::Pending(Box::new(inspect_vecadd(
                        owner, budget,
                    )?)));
                    Ok(())
                });
                if !matches!(result, Err(ref error) if matches!(**error, PipelineError::PendingScopedObservationIncomplete))
                {
                    return Err(format!(
                        "vecadd pending observation did not reach its terminal sentinel: {result:?}"
                    ));
                }
            } else {
                let result = transaction.observe_context_handoff_v29(|root, budget| {
                    assert!(observation.is_none());
                    observation = Some(Observation::Root(inspect_root(root, budget)?));
                    Ok(())
                });
                if !matches!(result, Err(ref error) if matches!(**error,
                    PipelineError::PreRankedMaterialization(fe2o3_lower_mir_kernel::ProductionPreRankedKirErrorV1::Lowering(
                        fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::Unsupported {
                            detail: "execution capabilities require checked canonical KIR materialization", ..
                        }
                    ))
                )) {
                    return Err(format!(
                        "vecadd stopped at an unexpected production boundary: {result:?}"
                    ));
                }
            }
            observation
                .ok_or_else(|| "vecadd refused before the requested source observation".into())
        })());
        Compilation::Stop
    }
}

fn child(pending: bool) {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = VecaddCallbacks {
        pending,
        result: None,
    };
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("vecadd source callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("vecadd observation path"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "vecadd source observation: {result:?}");
}

#[test]
#[ignore = "process helper; requires the parent's actual-source request"]
fn context_vecadd_root_child() {
    child(false);
}

#[test]
#[ignore = "process helper; requires the parent's actual-source request"]
fn context_vecadd_pending_child() {
    child(true);
}

fn run(pending: bool) {
    let child = format!(
        "{CHILD_PREFIX}context_vecadd_{}_child",
        if pending { "pending" } else { "root" }
    );
    run_actual_sources::<Observation>(
        &[("vecadd", ""), ("vecadd", "")],
        &[(0, 0), (0, 2), (3, 0), (3, 2)],
        &child,
        "CONTEXT_VECADD_SOURCE_OBSERVATION",
        |_| SOURCE.to_owned(),
        |_, _, label, observation, previous| {
            match &observation {
                Observation::Root(root) => {
                    assert!(!pending);
                    assert_eq!((root.physical_arguments, root.logical_arguments), (3, 4));
                    assert_eq!(root.workgroup, [256, 1, 1]);
                }
                Observation::Pending(value) => {
                    assert!(pending);
                    assert_eq!(value.pending.kernels, 1);
                    assert_eq!(value.pending.context_issues, 1);
                    assert_eq!((value.pending.derives, value.pending.scope_ends), (0, 0));
                    assert_eq!(value.pending.global_stores, 1);
                    assert!(value.pending.replay_storage_stable);
                }
            }
            if let Some(previous) = previous.get(label) {
                assert_eq!(
                    &observation, previous,
                    "fresh-process exact source/graph/site replay"
                );
            } else {
                previous.insert(label.to_owned(), observation);
            }
        },
    );
}

#[test]
#[ignore = "requires pinned nightly rust-src and authentic AMD source dependencies"]
fn actual_context_vecadd_preserves_three_physical_arguments_then_refuses() {
    run(false);
}

#[test]
#[ignore = "requires pinned nightly rust-src and authentic AMD source dependencies; pending only"]
fn actual_context_vecadd_observes_pending_memory_arithmetic_and_control() {
    run(true);
}
