/// Closed atomic forms used by the static single-publication protocol.
///
/// A live acquire produces its actual zero-extended u32 value. The two writes
/// retain their exact values in the operation, not in detached proof metadata.
/// These local shapes alone do not establish a publication or source proof.
#[pliron_attr(name = "kernel.publication_atomic", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PublicationAtomicAccessAttr {
    ReleaseRequestU32,
    ReleaseReadyU32,
    AcquireU32,
}

impl PublicationAtomicAccessAttr {
    pub const fn stored_value(self) -> Option<u32> {
        match self {
            Self::ReleaseRequestU32 => Some(1),
            Self::ReleaseReadyU32 => Some(2),
            Self::AcquireU32 => None,
        }
    }
}

impl RankedAccessOp {
    /// Constructs one exact System Release of REQUEST=1 or READY=2.
    pub fn new_publication_store_u32(
        context: &mut Context,
        view: Value,
        index: Value,
        value: u32,
    ) -> Result<Self, RankedMemoryError> {
        let kind = match value {
            1 => PublicationAtomicAccessAttr::ReleaseRequestU32,
            2 => PublicationAtomicAccessAttr::ReleaseReadyU32,
            _ => {
                return Err(RankedMemoryError::MalformedPayload(
                    "invalid publication marker",
                ));
            }
        };
        Self::new_publication_atomic(context, view, index, kind)
    }

    /// Constructs a result-bearing u32 System Acquire, not a symbolic value.
    pub fn new_publication_load_u32(
        context: &mut Context,
        view: Value,
        index: Value,
    ) -> Result<Self, RankedMemoryError> {
        Self::new_publication_atomic(
            context,
            view,
            index,
            PublicationAtomicAccessAttr::AcquireU32,
        )
    }

    fn new_publication_atomic(
        context: &mut Context,
        view: Value,
        index: Value,
        kind: PublicationAtomicAccessAttr,
    ) -> Result<Self, RankedMemoryError> {
        validate_publication_atomic_view(context, view)?;
        if !is_index_type(index, context) {
            return Err(RankedMemoryError::ForeignIndexType { operand: 1 });
        }
        let read = kind == PublicationAtomicAccessAttr::AcquireU32;
        let results = if read {
            vec![IndexType::get(context).into()]
        } else {
            vec![]
        };
        let op = Self::from_operation(Operation::new(
            context,
            Self::get_concrete_op_info(),
            results,
            vec![view, index],
            vec![],
            0,
        ));
        op.set_attr_kernel_access_kind(
            context,
            if read {
                AccessKindAttr::AtomicRead
            } else {
                AccessKindAttr::AtomicWrite
            },
        );
        op.set_attr_kernel_atomic_ordering(
            context,
            if read {
                AtomicOrderingAttr::Acquire
            } else {
                AtomicOrderingAttr::Release
            },
        );
        op.set_attr_kernel_atomic_scope(context, AtomicScopeAttr::System);
        op.set_attr_kernel_publication_atomic(context, kind);
        Ok(op)
    }

    pub fn publication_atomic_kind(
        &self,
        context: &Context,
    ) -> Option<PublicationAtomicAccessAttr> {
        self.get_attr_kernel_publication_atomic(context)
            .map(|kind| *kind)
    }

    pub fn publication_read_result(&self, context: &Context) -> Option<Value> {
        let raw = self.get_operation().deref(context);
        (self.publication_atomic_kind(context) == Some(PublicationAtomicAccessAttr::AcquireU32)
            && raw.get_num_results() == 1)
            .then(|| raw.get_result(0))
    }

    fn verify_publication_atomic(
        &self,
        context: &Context,
        publication: PublicationAtomicAccessAttr,
    ) -> Result<(), RankedMemoryError> {
        let read = publication == PublicationAtomicAccessAttr::AcquireU32;
        let raw = self.get_operation().deref(context);
        if raw.get_num_operands() != 2
            || raw.get_num_results() != usize::from(read)
            || self.kind(context)
                != Some(if read {
                    AccessKindAttr::AtomicRead
                } else {
                    AccessKindAttr::AtomicWrite
                })
            || self.atomic_ordering(context)
                != Some(if read {
                    AtomicOrderingAttr::Acquire
                } else {
                    AtomicOrderingAttr::Release
                })
            || self.atomic_scope(context) != Some(AtomicScopeAttr::System)
            || (read && !is_index_type(raw.get_result(0), context))
        {
            return Err(RankedMemoryError::MalformedPayload(
                "publication atomic changed its exact result, kind, order or scope",
            ));
        }
        validate_publication_atomic_view(context, self.view(context))
    }
}

fn validate_publication_atomic_view(
    context: &Context,
    view: Value,
) -> Result<(), RankedMemoryError> {
    let ty = ranked_view_type(view, context).ok_or(RankedMemoryError::ForeignViewType)?;
    let ty = ty.deref(context);
    if ty.element_width() != 32 || ty.shape() != [DYNAMIC_EXTENT] || !ty.writable() {
        return Err(RankedMemoryError::MalformedPayload(
            "publication atomics require a writable dynamic rank-one u32 view",
        ));
    }
    let definition = view
        .defining_op()
        .ok_or(RankedMemoryError::ForeignViewType)?;
    let definition = Operation::get_op_dyn(definition, context);
    let definition = definition
        .downcast_ref::<RankedViewOp>()
        .ok_or(RankedMemoryError::ForeignViewType)?;
    if definition.memory_space(context) != Some(MemorySpaceAttr::Global) {
        return Err(RankedMemoryError::MalformedPayload(
            "publication atomics require Global storage",
        ));
    }
    Ok(())
}

/// Exact total-read predicate: `index < physical_extent && acquired == 2`.
///
/// Result zero retains the input index; result one is the structural checked
/// condition used by a predicated payload access. The acquired word must be
/// the genuine result of the same-cell publication atomic load. Whole-function
/// analysis must still prove the matching request, unique publisher and roles.
#[pliron_op(
    name = "kernel.publication_read_guard",
    format,
    interfaces = [NResultsInterface<2>, NRegionsInterface<0>]
)]
pub struct PublicationReadGuardOp;

impl PublicationReadGuardOp {
    pub fn new(
        context: &mut Context,
        index: Value,
        physical_extent: Value,
        acquired: Value,
    ) -> Result<Self, RankedMemoryError> {
        validate_publication_read_guard_inputs(context, index, physical_extent, acquired)?;
        Ok(Self::from_operation(Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![
                IndexType::get(context).into(),
                CheckedAccessCapabilityType::get(context).into(),
            ],
            vec![index, physical_extent, acquired],
            vec![],
            0,
        )))
    }

    pub fn index(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_operand(0)
    }
    pub fn physical_extent(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_operand(1)
    }
    pub fn acquired(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_operand(2)
    }
    pub fn result(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_result(0)
    }
    pub fn success(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_result(1)
    }
}

impl Verify for PublicationReadGuardOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        verify_no_regions_results_successors(self, context, 2, 0)?;
        let raw = self.get_operation().deref(context);
        if raw.get_num_operands() != 3
            || payload_attribute_count(&raw) != 0
            || !is_index_type(raw.get_result(0), context)
            || !is_checked_access_capability(raw.get_result(1), context)
        {
            return verify_err!(
                self.loc(context),
                RankedMemoryError::MalformedPayload("publication read guard has malformed payload")
            );
        }
        if let Err(error) = validate_publication_read_guard_inputs(
            context,
            self.index(context),
            self.physical_extent(context),
            self.acquired(context),
        ) {
            return verify_err!(self.loc(context), error);
        }
        Ok(())
    }
}

fn validate_publication_read_guard_inputs(
    context: &Context,
    index: Value,
    extent: Value,
    acquired: Value,
) -> Result<(), RankedMemoryError> {
    for (operand, value) in [index, extent, acquired].into_iter().enumerate() {
        if !is_index_type(value, context) {
            return Err(RankedMemoryError::ForeignIndexType { operand });
        }
    }
    let definition = acquired
        .defining_op()
        .ok_or(RankedMemoryError::MalformedPayload(
            "publication guard requires an actual acquire result",
        ))?;
    let definition = Operation::get_op_dyn(definition, context);
    let acquire =
        definition
            .downcast_ref::<RankedAccessOp>()
            .ok_or(RankedMemoryError::MalformedPayload(
                "publication guard requires an actual acquire result",
            ))?;
    acquire.verify_publication_atomic(context, PublicationAtomicAccessAttr::AcquireU32)?;
    if acquire.publication_atomic_kind(context) != Some(PublicationAtomicAccessAttr::AcquireU32)
        || acquire.publication_read_result(context) != Some(acquired)
        || acquire.indices(context) != [index]
    {
        return Err(RankedMemoryError::MalformedPayload(
            "publication guard changed its acquire result or cell",
        ));
    }
    Ok(())
}
