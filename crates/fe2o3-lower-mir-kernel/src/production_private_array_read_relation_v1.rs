use super::*;

impl PrivateArrayFinalRelationV1<'_> {
    // This first read rule deliberately requires a same-block source Store.
    // It does not infer initialization from a ranked bounds report or from a
    // physically preceding operation in another block.
    pub(super) fn check_read_initialization(
        &self,
        read: &PrivateArrayEffectV1,
        slot: &PrivateArraySlotV1,
        offset: u64,
        work: &mut PrivateArrayCorrelationWorkV1<'_>,
    ) -> Result<(), ProductionMirPlironTranslationErrorV1> {
        let mismatch = || ProductionMirPlironTranslationErrorV1::AllocationOriginMismatch {
            location: FunctionOperationLocation::new(
                read.memory_location.block,
                read.memory_location.operation,
            ),
        };
        let mut latest: Option<&PrivateArrayEffectV1> = None;
        for candidate in self.effects {
            work.charge_private_array_work(6)?;
            if candidate.semantic_block != read.semantic_block
                || candidate.local != read.local
                || candidate.access != PrivateArrayAccessV1::Write
                || candidate.memory_location.operation >= read.memory_location.operation
                || candidate.semantic_statement > read.semantic_statement
            {
                continue;
            }
            let candidate_offset = private_array_exact_relation_v1(
                self.semantic.types(),
                self.function,
                self.body,
                self.owner,
                self.function_id,
                slot,
                candidate,
                self.max_operations,
                work,
            )
            .map_err(|error| match error {
                PrivateArrayRelationErrorV1::Work(error) => error,
                _ => mismatch(),
            })?;
            work.charge_private_array_work(3)?;
            if candidate_offset == offset
                && latest.is_none_or(|old| {
                    old.memory_location.operation < candidate.memory_location.operation
                })
            {
                latest = Some(candidate);
            }
        }
        work.charge_private_array_work(3)?;
        let latest = latest.ok_or_else(mismatch)?;
        let statements = self
            .function
            .blocks()
            .get(read.semantic_block as usize)
            .ok_or_else(mismatch)?
            .statements();
        let end = (read.semantic_statement as usize)
            .checked_add(1)
            .ok_or_else(mismatch)?;
        let statements = statements
            .get(latest.semantic_statement as usize..end)
            .ok_or_else(mismatch)?;
        for statement in statements {
            work.charge_private_array_work(2)?;
            let invalidated = match statement.kind() {
                SemanticStatementKindV1::StorageLive(local)
                | SemanticStatementKindV1::StorageDead(local) => local.index() == slot.local,
                SemanticStatementKindV1::Deinitialize(place) => place.local().index() == slot.local,
                SemanticStatementKindV1::Assign(assignment) => {
                    let mut moved = false;
                    private_array_visit_rvalue_operands_v1(
                        assignment.value().kind(),
                        |operand, _| {
                            work.charge_private_array_work(2)?;
                            moved |= matches!(operand, SemanticOperandV1::Move(place) if place.local().index() == slot.local);
                            Ok::<_, ProductionMirPlironTranslationErrorV1>(())
                        },
                    )?;
                    moved
                }
                SemanticStatementKindV1::Store(store) => {
                    work.charge_private_array_work(2)?;
                    matches!(store.value(), SemanticOperandV1::Move(place) if place.local().index() == slot.local)
                }
                _ => false,
            };
            if invalidated {
                return Err(mismatch());
            }
        }
        Ok(())
    }
}
