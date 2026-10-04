//! Exact original-MIR/Policy11/LICM/forwarding typed execution, not device proof.
//! Public request preparation and signed bytes remain inert. Only the admitted
//! runtime execution below constructs the move-only executed owner.

#[path = "mixed_optimizer_typed_source_lineage_v50.rs"]
mod lineage;

#[path = "mixed_optimizer_typed_source_receipt_view_v53.rs"]
mod inert_view;
pub use inert_view::{InertTypedSourceReceiptV53, check_inert_typed_source_receipt_v53};

use super::*;
use crate::functional_refinement_receipt_v2::{
    MAX_FUNCTIONAL_REFINEMENT_VERUS_OUTPUT_BYTES_V2 as OUTPUT_LIMIT,
    MAX_FUNCTIONAL_REFINEMENT_VERUS_TIMEOUT_SECONDS_V2 as TIMEOUT_LIMIT, validate_proved_output,
};
use crate::functional_refinement_runtime_v1::{
    FunctionalRefinementAttemptV1, FunctionalRefinementRuntimeProcessOutputV1,
};
use crate::{
    FunctionalRefinementVerusRuntimeLeaseV1 as Runtime,
    MixedOptimizerRefinementErrorV26 as ProofError,
    functional_refinement_verus_toolchain_identity_v2,
};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use fe2o3_functional_proof::VerusToolchainIdentityV2;
use rand_core::OsRng;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::time::{Duration, Instant};

type Request<'h, 'n, 'p, 'v, 's> = PreparedTypedSourceTailV50<'h, 'n, 'p, 'v, 's>;
const MAGIC: &[u8] = b"FE2O3/MIXED/TYPED-SOURCE-TAIL/V50\0";
const EXECUTION_DOMAIN: &[u8] = b"FE2O3/V18/POLICY11/TYPED-SOURCE-TAIL/EXECUTION/V50\0";
const EXECUTION_DOMAIN_V66: &[u8] = b"FE2O3/V18/POLICY11/TYPED-SOURCE-TAIL/EXECUTION/V66\0";
const UNSIGNED: usize = MAGIC.len() + 6 + 2 * 32 + 4 * 40 + 4 * 32 + 2 * 8 + 6 * 8 + 8 * 32;
const WIRE: usize = UNSIGNED + 64;

fn refusal(message: &'static str) -> Error {
    ProofError::Receipt(message).into()
}
fn runtime(error: crate::FunctionalRefinementRuntimeErrorV1) -> Error {
    ProofError::Runtime(error).into()
}
fn execution(error: crate::FunctionalRefinementVerusExecutionErrorV2) -> Error {
    ProofError::Execution(error).into()
}
fn count(value: usize) -> Result<u64> {
    value.try_into().map_err(|_| Resource::Arithmetic.into())
}

fn bind_execution_policy(
    digest: &mut Sha256,
    policy: crate::retained_functional_refinement_runtime_v1::GeneratedProofProcessPolicyV2,
    budget: &mut Budget<'_>,
) -> Result<()> {
    if policy.is_legacy() {
        digest.update(EXECUTION_DOMAIN);
    } else {
        budget.check_prior_denials_v1()?;
        // Equal-length domains preserve the old precharge; these bytes are additional.
        budget.charge_work(
            policy
                .canonical_bytes()
                .len()
                .checked_add(8)
                .ok_or(Resource::Arithmetic)?,
        )?;
        digest.update(EXECUTION_DOMAIN_V66);
        digest.update(count(policy.canonical_bytes().len())?.to_le_bytes());
        digest.update(policy.canonical_bytes());
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct Binding {
    policy: u16,
    source: [[u8; 32]; 2],
    graphs: [([u8; 32], u64); 4],
    statement: [u8; 32],
    generated: [u8; 32],
    witness: [u8; 32],
    context: [u8; 32],
    witness_bytes: u64,
    rounds: u64,
    census: [u64; 6],
}
impl Binding {
    fn derive(request: &Request<'_, '_, '_, '_, '_>, budget: &mut Budget<'_>) -> Result<Self> {
        let subject = request.subject(budget)?;
        let prefix = request
            .handoff
            .relocation(budget)?
            .prefix(budget)?
            .output(budget)?;
        let witness = prefix.execution();
        budget.charge_work(witness.canonical_bytes().len())?;
        let witness_digest: [u8; 32] = Sha256::digest(witness.canonical_bytes()).into();
        if witness.policy_version() != 11
            || witness_digest != subject.prefix_execution_identity()
            || !(1..=32).contains(&witness.rounds())
            || request.handoff.store_consensus_v46(budget)?.is_none()
        {
            return Err(refusal(
                "exact Policy11 composed request and complete round witness",
            ));
        }
        let identities = subject.graph_identities();
        let census = subject.census();
        Ok(Self {
            policy: 11,
            source: [
                subject.source_semantic_identity(),
                subject.source_ssa_identity(),
            ],
            graphs: identities.map(|identity| (*identity.digest(), identity.canonical_length())),
            statement: subject.statement_identity(),
            generated: request.generated.identity().as_bytes(),
            witness: witness_digest,
            context: subject.runtime_identity(),
            witness_bytes: count(witness.canonical_bytes().len())?,
            rounds: count(witness.rounds())?,
            census: [
                count(census[0])?,
                count(census[1])?,
                count(census[2])?,
                count(census[3])?,
                count(census[4])?,
                count(census[5])?,
            ],
        })
    }
    fn valid(&self) -> bool {
        self.policy == 11
            && (1..=32).contains(&self.rounds)
            && self.witness_bytes > 0
            && self.graphs.iter().all(|(_, n)| *n > 0)
            && self.census[0] > 0
            && self.census[1] > 0
    }
}

#[derive(Clone, Copy)]
struct Expected {
    binding: Binding,
    runtime: [u8; 32],
    toolchain: [[u8; 32]; 5],
    execution: [u8; 32],
    key: [u8; 32],
}
impl Expected {
    fn unsigned(&self) -> [u8; UNSIGNED] {
        let mut bytes = [0; UNSIGNED];
        let mut cursor = 0;
        let mut put = |value: &[u8]| {
            bytes[cursor..cursor + value.len()].copy_from_slice(value);
            cursor += value.len();
        };
        put(MAGIC);
        put(&50u16.to_le_bytes());
        put(&[4, 1]); // Four-graph typed source trace; executed proved.
        put(&self.binding.policy.to_le_bytes());
        for identity in &self.binding.source {
            put(identity);
        }
        for (identity, length) in &self.binding.graphs {
            put(identity);
            put(&length.to_le_bytes());
        }
        put(&self.binding.statement);
        put(&self.binding.generated);
        put(&self.binding.witness);
        put(&self.binding.context);
        put(&self.binding.witness_bytes.to_le_bytes());
        put(&self.binding.rounds.to_le_bytes());
        for count in &self.binding.census {
            put(&count.to_le_bytes());
        }
        put(&self.runtime);
        for identity in &self.toolchain {
            put(identity);
        }
        put(&self.execution);
        put(&self.key);
        debug_assert_eq!(cursor, UNSIGNED);
        bytes
    }
}
fn import(expected: &Expected, wire: &[u8]) -> Result<()> {
    if wire.len() != WIRE || !expected.binding.valid() {
        return Err(refusal("exact nonempty Policy11 composed receipt"));
    }
    if wire[..UNSIGNED] != expected.unsigned() {
        return Err(refusal(
            "composed owners, statement, round witness, runtime or signer",
        ));
    }
    let key =
        VerifyingKey::from_bytes(&expected.key).map_err(|_| refusal("composed signer encoding"))?;
    if key.is_weak() {
        return Err(refusal("weak composed signer"));
    }
    let signature = Signature::from_slice(&wire[UNSIGNED..])
        .map_err(|_| refusal("composed signature extent"))?;
    key.verify_strict(&wire[..UNSIGNED], &signature)
        .map_err(|_| refusal("strict composed signature"))
}
fn import_metered(expected: &Expected, wire: &[u8], budget: &mut Budget<'_>) -> Result<()> {
    budget.charge_work(WIRE + UNSIGNED)?;
    import(expected, wire)
}

/// Bounded execution preparation over the actual nominal Policy11 request.
/// This owner has not run Verus and grants no proof or publication authority.
#[must_use = "execute or discard while the original request remains retained"]
pub struct PreparedTypedSourceTailExecutionV50<'r, 'h, 'n, 'p, 'v, 's> {
    request: &'r Request<'h, 'n, 'p, 'v, 's>,
    binding: Binding,
    timeout_seconds: u32,
    retained: usize,
    required: usize,
}

/// A successful admitted runtime attempt over the exact composed model.
/// The typed source and actual byte interpreters are included. This is not
/// concrete device, protected compiler, finalizer, or launch authority.
///
/// ```compile_fail
/// use fe2o3_verifier::{PreparedTypedSourceTailExecutionV50, ExecutedTypedSourceTailV50};
/// fn promote<'a>(pending: PreparedTypedSourceTailExecutionV50<'a,'a,'a,'a,'a,'a>)
///     -> ExecutedTypedSourceTailV50<'a,'a,'a,'a,'a,'a> { pending }
/// ```
#[must_use = "retain the request and runtime, then explicitly discard this receipt"]
pub struct ExecutedTypedSourceTailV50<'r, 'h, 'n, 'p, 'v, 's> {
    prepared: PreparedTypedSourceTailExecutionV50<'r, 'h, 'n, 'p, 'v, 's>,
    runtime: &'r Runtime,
    expected: Expected,
    wire: [u8; WIRE],
}

type PreparationCapture<'a, 'w, T> = (&'a T, &'a mut Budget<'w>, usize);
type ExecutionCapture<'a, 'w, T> = (&'a T, &'a Runtime, &'a mut Budget<'w>);
type StaticRequest = Request<'static, 'static, 'static, 'static, 'static>;
type StaticPreparation =
    PreparedTypedSourceTailExecutionV50<'static, 'static, 'static, 'static, 'static, 'static>;

fn headers() -> Result<usize> {
    // Typed fixed frames plus bounded runtime output. Existing retained-runtime
    // internals use their own process/output policy, not this KIR ledger.
    [
        size_of::<
            PreparedTypedSourceTailExecutionV50<
                'static,
                'static,
                'static,
                'static,
                'static,
                'static,
            >,
        >(),
        size_of::<ExecutedTypedSourceTailV50<'static, 'static, 'static, 'static, 'static, 'static>>(
        ),
        size_of::<Binding>(),
        size_of::<Expected>(),
        size_of::<FunctionalRefinementAttemptV1>(),
        size_of::<FunctionalRefinementRuntimeProcessOutputV1>(),
        size_of::<VerusToolchainIdentityV2>(),
        size_of::<SigningKey>() + size_of::<VerifyingKey>() + 2 * size_of::<Signature>(),
        size_of::<Sha256>(),
        2 * size_of::<Instant>() + size_of::<Duration>(),
        size_of::<[&[u8]; 5]>() + size_of::<[usize; 8]>(),
        size_of::<[super::Identity; 4]>() + size_of::<[usize; 6]>(),
        2 * UNSIGNED + WIRE + 4 * OUTPUT_LIMIT,
        size_of::<Result<Binding>>(),
        size_of::<std::thread::Result<Result<Binding>>>(),
        size_of::<Result<(Expected, [u8; WIRE])>>(),
        size_of::<std::thread::Result<Result<(Expected, [u8; WIRE])>>>(),
        size_of::<
            Result<
                ExecutedTypedSourceTailV50<'static, 'static, 'static, 'static, 'static, 'static>,
            >,
        >(),
        size_of::<
            Result<
                PreparedTypedSourceTailExecutionV50<
                    'static,
                    'static,
                    'static,
                    'static,
                    'static,
                    'static,
                >,
            >,
        >(),
        3 * size_of::<Result<()>>(),
        size_of::<PreparationCapture<'static, 'static, StaticRequest>>(),
        align_of::<PreparationCapture<'static, 'static, StaticRequest>>(),
        size_of::<AssertUnwindSafe<PreparationCapture<'static, 'static, StaticRequest>>>(),
        size_of::<ExecutionCapture<'static, 'static, StaticPreparation>>(),
        align_of::<ExecutionCapture<'static, 'static, StaticPreparation>>(),
        size_of::<AssertUnwindSafe<ExecutionCapture<'static, 'static, StaticPreparation>>>(),
    ]
    .into_iter()
    .try_fold(0usize, |n, value| {
        n.checked_add(value).ok_or(Resource::Arithmetic.into())
    })
}

impl<'h, 'n, 'p, 'v, 's> Request<'h, 'n, 'p, 'v, 's> {
    /// Reserve bounded execution frames and recheck the actual composed owner.
    /// No runtime executes and no receipt is constructed by this transition.
    pub fn prepare_typed_execution_v50<'r>(
        &'r self,
        budget: &mut Budget<'_>,
        timeout_seconds: u32,
    ) -> Result<PreparedTypedSourceTailExecutionV50<'r, 'h, 'n, 'p, 'v, 's>> {
        self.check(budget)?;
        if timeout_seconds == 0 || timeout_seconds > TIMEOUT_LIMIT {
            return Err(refusal("bounded composed execution timeout"));
        }
        let retained = headers()?;
        let required = budget
            .storage()
            .checked_add(retained)
            .ok_or(Resource::Arithmetic)?;
        budget
            .reserve_storage(retained)
            .map_err(|error| self.source.retain_query_resource_error_v18(error))?;
        let capture: PreparationCapture<'_, '_, Self> = (self, &mut *budget, required);
        let prepare = move || -> Result<Binding> {
            let (request, budget, required) = std::convert::identity(capture);
            request.replay(budget)?;
            let binding = Binding::derive(request, budget)?;
            if !binding.valid() {
                return Err(refusal("nonempty composed scope"));
            }
            budget.charge_work(
                request
                    .generated
                    .source()
                    .len()
                    .checked_add(4 * WIRE + 2 * OUTPUT_LIMIT)
                    .ok_or(Resource::Arithmetic)?,
            )?;
            request
                .handoff
                .observe_retained_storage_v28(required, budget)?;
            if budget.storage() != required {
                return Err(request
                    .source
                    .retain_query_resource_error_v18(Resource::Accounting)
                    .into());
            }
            Ok(binding)
        };
        #[cfg(test)]
        {
            assert_eq!(
                std::mem::size_of_val(&prepare),
                size_of::<PreparationCapture<'_, '_, Self>>()
            );
            assert_eq!(
                std::mem::align_of_val(&prepare),
                align_of::<PreparationCapture<'_, '_, Self>>()
            );
        }
        let selected = catch_unwind(AssertUnwindSafe(prepare));
        match selected {
            Ok(Ok(binding)) => Ok(PreparedTypedSourceTailExecutionV50 {
                request: self,
                binding,
                timeout_seconds,
                retained,
                required,
            }),
            other => {
                let custody = self.handoff.observe_retained_storage_v28(required, budget);
                if custody.is_ok() {
                    let _ = budget.release_storage(retained);
                }
                match other {
                    Ok(Err(Error::Resource(error))) => {
                        Err(self.source.retain_query_resource_error_v18(error).into())
                    }
                    Ok(Err(error)) => Err(retain(error, self.source)),
                    Err(payload) => resume_unwind(payload),
                    _ => unreachable!(),
                }
            }
        }
    }
}
impl<'r, 'h, 'n, 'p, 'v, 's> PreparedTypedSourceTailExecutionV50<'r, 'h, 'n, 'p, 'v, 's> {
    fn check(&self, budget: &Budget<'_>) -> Result<()> {
        let custody = self
            .request
            .handoff
            .observe_retained_storage_v28(self.required, budget);
        self.request.check(budget)?;
        custody?;
        Ok(())
    }
    /// Complete unmodified round-witness bytes from the retained nominal owner.
    pub fn prefix_witness<'a>(&'a self, budget: &Budget<'_>) -> Result<&'a [u8]> {
        self.check(budget)?;
        Ok(self
            .request
            .handoff
            .relocation(budget)?
            .prefix(budget)?
            .output(budget)?
            .execution()
            .canonical_bytes())
    }
    /// This preparation alone authenticates no proof.
    pub const fn authenticates_executed_proof(&self) -> bool {
        false
    }
    /// Release only this preparation's credit on its original custody ledger.
    pub fn discard(self, budget: &mut Budget<'_>) -> Result<()> {
        let checked = self.check(budget);
        let custody = self
            .request
            .handoff
            .observe_retained_storage_v28(self.required, budget);
        let source = self.request.source;
        let retained = self.retained;
        drop(self);
        let settled = custody.and_then(|()| {
            budget
                .release_storage(retained)
                .map_err(|error| source.retain_query_resource_error_v18(error))
        });
        checked?;
        settled.map_err(Error::from)
    }
    /// Only successful retained-runtime execution can construct an executed owner.
    /// The runtime lease and entire request remain borrowed by the result.
    pub fn execute(
        self,
        runtime_owner: &'r Runtime,
        budget: &mut Budget<'_>,
    ) -> Result<ExecutedTypedSourceTailV50<'r, 'h, 'n, 'p, 'v, 's>> {
        let capture: ExecutionCapture<'_, '_, Self> = (&self, runtime_owner, &mut *budget);
        let execute = move || -> Result<(Expected, [u8; WIRE])> {
            let (prepared, runtime_owner, budget) = std::convert::identity(capture);
            prepared.check(budget)?;
            let deadline = Instant::now()
                .checked_add(Duration::from_secs(u64::from(prepared.timeout_seconds)))
                .ok_or_else(|| refusal("composed execution deadline overflow"))?;
            let mut attempt = runtime_owner.begin_attempt().map_err(runtime)?;
            let toolchain = functional_refinement_verus_toolchain_identity_v2(runtime_owner)
                .map_err(execution)?;
            let observed = runtime_owner
                .execute_generated_rust_verify(
                    &mut attempt,
                    &prepared.request.generated,
                    deadline,
                    OUTPUT_LIMIT,
                )
                .map_err(runtime)?;
            let output_capacity = observed
                .stdout
                .capacity()
                .checked_add(observed.stderr.capacity())
                .ok_or(Resource::Arithmetic)?;
            if output_capacity > 4 * OUTPUT_LIMIT {
                // An unexpectedly overallocated runtime return must be charged
                // before refusal and cannot be refunded through this owner.
                budget.reserve_storage(output_capacity - 4 * OUTPUT_LIMIT)?;
                return Err(prepared
                    .request
                    .source
                    .retain_query_resource_error_v18(Resource::Accounting)
                    .into());
            }
            validate_proved_output(&observed).map_err(execution)?;
            runtime_owner.revalidate().map_err(runtime)?;
            if Instant::now() >= deadline {
                return Err(refusal("composed execution deadline elapsed"));
            }
            let mut digest = Sha256::new();
            bind_execution_policy(&mut digest, observed.policy, budget)?;
            for bytes in [
                runtime_owner.identity().as_bytes().as_slice(),
                prepared.binding.generated.as_slice(),
                prepared.binding.statement.as_slice(),
                observed.stdout.as_slice(),
                observed.stderr.as_slice(),
            ] {
                digest.update(count(bytes.len())?.to_le_bytes());
                digest.update(bytes);
            }
            digest.update(observed.exit_code.unwrap_or(-1).to_le_bytes());
            digest.update(observed.signal.unwrap_or(0).to_le_bytes());
            let signing = SigningKey::generate(&mut OsRng);
            let expected = Expected {
                binding: prepared.binding,
                runtime: runtime_owner.identity().as_bytes(),
                toolchain: [
                    *toolchain.verus_executable().as_bytes(),
                    *toolchain.verus_configuration().as_bytes(),
                    *toolchain.solver_executable().as_bytes(),
                    *toolchain.solver_configuration().as_bytes(),
                    *toolchain.runtime_closure().as_bytes(),
                ],
                execution: digest.finalize().into(),
                key: signing.verifying_key().to_bytes(),
            };
            let unsigned = expected.unsigned();
            let mut wire = [0; WIRE];
            wire[..UNSIGNED].copy_from_slice(&unsigned);
            wire[UNSIGNED..].copy_from_slice(&signing.sign(&unsigned).to_bytes());
            import(&expected, &wire)?;
            prepared.check(budget)?;
            attempt.complete().map_err(runtime)?;
            Ok((expected, wire))
        };
        #[cfg(test)]
        {
            assert_eq!(
                std::mem::size_of_val(&execute),
                size_of::<ExecutionCapture<'_, '_, Self>>()
            );
            assert_eq!(
                std::mem::align_of_val(&execute),
                align_of::<ExecutionCapture<'_, '_, Self>>()
            );
        }
        let selected = catch_unwind(AssertUnwindSafe(execute));
        match selected {
            Ok(Ok((expected, wire))) => Ok(ExecutedTypedSourceTailV50 {
                prepared: self,
                runtime: runtime_owner,
                expected,
                wire,
            }),
            other => {
                let source = self.request.source;
                let settled = self.discard(budget);
                match other {
                    Ok(Err(Error::Resource(error))) => {
                        Err(source.retain_query_resource_error_v18(error).into())
                    }
                    Ok(Err(error)) => {
                        let _ = settled;
                        Err(retain(error, source))
                    }
                    Err(payload) => resume_unwind(payload),
                    _ => unreachable!(),
                }
            }
        }
    }
}
impl ExecutedTypedSourceTailV50<'_, '_, '_, '_, '_, '_> {
    fn check(&self, budget: &Budget<'_>) -> Result<()> {
        self.prepared.check(budget)?;
        if self.runtime.identity().as_bytes() != self.expected.runtime {
            return Err(refusal("retained composed runtime identity"));
        }
        Ok(())
    }
    /// Recheck current runtime custody and strictly replay the locally owned signature.
    pub fn replay_signed_receipt(&self, budget: &mut Budget<'_>) -> Result<()> {
        self.check(budget)?;
        self.runtime.revalidate().map_err(runtime)?;
        import_metered(&self.expected, &self.wire, budget).map_err(|error| match error {
            Error::Resource(error) => self
                .prepared
                .request
                .source
                .retain_query_resource_error_v18(error)
                .into(),
            other => other,
        })
    }
    /// Exact original/prefix/final subject, not an independent admission token.
    pub fn subject(&self, budget: &Budget<'_>) -> Result<TypedSourceTailSubjectV50> {
        self.check(budget)?;
        self.prepared.request.subject(budget)
    }
    /// Local signature transport is inert outside this retained runtime/request owner.
    pub fn signed_receipt(&self, budget: &Budget<'_>) -> Result<&[u8]> {
        self.check(budget)?;
        Ok(&self.wire)
    }
    /// Exact full variable-round witness, never a synthetic one-round replacement.
    pub fn prefix_witness(&self, budget: &Budget<'_>) -> Result<&[u8]> {
        self.check(budget)?;
        self.prepared.prefix_witness(budget)
    }
    /// Execution proves only the generated conditional composed CFG model.
    pub const fn grants_publication_or_launch_authority(&self) -> bool {
        false
    }
    /// Native device semantics and concrete runtime premises remain obligations.
    pub const fn proves_mir_to_native_lowering(&self) -> bool {
        false
    }
    /// Retain the borrowed runtime/request until all local receipt storage is dead.
    pub fn discard(self, budget: &mut Budget<'_>) -> Result<()> {
        let checked = self.check(budget);
        let Self { prepared, .. } = self;
        let settled = prepared.discard(budget);
        checked?;
        settled
    }
}

#[cfg(test)]
#[path = "mixed_optimizer_typed_source_receipt_v50_tests.rs"]
mod tests;
