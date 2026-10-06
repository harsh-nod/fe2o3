use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) enum OwnerTransportFault {
    None,
    CallerDuplicate,
    CallerMixed,
    CallerTuple,
    CallerType,
    CalleeIncoming,
    CalleeProjection,
    CalleeType,
    PlanProjection,
    PlanType,
    PlanValue,
}

pub(in super::super) fn mutate_caller(
    fault: OwnerTransportFault,
    signatures: &mut BTreeMap<SemanticFunctionIdV1, LoweredFunctionSignatureV1>,
) -> bool {
    let signature = signatures.get_mut(&HELPER).unwrap();
    assert_eq!(
        selectors(&signature.call_arguments),
        [(2, None, Some(0)), (1, None, None)]
    );
    match fault {
        OwnerTransportFault::CallerDuplicate | OwnerTransportFault::CallerMixed => {
            let mut row = signature.call_arguments[1].clone();
            if fault == OwnerTransportFault::CallerMixed {
                row.component = Some(0);
            }
            signature.call_arguments.push(row);
            signature
                .parameter_types
                .push(signature.parameter_types[1].clone());
        }
        OwnerTransportFault::CallerTuple => signature.call_arguments[1].tuple_field = Some(0),
        OwnerTransportFault::CallerType => {
            signature.parameter_types[1] = Type::pointer(
                Type::Scalar(ScalarType::U64),
                AddressSpace::Global,
                AccessMode::ReadWrite,
            );
        }
        _ => return false,
    }
    true
}

pub(in super::super) fn mutate_callee(
    fault: OwnerTransportFault,
    arguments: &mut PreparedDefinedCallArgumentsV1<'_>,
    plan: &mut LoweredFunctionPlanV1,
) -> bool {
    let origin = arguments.execution.as_mut().unwrap();
    assert_eq!(
        selectors(&origin.projections),
        [(2, None, Some(0)), (1, None, None)]
    );
    match fault {
        OwnerTransportFault::CalleeIncoming => arguments.arguments[1] = arguments.arguments[0],
        OwnerTransportFault::CalleeProjection => origin.projections[1].component = Some(0),
        OwnerTransportFault::CalleeType => {
            origin.parameter_types[1] = Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::ReadOnly,
            );
        }
        OwnerTransportFault::PlanProjection => plan.call_arguments[1].source_argument = 2,
        OwnerTransportFault::PlanType => {
            plan.parameter_types[1] = Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Workgroup,
                AccessMode::ReadWrite,
            );
        }
        OwnerTransportFault::PlanValue => plan.parameter_values[1] = plan.parameter_values[0],
        _ => return false,
    }
    true
}

pub(in super::super) fn check_reconstructed_owner(
    parameters: &PreparedExecutionParametersV29<'_>,
    plan: &LoweredFunctionPlanV1,
    incoming: ValueId,
) {
    let (local, binding) = parameters
        .locals
        .iter()
        .find(|(local, _)| *local == 6)
        .unwrap();
    assert_eq!(*local, 6);
    let SemanticValueBindingV1::Value { id, ty } = binding else {
        panic!("whole owner");
    };
    assert_ne!(*id, incoming);
    assert_eq!(*id, plan.parameter_values[1]);
    assert_eq!(ty, &plan.parameter_types[1]);
    assert_eq!(
        *ty,
        Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadWrite
        )
    );
    assert_eq!(parameters.values[1].id, *id);
    assert_eq!(&parameters.values[1].ty, ty);
}

thread_local! {
    static OWNER_LIFECYCLE_INPUT_V29: std::cell::RefCell<Option<Vec<usize>>> = const { std::cell::RefCell::new(None) };
    static OWNER_LIFECYCLE_OBSERVED_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static OWNER_LIFECYCLE_ASSEMBLED_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn inspect_original_owner_v29(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    _slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    assert_eq!(emitted.len(), 3);
    let helper = (0..instances.instances().len())
        .find(|&index| {
            instances
                .instance(instances.id_at(index).unwrap())
                .unwrap()
                .function()
                == HELPER
        })
        .unwrap();
    let id = instances.id_at(helper).unwrap();
    let callee = emitted[helper].as_ref().unwrap();
    let incoming = instances.incoming(id).unwrap().occurrence();
    let caller = emitted[incoming.caller.index()].as_ref().unwrap();
    let mut calls = caller
        .function
        .body
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            OperationKind::Call {
                callee: target,
                arguments,
            } if target == &callee.function.id => Some(arguments),
            _ => None,
        });
    let arguments = calls.next().expect("one actual owner transport call");
    assert!(calls.next().is_none());
    let signature = execution_function_signature_v29(instances, id, budget)?;
    assert_eq!(
        selectors(&signature.call_arguments),
        [(2, None, Some(0)), (1, None, None)]
    );
    let body = callee.function.body.as_ref().unwrap();
    assert_eq!(
        (
            arguments.len(),
            body.parameters.len(),
            callee.function.signature.parameters.len()
        ),
        (2, 2, 2)
    );
    let archive = callee
        .execution_observation
        .as_ref()
        .expect("original helper entry archive");
    let SemanticValueBindingV1::Value { id: value, ty } = archive.locals[6].as_ref().unwrap()
    else {
        panic!("whole owner must survive the original helper entry");
    };
    assert_ne!(*value, arguments[1]);
    assert_eq!(*value, body.parameters[1]);
    assert_eq!(ty, &callee.function.signature.parameters[1]);
    assert_eq!(
        *ty,
        Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadWrite
        )
    );
    let rows = emitted
        .iter()
        .map(|row| {
            row.as_ref()
                .unwrap()
                .lifecycle_events
                .as_ref()
                .unwrap()
                .rows
                .as_ptr() as usize
        })
        .collect();
    assert!(OWNER_LIFECYCLE_INPUT_V29.replace(Some(rows)).is_none());
    OWNER_LIFECYCLE_OBSERVED_V29.set(OWNER_LIFECYCLE_OBSERVED_V29.get() + 1);
    Ok(())
}

fn inspect_original_owner_assembly_v29(
    pending: &mut PendingScopedRootEmissionV29,
    _instances: &ExecutionInstancesV29<'_>,
    _plan: &SourceReferencePlanV29<'_, '_>,
    _budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let before = OWNER_LIFECYCLE_INPUT_V29
        .take()
        .expect("this original candidate's lifecycle rows");
    assert_eq!(pending.sidecars.rows.len(), before.len());
    let mut events = 0;
    for (sidecar, before) in pending.sidecars.rows.iter().zip(before) {
        let rows = sidecar.lifecycle_events.as_ref().unwrap();
        assert_eq!(rows.rows.as_ptr() as usize, before);
        assert_eq!(
            rows.retained_storage,
            rows.rows.capacity() * std::mem::size_of::<DeferredLifecycleEventV29>()
        );
        events += rows.rows.len();
    }
    assert_eq!(events, 3);
    OWNER_LIFECYCLE_ASSEMBLED_V29.set(OWNER_LIFECYCLE_ASSEMBLED_V29.get() + 1);
    Ok(())
}

fn run_original_owner_v29(
    work: usize,
    storage: usize,
) -> (SourceOwnedResultV18<()>, usize, usize, bool) {
    struct Restore(Option<RootExecutionArchiveObserverV29>);
    impl Drop for Restore {
        fn drop(&mut self) {
            ROOT_EXECUTION_ARCHIVE_OBSERVER_V29.set(self.0);
            OWNER_LIFECYCLE_INPUT_V29.take();
        }
    }
    OWNER_LIFECYCLE_OBSERVED_V29.set(0);
    OWNER_LIFECYCLE_ASSEMBLED_V29.set(0);
    assert!(OWNER_LIFECYCLE_INPUT_V29.take().is_none());
    let _restore = Restore(
        ROOT_EXECUTION_ARCHIVE_OBSERVER_V29.replace(Some(inspect_original_owner_assembly_v29)),
    );
    super::super::scoped_root_tests::source_slot_tests::run_original_source_fixture_v29(
        || owner_parameter_fixture(OwnerParameterCase::Valid).0,
        false,
        false,
        0,
        inspect_original_owner_v29,
        work,
        storage,
    )
}

#[test]
fn owner_parameter_crosses_the_real_source_lifecycle_boundary() {
    let (result, work, peak, completed) = run_original_owner_v29(10_000_000, 10_000_000);
    result.unwrap();
    assert!(completed);
    assert_eq!(
        (
            OWNER_LIFECYCLE_OBSERVED_V29.get(),
            OWNER_LIFECYCLE_ASSEMBLED_V29.get()
        ),
        (3, 3)
    );
    let (result, exact_work, exact_peak, completed) = run_original_owner_v29(work, peak);
    result.unwrap();
    assert!(completed);
    assert_eq!((exact_work, exact_peak), (work, peak));
    assert_eq!(
        (
            OWNER_LIFECYCLE_OBSERVED_V29.get(),
            OWNER_LIFECYCLE_ASSEMBLED_V29.get()
        ),
        (3, 3)
    );
    let (result, _, _, completed) = run_original_owner_v29(work - 1, peak);
    assert!(!completed);
    assert!(matches!(
        super::super::scoped_root_tests::source_slot_tests::original_repeated_source_resource_v29(
            result.unwrap_err()
        ),
        ArgumentResourceV1::Work(_)
    ));
    let (result, _, _, completed) = run_original_owner_v29(work, peak - 1);
    assert!(!completed);
    let ArgumentResourceV1::Storage(limit) =
        super::super::scoped_root_tests::source_slot_tests::original_repeated_source_resource_v29(
            result.unwrap_err(),
        )
    else {
        panic!("exact original-owner storage refusal required");
    };
    assert_eq!(limit.actual(), peak);
    assert_eq!(limit.limit(), peak - 1);
}

#[test]
fn owner_parameter_rejects_actual_caller_and_callee_substitutions() {
    for fault in [
        OwnerTransportFault::CallerDuplicate,
        OwnerTransportFault::CallerMixed,
        OwnerTransportFault::CallerTuple,
        OwnerTransportFault::CallerType,
        OwnerTransportFault::CalleeIncoming,
        OwnerTransportFault::CalleeProjection,
        OwnerTransportFault::CalleeType,
        OwnerTransportFault::PlanProjection,
        OwnerTransportFault::PlanType,
        OwnerTransportFault::PlanValue,
    ] {
        let (result, _, _) =
            run_lifecycle(false, Fault::OwnerParameter(fault), 10_000_000, 10_000_000);
        assert!(
            matches!(
                result,
                Err(ProductionSemanticKirErrorV1::Unsupported { .. })
            ),
            "{fault:?}: {result:?}"
        );
    }
}

#[test]
fn owner_parameter_borrowed_assembly_keeps_the_consuming_physical_gate() {
    let (result, _, _) = run_lifecycle(
        false,
        Fault::OwnerParameter(OwnerTransportFault::None),
        10_000_000,
        10_000_000,
    );
    assert!(matches!(
        result,
        Err(ProductionSemanticKirErrorV1::Unsupported {
            detail: "original raw source requires consuming expanded physical admission",
            ..
        })
    ));
}
