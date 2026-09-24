//! Executed conditional memory transitions over the retained production graph.
//!
//! This is deliberately not the ordinary MIR/Pliron semantic-contract owner.
//! The source request authenticates GPU correspondence; its CPU subject hashes
//! do not authenticate CPU MIR or the association of symbolic input reads. The
//! backend must retain that separate join. Runtime premises are not discharged
//! here, and this module grants no lowering, artifact, or launch authority.

use std::{fmt, fmt::Write as _};

use fe2o3_functional_proof::{FunctionalRefinementBindingV2, FunctionalRefinementBoundaryV2};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_lower_mir_kernel::ProductionSourceBoundConditionalAggregateRequestV1;
use fe2o3_pliron::ProductionConditionalAggregateInputV1;
use fe2o3_proof_contracts::DigestV1;
use sha2::{Digest as _, Sha256};

use crate::functional_refinement_receipt_v2::{
    RetainedImportedFunctionalRefinementReceiptV2,
    execute_and_import_generated_mir_pliron_composition_locally_v1,
    ranked_effect_formula_replay_prelude_v2, with_conditional_memory_formula_v1,
};
use crate::{
    CanonicalGeneratedVerusProofInputV3, FunctionalRefinementVerusExecutionErrorV2,
    FunctionalRefinementVerusRuntimeLeaseV1, MAX_GENERATED_VERUS_PROOF_SOURCE_BYTES_V3,
};

#[path = "conditional_ranked_memory_source_v1.rs"]
mod memory;
#[path = "conditional_ranked_formula_retention_v1.rs"]
mod retention;
#[path = "conditional_ranked_formula_source_v1.rs"]
mod source;
pub use retention::{
    RetainedProductionConditionalFormulaV1, execute_and_retain_conditional_ranked_formula_v1,
};

// Distinct from the old expression-only statement, including byte layout and
// the deliberately shared (not ISA-proved) IEEE operator interpretation.
const OBLIGATION_DOMAIN: &[u8] = b"FE2O3/CONDITIONAL-MEMORY-TRANSITION/V1/LE/SHARED-IEEE\0";
const SOURCE_LIMIT: usize = MAX_GENERATED_VERUS_PROOF_SOURCE_BYTES_V3;

/// Identities of an executed ranked memory-model theorem, not an independently
/// authenticated CPU-reference, hardware numerical, or safe-launch claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionConditionalFormulaReportV1 {
    statement: DigestV1,
    generated_source: DigestV1,
    binding: FunctionalRefinementBindingV2,
    execution: DigestV1,
    receipt: DigestV1,
}

impl ProductionConditionalFormulaReportV1 {
    pub const fn statement_identity(self) -> DigestV1 {
        self.statement
    }
    pub const fn generated_source_identity(self) -> DigestV1 {
        self.generated_source
    }
    pub const fn binding(self) -> FunctionalRefinementBindingV2 {
        self.binding
    }
    pub const fn execution_identity(self) -> DigestV1 {
        self.execution
    }
    pub const fn receipt_identity(self) -> DigestV1 {
        self.receipt
    }
}

/// Borrowed only within the execution callback. No ordinary proof or lowering
/// owner can be recovered from this value; copying a report or wire is inert.
pub struct ProductionConditionalFormulaExecutionV1 {
    report: ProductionConditionalFormulaReportV1,
    retained: RetainedImportedFunctionalRefinementReceiptV2,
}

impl ProductionConditionalFormulaExecutionV1 {
    pub const fn report(&self) -> ProductionConditionalFormulaReportV1 {
        self.report
    }
    pub const fn signed_receipt_wire(&self) -> &[u8] {
        self.retained.wire()
    }
    pub const fn receipt_verifying_key(&self) -> &[u8; 32] {
        self.retained.verifying_key()
    }
}

#[derive(Debug)]
pub enum ProductionConditionalFormulaErrorV1 {
    Resource(Resource),
    Graph(Box<fe2o3_pliron::ProductionConditionalAggregateErrorV1>),
    Execution(FunctionalRefinementVerusExecutionErrorV2),
    Subject(&'static str),
}
impl From<Resource> for ProductionConditionalFormulaErrorV1 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl From<FunctionalRefinementVerusExecutionErrorV2> for ProductionConditionalFormulaErrorV1 {
    fn from(error: FunctionalRefinementVerusExecutionErrorV2) -> Self {
        Self::Execution(error)
    }
}
impl fmt::Display for ProductionConditionalFormulaErrorV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "conditional formula replay: {self:?}")
    }
}
impl std::error::Error for ProductionConditionalFormulaErrorV1 {}
type Error = ProductionConditionalFormulaErrorV1;

/// Replay and execute the exact graph's formulas under explicit host premises.
/// The receipt, policy and generated buffers are dropped before scratch is
/// released. Accepted work and denial history stay on the caller's ledger.
/// Existing effect generation and protected execution retain their separately
/// bounded resource domains; this wrapper does not reset either one's limits.
///
/// A successful callback is not authorization to ignore the CPU-source join,
/// discharge host premises, or bypass any remaining production gate.
///
/// ```compile_fail
/// use fe2o3_verifier::{with_conditional_ranked_formula_execution_v1, FunctionalRefinementVerusRuntimeLeaseV1};
/// use fe2o3_lower_mir_kernel::ProductionSourceBoundConditionalAggregateRequestV1;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn escape(runtime: &FunctionalRefinementVerusRuntimeLeaseV1,
///     request: &ProductionSourceBoundConditionalAggregateRequestV1<'_>,
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let escaped = with_conditional_ranked_formula_execution_v1(
///         runtime, request, budget, 30, |execution, _| execution).unwrap();
///     let _ = escaped.signed_receipt_wire();
/// }
/// ```
pub fn with_conditional_ranked_formula_execution_v1<R>(
    runtime: &FunctionalRefinementVerusRuntimeLeaseV1,
    request: &ProductionSourceBoundConditionalAggregateRequestV1<'_>,
    budget: &mut Budget<'_>,
    timeout_seconds: u32,
    consume: impl for<'proof> FnOnce(
        &'proof ProductionConditionalFormulaExecutionV1,
        &mut Budget<'_>,
    ) -> R,
) -> Result<R, Error> {
    with_scratch(budget, retention::RETAINED_STORAGE, |budget| {
        let retained = execute_formula(runtime, request, budget, timeout_seconds)?;
        let result = consume(&retained.execution, budget);
        current(request.pliron_input(), budget)?;
        Ok(result)
    })
}

// The borrowed and retained wrappers share this exact preparation, execution,
// import and policy check. Neither wrapper accepts caller-generated proof text.
fn execute_formula(
    runtime: &FunctionalRefinementVerusRuntimeLeaseV1,
    request: &ProductionSourceBoundConditionalAggregateRequestV1<'_>,
    budget: &mut Budget<'_>,
    timeout_seconds: u32,
) -> Result<RetainedProductionConditionalFormulaV1, Error> {
    current(request.pliron_input(), budget)?;
    // Replay text, the fixed-capacity writer and its boxed copy can coexist.
    // The legacy generator's intermediate graph and execution domains remain
    // separately bounded; returned text/symbols and this owner are prepaid here.
    with_scratch(budget, retention::PREPARATION_STORAGE, |budget| {
        let prepared = prepare(request, budget)?;
        let (retained, policy) = execute_and_import_generated_mir_pliron_composition_locally_v1(
            runtime,
            prepared.source,
            prepared.binding,
            timeout_seconds,
        )
        .map_err(Error::Execution)?;
        let proof = retained.proof();
        if proof.binding() != prepared.binding
            || proof.boundary() != FunctionalRefinementBoundaryV2::SafeReferenceMirToLivePliron
            || !proof.signature_and_policy_verified()
            || !policy.accepts_signer(proof.signer_identity())
            || policy.toolchain() != proof.toolchain()
        {
            return Err(Error::Subject("imported conditional formula receipt"));
        }
        let accepted_policy = retention::accepted_policy(&retained, &policy)?;
        current(request.pliron_input(), budget)?;
        let execution = ProductionConditionalFormulaExecutionV1 {
            report: ProductionConditionalFormulaReportV1 {
                statement: prepared.binding.normalized_obligation_effect_ir_hash(),
                generated_source: prepared.generated_source,
                binding: prepared.binding,
                execution: proof.execution_identity(),
                receipt: proof.receipt_identity().digest(),
            },
            retained,
        };
        Ok(retention::retain_checked(
            execution,
            accepted_policy,
            request,
        ))
    })
}

// Release only this module's reservation. A downstream consuming callback may
// retain its own owner and charge; resetting all storage to our entry floor
// would silently erase that charge. Unwinding drops proof/source before release.
fn with_scratch<R>(
    budget: &mut Budget<'_>,
    bytes: usize,
    run: impl FnOnce(&mut Budget<'_>) -> Result<R, Error>,
) -> Result<R, Error> {
    use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
    budget.charge_work(1)?;
    let account = budget.work_ledger_identity_v1();
    budget.reserve_storage(bytes)?;
    let protected = budget.storage();
    let result = catch_unwind(AssertUnwindSafe(|| run(budget)));
    let cleanup = if budget.work_ledger_identity_v1() == account && budget.storage() >= protected {
        budget.release_storage(bytes)
    } else {
        Err(Resource::Accounting)
    };
    match result {
        Ok(result) => {
            cleanup?;
            result
        }
        Err(panic) => resume_unwind(panic),
    }
}

fn current(
    input: &ProductionConditionalAggregateInputV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    input
        .require_current_graph_v1(budget)
        .map_err(|error| Error::Graph(Box::new(error)))
}

struct Prepared {
    source: CanonicalGeneratedVerusProofInputV3,
    generated_source: DigestV1,
    binding: FunctionalRefinementBindingV2,
}

// Kept private: callers cannot select source, subjects, or a different premise
// roster. Native admission must rederive this same preparation, not import a
// conditional receipt through the ordinary clean-graph aggregate gate.
fn prepare(
    request: &ProductionSourceBoundConditionalAggregateRequestV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<Prepared, Error> {
    let input = request.pliron_input();
    budget.charge_work(8)?;
    let [output] = input.outputs() else {
        return Err(Error::Subject("conditional output roster"));
    };
    let [staging] = input.retained_policy_checked_refinement_staging() else {
        return Err(Error::Subject("conditional effect staging roster"));
    };
    if staging.boundary() != FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir
        || staging.binding().subjects() != input.reference_subjects()
        || input.typed_root_commitments().is_empty()
        || input.effect_contract(output).is_none()
    {
        return Err(Error::Subject("conditional effect subjects or typed roots"));
    }
    // Charge all new source generation/hash traversals before allocating. Formula
    // replay below retains the preexisting bounded generator, not a fresh ledger.
    budget.charge_work(SOURCE_LIMIT.checked_mul(16).ok_or(Resource::Arithmetic)?)?;
    require_read_roster(request, budget)?;
    let (block, operation) = output.effect_site();
    let generated = with_conditional_memory_formula_v1(
        input.kernel(),
        block as usize,
        operation as usize,
        |formula| -> Result<BoundedSource, Error> {
            let mut generated = BoundedSource::new()?;
            generated
                .write_str("use vstd::prelude::*;\nverus! {\n")
                .map_err(source_limit)?;
            generated
                .write_str(ranked_effect_formula_replay_prelude_v2())
                .map_err(source_limit)?;
            memory::append_memory_theorem(
                &mut generated,
                input.premises(),
                output.canonical_parameter(),
                formula,
                |load, budget| resolve_memory_load(request, load, budget),
                budget,
            )?;
            generated.write_str("}\n").map_err(source_limit)?;
            Ok(generated)
        },
    )?;
    let source = CanonicalGeneratedVerusProofInputV3::new(generated.0.into_bytes())
        .map_err(|_| Error::Subject("canonical conditional source"))?;
    let generated_source = DigestV1::from_untrusted_bytes(source.identity().as_bytes());
    // The conditional statement already commits to exact source/graph, typed
    // roots, subjects, argument/read occurrences and the closed premise roster.
    let obligation = obligation_identity([
        input.identity(),
        generated_source,
        staging.receipt_identity().digest(),
        staging.binding().normalized_obligation_effect_ir_hash(),
        staging.signer_identity(),
        staging.execution_identity(),
    ]);
    let binding =
        FunctionalRefinementBindingV2::from_subjects(input.reference_subjects(), obligation)
            .map_err(|_| Error::Subject("conditional formula binding"))?;
    Ok(Prepared {
        source,
        generated_source,
        binding,
    })
}

fn require_read_roster(
    request: &ProductionSourceBoundConditionalAggregateRequestV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    use fe2o3_pliron::ProductionConditionalRuntimePremiseV1 as P;
    let input = request.pliron_input();
    let output = input.outputs()[0].canonical_parameter();
    let expected = input
        .reads()
        .len()
        .checked_mul(3)
        .and_then(|n| n.checked_add(4))
        .ok_or(Resource::Arithmetic)?;
    if input.premises().len() != expected {
        return Err(Error::Subject("memory executed-read roster"));
    }
    for (read, row) in input
        .reads()
        .iter()
        .zip(input.premises()[4..].chunks_exact(3))
    {
        budget.charge_work(3)?;
        let read = read.canonical();
        let p = read.parameter();
        if row
            != [
                P::ReadableInput {
                    parameter: p,
                    domain: read.access_domain(),
                },
                P::SeparateInputOutput { input: p, output },
                P::RepresentableAddress {
                    parameter: p,
                    domain: read.address_domain(),
                    element_bytes: read.element_bytes(),
                    alignment: read.alignment(),
                },
            ]
        {
            return Err(Error::Subject("memory executed-read premise mismatch"));
        }
    }
    Ok(())
}

fn resolve_memory_load(
    request: &ProductionSourceBoundConditionalAggregateRequestV1<'_>,
    load: &fe2o3_pliron::ProductionSemanticLoadV2,
    budget: &mut Budget<'_>,
) -> Result<memory::MemoryLoad, Error> {
    let mut found = None;
    for read in request.pliron_input().reads() {
        budget.charge_work(1)?;
        if read.site().block != load.block || read.site().operation != load.operation {
            continue;
        }
        if found.is_some()
            || load.view != read.view()
            || load.indices.as_ref() != [read.index()]
            || u64::from(load.scalar.bit_width())
                != read
                    .canonical()
                    .element_bytes()
                    .checked_mul(8)
                    .ok_or(Resource::Arithmetic)?
        {
            return Err(Error::Subject("conditional memory load occurrence"));
        }
        let parameter = read.canonical().parameter();
        // Request construction already rejoins the complete occurrence roster
        // to these checked coordinates, including equal repeated argument rows.
        let argument = read.source();
        budget.charge_work(1)?;
        if argument.canonical_parameter() != parameter {
            return Err(Error::Subject("conditional memory source parameter"));
        }
        require_memory_load_origins(
            request.pliron_input().kernel().blocks(),
            load,
            argument.source_argument(),
            argument.adjusted_argument(),
            budget,
        )?;
        found = Some(memory::MemoryLoad {
            parameter,
            width: read.canonical().element_bytes(),
        });
    }
    found.ok_or(Error::Subject("unbound conditional memory load"))
}

// Both ordinals come from the matched source-checked read, whose full argument
// row is rejoined by request construction. Recipe views use source ordinals;
// the effect DAG (including its CPU expression) uses adjusted ordinals.
// Equality of the two ordinal spaces is not an ABI invariant.
fn require_memory_load_origins(
    blocks: &[fe2o3_pliron::ProductionRankedBlockV1],
    load: &fe2o3_pliron::ProductionSemanticLoadV2,
    source_argument: u32,
    adjusted_argument: u32,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    use fe2o3_pliron::{ProductionRankedOperationV1 as Op, ProductionRankedValueV1 as Value};
    budget.charge_work(3)?;
    if load.allocation_origin != u64::from(adjusted_argument) + 1 {
        return Err(Error::Subject("conditional memory expression origin"));
    }
    let mut found = false;
    for block in blocks {
        budget.charge_work(1)?;
        for operation in block.operations() {
            budget.charge_work(4)?;
            if let Op::View {
                result,
                allocation_origin,
                ..
            }
            | Op::ViewInSpace {
                result,
                allocation_origin,
                ..
            } = operation
                && Value::Local(*result) == load.view
            {
                if found || *allocation_origin != u64::from(source_argument) + 1 {
                    return Err(Error::Subject("conditional memory recipe origin"));
                }
                found = true;
            }
        }
    }
    if !found {
        return Err(Error::Subject("missing conditional memory recipe view"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "conditional_ranked_load_origins_v1_tests.rs"]
mod load_origin_tests;

fn obligation_identity(commitments: [DigestV1; 6]) -> DigestV1 {
    let mut hash = Sha256::new();
    hash.update(OBLIGATION_DOMAIN);
    for identity in commitments {
        hash.update(identity.as_bytes());
    }
    DigestV1::from_untrusted_bytes(hash.finalize().into())
}

struct BoundedSource(String);
impl BoundedSource {
    fn new() -> Result<Self, Error> {
        let mut source = String::new();
        source
            .try_reserve_exact(SOURCE_LIMIT)
            .map_err(|_| Resource::Allocation)?;
        // The reservation above must cover actual retained capacity too.
        if source.capacity() > SOURCE_LIMIT {
            return Err(Resource::Accounting.into());
        }
        Ok(Self(source))
    }
}
impl fmt::Write for BoundedSource {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if self
            .0
            .len()
            .checked_add(value.len())
            .is_none_or(|n| n > SOURCE_LIMIT)
        {
            return Err(fmt::Error);
        }
        self.0.push_str(value);
        Ok(())
    }
}
fn source_limit(_: fmt::Error) -> Error {
    Error::Subject("conditional generated source limit")
}

#[cfg(all(test, target_os = "linux"))]
#[path = "conditional_ranked_formula_development_v1_tests.rs"]
mod development;

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

    #[test]
    fn conditional_obligation_binds_every_ordered_axis_in_a_separate_domain() {
        let commitments =
            std::array::from_fn(|i| DigestV1::from_untrusted_bytes([i as u8 + 1; 32]));
        let original = obligation_identity(commitments);
        assert_eq!(original, obligation_identity(commitments));
        for index in 0..commitments.len() {
            let mut changed = commitments;
            changed[index] = DigestV1::from_untrusted_bytes([99; 32]);
            assert_ne!(original, obligation_identity(changed));
            changed = commitments;
            changed.swap(index, (index + 1) % commitments.len());
            assert_ne!(original, obligation_identity(changed));
        }
        let mut ordinary = Sha256::new();
        ordinary.update(b"FE2O3/MIR-PLIRON/PER-COMPILATION-VERUS-OBLIGATION/V1\0");
        for identity in commitments {
            ordinary.update(identity.as_bytes());
        }
        assert_ne!(
            original,
            DigestV1::from_untrusted_bytes(ordinary.finalize().into())
        );
        let mut old = Sha256::new();
        old.update(b"FE2O3/CONDITIONAL-RANKED-FORMULAS/V1\0");
        for identity in commitments {
            old.update(identity.as_bytes());
        }
        assert_ne!(
            original,
            DigestV1::from_untrusted_bytes(old.finalize().into())
        );
    }

    #[test]
    fn source_writer_never_grows_past_reserved_capacity() {
        let mut source = BoundedSource::new().unwrap();
        let capacity = source.0.capacity();
        for _ in 0..SOURCE_LIMIT / 8 {
            source.write_str("12345678").unwrap();
        }
        assert!(source.write_str("x").is_err());
        assert_eq!(source.0.capacity(), capacity);
        assert_eq!(source.0.len(), SOURCE_LIMIT);
    }

    #[test]
    fn scratch_preserves_downstream_owner_charges_and_work() {
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, 100);
        budget.reserve_storage(7).unwrap();
        with_scratch(&mut budget, 20, |budget| {
            assert_eq!(budget.storage(), 27);
            budget.charge_work(11)?;
            budget.reserve_storage(13)?;
            Ok(())
        })
        .unwrap();
        assert_eq!(
            (budget.storage(), budget.work(), budget.peak_storage()),
            (20, 12, 40)
        );
    }

    #[test]
    fn scratch_is_released_on_refusal_and_unwind() {
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, 100);
        budget.reserve_storage(7).unwrap();
        let result = with_scratch::<()>(&mut budget, 20, |budget| {
            budget.charge_work(11)?;
            Err(Error::Subject("test"))
        });
        assert!(matches!(result, Err(Error::Subject("test"))));
        assert_eq!((budget.storage(), budget.work()), (7, 12));
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = with_scratch::<()>(&mut budget, 20, |_| panic!("consumer unwound"));
        }));
        assert!(panic.is_err());
        assert_eq!((budget.storage(), budget.work()), (7, 13));
    }

    #[test]
    fn scratch_quota_refuses_before_callback_and_remembers_denial() {
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, 19);
        let result = with_scratch::<()>(&mut budget, 20, |_| panic!("quota bypass"));
        assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
        assert_eq!(
            (budget.storage(), budget.work(), budget.failed_storage()),
            (0, 1, Some(20))
        );
    }
}
