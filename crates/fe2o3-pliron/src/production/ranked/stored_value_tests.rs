use super::*;

#[test]
fn materialization_preserves_scalar_and_tensor_rhs_for_plain_and_atomic_writes() {
    type O = ProductionRankedOperationV1;
    let id = ProductionRankedValueIdV1::new;
    let local = |value| ProductionRankedValueV1::Local(id(value));
    for tensor in [false, true] {
        let scalar = ProductionSemanticScalarTypeV2::Float { bits: 32 };
        let expression = ProductionSemanticExpressionV2::Constant { scalar, bits: 0 };
        let numerical_contract = ProductionNumericalContractV2::exact_for_expression(&expression);
        let root = DigestV1::from_untrusted_bytes([19; 32]);
        let producer = if tensor {
            O::TensorResultComponent {
                result: id(2),
                tensor_result_root: root,
                component: 0,
                scalar,
                numerical_contract,
            }
        } else {
            O::SemanticExpression {
                result: id(2),
                expression,
                numerical_contract,
            }
        };
        let kernel = ProductionRankedKernelV1::new(
            "stored_values",
            0,
            vec![ProductionRankedBlockV1::new(
                vec![
                    O::View {
                        result: id(0),
                        element_width: 32,
                        writable: true,
                        shape: vec![8],
                        dynamic_extents: vec![],
                        allocation_origin: 17,
                        noalias_class: 17,
                    },
                    O::IndexConstant {
                        result: id(1),
                        value: 0,
                    },
                    producer,
                    O::ValueAccess {
                        kind: AccessKindAttr::Write,
                        view: local(0),
                        indices: vec![local(1)],
                        value: local(2),
                    },
                    O::AtomicValueAccess {
                        kind: AccessKindAttr::AtomicWrite,
                        ordering: AtomicOrderingAttr::Release,
                        scope: AtomicScopeAttr::Device,
                        view: local(0),
                        indices: vec![local(1)],
                        value: local(2),
                    },
                ],
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .unwrap();
        let mut session = ProductionPlironSessionV1::new(
            ProductionSessionLimitsV1::default(),
            [
                dialect_kernel::dialect_registration().unwrap(),
                dialect_gpu::dialect_registration().unwrap(),
                dialect_proof::dialect_registration().unwrap(),
            ],
        )
        .unwrap();
        // Inspect the live production materializer, without creating proof evidence.
        let materialized = session
            .materialize_ranked_kernel("stored_values", kernel, vec![])
            .unwrap();
        let function = FuncOp::from_operation(materialized.ranked_function.unwrap());
        let context = &session.inner.context;
        let operations: Vec<_> = function
            .get_entry_block(context)
            .deref(context)
            .iter(context)
            .collect();
        let producer = operations
            .iter()
            .copied()
            .find(|op| {
                if tensor {
                    Operation::is_op::<TensorResultComponentOp>(*op, context)
                } else {
                    Operation::is_op::<SemanticTypedExpressionRootOp>(*op, context)
                }
            })
            .unwrap();
        let value = producer.deref(context).get_result(0);
        let accesses: Vec<_> = operations
            .iter()
            .copied()
            .filter(|op| Operation::is_op::<RankedAccessOp>(*op, context))
            .map(RankedAccessOp::from_operation)
            .collect();
        assert_eq!(accesses.len(), 2);
        for access in &accesses {
            pliron::op::verify_op(access, context).unwrap();
            assert_eq!(access.stored_value(context), Some(value));
            assert_eq!(access.indices(context).len(), 1);
            assert_eq!(access.get_operation().deref(context).get_num_operands(), 3);
        }
        assert_eq!(
            accesses[1].atomic_ordering(context),
            Some(AtomicOrderingAttr::Release)
        );
        assert_eq!(
            accesses[1].atomic_scope(context),
            Some(AtomicScopeAttr::Device)
        );
    }
}
