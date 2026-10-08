//! Exact AtomicRmw transport through the checked optimized successor.
//! This query neither admits a source pointer nor activates an execution policy.
use super::*;
use fe2o3_kernel_analysis::CanonicalKirOutputUseV1;

/// Disposition of one original source-owned RMW. Removal means checked dead
/// control, never a claim that a reachable atomic effect is pure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionOptimizedSourceAtomicDispositionV1 {
    /// Exact actual uses and old-value result of the retained occurrence.
    Retained {
        /// Actual optimized operation coordinate.
        operation: OpCoordinate,
        /// Actual pointer occurrence and its checked definition.
        pointer: CanonicalKirOutputUseV1,
        /// Actual RMW input occurrence and its checked definition.
        value: CanonicalKirOutputUseV1,
        /// Actual old-value result, including its result ordinal.
        result: Definition,
    },
    /// The checked transition removed this original operation as unreachable.
    RemovedUnreachable,
}

/// Borrowed original source anchor plus exact atomic effect and disposition.
/// This grants no shared-reference permission, source currentness, target,
/// formal, safety, native execution or ordinary-policy authority.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionOptimizedSourceAtomicRmwV1;
/// fn forge() -> ProductionOptimizedSourceAtomicRmwV1<'static> {
///     ProductionOptimizedSourceAtomicRmwV1 { original: panic!() }
/// }
/// ```
pub struct ProductionOptimizedSourceAtomicRmwV1<'scope> {
    original: SourcePhysicalAccessV18<'scope>,
    input: OpCoordinate,
    effect: ScopedAtomicEffectV1,
    scalar: ScalarType,
    disposition: ProductionOptimizedSourceAtomicDispositionV1,
}

impl ProductionOptimizedSourceAtomicRmwV1<'_> {
    /// Original source instance; not an optimized-instance substitution.
    pub fn instance(&self) -> usize {
        self.original.instance
    }
    /// Original physical operation selected by its source owner.
    pub fn input(&self) -> OpCoordinate {
        self.input
    }
    /// Original and, when retained, exactly equal output RMW operation kind.
    pub fn kind(&self) -> AtomicKind {
        self.effect.kind
    }
    /// Exact original address space, alignment and volatility contract.
    pub fn access(&self) -> MemoryAccess {
        self.effect.access
    }
    /// Exact original synchronization scope.
    pub fn scope(&self) -> SynchronizationScope {
        self.effect.scope
    }
    /// Exact original memory ordering, never weakened by this transport.
    pub fn ordering(&self) -> MemoryOrdering {
        self.effect.ordering
    }
    /// Exact old-value scalar type, retaining signedness.
    pub fn scalar(&self) -> ScalarType {
        self.scalar
    }
    /// Checked actual output, or explicit unreachable removal.
    pub fn disposition(&self) -> ProductionOptimizedSourceAtomicDispositionV1 {
        self.disposition
    }
}

// Only closed shape checking; the public query separately authenticates source,
// occurrence ownership and checked control. This cannot create a source anchor.
pub(in super::super) fn atomic_output_shape_v1(
    input: &Operation,
    output: &Operation,
    pointer: ValueId,
    value: ValueId,
    result: ValueId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<ScopedAtomicEffectV1> {
    budget.charge_work(8)?;
    let OperationKind::Atomic(original) = &input.kind else {
        return resources::binding("atomic transport original kind");
    };
    let effect = ScopedAtomicEffectV1::from_operation(original).ok_or(
        ProductionSourceOwnedViewErrorV18::Binding("atomic transport unsupported original RMW"),
    )?;
    let [before] = input.results.as_slice() else {
        return resources::binding("atomic transport original result census");
    };
    let [after] = output.results.as_slice() else {
        return resources::binding("atomic transport output result census");
    };
    let (Type::Scalar(before_scalar), Type::Scalar(after_scalar)) = (&before.ty, &after.ty) else {
        return resources::binding("atomic transport non-scalar result");
    };
    if before_scalar != after_scalar || after.id != result {
        return resources::binding("atomic transport result type or identity differs");
    }
    if tile_atomic_payload_operand_v1(output, pointer, value, result, effect, budget)
        .map_err(source_attachment_error_v18)?
        != 1
    {
        return resources::binding("atomic transport output RHS ordinal");
    }
    Ok(effect)
}

pub(in super::super) fn atomic_transport_headers_v1() -> Result<usize, ArgumentResourceV1> {
    // The returned borrowed header remains charged until the caller's enclosing
    // scratch scope drops it. No work debit or storage is refunded here.
    argument_sum_v1(&[
        size_of::<SourcePhysicalAccessV18<'_>>(),
        size_of::<Option<SourcePhysicalAccessV18<'_>>>(),
        size_of::<SourceOwnedResultV18<Option<SourcePhysicalAccessV18<'_>>>>(),
        size_of::<SourcePhysicalPayloadV18<'_>>(),
        size_of::<Option<SourcePhysicalPayloadV18<'_>>>(),
        size_of::<SourceOwnedResultV18<Option<SourcePhysicalPayloadV18<'_>>>>(),
        size_of::<ProductionOptimizedSourceAtomicRmwV1<'_>>(),
        size_of::<SourceOwnedResultV18<Option<ProductionOptimizedSourceAtomicRmwV1<'_>>>>(),
        size_of::<ProductionOptimizedSourceAtomicDispositionV1>(),
        argument_product_v1(2, size_of::<ScopedAtomicEffectV1>())?,
        size_of::<SourceOwnedResultV18<ScopedAtomicEffectV1>>(),
        argument_product_v1(3, size_of::<OpCoordinate>())?,
        size_of::<Option<OpCoordinate>>(),
        size_of::<SourceOwnedResultV18<Option<OpCoordinate>>>(),
        argument_product_v1(2, size_of::<CanonicalKirOutputUseV1>())?,
        size_of::<SourceOwnedResultV18<CanonicalKirOutputUseV1>>(),
        argument_product_v1(2, size_of::<UseCoordinate>())?,
        argument_product_v1(2, size_of::<Definition>())?,
        argument_product_v1(3, size_of::<ValueId>())?,
        argument_product_v1(4, size_of::<usize>())?,
        size_of::<SourceOwnedResultV18<usize>>(),
        argument_product_v1(2, size_of::<&Operation>())?,
        argument_product_v1(2, size_of::<&Atomic>())?,
        argument_product_v1(2, size_of::<&ValueDef>())?,
        argument_product_v1(2, size_of::<&ScalarType>())?,
        size_of::<ScalarType>(),
        size_of::<SourceOwnedResultV18<()>>(),
    ])
}

impl ProductionOptimizedSourceCorrespondenceV18<'_> {
    /// Transport a source-owned RMW through this exact checked successor.
    ///
    /// Non-atomic operations return `None`; an Atomic with missing source
    /// backing/payload or an unsupported shape refuses. This is deliberately
    /// separate from `scalar_memory_access`, whose Atomic refusal is unchanged.
    /// Original source admission (including shared/UnsafeCell provenance) must
    /// already have succeeded. The query cannot make that pending route valid.
    ///
    /// The caller must retain the returned header's storage charge until drop,
    /// normally inside the existing source-owned scratch-scope protocol.
    pub fn atomic_memory_access_v1(
        &self,
        root: usize,
        input: OpCoordinate,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<ProductionOptimizedSourceAtomicRmwV1<'_>>> {
        self.retain((|| {
            self.query(budget)?;
            budget.charge_work(1)?;
            if input.block.function.0 as usize
                != self.original.source.root_row(root)?.function_ordinal
            {
                return resources::binding("source atomic access changed root");
            }
            let ordinal = resources::operation_index(self.checked.input(), input, budget)?;
            let op = self.checked.input().operations()[ordinal].operation;
            let OperationKind::Atomic(atomic) = &op.kind else {
                return Ok(None);
            };
            budget.reserve_storage(atomic_transport_headers_v1()?)?;
            budget.charge_work(4)?;
            let effect = ScopedAtomicEffectV1::from_operation(atomic).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("source atomic RMW shape unsupported"),
            )?;
            let [before] = op.results.as_slice() else {
                return resources::binding("source atomic original result census");
            };
            let Type::Scalar(scalar) = &before.ty else {
                return resources::binding("source atomic original result type");
            };
            let original = self
                .original
                .retained_memory_access(root, input, atomic.pointer, budget)?
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "source atomic access has no original anchor",
                ))?;
            let payload = self
                .original
                .retained_scalar_payload_v18(root, input, &original, budget)?
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "source atomic access has no original payload",
                ))?;
            let ScopedMemoryPayloadV29::AtomicRmw {
                effect: source_effect,
                ..
            } = payload.source
            else {
                return resources::binding("source atomic access has non-atomic payload");
            };
            let value_use = UseCoordinate::OperationOperand {
                operation: input,
                operand: 1,
            };
            if *source_effect != effect
                || payload.value != before.id
                || payload.store_use != Some(value_use)
            {
                return resources::binding("source atomic original effect/result/RHS differs");
            }
            let disposition = match self.ordered_output(input, budget)? {
                None => ProductionOptimizedSourceAtomicDispositionV1::RemovedUnreachable,
                Some(output) => {
                    let pointer = self.exact_output_operand(
                        UseCoordinate::OperationOperand {
                            operation: input,
                            operand: 0,
                        },
                        output,
                        budget,
                    )?;
                    let value = self.exact_output_operand(value_use, output, budget)?;
                    if pointer.coordinate
                        != (UseCoordinate::OperationOperand {
                            operation: output,
                            operand: 0,
                        })
                        || value.coordinate
                            != (UseCoordinate::OperationOperand {
                                operation: output,
                                operand: 1,
                            })
                    {
                        return resources::binding(
                            "source atomic output occurrence ordinal differs",
                        );
                    }
                    let result = Definition::Result {
                        operation: output,
                        result: 0,
                    };
                    self.exact_output_result(
                        Definition::Result {
                            operation: input,
                            result: 0,
                        },
                        result,
                        budget,
                    )?;
                    let output_ordinal =
                        resources::operation_index(self.checked.output(), output, budget)?;
                    let pointer_row = resources::definition_index(
                        self.checked.output(),
                        pointer.definition,
                        budget,
                    )?;
                    let value_row = resources::definition_index(
                        self.checked.output(),
                        value.definition,
                        budget,
                    )?;
                    let result_row =
                        resources::definition_index(self.checked.output(), result, budget)?;
                    let actual_value = |row: usize| {
                        self.checked.output().definitions()[row].value.ok_or(
                            ProductionSourceOwnedViewErrorV18::Binding(
                                "source atomic output definition has no value",
                            ),
                        )
                    };
                    atomic_output_shape_v1(
                        op,
                        self.checked.output().operations()[output_ordinal].operation,
                        actual_value(pointer_row)?,
                        actual_value(value_row)?,
                        actual_value(result_row)?,
                        budget,
                    )?;
                    ProductionOptimizedSourceAtomicDispositionV1::Retained {
                        operation: output,
                        pointer,
                        value,
                        result,
                    }
                }
            };
            Ok(Some(ProductionOptimizedSourceAtomicRmwV1 {
                original,
                input,
                effect,
                scalar: *scalar,
                disposition,
            }))
        })())
    }
}
