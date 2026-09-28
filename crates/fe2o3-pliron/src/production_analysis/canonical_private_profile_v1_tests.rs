use super::*;
use pliron::{
    builtin::attributes::{IdentifierAttr, StringAttr},
    context::{Context, Ptr},
    linked_list::ContainsLinkedList,
    operation::Operation as NativeOperation,
};

fn function(context: &Context, root: Ptr<NativeOperation>, ordinal: usize) -> Ptr<NativeOperation> {
    root.deref(context)
        .get_region(0)
        .deref(context)
        .iter(context)
        .next()
        .unwrap()
        .deref(context)
        .iter(context)
        .nth(ordinal)
        .unwrap()
}
fn operation(
    context: &Context,
    root: Ptr<NativeOperation>,
    function_ordinal: usize,
    operation_ordinal: usize,
) -> Ptr<NativeOperation> {
    function(context, root, function_ordinal)
        .deref(context)
        .get_region(0)
        .deref(context)
        .iter(context)
        .next()
        .unwrap()
        .deref(context)
        .iter(context)
        .nth(operation_ordinal)
        .unwrap()
}

#[test]
fn native_original_callee_and_generated_symbol_are_distinct_exact_joins() {
    with_projection(&fixture(), |projection, budget| {
        projection.check(budget)?;
        projection.test_live(|context, root| {
            let pointer = operation(context, root, 0, 0);
            let call =
                NativeOperation::get_op::<dialect_gpu::optimization_v1::CallOp>(pointer, context)
                    .unwrap();
            assert_eq!(
                call.get_attr_gpu_call_callee(context).unwrap().as_str(),
                "original_helper"
            );
            let native = function(context, root, 1);
            assert_ne!(native, function(context, root, 0));
        });
        Ok(())
    })
    .unwrap();
}

#[test]
fn changed_call_name_and_generated_function_symbol_are_rejected_after_epoch_rebase() {
    for rename_function in [false, true] {
        with_projection(&fixture(), |projection, budget| {
            projection.test_live(|context, root| {
                if rename_function {
                    function(context, root, 1)
                        .deref_mut(context)
                        .attributes
                        .set(
                            "sym_name".try_into().unwrap(),
                            IdentifierAttr::new("wrong_symbol".try_into().unwrap()),
                        );
                } else {
                    operation(context, root, 0, 0)
                        .deref_mut(context)
                        .attributes
                        .set(
                            "gpu_call_callee".try_into().unwrap(),
                            StringAttr::new("kir_fn_1".into()),
                        );
                }
            });
            assert!(matches!(projection.check(budget), Err(Failure::Mutation)));
            projection.test_rebase_epoch();
            assert!(projection.check(budget).is_err());
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn private_occurrence_schema_rejects_extra_attribute_even_after_epoch_rebase() {
    for (function, index) in [(0, 0), (1, 1), (1, 2), (1, 3)] {
        with_projection(&fixture(), |projection, budget| {
            projection.test_live(|context, root| {
                operation(context, root, function, index)
                    .deref_mut(context)
                    .attributes
                    .set(
                        "unproved_private_semantics".try_into().unwrap(),
                        StringAttr::new("claim".into()),
                    );
            });
            assert!(matches!(projection.check(budget), Err(Failure::Mutation)));
            projection.test_rebase_epoch();
            assert!(matches!(
                projection.check(budget),
                Err(Failure::NativeSchema)
            ));
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn mutate_restore_is_still_rejected_by_whole_context_epoch() {
    with_projection(&fixture(), |projection, budget| {
        projection.test_live(|context, root| {
            let pointer = operation(context, root, 1, 2);
            let saved = pointer.deref(context).attributes.clone();
            pointer.deref_mut(context).attributes = saved;
        });
        assert!(matches!(projection.check(budget), Err(Failure::Mutation)));
        Ok(())
    })
    .unwrap();
}

#[test]
fn private_admission_does_not_authorize_a_different_native_occurrence() {
    with_projection(&fixture(), |projection, budget| {
        projection.with_function(0, budget, |input| {
            let root = input.function().get_operation();
            assert!(input.operation(input.context(), root).is_none());
        })?;
        Ok(())
    })
    .unwrap();
}

#[test]
fn changed_memory_values_and_call_signature_are_not_authorized_by_attribute_names() {
    for change in 0..3 {
        with_projection(&fixture(), |projection, budget| {
            projection.test_live(|context, root| {
                if change == 0 {
                    operation(context, root, 1, 3)
                        .deref_mut(context)
                        .attributes
                        .set(
                            "gpu_load_volatile".try_into().unwrap(),
                            dialect_gpu::optimization_v1::VolatileAttr(true),
                        );
                } else if change == 1 {
                    operation(context, root, 1, 2)
                        .deref_mut(context)
                        .attributes
                        .set(
                            "gpu_store_alignment".try_into().unwrap(),
                            dialect_gpu::optimization_v1::MemoryAlignmentAttr(4),
                        );
                } else {
                    // A wrong attribute type is not a legal call signature.
                    operation(context, root, 0, 0)
                        .deref_mut(context)
                        .attributes
                        .set(
                            "gpu_call_signature".try_into().unwrap(),
                            StringAttr::new("same-looking-signature".into()),
                        );
                }
            });
            projection.test_rebase_epoch();
            assert!(projection.check(budget).is_err());
            Ok(())
        })
        .unwrap();
    }
}
