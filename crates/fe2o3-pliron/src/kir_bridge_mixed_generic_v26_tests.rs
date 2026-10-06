use super::*;

fn generic_fixture(origin: Option<AddressSpace>) -> Module {
    let mut module = fixture();
    let scalar = Type::Scalar(ScalarType::U32);
    let generic = Type::pointer(scalar.clone(), AddressSpace::Generic, AccessMode::ReadWrite);
    let function = &mut module.functions[0];
    if let Some(space) = origin {
        function
            .signature
            .parameters
            .push(Type::pointer(scalar, space, AccessMode::ReadWrite));
        function.body.as_mut().unwrap().parameters.push(ValueId(
            if space == AddressSpace::Generic {
                13
            } else {
                14
            },
        ));
    }
    let block = &mut function.body.as_mut().unwrap().blocks[1];
    for operation in &mut block.operations {
        match &mut operation.kind {
            OperationKind::Load { pointer, access }
            | OperationKind::Store {
                pointer, access, ..
            } => {
                *pointer = ValueId(13);
                access.address_space = AddressSpace::Generic;
            }
            _ => (),
        }
    }
    if origin != Some(AddressSpace::Generic) {
        block.operations.insert(
            2,
            value(
                13,
                generic.clone(),
                OperationKind::Cast {
                    kind: fe2o3_kernel_ir::CastKind::PointerToGeneric,
                    value: ValueId(if origin.is_some() { 14 } else { 6 }),
                    to: generic,
                },
            ),
        );
    }
    module
}

#[test]
fn mixed_native_generic_representation_requires_checked_global_slice_origin() {
    for module in [fixture(), generic_fixture(None)] {
        let (result, _, _, calls) = run_module_case(&module, AMPLE, AMPLE, 0, |input| {
            assert_eq!(input.global_counts_v26(), [1, 1]);
            let outcome = crate::canonical_private_v1::run_mixed_v26(
                input,
                crate::ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
                None,
            )
            .unwrap();
            assert!(outcome.report.reports().is_clean());
            assert!(!outcome.report.grants_artifact_or_launch_authority());
            for stage in 0..9 {
                assert_eq!(outcome.report.global_access_counts(stage), Some([1, 1]));
            }
            Ok(())
        });
        result.unwrap();
        assert_eq!(calls, 1);
    }
}

#[test]
fn mixed_native_generic_private_workgroup_and_unknown_origins_cannot_reach_consumer() {
    for origin in [
        AddressSpace::Private,
        AddressSpace::Workgroup,
        AddressSpace::Generic,
    ] {
        let module = generic_fixture(Some(origin));
        let (result, _, _, calls) = run_module_case(&module, AMPLE, AMPLE, 0, |_| {
            panic!("unproved Generic origin reached native consumer")
        });
        assert!(
            matches!(result, Err(Failure::NativeSchema | Failure::ExactGraph)),
            "expected typed origin/schema refusal for {origin:?}: {result:?}"
        );
        assert_eq!(calls, 0);
    }
}
