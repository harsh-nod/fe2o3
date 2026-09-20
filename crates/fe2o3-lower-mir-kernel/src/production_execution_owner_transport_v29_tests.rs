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

#[test]
fn owner_parameter_crosses_the_real_source_lifecycle_boundary() {
    let (result, work, peak) = run_lifecycle(
        false,
        Fault::OwnerParameter(OwnerTransportFault::None),
        10_000_000,
        10_000_000,
    );
    assert_eq!(result.unwrap().len(), 3);
    assert!(
        run_lifecycle(
            false,
            Fault::OwnerParameter(OwnerTransportFault::None),
            work,
            peak
        )
        .0
        .is_ok()
    );
    assert!(matches!(
        run_lifecycle(
            false,
            Fault::OwnerParameter(OwnerTransportFault::None),
            work - 1,
            peak
        )
        .0,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(
                _
            ))
        )
    ));
    let short_storage = run_lifecycle(
        false,
        Fault::OwnerParameter(OwnerTransportFault::None),
        work,
        peak - 1,
    )
    .0;
    let Err(ProductionSemanticKirErrorV1::AssertOrigin(SemanticKirAssertOriginErrorV1::Resource(
        ArgumentResourceV1::Storage(limit),
    ))) = short_storage
    else {
        panic!("{short_storage:?}");
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
