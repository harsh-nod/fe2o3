use super::*;

pub(crate) fn try_verify_execution_operation_with_sink_v3<S: SemanticOperationIssueSinkV1>(
    operation: &ExecutionOperationV15,
    context: SemanticOperationBorrowedVerificationContextV1<'_>,
    sink: &mut S,
) -> Result<(), S::Error> {
    verify_execution_types_v3(
        operation,
        context.operands,
        context.results,
        context.operand_types,
        sink,
    )
}

impl ExecutionOperationV15 {
    /// Local structure only; module verification additionally checks lifecycle.
    pub fn verify_v3(
        &self,
        context: SemanticOperationVerificationContext<'_>,
    ) -> Vec<SemanticOperationIssue> {
        let mut issues = Vec::new();
        let mut sink = LegacySemanticOperationIssueSinkV1 {
            issues: &mut issues,
        };
        if let Err(never) = verify_execution_types_v3(
            self,
            context.operands,
            context.results,
            context.operand_types,
            &mut sink,
        ) {
            match never {}
        }
        issues
    }
}

fn verify_execution_types_v3<
    T: SemanticOperandTypeViewV1 + ?Sized,
    S: SemanticOperationIssueSinkV1,
>(
    operation: &ExecutionOperationV15,
    operands: &[ValueId],
    results: &[ValueDef],
    types: &T,
    sink: &mut S,
) -> Result<(), S::Error> {
    sink.charge_work(1)?;
    // Prepay the only variable payload scan; invalid oversized rosters do not scan.
    if let ExecutionOperationV15::ScopeEnd { discarded, .. } = operation
        && discarded.len() < crate::MAX_VALUE_ARGUMENTS_V1
    {
        sink.charge_work(discarded.len())?;
    }
    if let Err(error) = operation.validate_payload() {
        return sink.emit(
            SemanticOperationIssueKind::InvalidStructure,
            SEMANTIC_FIXED_MESSAGE_WORK_UPPER_V1,
            format_args!("{error}"),
        );
    }
    let (operand_count, result_count) = match operation {
        ExecutionOperationV15::ContextIssue => (0, 1),
        ExecutionOperationV15::WorkgroupDerive { .. } => (1, 1),
        ExecutionOperationV15::ScopeEnd { discarded, .. } => (1 + discarded.len(), 0),
        ExecutionOperationV15::MaskedTileLoadU32 { .. } => (3, 1),
        ExecutionOperationV15::TileIntoFragmentU32 { .. } => (1, 1),
        ExecutionOperationV15::FragmentIntoPartsU32 { elements, .. } => {
            (1, 2 * usize::from(*elements))
        }
    };
    sink.charge_work(3)?;
    if operands.len() != operand_count || types.len() != operand_count {
        return emit_semantic_fixed_v1(
            sink,
            SemanticOperationIssueKind::InvalidStructure,
            "execution operand/type roster differs from its exact contract",
        );
    }
    if results.len() != result_count {
        return emit_semantic_fixed_v1(
            sink,
            SemanticOperationIssueKind::ResultArity,
            "execution result roster differs from its exact contract",
        );
    }
    let mut index = 0;
    operation.try_visit_operands_v1(|expected| -> Result<(), S::Error> {
        sink.charge_work(1)?;
        if operands[index] != expected {
            emit_semantic_fixed_v1(
                sink,
                SemanticOperationIssueKind::InvalidStructure,
                "execution operand IDs differ from the operation payload",
            )?;
        }
        if let Some(Some(actual)) = types.get_type(index)
            && !execution_operand_type_v3(operation, index, actual)
        {
            emit_semantic_fixed_v1(
                sink,
                SemanticOperationIssueKind::InvalidOperandType,
                "execution operand type differs from its exact role or masked-load profile",
            )?;
        }
        index += 1;
        Ok(())
    })?;
    for (index, result) in results.iter().enumerate() {
        sink.charge_work(1)?;
        if !execution_result_type_v3(operation, index, &result.ty) {
            emit_semantic_fixed_v1(
                sink,
                SemanticOperationIssueKind::TypeMismatch,
                "execution result type differs from its exact role or parts ordering",
            )?;
        }
    }
    Ok(())
}

fn execution_operand_type_v3(operation: &ExecutionOperationV15, index: usize, ty: &Type) -> bool {
    use ExecutionOperationV15 as Op;
    use ExecutionRoleV15 as Role;
    match (operation, index, ty) {
        (Op::WorkgroupDerive { .. }, 0, Type::Execution(Role::Context))
        | (
            Op::ScopeEnd { .. } | Op::MaskedTileLoadU32 { .. },
            0,
            Type::Execution(Role::Workgroup),
        ) => true,
        (
            Op::ScopeEnd { .. },
            index,
            Type::Execution(role @ (Role::MaskedTileU32 { .. } | Role::LaneFragmentU32 { .. })),
        ) => index > 0 && role.validate().is_ok(),
        (Op::MaskedTileLoadU32 { .. }, 1, Type::Slice(slice)) => {
            slice.address_space == AddressSpace::Global
                && slice.access == AccessMode::ReadOnly
                && matches!(slice.element.as_ref(), Type::Scalar(ScalarType::U32))
        }
        (Op::MaskedTileLoadU32 { .. }, 2, Type::Scalar(ScalarType::Index)) => true,
        (
            Op::TileIntoFragmentU32 {
                lanes, elements, ..
            },
            0,
            Type::Execution(Role::MaskedTileU32 {
                lanes: actual_lanes,
                elements: actual_elements,
            }),
        )
        | (
            Op::FragmentIntoPartsU32 {
                lanes, elements, ..
            },
            0,
            Type::Execution(Role::LaneFragmentU32 {
                lanes: actual_lanes,
                elements: actual_elements,
            }),
        ) => lanes == actual_lanes && elements == actual_elements,
        _ => false,
    }
}

fn execution_result_type_v3(operation: &ExecutionOperationV15, index: usize, ty: &Type) -> bool {
    use ExecutionOperationV15 as Op;
    use ExecutionRoleV15 as Role;
    match (operation, ty) {
        (Op::ContextIssue, Type::Execution(Role::Context))
        | (Op::WorkgroupDerive { .. }, Type::Execution(Role::Workgroup)) => true,
        (
            Op::MaskedTileLoadU32 {
                lanes, elements, ..
            },
            Type::Execution(Role::MaskedTileU32 {
                lanes: actual_lanes,
                elements: actual_elements,
            }),
        )
        | (
            Op::TileIntoFragmentU32 {
                lanes, elements, ..
            },
            Type::Execution(Role::LaneFragmentU32 {
                lanes: actual_lanes,
                elements: actual_elements,
            }),
        ) => lanes == actual_lanes && elements == actual_elements,
        (Op::FragmentIntoPartsU32 { elements, .. }, Type::Scalar(scalar)) => {
            *scalar
                == if index < usize::from(*elements) {
                    ScalarType::U32
                } else {
                    ScalarType::Bool
                }
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Meter {
        remaining: usize,
        used: usize,
        issues: usize,
    }

    impl SemanticOperationIssueSinkV1 for Meter {
        type Error = ();

        fn charge_work(&mut self, amount: usize) -> Result<(), Self::Error> {
            self.remaining = self.remaining.checked_sub(amount).ok_or(())?;
            self.used += amount;
            Ok(())
        }

        fn emit(
            &mut self,
            _kind: SemanticOperationIssueKind,
            work: usize,
            _message: fmt::Arguments<'_>,
        ) -> Result<(), Self::Error> {
            self.charge_work(work)?;
            self.issues += 1;
            Ok(())
        }
    }

    #[test]
    fn execution_registered_sink_exact_and_one_short_parts_budget() {
        let execution = ExecutionOperationV15::FragmentIntoPartsU32 {
            fragment: ValueId(0),
            lanes: 256,
            elements: 125,
        };
        let ty = Type::Execution(ExecutionRoleV15::LaneFragmentU32 {
            lanes: 256,
            elements: 125,
        });
        let results: Vec<_> = execution
            .try_contract_v3()
            .unwrap()
            .result_types
            .into_iter()
            .enumerate()
            .map(|(index, ty)| ValueDef::new(ValueId(1 + index as u32), ty))
            .collect();
        let operation = OperationKind::Execution(execution);
        let context = SemanticOperationBorrowedVerificationContextV1 {
            operands: &[ValueId(0)],
            operand_types: &[Some(&ty)],
            results: &results,
        };
        // Payload check + three roster checks + operand + 250 ordinary results.
        for (limit, expected) in [(255, Ok(true)), (254, Err(()))] {
            let mut sink = Meter {
                remaining: limit,
                used: 0,
                issues: 0,
            };
            assert_eq!(
                try_verify_semantic_operation_with_sink_v1(&operation, context, &mut sink),
                expected
            );
            assert!(sink.used <= limit);
            assert_eq!(sink.issues, 0);
        }
    }

    #[test]
    fn execution_registered_sink_prepays_discard_scan_and_refuses_bad_rosters() {
        let workgroup = Type::Execution(ExecutionRoleV15::Workgroup);
        let tile = Type::Execution(ExecutionRoleV15::MaskedTileU32 {
            lanes: 1,
            elements: 1,
        });
        let operation = OperationKind::Execution(ExecutionOperationV15::ScopeEnd {
            workgroup: ValueId(0),
            discarded: vec![ValueId(1), ValueId(2)],
        });
        let context = SemanticOperationBorrowedVerificationContextV1 {
            operands: &[ValueId(0), ValueId(1), ValueId(2)],
            operand_types: &[Some(&workgroup), Some(&tile), Some(&tile)],
            results: &[],
        };
        for (limit, expected) in [(9, Ok(true)), (8, Err(()))] {
            let mut sink = Meter {
                remaining: limit,
                used: 0,
                issues: 0,
            };
            assert_eq!(
                try_verify_semantic_operation_with_sink_v1(&operation, context, &mut sink),
                expected
            );
            assert_eq!(sink.issues, 0);
        }
        let bad = OperationKind::Execution(ExecutionOperationV15::ScopeEnd {
            workgroup: ValueId(0),
            discarded: vec![ValueId(2), ValueId(1)],
        });
        let mut sink = Meter {
            remaining: usize::MAX,
            used: 0,
            issues: 0,
        };
        assert_eq!(
            try_verify_semantic_operation_with_sink_v1(&bad, context, &mut sink),
            Ok(true)
        );
        assert_eq!(sink.issues, 1);
        let wrong_ids = SemanticOperationBorrowedVerificationContextV1 {
            operands: &[ValueId(0), ValueId(2), ValueId(1)],
            ..context
        };
        let mut sink = Meter {
            remaining: usize::MAX,
            used: 0,
            issues: 0,
        };
        assert_eq!(
            try_verify_semantic_operation_with_sink_v1(&operation, wrong_ids, &mut sink),
            Ok(true)
        );
        assert_eq!(sink.issues, 2);
    }

    #[test]
    fn execution_registered_sink_prepays_even_an_immediately_invalid_discard_scan() {
        let operation = OperationKind::Execution(ExecutionOperationV15::ScopeEnd {
            workgroup: ValueId(0),
            discarded: vec![ValueId(1); 128],
        });
        let context = SemanticOperationBorrowedVerificationContextV1 {
            operands: &[],
            operand_types: &[],
            results: &[],
        };
        // The malformed first pair must not be inspected before reserving all
        // 128 comparison slots. Diagnostic emission is separately budgeted.
        for (limit, used) in [(128, 1), (129, 129)] {
            let mut sink = Meter {
                remaining: limit,
                used: 0,
                issues: 0,
            };
            assert_eq!(
                try_verify_semantic_operation_with_sink_v1(&operation, context, &mut sink),
                Err(())
            );
            assert_eq!(sink.used, used);
            assert_eq!(sink.issues, 0);
        }
    }
}
