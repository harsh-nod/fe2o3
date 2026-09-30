use super::*;

pub(super) struct Legacy<'query, 'analysis, 'source, 'reader, R> {
    pub(super) definitions: &'analysis Definitions<'source>,
    pub(super) value_types: &'analysis BTreeMap<ValueId, Type>,
    pub(super) context: &'query mut AccessDerivationContext<'analysis, 'source>,
    pub(super) reasons: &'query mut BTreeSet<FormalMemoryIncompleteReason>,
    pub(super) accesses: &'query mut Vec<FormalMemoryAccess>,
    pub(super) effects: &'reader mut R,
    pub(super) ordered: bool,
    pub(super) complete: bool,
}

impl<R: effect_reader_v19::EffectReaderV19> body_engine_v19::State for Legacy<'_, '_, '_, '_, R> {
    type Error = effect_reader_v19::FormalEffectEngineErrorV19<R::Error>;
    fn step(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    fn reachable(&mut self, block: BlockId) -> Result<bool, Self::Error> {
        Ok(self.definitions.is_reachable(block))
    }
    fn proven_private(&mut self, pointer: ValueId) -> Result<bool, Self::Error> {
        let exact = self
            .definitions
            .exact_ssa_origin(pointer, self.value_types)
            .and_then(|origin| self.value_types.get(&origin))
            .is_some_and(
                |ty| matches!(ty, Type::Pointer(p) if p.address_space == AddressSpace::Private),
            );
        Ok(exact
            || if let Some(guarded) = &mut self.context.guarded {
                guarded.proven_pointer_space_v18(pointer)? == Some(AddressSpace::Private)
            } else {
                false
            })
    }
    fn ordered_composition(&self) -> bool {
        self.ordered
    }
    fn complete_body_v19(&self) -> bool {
        self.complete
    }
    fn call_pure(&mut self, operation: &Operation) -> Result<bool, Self::Error> {
        self.effects
            .is_complete_and_pure(operation)
            .map_err(effect_reader_v19::FormalEffectEngineErrorV19::Reader)
    }
    fn call_reason(
        &mut self,
        location: FunctionOperationLocation,
        callee: &FunctionId,
    ) -> Result<(), Self::Error> {
        self.reasons
            .insert(FormalMemoryIncompleteReason::CallEffectsUnavailable {
                location,
                callee: callee.clone(),
            });
        Ok(())
    }
    fn reason(&mut self, reason: FormalMemoryIncompleteReason) -> Result<(), Self::Error> {
        self.reasons.insert(reason);
        Ok(())
    }
    fn access(
        &mut self,
        location: FunctionOperationLocation,
        operation: &Operation,
        invocations: InvocationRange1d,
        conservative: bool,
    ) -> Result<Result<FormalMemoryAccess, FormalMemoryIncompleteReason>, Self::Error> {
        let (pointer, access, kind, predicate) = match &operation.kind {
            OperationKind::Load { pointer, access } => {
                (*pointer, *access, FormalMemoryAccessKind::Read, None)
            }
            OperationKind::Store {
                pointer, access, ..
            } => (*pointer, *access, FormalMemoryAccessKind::Write, None),
            OperationKind::GuardedLoad {
                pointer,
                access,
                predicate,
                ..
            } => (
                *pointer,
                *access,
                FormalMemoryAccessKind::Read,
                Some(*predicate),
            ),
            OperationKind::GuardedStore {
                pointer,
                access,
                predicate,
                ..
            } => (
                *pointer,
                *access,
                FormalMemoryAccessKind::Write,
                Some(*predicate),
            ),
            OperationKind::Atomic(atomic) => (
                atomic.pointer,
                atomic.access,
                FormalMemoryAccessKind::Atomic,
                None,
            ),
            _ => unreachable!("shared formal body memory arm"),
        };
        if conservative {
            return Ok(derive_conservative_guarded_access(
                location,
                pointer,
                access,
                invocations,
                self.context,
            ));
        }
        match derive_access(
            location,
            pointer,
            kind,
            access,
            invocations,
            predicate,
            self.context,
        ) {
            Ok(access) => Ok(Ok(access)),
            Err(AccessDerivationError::Incomplete(reason)) => Ok(Err(reason)),
            Err(AccessDerivationError::Resource(error)) => Err(error.into()),
        }
    }
    fn push(&mut self, access: FormalMemoryAccess) -> Result<(), Self::Error> {
        guarded_access_v1::report_push(&mut self.context.guarded, self.accesses, access)?;
        Ok(())
    }
    fn closed_assembly(&mut self, operation: &Operation) -> Result<bool, Self::Error> {
        Ok(gfx942_inline_u32_v30::has_closed_memory_effects(
            operation,
            self.value_types,
        ))
    }
    fn local_effects_empty(&mut self, operation: &Operation) -> Result<bool, Self::Error> {
        Ok(match &operation.kind {
            OperationKind::Matrix(matrix) => matrix.memory_effects().is_empty(),
            _ => operation.memory_effects().is_empty(),
        })
    }
}
