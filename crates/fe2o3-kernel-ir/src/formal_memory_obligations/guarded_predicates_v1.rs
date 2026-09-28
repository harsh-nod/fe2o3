use super::*;

#[derive(Clone, Copy)]
pub(super) struct PredicateRow {
    pub(super) value: ValueId,
    pub(super) truth: bool,
    pub(super) edge: Edge,
    pub(super) interval: (u32, u32),
    pub(super) covering: usize,
}

impl<'module, M: GuardMeter> GuardedAnalysisV1<'module, M> {
    pub(super) fn boolean_carrier_definition(
        &mut self,
        value: ValueId,
    ) -> Result<Option<usize>, ResourceError> {
        // Keep the new result slot conservatively paid until function cleanup.
        self.ledger.storage(size_of::<Option<usize>>())?;
        let Some(origin) = self
            .ledger
            .find(&self.runtime_reads.origins, |row| row.value.cmp(&value))?
        else {
            return Ok(None);
        };
        self.ledger.charge(4)?;
        let row = self.runtime_reads.origins[origin];
        if row.ty != &Type::BOOL {
            return Ok(None);
        }
        let Some(value) = row.origin else {
            return Ok(None);
        };
        let Some(definition) = self
            .ledger
            .find(&self.definitions, |row| row.value.cmp(&value))?
        else {
            return Ok(None);
        };
        let operation = self.definitions[definition].operation;
        self.ledger.charge(operation.results.len())?;
        Ok(operation
            .results
            .iter()
            .any(|result| result.id == value && result.ty == Type::BOOL)
            .then_some(definition))
    }

    // Only true-And and Not are introduction rules. Or, false-And and all
    // non-Boolean operators remain opaque even when a surrounding guard holds.
    pub(super) fn expanded_predicates(&mut self) -> Result<Vec<PredicateRow>, ResourceError> {
        self.ledger.storage(
            4_usize
                .checked_mul(size_of::<Vec<()>>())
                .ok_or(ResourceError::Arithmetic)?,
        )?;
        let count = self.definitions.len();
        let mut visited = Vec::new();
        self.ledger.reserve(&mut visited, count)?;
        self.ledger.charge(count)?;
        visited.resize(count, [0_usize; 2]);
        let mut pending = Vec::new();
        let mut output = Vec::new();
        let roots = self.truths.len();
        for ordinal in 0..roots {
            self.ledger.charge(4)?;
            let root = self.truths[ordinal];
            let stamp = ordinal.checked_add(1).ok_or(ResourceError::Arithmetic)?;
            self.ledger.push(&mut pending, (root.predicate, true))?;
            while let Some((value, truth)) = pending.pop() {
                self.ledger.charge(4)?;
                let mut definition = self
                    .ledger
                    .find(&self.definitions, |row| row.value.cmp(&value))?;
                if definition.is_none() {
                    definition = self.boolean_carrier_definition(value)?;
                }
                let Some(definition) = definition else {
                    continue;
                };
                self.ledger.charge(4)?;
                let value = self.definitions[definition].value;
                let slot = usize::from(truth);
                if visited[definition][slot] == stamp {
                    continue;
                }
                visited[definition][slot] = stamp;
                let operation = self.definitions[definition].operation;
                self.ledger.charge(operation.results.len())?;
                if !operation
                    .results
                    .iter()
                    .any(|result| result.id == value && result.ty == Type::BOOL)
                {
                    continue;
                }
                self.ledger.push(
                    &mut output,
                    PredicateRow {
                        value,
                        truth,
                        edge: root.edge,
                        interval: root.interval,
                        covering: 0,
                    },
                )?;
                match operation.kind {
                    OperationKind::Binary {
                        op: BinaryOp::BitAnd,
                        lhs,
                        rhs,
                    } if truth && single_type(operation, &Type::BOOL) => {
                        self.ledger.push(&mut pending, (rhs, true))?;
                        self.ledger.push(&mut pending, (lhs, true))?;
                    }
                    OperationKind::Unary {
                        op: crate::UnaryOp::Not,
                        operand,
                    } if single_type(operation, &Type::BOOL) => {
                        self.ledger.push(&mut pending, (operand, !truth))?;
                    }
                    _ => {}
                }
            }
        }
        self.ledger.sort(&mut output, 8, |a, b| {
            (
                a.value,
                a.truth,
                a.interval,
                a.edge.source,
                a.edge.ordinal,
                a.edge.target,
            )
                .cmp(&(
                    b.value,
                    b.truth,
                    b.interval,
                    b.edge.source,
                    b.edge.ordinal,
                    b.edge.target,
                ))
        })?;
        for ordinal in 0..output.len() {
            self.ledger.charge(8)?;
            let row = output[ordinal];
            output[ordinal].covering = if ordinal > 0 {
                let previous = output[ordinal - 1];
                let cover = output[previous.covering];
                if (previous.value, previous.truth) == (row.value, row.truth)
                    && cover.interval.1 >= row.interval.1
                {
                    previous.covering
                } else {
                    ordinal
                }
            } else {
                ordinal
            };
        }
        let mut truths = Vec::new();
        self.ledger.reserve(&mut truths, output.len())?;
        for row in &output {
            self.ledger.charge(2)?;
            if row.truth {
                truths.push(TrueRow {
                    predicate: row.value,
                    edge: row.edge,
                    interval: row.interval,
                    ambiguous: false,
                });
            }
        }
        // Scratch credits remain reserved until the enclosing function analysis
        // drops all backing. No detached report is returned with refunded credit.
        self.truths = truths;
        Ok(output)
    }
}
