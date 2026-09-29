//! A fresh, read-only runtime bound. It never supplies an affine address.
use super::*;

#[derive(Clone, Copy)]
pub(super) struct Origin<'module> {
    pub(super) value: ValueId,
    pub(super) origin: Option<ValueId>,
    pub(super) ty: &'module Type,
}

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub(super) enum ReadIndex {
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
    length_origin: ValueId,
    predicate: ValueId,
    edge: Edge,
    interval: (u32, u32),
    covering: usize,
}

#[derive(Clone, Copy)]
pub(super) struct RuntimeSliceReadConditionsV1 {
    pub(super) domain: FormalRuntimeSliceReadDomainV1,
    pub(super) index_origin: ReadIndex,
    pub(super) length_origin: ValueId,
}

fn read_conditions_frame_bytes<M: GuardMeter>() -> Result<usize, ResourceError> {
    4_usize
        .checked_mul(size_of::<ReadGuard>())
        .and_then(|n| n.checked_add(size_of::<FormalRuntimeSliceReadDomainV1>()))
        .and_then(|n| n.checked_add(2 * size_of::<RuntimeSliceReadConditionsV1>()))
        .and_then(|n| {
            n.checked_add(
                2 * size_of::<Result<Option<RuntimeSliceReadConditionsV1>, ResourceError>>(),
            )
        })
        .and_then(|n| {
            n.checked_add(size_of::<
                Result<Option<FormalRuntimeSliceReadDomainV1>, ResourceError>,
            >())
        })
        .and_then(|n| n.checked_add(size_of::<Option<RuntimeSliceReadConditionsV1>>()))
        .and_then(|n| {
            n.checked_add(size_of::<(
                &mut GuardedAnalysisV1<'_, M>,
                FunctionOperationLocation,
                ValueId,
                FormalMemoryAccessKind,
                MemoryAccess,
                Option<ValueId>,
            )>())
        })
        .ok_or(ResourceError::Arithmetic)
}

fn reserve_read_conditions_frame<M: GuardMeter>(meter: &mut M) -> Result<(), ResourceError> {
    meter.storage(read_conditions_frame_bytes::<M>()?)
}

#[derive(Default)]
pub(super) struct RuntimeReadState<'module> {
    pub(super) origins: Vec<Origin<'module>>,
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

impl<'module, M: GuardMeter> GuardedAnalysisV1<'module, M> {
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
        self.collect_runtime_access_guards_v24::<false>(function)
    }

    pub(super) fn collect_runtime_access_guards_v24<const STORES: bool>(
        &mut self,
        function: &'module Function,
    ) -> Result<(), ResourceError> {
        self.collect_read_representations(function)?;
        // Prepay reused constructor/query carriers once, not on every read.
        reserve_read_conditions_frame(&mut self.ledger)?;
        self.ledger
            .reserve(&mut self.runtime_reads.guards, self.truths.len())?;
        for ordinal in 0..self.truths.len() {
            self.ledger.charge(25)?;
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
            let Some((parameter, _)) = self.runtime_slice_parameter_v24::<STORES>(slice)? else {
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
                length_origin: length,
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

    pub(super) fn runtime_type(
        &mut self,
        value: ValueId,
    ) -> Result<Option<&'module Type>, ResourceError> {
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

    pub(super) fn runtime_origin(
        &mut self,
        value: ValueId,
    ) -> Result<Option<ValueId>, ResourceError> {
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

    fn runtime_slice_parameter_v24<const STORES: bool>(
        &mut self,
        value: ValueId,
    ) -> Result<Option<(ParameterRow<'module>, &'module crate::SliceType)>, ResourceError> {
        let Some(Type::Slice(actual)) = self.runtime_type(value)? else {
            return Ok(None);
        };
        if !matches!(
            actual.address_space,
            AddressSpace::Global | AddressSpace::Generic
        ) || !(matches!(actual.access, AccessMode::ReadOnly | AccessMode::ReadWrite)
            || STORES && actual.access == AccessMode::WriteOnly)
            || actual
                .element
                .as_scalar()
                .and_then(scalar_byte_width)
                .is_none_or(|width| !matches!(width, 1 | 2 | 4 | 8))
        {
            return Ok(None);
        }
        let origin = if actual.address_space == AddressSpace::Generic {
            self.peel_slice_casts(value)?
        } else {
            self.runtime_origin(value)?
        };
        let Some(origin) = origin else {
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
        if formal.address_space != AddressSpace::Global
            || formal.element.as_scalar().is_none()
            || actual.element != formal.element
            || actual.access != formal.access
        {
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
        let Some(domain) =
            self.runtime_slice_read_domain(location, pointer, kind, access, predicate)?
        else {
            return Ok(None);
        };
        Ok(Some(FormalMemoryAccess {
            location,
            allocation: domain.allocation,
            kind: FormalMemoryAccessKind::Read,
            address_space: AddressSpace::Global,
            byte_offset: ByteExpression::Unbounded,
            byte_width: domain.element_bytes,
            alignment: u64::from(access.alignment),
            invocations,
            domain: FormalAccessDomainV1::RuntimeSliceReadBounded(domain),
        }))
    }

    pub(super) fn runtime_slice_read_domain(
        &mut self,
        location: FunctionOperationLocation,
        pointer: ValueId,
        kind: FormalMemoryAccessKind,
        access: MemoryAccess,
        predicate: Option<ValueId>,
    ) -> Result<Option<FormalRuntimeSliceReadDomainV1>, ResourceError> {
        Ok(self
            .runtime_slice_access_conditions_v24::<false>(
                location, pointer, kind, access, predicate,
            )?
            .map(|conditions| conditions.domain))
    }

    pub(super) fn runtime_slice_access_conditions_v24<const STORE: bool>(
        &mut self,
        location: FunctionOperationLocation,
        pointer: ValueId,
        kind: FormalMemoryAccessKind,
        access: MemoryAccess,
        predicate: Option<ValueId>,
    ) -> Result<Option<RuntimeSliceReadConditionsV1>, ResourceError> {
        self.ledger.charge(24)?;
        if kind
            != if STORE {
                FormalMemoryAccessKind::Write
            } else {
                FormalMemoryAccessKind::Read
            }
            || !matches!(
                access.address_space,
                AddressSpace::Global | AddressSpace::Generic
            )
            || access.volatile
            || predicate.is_some()
            || self.runtime_reads.guards.is_empty()
        {
            return Ok(None);
        }
        let Some(pointer_operation) = self.definition(pointer)? else {
            return Ok(None);
        };
        let (gep_pointer, gep) = if matches!(
            pointer_operation.kind,
            OperationKind::Cast {
                kind: CastKind::PointerToGeneric | CastKind::RestrictPointerAccess,
                ..
            }
        ) {
            let Some(source) = self.peel_pointer_casts(pointer)? else {
                return Ok(None);
            };
            let Some(operation) = self.definition(source)? else {
                return Ok(None);
            };
            (source, operation)
        } else {
            (pointer, pointer_operation)
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
        if result.id != gep_pointer
            || !matches!(
                pointer_type.address_space,
                AddressSpace::Global | AddressSpace::Generic
            )
            || !(pointer_type.access == AccessMode::ReadWrite
                || pointer_type.access
                    == if STORE {
                        AccessMode::WriteOnly
                    } else {
                        AccessMode::ReadOnly
                    })
            || !matches!(element_bytes, 1 | 2 | 4 | 8)
            || !access.alignment.is_power_of_two()
            || u64::from(access.alignment) > element_bytes
        {
            return Ok(None);
        }
        let actual_type = if gep_pointer == pointer {
            &result.ty
        } else {
            let Some(actual) = self.runtime_type(pointer)? else {
                return Ok(None);
            };
            actual
        };
        if !matches!(actual_type, Type::Pointer(p) if p.address_space == access.address_space) {
            return Ok(None);
        }
        let Some(base_operation) = self.definition(base)? else {
            return Ok(None);
        };
        if !single_type(base_operation, &result.ty) {
            return Ok(None);
        }
        let data = if matches!(
            base_operation.kind,
            OperationKind::Cast {
                kind: CastKind::PointerToGeneric | CastKind::RestrictPointerAccess,
                ..
            }
        ) {
            let Some(source) = self.peel_pointer_casts(base)? else {
                return Ok(None);
            };
            let Some(operation) = self.definition(source)? else {
                return Ok(None);
            };
            operation
        } else {
            base_operation
        };
        let [data_result] = data.results.as_slice() else {
            return Ok(None);
        };
        let OperationKind::SliceData { slice } = data.kind else {
            return Ok(None);
        };
        let Type::Pointer(data_type) = &data_result.ty else {
            return Ok(None);
        };
        let Some((parameter, slice_type)) = self.runtime_slice_parameter_v24::<STORE>(slice)?
        else {
            return Ok(None);
        };
        self.ledger.charge(8)?;
        if slice_type.element != pointer_type.pointee
            || slice_type.element != data_type.pointee
            || slice_type.access != data_type.access
            || (slice_type.address_space != data_type.address_space
                && !matches!(self.runtime_type(slice)?, Some(Type::Slice(actual))
                    if actual.element == data_type.pointee && actual.access == data_type.access
                        && actual.address_space == data_type.address_space))
        {
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
        let selected = self
            .ledger
            .find_width(&self.runtime_reads.guards, 4, |row| {
                match (row.index, row.slice).cmp(&(index, parameter.value)) {
                    std::cmp::Ordering::Equal if row.interval.0 <= start => {
                        std::cmp::Ordering::Equal
                    }
                    std::cmp::Ordering::Equal => std::cmp::Ordering::Greater,
                    ordering => ordering,
                }
            })?;
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
        self.ledger.charge(4)?;
        Ok(Some(RuntimeSliceReadConditionsV1 {
            domain,
            index_origin: guard.index,
            length_origin: guard.length_origin,
        }))
    }
}

#[cfg(test)]
#[path = "runtime_slice_read_v1_tests.rs"]
pub(super) mod tests;

#[cfg(test)]
#[path = "runtime_slice_read_origin_frames_v1_tests.rs"]
mod origin_frames;
