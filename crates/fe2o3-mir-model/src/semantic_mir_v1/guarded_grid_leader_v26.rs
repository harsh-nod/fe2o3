//! V26 defined tag 10. Guarded original-source issuance, not GridLeaderCurrent.
use super::*;
include!("guarded_grid_leader_v26/types.rs");
mod body;
mod receiver;

const MAX_BODY_BYTES: u64 = 262_144;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticGuardedGridBodyIdentityV1 {
    pub function: SemanticFunctionIdV1,
    pub source: SemanticFunctionIdentityV1,
    pub abi: SemanticAbiIdentityV1,
    pub body: [u8; 32],
}

#[must_use]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticGuardedGridLeaderV1 {
    origin: SemanticGuardedGridBodyIdentityV1,
    issuer: SemanticGuardedGridBodyIdentityV1,
    caller: SemanticGuardedGridBodyIdentityV1,
    grid_getter: SemanticGuardedGridBodyIdentityV1,
    grid_current: SemanticGuardedGridBodyIdentityV1,
    types: SemanticGuardedGridLeaderTypesV1,
    source: SemanticGuardedGridLeaderSourceV1,
    roles: SemanticGuardedGridLeaderBodyV1,
    provenance: SemanticKernelCapabilityProvenanceV1,
    brand: SemanticTypeIdentityV1,
}

fn require(condition: bool) -> Result<(), SemanticMirErrorV1> {
    if condition {
        Ok(())
    } else {
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    }
}
fn debit(work: &mut u64, amount: usize) -> Result<(), SemanticMirErrorV1> {
    *work = work
        .checked_sub(amount as u64)
        .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
    Ok(())
}
fn defined<'a>(
    functions: &'a [SemanticFunctionDeclV1],
    callables: &[SemanticCallableDeclV1],
    id: SemanticFunctionIdV1,
) -> Result<&'a SemanticFunctionDeclV1, SemanticMirErrorV1> {
    require(callables.get(id.index() as usize) == Some(&SemanticCallableDeclV1::defined(id)))?;
    functions
        .get(id.index() as usize)
        .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)
}
fn identity(
    id: SemanticFunctionIdV1,
    f: &SemanticFunctionDeclV1,
    work: &mut u64,
) -> Result<SemanticGuardedGridBodyIdentityV1, SemanticMirErrorV1> {
    let (body, bytes) =
        canonical_semantic_source_body_sha256_v25(f, (*work / 2).min(MAX_BODY_BYTES))?;
    debit(
        work,
        bytes
            .checked_mul(2)
            .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?,
    )?;
    Ok(SemanticGuardedGridBodyIdentityV1 {
        function: id,
        source: f.identity(),
        abi: f.abi().identity(),
        body,
    })
}

impl SemanticGuardedGridLeaderV1 {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn from_encoded_parts(
        bodies: [SemanticGuardedGridBodyIdentityV1; 5],
        types: SemanticGuardedGridLeaderTypesV1,
        source: SemanticGuardedGridLeaderSourceV1,
        roles: SemanticGuardedGridLeaderBodyV1,
        provenance: SemanticKernelCapabilityProvenanceV1,
        brand: SemanticTypeIdentityV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        let [origin, issuer, caller, grid_getter, grid_current] = bodies;
        require(
            caller.function == source.caller
                && grid_getter.function == source.grid_getter
                && grid_current.function == source.grid_current
                && brand.as_bytes() != &[0; 32],
        )?;
        require(bodies.iter().enumerate().all(|(i, b)| {
            b.body != [0; 32]
                && b.source.as_bytes() != &[0; 32]
                && b.abi.as_bytes() != &[0; 32]
                && !bodies[..i].iter().any(|p| p.function == b.function)
        }))?;
        let blocks = [
            roles.guard,
            roles.issuer,
            roles.some,
            roles.none,
            roles.exit,
        ];
        require(
            blocks
                .iter()
                .enumerate()
                .all(|(i, b)| !blocks[..i].contains(b)),
        )?;
        Ok(Self {
            origin,
            issuer,
            caller,
            grid_getter,
            grid_current,
            types,
            source,
            roles,
            provenance,
            brand,
        })
    }
    /// Inert complete-body/receiver recipe. The importer must authenticate all
    /// five live Rust sources and the brand before this becomes source evidence.
    #[allow(clippy::too_many_arguments)]
    pub fn for_defined_function(
        function: SemanticFunctionIdV1,
        functions: &[SemanticFunctionDeclV1],
        callables: &[SemanticCallableDeclV1],
        declarations: &[SemanticTypeDeclV1],
        types: SemanticGuardedGridLeaderTypesV1,
        source: SemanticGuardedGridLeaderSourceV1,
        provenance: SemanticKernelCapabilityProvenanceV1,
        brand: SemanticTypeIdentityV1,
        work: &mut u64,
    ) -> Result<Self, SemanticMirErrorV1> {
        debit(work, 128)?;
        require(body::types_match(declarations, types) && brand.as_bytes() != &[0; 32])?;
        let getter = defined(functions, callables, function)?;
        let roles = body::observe(getter, types).ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
        let Some(SemanticCallableDeclV1::Defined {
            function: issuer_id,
        }) = callables.get(roles.issuer_callable.index() as usize)
        else {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi);
        };
        let issuer = defined(functions, callables, *issuer_id)?;
        require(body::issuer_matches(issuer, types.leader))?;
        let caller = defined(functions, callables, source.caller)?;
        receiver::observe(caller, function, source, types, callables, work)?;
        let grid_getter = defined(functions, callables, source.grid_getter)?;
        let grid_current = defined(functions, callables, source.grid_current)?;
        require(
            grid_forwarder(grid_getter, types, source.grid_current, callables)
                && grid_current.role() == SemanticFunctionRoleV1::InternalHelper
                && grid_current.export().is_none()
                && grid_current.abi().source_input_types().is_empty()
                && grid_current.abi().source_output_type() == types.grid_option
                && !grid_current.abi().can_unwind(),
        )?;
        let ids = [
            function,
            *issuer_id,
            source.caller,
            source.grid_getter,
            source.grid_current,
        ];
        require(ids.iter().enumerate().all(|(i, id)| !ids[..i].contains(id)))?;
        Self::from_encoded_parts(
            [
                identity(function, getter, work)?,
                identity(*issuer_id, issuer, work)?,
                identity(source.caller, caller, work)?,
                identity(source.grid_getter, grid_getter, work)?,
                identity(source.grid_current, grid_current, work)?,
            ],
            types,
            source,
            roles,
            provenance,
            brand,
        )
    }
    pub const fn function(self) -> SemanticFunctionIdV1 {
        self.origin.function
    }
    pub const fn source_identity(self) -> SemanticFunctionIdentityV1 {
        self.origin.source
    }
    pub const fn abi_identity(self) -> SemanticAbiIdentityV1 {
        self.origin.abi
    }
    pub const fn body_identity(&self) -> &[u8; 32] {
        &self.origin.body
    }
    pub const fn issuer(self) -> SemanticGuardedGridBodyIdentityV1 {
        self.issuer
    }
    pub const fn caller(self) -> SemanticGuardedGridBodyIdentityV1 {
        self.caller
    }
    pub const fn grid_getter(self) -> SemanticGuardedGridBodyIdentityV1 {
        self.grid_getter
    }
    pub const fn grid_current(self) -> SemanticGuardedGridBodyIdentityV1 {
        self.grid_current
    }
    pub const fn types(self) -> SemanticGuardedGridLeaderTypesV1 {
        self.types
    }
    pub const fn source(self) -> SemanticGuardedGridLeaderSourceV1 {
        self.source
    }
    pub const fn roles(self) -> SemanticGuardedGridLeaderBodyV1 {
        self.roles
    }
    pub const fn provenance(self) -> SemanticKernelCapabilityProvenanceV1 {
        self.provenance
    }
    pub const fn brand(self) -> SemanticTypeIdentityV1 {
        self.brand
    }
}

fn grid_forwarder(
    f: &SemanticFunctionDeclV1,
    t: SemanticGuardedGridLeaderTypesV1,
    current: SemanticFunctionIdV1,
    callables: &[SemanticCallableDeclV1],
) -> bool {
    if f.role() != SemanticFunctionRoleV1::InternalHelper
        || f.export().is_some()
        || f.blocks().len() != 2
        || f.locals().len() != 2
        || f.abi().source_input_types() != [t.context_reference]
        || f.abi().source_output_type() != t.grid_option
        || f.abi().can_unwind()
        || f.abi().source_argument_ownership() != [SemanticSourceArgumentOwnershipV1::SharedBorrow]
        || f.abi().canon_abi() != SemanticCanonAbiV1::Rust
        || f.abi().extern_abi() != SemanticExternAbiV1::Rust
    {
        return false;
    }
    let Some(entry) = f.blocks().get(f.entry().index() as usize) else {
        return false;
    };
    let SemanticTerminatorKindV1::Call(c) = entry.terminator().kind() else {
        return false;
    };
    let Some(d) = c.destination() else {
        return false;
    };
    let Some(exit) = f.blocks().get(d.edge().target().index() as usize) else {
        return false;
    };
    entry.statements().is_empty()
        && exit.statements().is_empty()
        && matches!(exit.terminator().kind(), SemanticTerminatorKindV1::Return)
        && callables.get(c.callee().index() as usize)
            == Some(&SemanticCallableDeclV1::defined(current))
        && c.arguments().is_empty()
        && c.unwind() == SemanticUnwindActionV1::Unreachable
        && d.edge().role() == SemanticEdgeRoleV1::CallReturn
        && d.edge().target() != f.entry()
        && d.place().projections().is_empty()
        && d.place().ty() == t.grid_option
        && f.locals()
            .get(d.place().local().index() as usize)
            .is_some_and(|l| l.role() == SemanticLocalRoleV1::Return && l.ty() == t.grid_option)
}

include!("guarded_grid_leader_v26/schema.rs");
#[cfg(test)]
#[path = "guarded_grid_leader_v26/tests.rs"]
mod tests;
