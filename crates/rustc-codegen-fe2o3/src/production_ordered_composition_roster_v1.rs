//! Bounded import-definition custody. Synthetic tests exercise only this state
//! machine; they cannot manufacture live rustc source authentication.
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBlockIdV1, SemanticBlockIdentityV1, SemanticCallDestinationV1, SemanticCallableIdV1,
    SemanticConstantValueV1, SemanticEdgeRoleV1, SemanticFunctionIdV1,
    SemanticGfx942OrderedProgramRegistersV32, SemanticGfx942U32ProgramV32, SemanticLocalIdV1,
    SemanticOperandV1, SemanticOrderedProgramSourceV32, SemanticTypeIdV1, SemanticUnwindActionV1,
};

pub(super) const MAX_FUNCTIONS: usize = 3;
pub(super) const MAX_MARKERS: usize = 8;
pub(super) const MAX_CALLS: usize = 8;
const MAX_ROWS: usize = MAX_MARKERS + MAX_CALLS;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Site {
    pub(super) function: SemanticFunctionIdV1,
    pub(super) raw_block: u32,
    pub(super) block: SemanticBlockIdV1,
    pub(super) block_identity: SemanticBlockIdentityV1,
}
impl Site {
    fn same_raw(self, other: Self) -> bool {
        self.function == other.function && self.raw_block == other.raw_block
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Operand {
    Copy {
        local: SemanticLocalIdV1,
        ty: SemanticTypeIdV1,
    },
    Move {
        local: SemanticLocalIdV1,
        ty: SemanticTypeIdV1,
    },
    Constant {
        bits: u32,
        bytes: u8,
        ty: SemanticTypeIdV1,
    },
}
impl Operand {
    pub(super) fn matches(self, actual: &SemanticOperandV1) -> bool {
        match (self, actual) {
            (Self::Copy { local, ty }, SemanticOperandV1::Copy(place))
            | (Self::Move { local, ty }, SemanticOperandV1::Move(place)) => {
                place.projections().is_empty() && place.local() == local && place.ty() == ty
            }
            (Self::Constant { bits, bytes, ty }, SemanticOperandV1::Constant(value)) => {
                value.ty() == ty
                    && matches!(value.value(), SemanticConstantValueV1::Scalar(scalar)
                    if scalar.bits() == u128::from(bits) && scalar.size_bytes() == bytes)
            }
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Transport {
    pub(super) arguments: [Option<Operand>; 8],
    pub(super) count: u8,
    pub(super) destination: SemanticLocalIdV1,
    pub(super) result_type: SemanticTypeIdV1,
    pub(super) target: SemanticBlockIdV1,
    pub(super) unwind: SemanticUnwindActionV1,
}
impl Transport {
    pub(super) fn matches(
        self,
        arguments: &[SemanticOperandV1],
        destination: Option<&SemanticCallDestinationV1>,
        unwind: SemanticUnwindActionV1,
    ) -> bool {
        let count = usize::from(self.count);
        count <= self.arguments.len()
            && count == arguments.len()
            && self.arguments[..count]
                .iter()
                .zip(arguments)
                .all(|(expected, actual)| expected.is_some_and(|operand| operand.matches(actual)))
            && self.arguments[count..].iter().all(Option::is_none)
            && self.unwind == unwind
            && destination.is_some_and(|destination| {
                destination.place().projections().is_empty()
                    && destination.place().local() == self.destination
                    && destination.place().ty() == self.result_type
                    && destination.edge().role() == SemanticEdgeRoleV1::CallReturn
                    && destination.edge().target() == self.target
            })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Binding<I> {
    pub(super) caller: I,
    pub(super) callee: I,
    pub(super) callable: SemanticCallableIdV1,
    pub(super) transport: Transport,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Marker {
    pub(super) program: SemanticGfx942U32ProgramV32,
    pub(super) registers: SemanticGfx942OrderedProgramRegistersV32,
}
#[derive(Clone, Copy, Debug)]
pub(super) enum Kind {
    Marker {
        actual: Marker,
        source: SemanticOrderedProgramSourceV32,
    },
    Defined,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct Row<I> {
    pub(super) site: Site,
    pub(super) binding: Binding<I>,
    pub(super) kind: Kind,
    pub(super) consumed: bool,
}

pub(super) struct Observation<'a, I> {
    pub(super) site: Site,
    pub(super) caller: I,
    pub(super) callee: I,
    pub(super) callable: SemanticCallableIdV1,
    pub(super) marker: Option<Marker>,
    pub(super) arguments: &'a [SemanticOperandV1],
    pub(super) destination: Option<&'a SemanticCallDestinationV1>,
    pub(super) unwind: SemanticUnwindActionV1,
}

/// Fixed maximum retained heap payload. Allocator bookkeeping and rustc queries
/// remain outside the semantic-construction work ledger; no RSS claim is made.
pub(super) struct Roster<I> {
    rows: Vec<Row<I>>,
    defined_functions: usize,
    markers: usize,
    calls: usize,
}
impl<I: Copy + Eq> Roster<I> {
    pub(super) fn new(defined_functions: usize) -> Result<Self, &'static str> {
        if !(1..=MAX_FUNCTIONS).contains(&defined_functions) {
            return Err("ordered composition requires one root and at most two helpers");
        }
        let mut rows = Vec::new();
        rows.try_reserve_exact(MAX_ROWS)
            .map_err(|_| "ordered composition roster allocation refused")?;
        if rows.capacity() > MAX_ROWS {
            return Err("ordered composition roster capacity exceeds its bounded payload");
        }
        Ok(Self {
            rows,
            defined_functions,
            markers: 0,
            calls: 0,
        })
    }
    pub(super) fn insert(&mut self, row: Row<I>) -> Result<(), &'static str> {
        if row.consumed || self.rows.iter().any(|old| old.site.same_raw(row.site)) {
            return Err("ordered composition duplicate or preconsumed source definition");
        }
        let (count, maximum) = match row.kind {
            Kind::Marker { .. } => (&mut self.markers, MAX_MARKERS),
            Kind::Defined => (&mut self.calls, MAX_CALLS),
        };
        if *count >= maximum || self.rows.len() >= MAX_ROWS {
            return Err("ordered composition source definition bound exceeded");
        }
        *count += 1;
        self.rows.push(row);
        Ok(())
    }
    pub(super) fn take(
        &mut self,
        actual: Observation<'_, I>,
    ) -> Result<Option<SemanticOrderedProgramSourceV32>, &'static str> {
        let Some(row) = self
            .rows
            .iter_mut()
            .find(|row| row.site.same_raw(actual.site))
        else {
            return if actual.marker.is_some()
                || (actual.callable.index() as usize) < self.defined_functions
            {
                Err("ordered composition source definition missing")
            } else {
                Ok(None)
            };
        };
        if row.consumed {
            return Err("ordered composition source definition was already consumed");
        }
        if row.site != actual.site
            || row.binding.caller != actual.caller
            || row.binding.callee != actual.callee
            || row.binding.callable != actual.callable
            || !row
                .binding
                .transport
                .matches(actual.arguments, actual.destination, actual.unwind)
        {
            return Err(
                "ordered composition source Instance, coordinate, callee or transport differs",
            );
        }
        let source = match (row.kind, actual.marker) {
            (
                Kind::Marker {
                    actual: expected,
                    source,
                },
                Some(found),
            ) if expected == found => Some(source),
            (Kind::Defined, None) => None,
            _ => return Err("ordered composition marker program, roles or call kind differs"),
        };
        row.consumed = true;
        Ok(source)
    }
    pub(super) fn require_drained(&self) -> Result<(), &'static str> {
        if self.markers == 0 || self.rows.iter().any(|row| !row.consumed) {
            Err("ordered composition source definitions were not completely consumed")
        } else {
            Ok(())
        }
    }
}

/// Pure bounded census over actual semantic function/call indices. This proves
/// only counts/reachability/depth; it is not source or effect authentication.
pub(super) fn validate_census(
    root: usize,
    functions: usize,
    markers: &[usize; MAX_FUNCTIONS],
    steps: &[usize; MAX_FUNCTIONS],
    calls: &[(usize, usize)],
) -> Result<(), &'static str> {
    if !(1..=MAX_FUNCTIONS).contains(&functions)
        || root >= functions
        || calls.len() > MAX_CALLS
        || markers[functions..].iter().any(|&count| count != 0)
        || steps[functions..].iter().any(|&count| count != 0)
    {
        return Err("ordered composition function/call census exceeds bounds");
    }
    let static_count = markers[..functions]
        .iter()
        .try_fold(0_usize, |sum, n| sum.checked_add(*n))
        .ok_or("ordered composition census arithmetic overflow")?;
    if !(1..=MAX_MARKERS).contains(&static_count) {
        return Err("ordered composition requires one to eight static marker definitions");
    }
    let mut reachable = [false; MAX_FUNCTIONS];
    reachable[root] = true;
    let mut expanded_count = markers[root];
    let mut expanded_steps = steps[root];
    for &(caller, callee) in calls {
        if caller != root || callee >= functions || callee == root {
            return Err("ordered composition rejects nested, recursive or foreign helper calls");
        }
        reachable[callee] = true;
        expanded_count = expanded_count
            .checked_add(markers[callee])
            .ok_or("ordered composition census arithmetic overflow")?;
        expanded_steps = expanded_steps
            .checked_add(steps[callee])
            .ok_or("ordered composition census arithmetic overflow")?;
    }
    if reachable[..functions].iter().any(|seen| !seen) {
        return Err("ordered composition rejects unreachable helper definitions");
    }
    if expanded_count > MAX_MARKERS || expanded_steps > 128 {
        return Err("ordered composition expanded region/instruction bound exceeded");
    }
    Ok(())
}

#[cfg(test)]
#[path = "production_ordered_composition_roster_v1_tests.rs"]
mod tests;
