//! Closed V26 defined family, outer defined tag9. No source authentication is
//! inferred from these inert fields; production replays the complete live plan.
use super::*;

#[path = "defined_reusable_phase_v26/body.rs"]
mod body;
#[path = "defined_reusable_phase_v26/codec.rs"]
mod codec;
#[path = "defined_reusable_phase_v26/incoming.rs"]
mod incoming;

/// Inert V25 operand commitment for a checked completion relay. This helper
/// does not authenticate a zero-sized value or create a completion token.
pub fn canonical_reusable_phase_operand_sha256_v25(
    operand: &SemanticOperandV1,
    remaining_work: &mut u64,
) -> Result<[u8; 32], SemanticMirErrorV1> {
    require(*remaining_work <= HARD_MAX_VALIDATION_WORK_V1)?;
    body::operand_digest(operand, remaining_work)
}

fn require(condition: bool) -> Result<(), SemanticMirErrorV1> {
    condition
        .then_some(())
        .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)
}

fn spend(work: &mut u64, amount: usize) -> Result<(), SemanticMirErrorV1> {
    let amount = u64::try_from(amount).map_err(|_| SemanticMirErrorV1::InvalidFunctionAbi)?;
    let before = *work;
    *work = before.saturating_sub(amount);
    if amount > before {
        return Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::ValidationWork,
            max: before,
            actual: amount,
        });
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticPhaseReferenceKindV1 {
    Shared,
    Unique,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticPhaseReferenceV1 {
    pub reference: SemanticTypeIdV1,
    pub pointee: SemanticTypeIdV1,
    pub kind: SemanticPhaseReferenceKindV1,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticPhaseBrandsV1 {
    pub root_brand: SemanticTypeIdV1,
    pub outer_workgroup_brand: SemanticTypeIdV1,
    pub phase_brand: SemanticTypeIdV1,
    pub dynamic_epoch: SemanticTypeIdV1,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticPhaseCallableV1 {
    pub callable: SemanticCallableIdV1,
    pub identity: SemanticFunctionIdentityV1,
    pub abi: SemanticAbiIdentityV1,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticPhaseCompletionFieldV1 {
    RetainedFinishResult,
    ErasedZstConstant { canonical_operand: [u8; 32] },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticPhaseCompletionDropV1 {
    RetainedPairField,
    ErasedZstConstant { canonical_operand: [u8; 32] },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticPhaseCompletionRelayV1 {
    pub closure_function: SemanticFunctionIdV1,
    pub closure_body: [u8; 32],
    pub finish: SemanticPhaseCallableV1,
    pub finish_call_block: SemanticBlockIdV1,
    pub finish_normal_target: SemanticBlockIdV1,
    pub closure_pack_block: SemanticBlockIdV1,
    pub closure_pack_statement: u32,
    pub closure_return_block: SemanticBlockIdV1,
    pub completion_field: SemanticPhaseCompletionFieldV1,
    pub wrapper_drop: SemanticPhaseCompletionDropV1,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticDefinedReusablePhaseRecipeV1 {
    OwnerConvert {
        workgroup: SemanticTypeIdV1,
        owner: SemanticTypeIdV1,
        root_brand: SemanticTypeIdV1,
        outer_workgroup_brand: SemanticTypeIdV1,
        input_epoch: SemanticTypeIdV1,
    },
    Issue {
        owner_reference: SemanticPhaseReferenceV1,
        owner: SemanticTypeIdV1,
        phase_workgroup: SemanticTypeIdV1,
        brands: SemanticPhaseBrandsV1,
    },
    WithPhase {
        owner_reference: SemanticPhaseReferenceV1,
        owner: SemanticTypeIdV1,
        closure: SemanticTypeIdV1,
        call_tuple: SemanticTypeIdV1,
        phase_workgroup: SemanticTypeIdV1,
        completion: SemanticTypeIdV1,
        result_pair: SemanticTypeIdV1,
        result: SemanticTypeIdV1,
        drop_result: SemanticTypeIdV1,
        brands: SemanticPhaseBrandsV1,
        issue: SemanticPhaseCallableV1,
        invoke: SemanticPhaseCallableV1,
        drop_completion: SemanticPhaseCallableV1,
        issue_block: SemanticBlockIdV1,
        invoke_block: SemanticBlockIdV1,
        drop_block: SemanticBlockIdV1,
        relay: SemanticPhaseCompletionRelayV1,
    },
    Bind {
        phase_reference: SemanticPhaseReferenceV1,
        storage_reference: SemanticPhaseReferenceV1,
        phase_workgroup: SemanticTypeIdV1,
        reusable_storage: SemanticTypeIdV1,
        phase_lds: SemanticTypeIdV1,
        element: SemanticTypeIdV1,
        uninitialized_marker: SemanticTypeIdV1,
        storage_marker: SemanticTypeIdV1,
        thread_marker: SemanticTypeIdV1,
        brands: SemanticPhaseBrandsV1,
        elements: u64,
    },
    Finish {
        workgroup_before_barrier: SemanticTypeIdV1,
        workgroup_after_barrier: SemanticTypeIdV1,
        completion: SemanticTypeIdV1,
        brands: SemanticPhaseBrandsV1,
        input_epoch: SemanticTypeIdV1,
        advanced_epoch: SemanticTypeIdV1,
        barrier: SemanticPhaseCallableV1,
        barrier_block: SemanticBlockIdV1,
        return_block: SemanticBlockIdV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticPhaseIncomingCommitmentV1 {
    count: u32,
    digest: [u8; 32],
}

impl SemanticPhaseIncomingCommitmentV1 {
    /// Reconstruct inert caller metadata using the caller's shared work budget.
    /// This does not authenticate source or attach a capability contract.
    pub fn reconstruct_for_defined_function(
        function: SemanticFunctionIdV1,
        functions: &[SemanticFunctionDeclV1],
        callables: &[SemanticCallableDeclV1],
        remaining_work: &mut u64,
    ) -> Result<Self, SemanticMirErrorV1> {
        require(
            *remaining_work <= HARD_MAX_VALIDATION_WORK_V1
                && functions.len() as u64 <= HARD_MAX_FUNCTIONS_V1
                && callables.len() as u64 <= HARD_MAX_CALLABLES_V1
                && (function.index() as usize) < functions.len(),
        )?;
        incoming::reconstruct(function, functions, callables, remaining_work)
    }

    pub(super) fn from_encoded_parts(
        count: u32,
        digest: [u8; 32],
    ) -> Result<Self, SemanticMirErrorV1> {
        require(count != 0 && u64::from(count) <= HARD_MAX_BLOCKS_V1 && digest != [0; 32])?;
        Ok(Self { count, digest })
    }
    pub const fn count(self) -> u32 {
        self.count
    }
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
}

#[must_use]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticDefinedReusablePhaseV1 {
    function: SemanticFunctionIdV1,
    source_identity: SemanticFunctionIdentityV1,
    abi_identity: SemanticAbiIdentityV1,
    body_identity: [u8; 32],
    provenance: SemanticKernelCapabilityProvenanceV1,
    incoming: SemanticPhaseIncomingCommitmentV1,
    source_binding: [u8; 32],
    recipe: SemanticDefinedReusablePhaseRecipeV1,
}

impl SemanticDefinedReusablePhaseV1 {
    /// Inert canonical construction. Production must independently authenticate
    /// and replay the complete original source owner before attaching this row.
    #[allow(clippy::too_many_arguments)]
    pub fn for_defined_function(
        function: SemanticFunctionIdV1,
        functions: &[SemanticFunctionDeclV1],
        callables: &[SemanticCallableDeclV1],
        types: &[SemanticTypeDeclV1],
        provenance: SemanticKernelCapabilityProvenanceV1,
        source_binding: [u8; 32],
        recipe: SemanticDefinedReusablePhaseRecipeV1,
        remaining_work: &mut u64,
    ) -> Result<Self, SemanticMirErrorV1> {
        Self::for_defined_function_with_dependencies(
            function,
            functions,
            callables,
            types,
            provenance,
            source_binding,
            recipe,
            &[],
            remaining_work,
        )
    }

    /// Typed construction before atomic attachment. Only exact Issue/Finish
    /// dependencies may be supplied; each is fully reconstructed, not trusted
    /// because it carries a source digest. This does not authenticate source.
    #[allow(clippy::too_many_arguments)]
    pub fn for_defined_function_with_dependencies(
        function: SemanticFunctionIdV1,
        functions: &[SemanticFunctionDeclV1],
        callables: &[SemanticCallableDeclV1],
        types: &[SemanticTypeDeclV1],
        provenance: SemanticKernelCapabilityProvenanceV1,
        source_binding: [u8; 32],
        recipe: SemanticDefinedReusablePhaseRecipeV1,
        dependencies: &[SemanticDefinedReusablePhaseV1],
        remaining_work: &mut u64,
    ) -> Result<Self, SemanticMirErrorV1> {
        require(*remaining_work <= HARD_MAX_VALIDATION_WORK_V1)?;
        let function_body = functions
            .get(function.index() as usize)
            .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
        body::validate(function_body, recipe, remaining_work)?;
        body::validate_types(types, recipe, remaining_work)?;
        body::validate_calls(
            functions,
            callables,
            types,
            provenance,
            recipe,
            dependencies,
            remaining_work,
        )?;
        let (body_identity, bytes) =
            canonical_semantic_source_body_sha256_v25(function_body, *remaining_work / 2)?;
        spend(
            remaining_work,
            bytes
                .checked_mul(2)
                .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?,
        )?;
        let incoming = incoming::reconstruct(function, functions, callables, remaining_work)?;
        Self::from_encoded_parts(
            function,
            function_body.identity(),
            function_body.abi().identity(),
            body_identity,
            provenance,
            incoming,
            source_binding,
            recipe,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn from_encoded_parts(
        function: SemanticFunctionIdV1,
        source_identity: SemanticFunctionIdentityV1,
        abi_identity: SemanticAbiIdentityV1,
        body_identity: [u8; 32],
        provenance: SemanticKernelCapabilityProvenanceV1,
        incoming: SemanticPhaseIncomingCommitmentV1,
        source_binding: [u8; 32],
        recipe: SemanticDefinedReusablePhaseRecipeV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        require(
            function != provenance.root()
                && u64::from(function.index()) < HARD_MAX_FUNCTIONS_V1
                && incoming.count != 0
                && u64::from(incoming.count) <= HARD_MAX_BLOCKS_V1
                && [
                    source_identity.as_bytes(),
                    abi_identity.as_bytes(),
                    &body_identity,
                    &incoming.digest,
                    &source_binding,
                ]
                .iter()
                .all(|id| **id != [0; 32]),
        )?;
        recipe.try_visit_types(|id| require(u64::from(id.index()) < HARD_MAX_TYPES_V1))?;
        recipe.try_visit_callables(|c| {
            require(
                u64::from(c.callable.index()) < HARD_MAX_CALLABLES_V1
                    && c.identity.as_bytes() != &[0; 32]
                    && c.abi.as_bytes() != &[0; 32],
            )
        })?;
        if let SemanticDefinedReusablePhaseRecipeV1::WithPhase { relay, .. } = recipe {
            require(
                relay.closure_function != function
                    && u64::from(relay.closure_function.index()) < HARD_MAX_FUNCTIONS_V1
                    && relay.closure_body != [0; 32],
            )?;
        }
        Ok(Self {
            function,
            source_identity,
            abi_identity,
            body_identity,
            provenance,
            incoming,
            source_binding,
            recipe,
        })
    }
    pub const fn function(self) -> SemanticFunctionIdV1 {
        self.function
    }
    pub const fn source_identity(self) -> SemanticFunctionIdentityV1 {
        self.source_identity
    }
    pub const fn abi_identity(self) -> SemanticAbiIdentityV1 {
        self.abi_identity
    }
    pub const fn body_identity(&self) -> &[u8; 32] {
        &self.body_identity
    }
    pub const fn provenance(self) -> SemanticKernelCapabilityProvenanceV1 {
        self.provenance
    }
    pub const fn incoming(self) -> SemanticPhaseIncomingCommitmentV1 {
        self.incoming
    }
    pub const fn source_binding(&self) -> &[u8; 32] {
        &self.source_binding
    }
    pub const fn recipe(self) -> SemanticDefinedReusablePhaseRecipeV1 {
        self.recipe
    }
}

/// Validate the entire, strictly ordered batch before touching a function.
/// No function, ABI, body, source, or local is cloned or rewritten.
pub fn attach_reusable_phase_contracts_v26(
    functions: &mut [SemanticFunctionDeclV1],
    callables: &[SemanticCallableDeclV1],
    types: &[SemanticTypeDeclV1],
    records: &[SemanticDefinedReusablePhaseV1],
    remaining_work: &mut u64,
) -> Result<(), SemanticMirErrorV1> {
    require(*remaining_work <= HARD_MAX_VALIDATION_WORK_V1 && records.len() <= functions.len())?;
    for (index, record) in records.iter().enumerate() {
        spend(remaining_work, 1)?;
        require(index == 0 || records[index - 1].function < record.function)?;
        let function = functions
            .get(record.function.index() as usize)
            .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
        require(
            function.defined_capability_contract.is_none()
                || function.defined_capability_contract
                    == Some(SemanticDefinedCapabilityContractV1::ReusablePhase(*record)),
        )?;
        let fresh = SemanticDefinedReusablePhaseV1::for_defined_function_with_dependencies(
            record.function,
            functions,
            callables,
            types,
            record.provenance,
            record.source_binding,
            record.recipe,
            records,
            remaining_work,
        )?;
        require(fresh == *record)?;
    }
    // Charge even the infallible commit before its first mutation.
    spend(remaining_work, records.len())?;
    for record in records {
        functions[record.function.index() as usize].defined_capability_contract =
            Some(SemanticDefinedCapabilityContractV1::ReusablePhase(*record));
    }
    Ok(())
}

pub(super) fn validate_attachment(
    function: &SemanticFunctionDeclV1,
    record: SemanticDefinedReusablePhaseV1,
) -> Result<(), SemanticMirErrorV1> {
    let mut work = HARD_MAX_VALIDATION_WORK_V1;
    body::validate(function, record.recipe, &mut work)?;
    require(
        function.identity() == record.source_identity
            && function.abi().identity() == record.abi_identity
            && canonical_semantic_source_body_sha256_v25(function, work / 2)?.0
                == record.body_identity,
    )
}

pub(super) fn validate(
    context: &mut ValidationContextV1<'_>,
    function: SemanticFunctionIdV1,
    record: SemanticDefinedReusablePhaseV1,
) -> Result<(), SemanticMirErrorV1> {
    context.one()?;
    require(
        record.function == function
            && kernel_capability_provenance_matches(context.request, record.provenance),
    )?;
    record.recipe.try_visit_types(|ty| {
        context.type_reference(ty, SemanticMirLocationV1::Function(function))
    })?;
    record.recipe.try_visit_callables(|c| {
        context.callable_reference(c.callable, SemanticMirLocationV1::Function(function))
    })?;
    if let SemanticDefinedReusablePhaseRecipeV1::WithPhase { relay, .. } = record.recipe {
        context.function_reference(
            relay.closure_function,
            SemanticMirLocationV1::Function(function),
        )?;
    }
    let before = context
        .limits
        .limit(SemanticMirResourceV1::ValidationWork)
        .saturating_sub(context.work);
    let mut remaining = before;
    let reconstructed = SemanticDefinedReusablePhaseV1::for_defined_function(
        function,
        &context.request.functions,
        &context.request.callables,
        &context.request.types,
        record.provenance,
        record.source_binding,
        record.recipe,
        &mut remaining,
    );
    charge_validation_work(
        context,
        usize::try_from(before - remaining).map_err(|_| SemanticMirErrorV1::InvalidFunctionAbi)?,
    )?;
    require(reconstructed? == record)
}

impl SemanticDefinedReusablePhaseRecipeV1 {
    /// Complete typed-edge visitation, including reference pointees and all
    /// nominal marker types. Equal IDs are allowed only when the closed recipe
    /// requires equality; validation must not globally require unique IDs.
    pub(super) fn try_visit_types<E>(
        self,
        mut visit: impl FnMut(SemanticTypeIdV1) -> Result<(), E>,
    ) -> Result<(), E> {
        use SemanticDefinedReusablePhaseRecipeV1 as R;
        let mut reference = |r: SemanticPhaseReferenceV1| -> Result<(), E> {
            visit(r.reference)?;
            visit(r.pointee)
        };
        match self {
            R::Issue {
                owner_reference, ..
            }
            | R::WithPhase {
                owner_reference, ..
            } => reference(owner_reference)?,
            R::Bind {
                phase_reference,
                storage_reference,
                ..
            } => {
                reference(phase_reference)?;
                reference(storage_reference)?;
            }
            R::OwnerConvert { .. } | R::Finish { .. } => {}
        }
        let (ids, brands): (&[SemanticTypeIdV1], Option<SemanticPhaseBrandsV1>) = match self {
            R::OwnerConvert {
                workgroup,
                owner,
                root_brand,
                outer_workgroup_brand,
                input_epoch,
            } => (
                &[
                    workgroup,
                    owner,
                    root_brand,
                    outer_workgroup_brand,
                    input_epoch,
                ],
                None,
            ),
            R::Issue {
                owner,
                phase_workgroup,
                brands,
                ..
            } => (&[owner, phase_workgroup], Some(brands)),
            R::WithPhase {
                owner,
                closure,
                call_tuple,
                phase_workgroup,
                completion,
                result_pair,
                result,
                drop_result,
                brands,
                ..
            } => (
                &[
                    owner,
                    closure,
                    call_tuple,
                    phase_workgroup,
                    completion,
                    result_pair,
                    result,
                    drop_result,
                ],
                Some(brands),
            ),
            R::Bind {
                phase_workgroup,
                reusable_storage,
                phase_lds,
                element,
                uninitialized_marker,
                storage_marker,
                thread_marker,
                brands,
                ..
            } => (
                &[
                    phase_workgroup,
                    reusable_storage,
                    phase_lds,
                    element,
                    uninitialized_marker,
                    storage_marker,
                    thread_marker,
                ],
                Some(brands),
            ),
            R::Finish {
                workgroup_before_barrier,
                workgroup_after_barrier,
                completion,
                input_epoch,
                advanced_epoch,
                brands,
                ..
            } => (
                &[
                    workgroup_before_barrier,
                    workgroup_after_barrier,
                    completion,
                    input_epoch,
                    advanced_epoch,
                ],
                Some(brands),
            ),
        };
        for id in ids {
            visit(*id)?;
        }
        if let Some(b) = brands {
            for id in [
                b.root_brand,
                b.outer_workgroup_brand,
                b.phase_brand,
                b.dynamic_epoch,
            ] {
                visit(id)?;
            }
        }
        Ok(())
    }

    pub(super) fn try_visit_callables<E>(
        self,
        mut visit: impl FnMut(SemanticPhaseCallableV1) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::WithPhase {
                issue,
                invoke,
                drop_completion,
                relay,
                ..
            } => {
                for call in [issue, invoke, drop_completion, relay.finish] {
                    visit(call)?;
                }
            }
            Self::Finish { barrier, .. } => visit(barrier)?,
            Self::OwnerConvert { .. } | Self::Issue { .. } | Self::Bind { .. } => {}
        }
        Ok(())
    }
}
