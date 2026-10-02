use super::super::super::resource_tests::argument_correspondence_tests::native_helper_argument_owner;
use super::*;

#[test]
fn checked_physical_slots_cover_tuples_zst_and_both_rust_call_forms() {
    for shape in [None, Some(false), Some(true)] {
        let owner = native_helper_argument_owner(shape);
        let module = owner.executable.module();
        let inputs = if shape.is_none() {
            vec![constant(7), constant(11)]
        } else {
            vec![constant(7), constant(11), constant(17)]
        };
        let expected = inputs.last().unwrap().clone();
        for root in owner.semantic_ssa.source_semantic().roots() {
            let association = owner
                .correspondence
                .lowered_functions
                .iter()
                .find(|row| row.correspondence_owner == *root && row.semantic_function == *root)
                .unwrap();
            let entry = module
                .functions
                .iter()
                .find(|function| function.id == association.kernel_ir_function)
                .unwrap();
            for block in &entry.body.as_ref().unwrap().blocks {
                for (ordinal, operation) in block.operations.iter().enumerate() {
                    let OperationKind::Call { callee, arguments } = &operation.kind else {
                        continue;
                    };
                    let helper = module
                        .functions
                        .iter()
                        .find(|function| function.id == *callee)
                        .unwrap();
                    assert_eq!(helper.body.as_ref().unwrap().blocks.len(), 1);
                    assert_eq!(arguments.len(), inputs.len());
                    assert_eq!(helper.signature.parameters.len(), arguments.len());
                    if shape.is_some() {
                        assert_eq!(arguments[0], arguments[2]);
                        assert_ne!(
                            helper.body.as_ref().unwrap().parameters[0],
                            helper.body.as_ref().unwrap().parameters[2]
                        );
                    }
                    let location = FunctionOperationLocation::new(block.id, ordinal);
                    let probe = |meter: &mut TestMeter<'_, '_>, _: &Cell<bool>| {
                        with_native_helper_values(
                            &owner.semantic_ssa,
                            module,
                            &owner.correspondence,
                            *root,
                            entry,
                            meter,
                            |context, meter| {
                                let template =
                                    context.root_call(entry, location, operation, meter)?;
                                let (expression, bytes) = template.instantiate(&inputs, meter)?;
                                assert_eq!(expression, expected);
                                drop(expression);
                                meter.release(bytes)?;
                                Ok(())
                            },
                        )
                    };
                    let (result, floor, work, peak, _) = run(10_000_000, 32 * 1024 * 1024, probe);
                    result.unwrap();
                    assert_eq!(floor, 4096);
                    let exact = run(work, peak, probe);
                    exact.0.unwrap();
                    assert_eq!(exact.1, 4096);
                    for (work, storage) in [(work - 1, peak), (work, peak - 1)] {
                        let short = run(work, storage, probe);
                        assert!(short.0.is_err());
                        assert_eq!(short.1, 4096);
                    }
                }
            }
        }
    }
}
