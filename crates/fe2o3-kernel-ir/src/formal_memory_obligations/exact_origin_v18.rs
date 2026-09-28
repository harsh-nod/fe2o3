//! Shared original exact-origin and address-cast grammar.
use super::*;
use std::convert::Infallible;

pub(super) trait Compare {
    type Error;
    fn equal(&mut self, left: &Type, right: &Type) -> Result<bool, Self::Error>;
    fn step(&mut self) -> Result<(), Self::Error>;
}
pub(super) trait State<'source>: Compare {
    fn enter(&mut self, value: ValueId) -> Result<bool, Self::Error>;
    fn origin(&mut self, value: ValueId) -> Result<Option<ValueId>, Self::Error>;
    fn ty(&mut self, value: ValueId) -> Result<Option<&'source Type>, Self::Error>;
    fn operation(&mut self, value: ValueId) -> Result<Option<&'source Operation>, Self::Error>;
}

pub(super) fn run<'source, S: State<'source>>(
    state: &mut S,
    mut current: ValueId,
) -> Result<Option<ValueId>, S::Error> {
    loop {
        state.step()?;
        if !state.enter(current)? {
            return Ok(None);
        }
        let Some(origin) = state.origin(current)? else {
            return Ok(None);
        };
        if origin != current {
            let current_ty = state.ty(current)?;
            let origin_ty = state.ty(origin)?;
            let equal = match (current_ty, origin_ty) {
                (None, None) => true,
                (Some(left), Some(right)) => state.equal(left, right)?,
                _ => false,
            };
            if !equal {
                return Ok(None);
            }
            current = origin;
            continue;
        }
        let Some(operation) = state.operation(current)? else {
            return Ok(Some(current));
        };
        if !matches!(
            operation.kind,
            OperationKind::Cast {
                kind: CastKind::RestrictPointerAccess
                    | CastKind::PointerToGeneric
                    | CastKind::SliceToGeneric,
                ..
            }
        ) {
            return Ok(Some(current));
        }
        let OperationKind::Cast { value: source, .. } = operation.kind else {
            unreachable!()
        };
        let Some(from) = state.ty(source)? else {
            return Ok(None);
        };
        let Some(source) = address(operation, from, state)? else {
            return Ok(None);
        };
        current = source;
    }
}

pub(super) fn address<C: Compare>(
    operation: &Operation,
    from: &Type,
    compare: &mut C,
) -> Result<Option<ValueId>, C::Error> {
    if matches!(
        operation.kind,
        OperationKind::Cast {
            kind: CastKind::SliceToGeneric,
            ..
        }
    ) {
        slice(operation, from, compare)
    } else {
        pointer(operation, from, compare)
    }
}
pub(super) fn slice<C: Compare>(
    operation: &Operation,
    from: &Type,
    compare: &mut C,
) -> Result<Option<ValueId>, C::Error> {
    compare.step()?;
    let OperationKind::Cast {
        kind: CastKind::SliceToGeneric,
        value,
        to,
    } = &operation.kind
    else {
        return Ok(None);
    };
    let [result] = operation.results.as_slice() else {
        return Ok(None);
    };
    let (Type::Slice(from), Type::Slice(target)) = (from, to) else {
        return Ok(None);
    };
    let valid = compare.equal(&result.ty, to)?
        && compare.equal(&from.element, &target.element)?
        && from.access == target.access
        && matches!(
            from.address_space,
            AddressSpace::Global
                | AddressSpace::Constant
                | AddressSpace::Private
                | AddressSpace::Workgroup
        )
        && target.address_space == AddressSpace::Generic
        && (from.address_space != AddressSpace::Constant || from.access == AccessMode::ReadOnly);
    Ok(valid.then_some(*value))
}
pub(super) fn pointer<C: Compare>(
    operation: &Operation,
    from: &Type,
    compare: &mut C,
) -> Result<Option<ValueId>, C::Error> {
    compare.step()?;
    let OperationKind::Cast { kind, value, to } = &operation.kind else {
        return Ok(None);
    };
    let [result] = operation.results.as_slice() else {
        return Ok(None);
    };
    let (Type::Pointer(from), Type::Pointer(target)) = (from, to) else {
        return Ok(None);
    };
    if !compare.equal(&result.ty, to)? || !compare.equal(&from.pointee, &target.pointee)? {
        return Ok(None);
    }
    let valid = match kind {
        CastKind::RestrictPointerAccess => {
            from.address_space == target.address_space
                && from.access == AccessMode::ReadWrite
                && target.access == AccessMode::ReadOnly
        }
        CastKind::PointerToGeneric => {
            matches!(
                from.address_space,
                AddressSpace::Global
                    | AddressSpace::Constant
                    | AddressSpace::Private
                    | AddressSpace::Workgroup
            ) && target.address_space == AddressSpace::Generic
                && from.access == target.access
                && (from.address_space != AddressSpace::Constant
                    || from.access == AccessMode::ReadOnly)
        }
        _ => false,
    };
    Ok(valid.then_some(*value))
}

pub(super) struct LegacyCompare;
impl Compare for LegacyCompare {
    type Error = Infallible;
    fn equal(&mut self, left: &Type, right: &Type) -> Result<bool, Infallible> {
        Ok(left == right)
    }
    fn step(&mut self) -> Result<(), Infallible> {
        Ok(())
    }
}
pub(super) fn infallible<T>(result: Result<T, Infallible>) -> T {
    match result {
        Ok(value) => value,
        Err(never) => match never {},
    }
}

struct Legacy<'view, 'module> {
    definitions: &'view Definitions<'module>,
    types: &'view BTreeMap<ValueId, Type>,
    visited: BTreeSet<ValueId>,
}
impl Compare for Legacy<'_, '_> {
    type Error = Infallible;
    fn equal(&mut self, left: &Type, right: &Type) -> Result<bool, Infallible> {
        Ok(left == right)
    }
    fn step(&mut self) -> Result<(), Infallible> {
        Ok(())
    }
}
impl<'view, 'module: 'view> State<'view> for Legacy<'view, 'module> {
    fn enter(&mut self, value: ValueId) -> Result<bool, Infallible> {
        Ok(self.visited.insert(value))
    }
    fn origin(&mut self, value: ValueId) -> Result<Option<ValueId>, Infallible> {
        Ok(self.definitions.unique_ssa_origin(value))
    }
    fn ty(&mut self, value: ValueId) -> Result<Option<&'view Type>, Infallible> {
        Ok(self.types.get(&value))
    }
    fn operation(&mut self, value: ValueId) -> Result<Option<&'view Operation>, Infallible> {
        Ok(self
            .definitions
            .operations
            .get(&value)
            .map(|(operation, _)| *operation))
    }
}
pub(super) fn legacy(
    definitions: &Definitions<'_>,
    value: ValueId,
    types: &BTreeMap<ValueId, Type>,
) -> Option<ValueId> {
    infallible(run(
        &mut Legacy {
            definitions,
            types,
            visited: BTreeSet::new(),
        },
        value,
    ))
}
