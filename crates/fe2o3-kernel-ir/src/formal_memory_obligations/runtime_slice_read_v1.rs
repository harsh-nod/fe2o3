//! A fresh, read-only runtime bound. It never supplies an affine address.
use super::*;

#[derive(Clone, Copy)]
struct Origin<'module> {
    value: ValueId,
    origin: Option<ValueId>,
    ty: &'module Type,
}

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
enum ReadIndex {
    ProvenOrigin(ValueId),
    ExactBlockParameter(ValueId),
}

#[derive(Clone, Copy)]
enum ReadLink {
    Terminal,
    Origin(ValueId),
    Bridge(ValueId),
}

#[derive(Clone, Copy)]
enum ReadResolution {
    Pending,
    Visiting,
    Resolved(ReadIndex),
    Unsupported,
}

#[derive(Clone, Copy)]
struct ReadRepresentation {
    value: ValueId,
    scalar: ScalarType,
    link: ReadLink,
    next: Option<usize>,
    resolution: ReadResolution,
}

fn representation_scalar(ty: &Type) -> Option<ScalarType> {
    match ty {
        Type::Scalar(s @ (ScalarType::Index | ScalarType::U64)) => Some(*s),
        _ => None,
    }
}

#[derive(Clone, Copy)]
struct ReadGuard {
    index: ReadIndex,
    slice: ValueId,
    allocation: FormalAllocationIdentity,
    guard_index: ValueId,
    length: ValueId,
    predicate: ValueId,
    edge: Edge,
    interval: (u32, u32),
    covering: usize,
}

#[derive(Default)]
pub(super) struct RuntimeReadState<'module> {
    origins: Vec<Origin<'module>>,
    guards: Vec<ReadGuard>,
    representations: Vec<ReadRepresentation>,
}

fn origin_lookup_work_v1(count: usize) -> Result<usize, ResourceError> {
    // Pinned rustc 55e86c996 uses BTreeMap nodes with at most 11 keys and
    // linear within-node search. Pay 12 per possible binary-tree level,
    // plus row handling; revisit this assumption when updating the toolchain.
    crate::verification_index_v1::verification_ceil_log2_v1(count)
        .checked_add(1)
        .and_then(|levels| levels.checked_mul(12))
        .and_then(|work| work.checked_add(4))
        .ok_or(ResourceError::Arithmetic)
}

impl<'module> GuardedAnalysisV1<'module> {
    pub(super) fn collect_runtime_reads(
        &mut self,
        definitions: &Definitions<'module>,
        function: &'module Function,
    ) -> Result<(), ResourceError> {
        let body = function.body.as_ref().ok_or(ResourceError::Accounting)?;
        self.ledger.reserve(
            &mut self.runtime_reads.origins,
            definitions.block_parameter_origins.len(),
        )?;
        let lookup = origin_lookup_work_v1(definitions.block_parameter_origins.len())?;
        for block in &body.blocks {
            self.ledger.charge(1)?;
            for parameter in &block.parameters {
                self.ledger.charge(lookup)?;
                let Some(origin) = definitions.block_parameter_origins.get(&parameter.id) else {
                    continue;
                };
                // The existing unique-origin SCC analysis considers every
                // reachable incoming edge and rejects an unseeded recurrence.
                self.runtime_reads.origins.push(Origin {
                    value: parameter.id,
                    origin: *origin,
                    ty: &parameter.ty,
                });
            }
        }
        self.ledger
            .sort(&mut self.runtime_reads.origins, 1, |a, b| {
                a.value.cmp(&b.value)
            })?;
        self.collect_read_representations(function)?;
        self.ledger
            .reserve(&mut self.runtime_reads.guards, self.truths.len())?;
        for ordinal in 0..self.truths.len() {
            self.ledger.charge(24)?;
            let truth = self.truths[ordinal];
            // Repeated predicates still have independently checked true edges.
            // This index retains every edge; the single-truth recipe does not.
            let Some(compare) = self.definition(truth.predicate)? else {
                continue;
            };
            let OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs,
                rhs,
            } = compare.kind
            else {
                continue;
            };
            let Some(index) = self.runtime_read_index(lhs)? else {
                continue;
            };
            let Some(length) = self.runtime_read_index(rhs)? else {
                continue;
            };
            let length = match length {
                ReadIndex::ProvenOrigin(value) => value,
                ReadIndex::ExactBlockParameter(_) => continue,
            };
            let Some(length_op) = self.definition(length)? else {
                continue;
            };
            if !single_type(length_op, &Type::INDEX) {
                continue;
            }
            let OperationKind::SliceLength { slice } = length_op.kind else {
                continue;
            };
            let Some((parameter, _)) = self.runtime_slice_parameter(slice)? else {
                continue;
            };
            self.runtime_reads.guards.push(ReadGuard {
                index,
                slice: parameter.value,
                allocation: FormalAllocationIdentity {
                    parameter_index: parameter.ordinal,
                },
                guard_index: lhs,
                length: rhs,
                predicate: truth.predicate,
                edge: truth.edge,
                interval: truth.interval,
                covering: 0,
            });
        }
        self.ledger
            .sort(&mut self.runtime_reads.guards, 6, |a, b| {
                (a.index, a.slice, a.interval, a.predicate).cmp(&(
                    b.index,
                    b.slice,
                    b.interval,
                    b.predicate,
                ))
            })?;
        // Prefix maxima select a covering successful edge in logarithmic work,
        // even when an earlier enclosing guard outlives a later sibling guard.
        for ordinal in 0..self.runtime_reads.guards.len() {
            self.ledger.charge(8)?;
            let row = self.runtime_reads.guards[ordinal];
            let covering = if ordinal > 0 {
                let previous = self.runtime_reads.guards[ordinal - 1];
                let cover = self.runtime_reads.guards[previous.covering];
                if (previous.index, previous.slice) == (row.index, row.slice)
                    && cover.interval.1 >= row.interval.1
                {
                    previous.covering
                } else {
                    ordinal
                }
            } else {
                ordinal
            };
            self.runtime_reads.guards[ordinal].covering = covering;
        }
        Ok(())
    }

    fn collect_read_representations(
        &mut self,
        function: &'module Function,
    ) -> Result<(), ResourceError> {
        if !self.runtime_reads.representations.is_empty() {
            return Err(ResourceError::Accounting);
        }
        let body = function.body.as_ref().ok_or(ResourceError::Accounting)?;
        // Count all possible rows first. Non-index rows remain unused capacity;
        // no push can reallocate while the dependency cache is being built.
        let count = self
            .parameters
            .len()
            .checked_add(self.runtime_reads.origins.len())
            .and_then(|v| v.checked_add(self.definitions.len()))
            .ok_or(ResourceError::Arithmetic)?;
        self.ledger
            .reserve(&mut self.runtime_reads.representations, count)?;
        for row in &self.parameters {
            self.ledger.charge(3)?;
            if let Some(scalar) = representation_scalar(row.ty) {
                self.runtime_reads.representations.push(ReadRepresentation {
                    value: row.value,
                    scalar,
                    link: ReadLink::Terminal,
                    next: None,
                    resolution: ReadResolution::Resolved(ReadIndex::ProvenOrigin(row.value)),
                });
            }
        }
        for row in &self.runtime_reads.origins {
            self.ledger.charge(4)?;
            if let Some(scalar) = representation_scalar(row.ty) {
                self.runtime_reads.representations.push(ReadRepresentation {
                    value: row.value,
                    scalar,
                    next: None,
                    link: row.origin.map_or(ReadLink::Terminal, ReadLink::Origin),
                    resolution: row.origin.map_or(
                        ReadResolution::Resolved(ReadIndex::ExactBlockParameter(row.value)),
                        |_| ReadResolution::Pending,
                    ),
                });
            }
        }
        for block in &body.blocks {
            self.ledger.charge(1)?;
            if self
                .control_row(block.id)?
                .is_none_or(|row| row.interval.is_none())
            {
                continue;
            }
            for operation in &block.operations {
                self.ledger.charge(3)?;
                for result in &operation.results {
                    self.ledger.charge(4)?;
                    let Some(scalar) = representation_scalar(&result.ty) else {
                        continue;
                    };
                    let link = match &operation.kind {
                        OperationKind::Cast {
                            kind: CastKind::Bitcast,
                            value,
                            to,
                        } if operation.results.len() == 1 && *to == result.ty => {
                            ReadLink::Bridge(*value)
                        }
                        _ => ReadLink::Terminal,
                    };
                    if self.runtime_reads.representations.len() == count {
                        return Err(ResourceError::Accounting);
                    }
                    self.runtime_reads.representations.push(ReadRepresentation {
                        value: result.id,
                        scalar,
                        link,
                        next: None,
                        resolution: match link {
                            ReadLink::Terminal => {
                                ReadResolution::Resolved(ReadIndex::ProvenOrigin(result.id))
                            }
                            _ => ReadResolution::Pending,
                        },
                    });
                }
            }
        }
        self.ledger
            .sort(&mut self.runtime_reads.representations, 1, |a, b| {
                a.value.cmp(&b.value)
            })?;
        for ordinal in 0..self.runtime_reads.representations.len() {
            self.ledger.charge(6)?;
            let row = self.runtime_reads.representations[ordinal];
            if ordinal > 0 && self.runtime_reads.representations[ordinal - 1].value == row.value {
                return Err(ResourceError::Accounting);
            }
            let (value, bridge) = match row.link {
                ReadLink::Terminal => continue,
                ReadLink::Origin(value) => (value, false),
                ReadLink::Bridge(value) => (value, true),
            };
            let next = self
                .ledger
                .find(&self.runtime_reads.representations, |r| r.value.cmp(&value))?;
            let valid = next.is_some_and(|next| {
                let from = self.runtime_reads.representations[next].scalar;
                if bridge {
                    matches!(
                        (from, row.scalar),
                        (ScalarType::U64, ScalarType::Index) | (ScalarType::Index, ScalarType::U64)
                    )
                } else {
                    from == row.scalar
                }
            });
            if valid {
                self.runtime_reads.representations[ordinal].next = next;
            } else {
                // An unrecognized cast stays its own opaque SSA value. It is
                // never equated with a separately computed signed/numeric cast.
                self.runtime_reads.representations[ordinal].resolution = if bridge {
                    ReadResolution::Resolved(ReadIndex::ProvenOrigin(row.value))
                } else {
                    ReadResolution::Unsupported
                };
            }
        }
        self.resolve_read_representations()
    }

    fn resolve_read_representations(&mut self) -> Result<(), ResourceError> {
        self.ledger.storage(size_of::<Vec<usize>>())?;
        let mut path = Vec::new();
        self.ledger
            .reserve(&mut path, self.runtime_reads.representations.len())?;
        for start in 0..self.runtime_reads.representations.len() {
            self.ledger.charge(1)?;
            if !matches!(
                self.runtime_reads.representations[start].resolution,
                ReadResolution::Pending
            ) {
                continue;
            }
            let mut current = start;
            let result = loop {
                self.ledger.charge(4)?;
                let row = self
                    .runtime_reads
                    .representations
                    .get(current)
                    .ok_or(ResourceError::Accounting)?;
                match row.resolution {
                    ReadResolution::Resolved(result) => break Some(result),
                    ReadResolution::Visiting | ReadResolution::Unsupported => break None,
                    ReadResolution::Pending => {
                        let next = row.next.ok_or(ResourceError::Accounting)?;
                        self.runtime_reads.representations[current].resolution =
                            ReadResolution::Visiting;
                        if path.len() == path.capacity() {
                            return Err(ResourceError::Accounting);
                        }
                        path.push(current);
                        current = next;
                    }
                }
            };
            while let Some(ordinal) = path.pop() {
                self.ledger.charge(2)?;
                self.runtime_reads.representations[ordinal].resolution =
                    result.map_or(ReadResolution::Unsupported, ReadResolution::Resolved);
            }
        }
        Ok(())
    }

    fn runtime_type(&mut self, value: ValueId) -> Result<Option<&'module Type>, ResourceError> {
        if let Some(ordinal) = self
            .ledger
            .find(&self.runtime_reads.origins, |row| row.value.cmp(&value))?
        {
            return Ok(Some(self.runtime_reads.origins[ordinal].ty));
        }
        if let Some(ordinal) = self
            .ledger
            .find(&self.parameters, |row| row.value.cmp(&value))?
        {
            return Ok(Some(self.parameters[ordinal].ty));
        }
        let Some(operation) = self.definition(value)? else {
            return Ok(None);
        };
        self.ledger.charge(operation.results.len())?;
        Ok(operation
            .results
            .iter()
            .find(|result| result.id == value)
            .map(|result| &result.ty))
    }

    fn runtime_origin(&mut self, value: ValueId) -> Result<Option<ValueId>, ResourceError> {
        Ok(self
            .ledger
            .find(&self.runtime_reads.origins, |row| row.value.cmp(&value))?
            .map_or(Some(value), |ordinal| {
                self.runtime_reads.origins[ordinal].origin
            }))
    }

    fn runtime_index_origin(&mut self, value: ValueId) -> Result<Option<ValueId>, ResourceError> {
        if self.runtime_type(value)? != Some(&Type::INDEX) {
            return Ok(None);
        }
        let Some(origin) = self.runtime_origin(value)? else {
            return Ok(None);
        };
        Ok((self.runtime_type(origin)? == Some(&Type::INDEX)).then_some(origin))
    }

    fn runtime_read_index(&mut self, value: ValueId) -> Result<Option<ReadIndex>, ResourceError> {
        self.ledger.charge(2)?;
        if let Some(origin) = self.runtime_index_origin(value)? {
            let operation = self.definition(origin)?;
            if operation.is_none_or(|op| {
                !matches!(
                    op.kind,
                    OperationKind::Cast {
                        kind: CastKind::Bitcast,
                        ..
                    }
                )
            }) {
                return Ok(Some(ReadIndex::ProvenOrigin(origin)));
            }
        }
        let Some(ordinal) = self
            .ledger
            .find(&self.runtime_reads.representations, |row| {
                row.value.cmp(&value)
            })?
        else {
            return Ok(None);
        };
        self.ledger.charge(4)?;
        let row = self.runtime_reads.representations[ordinal];
        // The bridge is internal: raw guards/GEPs must still use INDEX.
        Ok(match (row.scalar, row.resolution) {
            (ScalarType::Index, ReadResolution::Resolved(key)) => Some(key),
            _ => None,
        })
    }

    fn runtime_slice_parameter(
        &mut self,
        value: ValueId,
    ) -> Result<Option<(ParameterRow<'module>, &'module crate::SliceType)>, ResourceError> {
        let Some(Type::Slice(actual)) = self.runtime_type(value)? else {
            return Ok(None);
        };
        if actual.address_space != AddressSpace::Global
            || !matches!(actual.access, AccessMode::ReadOnly | AccessMode::ReadWrite)
            || actual
                .element
                .as_scalar()
                .and_then(scalar_byte_width)
                .is_none_or(|width| !matches!(width, 1 | 2 | 4 | 8))
        {
            return Ok(None);
        }
        let Some(origin) = self.runtime_origin(value)? else {
            return Ok(None);
        };
        let Some(ordinal) = self
            .ledger
            .find(&self.parameters, |row| row.value.cmp(&origin))?
        else {
            return Ok(None);
        };
        let parameter = self.parameters[ordinal];
        let Type::Slice(formal) = parameter.ty else {
            return Ok(None);
        };
        self.ledger.charge(8)?;
        if formal.element.as_scalar().is_none() || actual != formal {
            return Ok(None);
        }
        Ok(Some((parameter, formal)))
    }

    pub(in super::super) fn runtime_slice_read(
        &mut self,
        location: FunctionOperationLocation,
        pointer: ValueId,
        kind: FormalMemoryAccessKind,
        access: MemoryAccess,
        invocations: InvocationRange1d,
        predicate: Option<ValueId>,
    ) -> Result<Option<FormalMemoryAccess>, ResourceError> {
        self.ledger.charge(24)?;
        if kind != FormalMemoryAccessKind::Read
            || access.address_space != AddressSpace::Global
            || access.volatile
            || predicate.is_some()
            || self.runtime_reads.guards.is_empty()
        {
            return Ok(None);
        }
        let Some(gep) = self.definition(pointer)? else {
            return Ok(None);
        };
        let OperationKind::GetElementPointer { base, offset } = gep.kind else {
            return Ok(None);
        };
        let [result] = gep.results.as_slice() else {
            return Ok(None);
        };
        let Type::Pointer(pointer_type) = &result.ty else {
            return Ok(None);
        };
        let Some(element_bytes) = pointer_byte_width(&result.ty) else {
            return Ok(None);
        };
        if result.id != pointer
            || pointer_type.address_space != AddressSpace::Global
            || !matches!(
                pointer_type.access,
                AccessMode::ReadOnly | AccessMode::ReadWrite
            )
            || !matches!(element_bytes, 1 | 2 | 4 | 8)
            || !access.alignment.is_power_of_two()
            || u64::from(access.alignment) > element_bytes
        {
            return Ok(None);
        }
        let Some(data) = self.definition(base)? else {
            return Ok(None);
        };
        let OperationKind::SliceData { slice } = data.kind else {
            return Ok(None);
        };
        if !single_type(data, &result.ty) {
            return Ok(None);
        }
        let Some((parameter, slice_type)) = self.runtime_slice_parameter(slice)? else {
            return Ok(None);
        };
        self.ledger.charge(8)?;
        if slice_type.element != pointer_type.pointee || slice_type.access != pointer_type.access {
            return Ok(None);
        }
        let Some(index) = self.runtime_read_index(offset)? else {
            return Ok(None);
        };
        let Some(control) = self.control_row(location.block)? else {
            return Ok(None);
        };
        let Some((start, end)) = control.interval else {
            return Ok(None);
        };
        let selected = verification_find_last_by_v1(
            &self.runtime_reads.guards,
            4,
            &mut Budget::new(&mut self.ledger.work, 0),
            |row| match (row.index, row.slice).cmp(&(index, parameter.value)) {
                std::cmp::Ordering::Equal if row.interval.0 <= start => std::cmp::Ordering::Equal,
                std::cmp::Ordering::Equal => std::cmp::Ordering::Greater,
                ordering => ordering,
            },
        )?;
        let Some(selected) = selected else {
            return Ok(None);
        };
        self.ledger.charge(24)?;
        let guard = self.runtime_reads.guards[self.runtime_reads.guards[selected].covering];
        if guard.interval.0 > start || end > guard.interval.1 {
            return Ok(None);
        }
        let domain = FormalRuntimeSliceReadDomainV1 {
            allocation: guard.allocation,
            slice: parameter.value,
            index: offset,
            guard_index: guard.guard_index,
            length: guard.length,
            predicate: guard.predicate,
            pointer,
            element_bytes,
            path: FormalGuardedPathV1::TrueEdge {
                source: guard.edge.source,
                ordinal: guard.edge.ordinal,
                target: guard.edge.target,
            },
        };
        Ok(Some(FormalMemoryAccess {
            location,
            allocation: domain.allocation,
            kind: FormalMemoryAccessKind::Read,
            address_space: AddressSpace::Global,
            byte_offset: ByteExpression::Unbounded,
            byte_width: element_bytes,
            alignment: u64::from(access.alignment),
            invocations,
            domain: FormalAccessDomainV1::RuntimeSliceReadBounded(domain),
        }))
    }
}

#[cfg(test)]
#[path = "runtime_slice_read_v1_tests.rs"]
mod tests;
