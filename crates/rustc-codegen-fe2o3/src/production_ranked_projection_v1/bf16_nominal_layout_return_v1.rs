//! C3: qualified helper Matrix/layout and scalar-array return association.
//! Observational, borrowed, and usable only with C1's actual authenticated caller.
//! No output accumulator, final-pass decision, ranked recipe or normal admission.
use super::bf16_nominal_capabilities_v1::AuthenticatedNominalCallerV1;
use super::{AuthenticatedTensorInstructionV1, DigestV1};
use fe2o3_kernel_analysis::CanonicalKirBlockRefV1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirBlockCoordinateV1 as BlockCoordinate,
    CanonicalKirDefinitionCoordinateV1 as DefinitionCoordinate,
    CanonicalKirOperationCoordinateV1 as OperationCoordinate,
    CanonicalKirUseCoordinateV1 as UseCoordinate, OperationKind, TensorLayoutContractV1,
    Terminator, Type, ValueId,
};
use fe2o3_lower_mir_kernel::{
    Bf16CallInstanceRoleV1 as ProducerRole, Bf16NominalCallQueryErrorV1 as Error,
};
use sha2::{Digest, Sha256};
use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};

type Result<T> = std::result::Result<T, Error>;
const DOMAIN: &[u8] = b"FE2O3/NOMINAL-BF16-HELPER-LAYOUT-RETURN/V1\0";
const MAX_HELPER_BLOCKS: usize = 32;
const FIXED_JOIN_WORK: usize = 512;
// All digest input has fixed arity and length, below this byte/check envelope.
const DIGEST_WORK: usize = 2048;

/// Inert qualified locators, never a detached source/capability proof. Equal
/// numeric ValueIds in distinct function namespaces are intentionally distinct.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct NominalReturnComponentV1 {
    caller_definition: DefinitionCoordinate,
    caller_value: ValueId,
    helper_return: UseCoordinate,
    return_value: ValueId,
    matrix_definition: DefinitionCoordinate,
    matrix_value: ValueId,
}
impl NominalReturnComponentV1 {
    pub(super) const fn caller_definition(self) -> DefinitionCoordinate {
        self.caller_definition
    }
    pub(super) const fn caller_value(self) -> ValueId {
        self.caller_value
    }
    pub(super) const fn helper_return(self) -> UseCoordinate {
        self.helper_return
    }
    pub(super) const fn return_value(self) -> ValueId {
        self.return_value
    }
    pub(super) const fn matrix_definition(self) -> DefinitionCoordinate {
        self.matrix_definition
    }
    pub(super) const fn matrix_value(self) -> ValueId {
        self.matrix_value
    }
}

/// Retains the actual authentication and inventory Return row. No Clone/Copy or
/// public constructor exists. C2 selects final-pass use; C3 cannot confer it.
pub(super) struct NominalTensorOccurrenceV1<'a> {
    authenticated: &'a AuthenticatedNominalCallerV1<'a>,
    return_block: &'a CanonicalKirBlockRefV1<'a>,
    call: OperationCoordinate,
    matrix: OperationCoordinate,
    rows: [NominalReturnComponentV1; 4],
    permutation: [u8; 4],
    binding: DigestV1,
}
impl<'a> NominalTensorOccurrenceV1<'a> {
    pub(super) const fn authenticated(&self) -> &'a AuthenticatedNominalCallerV1<'a> {
        self.authenticated
    }
    pub(super) const fn call_coordinate(&self) -> OperationCoordinate {
        self.call
    }
    pub(super) const fn matrix_coordinate(&self) -> OperationCoordinate {
        self.matrix
    }
    pub(super) const fn return_coordinate(&self) -> BlockCoordinate {
        self.return_block.coordinate
    }
    pub(super) const fn return_block(&self) -> &'a CanonicalKirBlockRefV1<'a> {
        self.return_block
    }
    pub(super) const fn return_rows(&self) -> &[NominalReturnComponentV1; 4] {
        &self.rows
    }
    pub(super) const fn permutation(&self) -> [u8; 4] {
        self.permutation
    }
    /// Diagnostic identity only; never accepted without this live association.
    pub(super) const fn binding_digest(&self) -> DigestV1 {
        self.binding
    }
    /// The accumulator here is ONLY the original INPUT, not an array result.
    pub(super) fn input_tensor(&self) -> AuthenticatedTensorInstructionV1 {
        self.authenticated.tensor()
    }
}

fn require(value: bool, why: &'static str) -> Result<()> {
    if value {
        Ok(())
    } else {
        Err(Error::Unavailable(why))
    }
}

fn require_helper_block_bound(body_blocks: usize, start: usize, end: usize) -> Result<()> {
    require(
        body_blocks <= MAX_HELPER_BLOCKS && end.checked_sub(start) == Some(body_blocks),
        "nominal layout helper Return scan exceeds closed block bound",
    )
}

fn unique(values: &[ValueId; 4]) -> bool {
    (0..4).all(|i| (0..i).all(|j| values[i] != values[j]))
}

// Fixed mapping only. Production can call it solely after the borrowed C1/N1
// association and actual Return row have been rejoined; tests label it synthetic.
fn return_rows(
    call: OperationCoordinate,
    matrix: OperationCoordinate,
    helper_return: BlockCoordinate,
    caller: [ValueId; 4],
    returned: [ValueId; 4],
    produced: [ValueId; 4],
    permutation: [u8; 4],
) -> Result<[NominalReturnComponentV1; 4]> {
    require(
        call.block.function != matrix.block.function
            && helper_return.function == matrix.block.function,
        "nominal return function namespaces differ",
    )?;
    require(
        matches!(permutation, [0, 1, 2, 3] | [1, 0, 2, 3]),
        "nominal return permutation outside Identity/Swap01",
    )?;
    require(
        unique(&caller) && unique(&returned) && unique(&produced),
        "nominal return repeats a component within its namespace",
    )?;
    Ok(std::array::from_fn(|j| NominalReturnComponentV1 {
        caller_definition: DefinitionCoordinate::Result {
            operation: call,
            result: j as u32,
        },
        caller_value: caller[j],
        helper_return: UseCoordinate::TerminatorOperand {
            block: helper_return,
            operand: j as u32,
        },
        return_value: returned[j],
        matrix_definition: DefinitionCoordinate::Result {
            operation: matrix,
            result: u32::from(permutation[j]),
        },
        matrix_value: produced[permutation[j] as usize],
    }))
}

// The contents are fixed, inert hash inputs, NOT a capability token. In
// production they are derived only from the retained authenticated association.
#[derive(Clone, Copy)]
struct BindingInput {
    owner_digest: [u8; 32],
    owner_length: u64,
    // root/caller/helper, source Call/Matrix, array local/type/continuation.
    source: [u32; 8],
    // Call(function,ordinal block,op,raw block), Matrix(same), Return(fn,ordinal,raw).
    canonical: [u32; 11],
    // Context, A lane/allocation/noalias/writable/singleton,
    // B lane/allocation/noalias/writable/singleton, acc lane/value/flow.
    input_roots: [u64; 14],
    arguments: [[ValueId; 4]; 3],
    formals: [[ValueId; 4]; 3],
    conversion_values: [ValueId; 4],
    call_values: [ValueId; 4],
    returned: [ValueId; 4],
    produced: [ValueId; 4],
    permutation: [u8; 4],
}
fn binding_digest(input: &BindingInput) -> DigestV1 {
    let mut digest = Sha256::new();
    digest.update((DOMAIN.len() as u64).to_le_bytes());
    digest.update(DOMAIN);
    digest.update(input.owner_digest);
    digest.update(input.owner_length.to_le_bytes());
    // Closed C1/N2a contract: BF16/F32 m16n16k16, Wave64, zero-fill,
    // RowMajor source storage. These tags are not caller-selected metadata.
    digest.update([1, 64, 1, 1]);
    for value in input.source {
        digest.update(value.to_le_bytes());
    }
    for value in input.canonical {
        digest.update(value.to_le_bytes());
    }
    for value in input.input_roots {
        digest.update(value.to_le_bytes());
    }
    for values in input.arguments.into_iter().chain(input.formals) {
        for value in values {
            digest.update(value.0.to_le_bytes());
        }
    }
    for values in [
        input.conversion_values,
        input.call_values,
        input.returned,
        input.produced,
    ] {
        for value in values {
            digest.update(value.0.to_le_bytes());
        }
    }
    digest.update(input.permutation);
    DigestV1::from_untrusted_bytes(digest.finalize().into())
}

fn scope_storage<R>() -> Result<usize> {
    size_of::<NominalTensorOccurrenceV1<'static>>()
        .checked_add(size_of::<BindingInput>())
        .and_then(|n| n.checked_add(size_of::<Sha256>()))
        .and_then(|n| n.checked_add(4096))
        .and_then(|n| {
            size_of::<Result<R>>()
                .checked_mul(2)
                .and_then(|r| n.checked_add(r))
        })
        .ok_or(Resource::Arithmetic.into())
}

// Separate pure custody core permits honest synthetic accounting/unwind tests
// without ever constructing a fake AuthenticatedNominalCallerV1.
fn with_owned_scope<'w, R: Copy + 'static>(
    budget: &mut Budget<'w>,
    body: impl FnOnce(&mut Budget<'w>) -> Result<R>,
) -> Result<R> {
    if budget.failed_work().is_some() || budget.failed_storage().is_some() {
        return Err(Resource::Accounting.into());
    }
    let slot = budget as *const Budget<'w>;
    let ledger = budget.work_ledger_identity_v1();
    let reserved = scope_storage::<R>()?;
    budget.reserve_storage(reserved)?;
    let protected = budget.storage();
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        budget.charge_work(16)?;
        body(budget)
    }));
    let result = match outcome {
        Ok(Ok(_)) if budget.failed_work().is_some() || budget.failed_storage().is_some() => {
            Err(Resource::Accounting.into())
        }
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(Error::CallbackPanicked)
        }
    };
    if budget as *const Budget<'w> != slot
        || budget.work_ledger_identity_v1() != ledger
        || budget.storage() < protected
    {
        return Err(Resource::Accounting.into());
    }
    // The occurrence and all panic payloads have already dropped. No callback
    // surplus refund, no accepted-work refund, and no resetting sticky denial.
    budget.release_storage(reserved)?;
    result
}

/// Must execute within the SAME actual N2b HRTB visitor immediately after C1.
/// It borrows the authentication rather than reconstructing it from a digest.
/// R is only an inert Copy observation; no scoped association may be returned.
pub(super) fn with_nominal_layout_return_v1<'a, 'w, R: Copy + 'static>(
    authenticated: &'a AuthenticatedNominalCallerV1<'a>,
    budget: &mut Budget<'w>,
    inspect: impl for<'scope> FnOnce(
        &'scope NominalTensorOccurrenceV1<'a>,
        &mut Budget<'w>,
    ) -> Result<R>,
) -> Result<R> {
    with_owned_scope(budget, |budget| {
        let occurrence = construct(authenticated, budget)?;
        inspect(&occurrence, budget)
    })
}

fn construct<'a>(
    authenticated: &'a AuthenticatedNominalCallerV1<'a>,
    budget: &mut Budget<'_>,
) -> Result<NominalTensorOccurrenceV1<'a>> {
    budget.charge_work(FIXED_JOIN_WORK)?;
    let site = authenticated.site();
    let candidate = authenticated.candidate();
    let checked = candidate.call();
    let emission = checked.emission();
    let inventory = site.inventory();
    let tensor = authenticated.tensor();
    require(
        std::ptr::eq(site.owner(), emission.owner())
            && inventory.belongs_to(site.owner().executable())
            && checked.belongs_to(inventory)
            && std::ptr::eq(site.call(), checked.source_call()),
        "nominal layout live owner/inventory/call association differs",
    )?;
    require(
        site.root() == emission.root()
            && site.caller() == site.root()
            && site.block() == emission.source_call_block()
            && emission.helper() != site.caller(),
        "nominal layout qualified semantic namespace differs",
    )?;
    let caller = checked.caller();
    let helper = checked.helper();
    require(
        inventory
            .functions()
            .get(caller.coordinate.0 as usize)
            .is_some_and(|row| std::ptr::eq(row, caller))
            && inventory
                .functions()
                .get(helper.coordinate.0 as usize)
                .is_some_and(|row| std::ptr::eq(row, helper)),
        "nominal layout actual function inventory rows differ",
    )?;
    let call = checked.call();
    let matrix = checked.matrix();
    require(
        call.coordinate.block.function == caller.coordinate
            && matrix.coordinate.block.function == helper.coordinate
            && caller.coordinate != helper.coordinate,
        "nominal layout canonical namespaces differ",
    )?;
    let caller_body = caller
        .function
        .body
        .as_ref()
        .ok_or(Error::Unavailable("nominal layout caller body absent"))?;
    let helper_body = helper
        .function
        .body
        .as_ref()
        .ok_or(Error::Unavailable("nominal layout helper body absent"))?;
    let call_block = caller_body
        .blocks
        .get(call.coordinate.block.block as usize)
        .ok_or(Error::Unavailable(
            "nominal layout actual Call block absent",
        ))?;
    let matrix_block = helper_body
        .blocks
        .get(matrix.coordinate.block.block as usize)
        .ok_or(Error::Unavailable(
            "nominal layout actual Matrix block absent",
        ))?;
    require(
        call_block
            .operations
            .get(call.coordinate.operation as usize)
            .is_some_and(|op| std::ptr::eq(op, call.operation))
            && matrix_block
                .operations
                .get(matrix.coordinate.operation as usize)
                .is_some_and(|op| std::ptr::eq(op, matrix.operation))
            && emission.call_site() == (call_block.id, call.coordinate.operation)
            && emission.matrix_site() == (matrix_block.id, matrix.coordinate.operation),
        "nominal layout actual operation occurrence differs",
    )?;
    let required = TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64()
        .with_zero_filled_predicate_inputs();
    let OperationKind::Matrix(actual_matrix) = &matrix.operation.kind else {
        return Err(Error::Unavailable(
            "nominal layout actual Matrix kind differs",
        ));
    };
    require(
        tensor.contract == required
            && candidate.required_tensor_contract() == required
            && actual_matrix.tensor_layout == Some(required)
            && actual_matrix.active_lanes == 64,
        "nominal layout authenticated tensor or Wave64 contract differs",
    )?;

    // N1 already proved the complete argument, conversion and edge-transport
    // relation. Here retain the exact components without comparing unrelated
    // raw IDs or re-authenticating helper formal locals as caller capabilities.
    let OperationKind::Call { arguments, .. } = &call.operation.kind else {
        return Err(Error::Unavailable(
            "nominal layout actual Call kind differs",
        ));
    };
    require(
        arguments.len() == 12
            && helper_body.parameters.len() == 12
            && call.operation.results.len() == 4
            && matrix.operation.results.len() == 4,
        "nominal layout actual argument/result width differs",
    )?;
    require(
        emission.call_argument_components(0) == Some(&[][..])
            && emission.formal_components(0) == Some(&[][..]),
        "nominal layout context logical components differ",
    )?;
    let mut argument_rows = [[ValueId(0); 4]; 3];
    let mut formal_rows = [[ValueId(0); 4]; 3];
    for group in 0..3 {
        let actual = emission
            .call_argument_components(group + 1)
            .ok_or(Error::Unavailable("nominal layout argument group absent"))?;
        let formal = emission
            .formal_components(group + 1)
            .ok_or(Error::Unavailable("nominal layout formal group absent"))?;
        require(
            actual.len() == 4 && formal.len() == 4,
            "nominal layout argument/formal group width differs",
        )?;
        for component in 0..4 {
            require(
                actual[component] == arguments[group * 4 + component]
                    && formal[component] == helper_body.parameters[group * 4 + component],
                "nominal layout actual argument/formal component differs",
            )?;
            argument_rows[group][component] = actual[component];
            formal_rows[group][component] = formal[component];
        }
    }
    let caller_values = *emission.call_results();
    let returned = *emission.helper_return();
    let produced: [ValueId; 4] = emission
        .producer_components(ProducerRole::Result)
        .try_into()
        .map_err(|_| Error::Unavailable("nominal layout Matrix component width differs"))?;
    let conversion_values: [ValueId; 4] = emission
        .producer_components(ProducerRole::Values)
        .try_into()
        .map_err(|_| Error::Unavailable("nominal layout conversion component width differs"))?;
    for index in 0..4 {
        require(
            call.operation.results[index].id == caller_values[index]
                && matrix.operation.results[index].id == produced[index]
                && call.operation.results[index].ty == Type::F32
                && matrix.operation.results[index].ty == Type::F32,
            "nominal layout actual F32 result component differs",
        )?;
    }

    // Scan ONLY the actual helper's bounded block range. The sole Return may
    // use edge parameters. Preserve that actual operand, never demand it equals
    // the Matrix result ID: N1 authenticated its transport ancestry.
    require_helper_block_bound(
        helper_body.blocks.len(),
        helper.blocks.start,
        helper.blocks.end,
    )?;
    let block_rows = inventory
        .blocks()
        .get(helper.blocks.clone())
        .ok_or(Error::Unavailable(
            "nominal layout helper block inventory range differs",
        ))?;
    let mut returned_at = None;
    for (ordinal, block) in block_rows.iter().enumerate() {
        budget.charge_work(8)?;
        require(
            block.coordinate.function == helper.coordinate
                && block.coordinate.block == ordinal as u32
                && std::ptr::eq(block.block, &helper_body.blocks[ordinal])
                && block
                    .block
                    .terminator
                    .as_ref()
                    .is_some_and(|t| std::ptr::eq(t, block.terminator)),
            "nominal layout actual helper block/terminator differs",
        )?;
        if let Terminator::Return { values } = block.terminator {
            require(
                returned_at.is_none(),
                "nominal layout duplicate helper Return",
            )?;
            require(
                values.as_slice() == returned,
                "nominal layout actual helper Return operands differ",
            )?;
            returned_at = Some(block);
        }
    }
    let return_block =
        returned_at.ok_or(Error::Unavailable("nominal layout helper Return absent"))?;
    let permutation = candidate.required_result_permutation();
    require(
        permutation == emission.return_permutation(),
        "nominal layout candidate return permutation differs",
    )?;
    let rows = return_rows(
        call.coordinate,
        matrix.coordinate,
        return_block.coordinate,
        caller_values,
        returned,
        produced,
        permutation,
    )?;
    let owner_identity = site.owner().executable().canonical().identity();
    let destination = site.destination();
    let input = BindingInput {
        owner_digest: *owner_identity.digest(),
        owner_length: owner_identity.canonical_length(),
        source: [
            site.root().index(),
            site.caller().index(),
            emission.helper().index(),
            site.block().index(),
            emission.source_matrix_block().index(),
            destination.local().index(),
            destination.ty().index(),
            destination.continuation().index(),
        ],
        canonical: [
            caller.coordinate.0,
            call.coordinate.block.block,
            call.coordinate.operation,
            call_block.id.0,
            helper.coordinate.0,
            matrix.coordinate.block.block,
            matrix.coordinate.operation,
            matrix_block.id.0,
            return_block.coordinate.function.0,
            return_block.coordinate.block,
            return_block.block.id.0,
        ],
        input_roots: [
            tensor.context_root,
            tensor.lhs.lane_root,
            tensor.lhs.allocation.allocation_origin,
            tensor.lhs.allocation.noalias_class,
            u64::from(tensor.lhs.allocation.writable),
            u64::from(tensor.lhs.allocation.singleton_object),
            tensor.rhs.lane_root,
            tensor.rhs.allocation.allocation_origin,
            tensor.rhs.allocation.noalias_class,
            u64::from(tensor.rhs.allocation.writable),
            u64::from(tensor.rhs.allocation.singleton_object),
            tensor.accumulator.lane_root,
            tensor.accumulator.value_root,
            tensor.accumulator.flow_root,
        ],
        arguments: argument_rows,
        formals: formal_rows,
        conversion_values,
        call_values: caller_values,
        returned,
        produced,
        permutation,
    };
    budget.charge_work(DIGEST_WORK)?;
    let binding = binding_digest(&input);
    Ok(NominalTensorOccurrenceV1 {
        authenticated,
        return_block,
        call: call.coordinate,
        matrix: matrix.coordinate,
        rows,
        permutation,
        binding,
    })
}

#[cfg(test)]
#[path = "bf16_nominal_layout_return_v1_tests.rs"]
mod tests;
