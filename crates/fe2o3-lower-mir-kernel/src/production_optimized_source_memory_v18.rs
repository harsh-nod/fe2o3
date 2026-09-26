use super::*;
use attachments::ProductionOptimizedSourceOperationV18 as Placement;
use fe2o3_kernel_analysis::CanonicalKirOutputUseV1;

/// Exact output of an original source allocation. `None` is explicit checked
/// unreachable removal, not permission to substitute another allocation.
pub struct ProductionOptimizedSourceAllocationV18<'scope> {
    original: SourcePhysicalBackingV18<'scope>,
    input: OpCoordinate,
    output: Option<OpCoordinate>,
    pointer: Option<Definition>,
    count: Option<CanonicalKirOutputUseV1>,
}

impl ProductionOptimizedSourceAllocationV18<'_> {
    /// Original instance owning the allocation.
    pub fn instance(&self) -> usize {
        self.original.instance
    }
    /// Historical physical allocation coordinate.
    pub fn input(&self) -> OpCoordinate {
        self.input
    }
    /// Actual retained allocation, or explicit unreachable removal.
    pub fn output(&self) -> Option<OpCoordinate> {
        self.output
    }
    /// Actual retained pointer definition.
    pub fn pointer(&self) -> Option<Definition> {
        self.pointer
    }
    /// Actual dynamic count use, absent for a fixed or removed allocation.
    pub fn count(&self) -> Option<CanonicalKirOutputUseV1> {
        self.count
    }
}

/// Scalar memory payload selected by its actual output occurrence. This is
/// transport evidence only, not an expression equivalence or currentness proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionOptimizedSourcePayloadV18 {
    /// Exact result of the retained load occurrence.
    Load {
        /// Actual output result definition.
        output: Definition,
    },
    /// Exact RHS operand of the retained store occurrence.
    Store {
        /// Actual output use and the definition it consumes.
        output: CanonicalKirOutputUseV1,
    },
    /// The original payload's operation was removed by checked dead control.
    RemovedUnreachable,
}

/// Original scalar memory anchor joined to its checked actual successor.
pub struct ProductionOptimizedSourceMemoryAccessV18<'scope> {
    original: SourcePhysicalAccessV18<'scope>,
    input: OpCoordinate,
    output: Option<OpCoordinate>,
    pointer: Option<CanonicalKirOutputUseV1>,
    payload: Option<ProductionOptimizedSourcePayloadV18>,
}

impl ProductionOptimizedSourceMemoryAccessV18<'_> {
    /// Original source instance owning the access.
    pub fn instance(&self) -> usize {
        self.original.instance
    }
    /// Original access coordinate, not an output locator.
    pub fn input(&self) -> OpCoordinate {
        self.input
    }
    /// Actual retained occurrence, or explicit unreachable removal.
    pub fn output(&self) -> Option<OpCoordinate> {
        self.output
    }
    /// Actual pointer operand of the retained access.
    pub fn pointer(&self) -> Option<CanonicalKirOutputUseV1> {
        self.pointer
    }
    /// Exact scalar payload disposition when the original source supplied one.
    pub fn payload(&self) -> Option<ProductionOptimizedSourcePayloadV18> {
        self.payload
    }
}

impl ProductionOptimizedSourceCorrespondenceV18<'_> {
    fn ordered_output(
        &self,
        input: OpCoordinate,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<OpCoordinate>> {
        match attachments::operation(self.checked, self.control, self.index, input, budget)? {
            Placement::Retained { output, .. } => Ok(Some(output)),
            Placement::RemovedUnreachable { .. } => Ok(None),
            Placement::Rewritten { .. } => {
                resources::binding("reachable source memory occurrence erased")
            }
        }
    }

    fn exact_output_operand(
        &self,
        input: UseCoordinate,
        output_operation: OpCoordinate,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<CanonicalKirOutputUseV1> {
        let actual = self
            .control
            .operand(input, budget)
            .map_err(transition_error)?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "source memory output use removed",
            ))?;
        let UseCoordinate::OperationOperand { operation, operand } = actual.coordinate else {
            return resources::binding("source memory use became a terminator use");
        };
        if operation != output_operation {
            return resources::binding("source memory use changed operation");
        }
        let ordinal = resources::operation_index(self.checked.output(), operation, budget)?;
        let range = self.checked.output().operations()[ordinal].operands.clone();
        let offset = range
            .start
            .checked_add(operand as usize)
            .filter(|offset| *offset < range.end)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "source memory output operand interval",
            ))?;
        budget.charge_work(3)?;
        let usage = self.checked.output().uses().get(offset).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("source memory output operand census"),
        )?;
        let definition = self
            .checked
            .output()
            .definitions()
            .get(usage.definition)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "source memory output definition census",
            ))?;
        if usage.coordinate != actual.coordinate || definition.coordinate != actual.definition {
            return resources::binding("source memory actual output operand differs");
        }
        Ok(actual)
    }

    fn exact_output_result(
        &self,
        input: Definition,
        output: Definition,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        resources::definition_index(self.checked.output(), output, budget)?;
        let mut found = false;
        for row in self.definition_descendants(input, budget)? {
            budget.charge_work(1)?;
            if row.output == output {
                if found {
                    return resources::binding("repeated source memory result descendant");
                }
                found = true;
            }
        }
        if !found {
            return resources::binding("source memory result is not a checked descendant");
        }
        Ok(())
    }

    /// Joins original allocation backing and IDs before selecting actual output
    /// pointer and count occurrences. This alone grants no final memory proof.
    pub fn allocation(
        &self,
        root: usize,
        input: OpCoordinate,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<ProductionOptimizedSourceAllocationV18<'_>>> {
        self.retain((|| {
            self.query(budget)?;
            let Some(original) = self.original.retained_allocation(root, input, budget)? else {
                return Ok(None);
            };
            let output = self.ordered_output(input, budget)?;
            let Some(actual) = output else {
                return Ok(Some(ProductionOptimizedSourceAllocationV18 {
                    original,
                    input,
                    output,
                    pointer: None,
                    count: None,
                }));
            };
            let ordinal = resources::operation_index(self.checked.output(), actual, budget)?;
            let op = self.checked.output().operations()[ordinal].operation;
            let OperationKind::Alloca {
                element,
                count,
                address_space: AddressSpace::Private,
                alignment,
            } = &op.kind
            else {
                return resources::binding("source allocation changed output operation");
            };
            budget.charge_work(3)?;
            let [result] = op.results.as_slice() else {
                return resources::binding("source allocation output result census");
            };
            match original.slot.representation {
                ScopedSlotRepresentationV29::ScalarArray(scalar) => {
                    if scalar.element.alignment != *alignment
                        || !scalar.element.element.matches_borrowed(element, &mut SourceCorrespondenceWorkV18(budget))? {
                        return resources::binding("source allocation changed element or alignment");
                    }
                }
                ScopedSlotRepresentationV29::Object { schema, alignment, .. } => {
                    check_scoped_object_alloca_v29(op, result.id, schema, alignment, budget)
                        .map_err(|error| source_attachment_error_v18(error.into()))?;
                }
            }
            let pointer = Definition::Result {
                operation: actual,
                result: 0,
            };
            self.exact_output_result(
                Definition::Result {
                    operation: input,
                    result: 0,
                },
                pointer,
                budget,
            )?;
            let definition = resources::definition_index(self.checked.output(), pointer, budget)?;
            if self.checked.output().definitions()[definition].value != Some(result.id) {
                return resources::binding("source allocation actual pointer differs");
            }
            let count = match (original.slot.representation.count(), count) {
                (None, None) => None,
                (Some(_), Some(value)) => {
                    let actual_count = self.exact_output_operand(
                        UseCoordinate::OperationOperand {
                            operation: input,
                            operand: 0,
                        },
                        actual,
                        budget,
                    )?;
                    let definition = resources::definition_index(
                        self.checked.output(),
                        actual_count.definition,
                        budget,
                    )?;
                    if self.checked.output().definitions()[definition].value != Some(*value) {
                        return resources::binding("source allocation actual count differs");
                    }
                    Some(actual_count)
                }
                (None, Some(_)) | (Some(_), None) => {
                    return resources::binding("source allocation count shape changed");
                }
            };
            Ok(Some(ProductionOptimizedSourceAllocationV18 {
                original,
                input,
                output,
                pointer: Some(pointer),
                count,
            }))
        })())
    }

    /// Transports only ordinary scalar Load/Store and their guarded forms.
    /// Typed Storage explicitly refuses this scalar query. `None` for another
    /// unsupported family grants no source-census or currentness permission.
    pub fn scalar_memory_access(
        &self,
        root: usize,
        input: OpCoordinate,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<ProductionOptimizedSourceMemoryAccessV18<'_>>> {
        self.retain((|| {
            self.query(budget)?;
            budget.charge_work(1)?;
            if input.block.function.0 as usize != self.original.source.root_row(root)?.function_ordinal {
                return resources::binding("source scalar access changed root");
            }
            let ordinal = resources::operation_index(self.checked.input(), input, budget)?;
            let op = self.checked.input().operations()[ordinal].operation;
            let pointer = match &op.kind {
                OperationKind::Load { pointer, .. }
                | OperationKind::GuardedLoad { pointer, .. }
                | OperationKind::Store { pointer, .. }
                | OperationKind::GuardedStore { pointer, .. } => *pointer,
                OperationKind::Storage(_) => return resources::binding("typed Storage requires complete object source transport"),
                _ => return Ok(None),
            };
            let Some(original) = self
                .original
                .retained_memory_access(root, input, pointer, budget)?
            else {
                return Ok(None);
            };
            let payload = self
                .original
                .retained_scalar_payload_v18(root, input, &original, budget)?;
            let output = self.ordered_output(input, budget)?;
            let Some(actual) = output else {
                let payload =
                    payload.map(|_| ProductionOptimizedSourcePayloadV18::RemovedUnreachable);
                return Ok(Some(ProductionOptimizedSourceMemoryAccessV18 {
                    original,
                    input,
                    output,
                    pointer: None,
                    payload,
                }));
            };
            let actual_pointer = self.exact_output_operand(
                UseCoordinate::OperationOperand {
                    operation: input,
                    operand: 0,
                },
                actual,
                budget,
            )?;
            let ordinal = resources::operation_index(self.checked.output(), actual, budget)?;
            let out = self.checked.output().operations()[ordinal].operation;
            let pointer = match (&op.kind, &out.kind) {
                (OperationKind::Load { .. }, OperationKind::Load { pointer, .. })
                | (OperationKind::GuardedLoad { .. }, OperationKind::GuardedLoad { pointer, .. })
                | (OperationKind::Store { .. }, OperationKind::Store { pointer, .. })
                | (
                    OperationKind::GuardedStore { .. },
                    OperationKind::GuardedStore { pointer, .. },
                ) => *pointer,
                _ => return resources::binding("source memory access kind changed"),
            };
            let definition = resources::definition_index(
                self.checked.output(),
                actual_pointer.definition,
                budget,
            )?;
            if self.checked.output().definitions()[definition].value != Some(pointer) {
                return resources::binding("source memory actual output pointer differs");
            }
            let payload = match payload {
                None => None,
                Some(payload) => Some(match payload.source {
                    ScopedMemoryPayloadV29::Load { .. }
                    | ScopedMemoryPayloadV29::IndexLoad { .. } => {
                        let [result] = out.results.as_slice() else {
                            return resources::binding("source load output result census");
                        };
                        let output = Definition::Result {
                            operation: actual,
                            result: 0,
                        };
                        self.exact_output_result(
                            Definition::Result {
                                operation: input,
                                result: 0,
                            },
                            output,
                            budget,
                        )?;
                        let definition =
                            resources::definition_index(self.checked.output(), output, budget)?;
                        if self.checked.output().definitions()[definition].value != Some(result.id)
                        {
                            return resources::binding("source load actual output value differs");
                        }
                        ProductionOptimizedSourcePayloadV18::Load { output }
                    }
                    ScopedMemoryPayloadV29::Store { .. } => {
                        let source_use =
                            payload
                                .store_use
                                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                    "source Store lost exact original use",
                                ))?;
                        let output = self.exact_output_operand(source_use, actual, budget)?;
                        let UseCoordinate::OperationOperand { operand, .. } = output.coordinate
                        else {
                            return resources::binding("source Store output operand kind");
                        };
                        let value = match &out.kind {
                            OperationKind::Store { value, .. }
                            | OperationKind::GuardedStore { value, .. } => *value,
                            _ => return resources::binding("source Store output kind"),
                        };
                        if tile_store_payload_operand_v18(out, pointer, value, budget)
                            .map_err(source_attachment_error_v18)?
                            != operand
                        {
                            return resources::binding(
                                "source Store output use is not its actual RHS",
                            );
                        }
                        let definition = resources::definition_index(
                            self.checked.output(),
                            output.definition,
                            budget,
                        )?;
                        if self.checked.output().definitions()[definition].value != Some(value) {
                            return resources::binding("source Store output RHS differs");
                        }
                        ProductionOptimizedSourcePayloadV18::Store { output }
                    }
                }),
            };
            Ok(Some(ProductionOptimizedSourceMemoryAccessV18 {
                original,
                input,
                output,
                pointer: Some(actual_pointer),
                payload,
            }))
        })())
    }
}
