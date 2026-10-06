//! Distinct source-owned Policy9/Policy10 entrances to the V27 CFG model.
//! Preparation generates obligations; it does not authenticate proof execution.
//!
//! ```compile_fail
//! use fe2o3_verifier::{PreparedMixedWorklistCfgRefinementV27, PreparedMixedPureCseCfgRefinementV27};
//! fn relabel<'h, 'v, 's>(old: PreparedMixedWorklistCfgRefinementV27<'h, 'v, 's>)
//!     -> PreparedMixedPureCseCfgRefinementV27<'h, 'v, 's> { old }
//! ```

use super::*;
use fe2o3_kernel_ir::{
    CanonicalKirTransitionCandidateV1 as Rows, VerifiedCanonicalKernelIrModuleV18 as Owner,
};
use fe2o3_lower_mir_kernel::ProductionConditionalMixedPureCseOutputHandoffV26 as PureCseHandoff;

const CFG_DOMAIN: &[u8] = b"FE2O3/V18/EXACT-TOTAL-OPERATORS/CONDITIONAL-CFG-REFINEMENT/V27\0";

#[path = "mixed_optimizer_cfg_receipt_v27.rs"]
mod receipt;
pub use receipt::{
    ExecutedMixedPureCseCfgRefinementV27, ExecutedMixedWorklistCfgRefinementV27,
    execute_mixed_pure_cse_cfg_refinement_v27, execute_mixed_worklist_cfg_refinement_v27,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MixedOptimizerCfgSubjectV27 {
    source_semantic: [u8; 32],
    source_ssa: [u8; 32],
    input: VerifiedCanonicalKernelIrIdentityV18,
    output: VerifiedCanonicalKernelIrIdentityV18,
    policy: u16,
    execution: [u8; 32],
    statement: [u8; 32],
    functions: usize,
    blocks: usize,
}
impl MixedOptimizerCfgSubjectV27 {
    pub const fn source_semantic_identity(self) -> [u8; 32] {
        self.source_semantic
    }
    pub const fn source_ssa_identity(self) -> [u8; 32] {
        self.source_ssa
    }
    pub const fn input(self) -> VerifiedCanonicalKernelIrIdentityV18 {
        self.input
    }
    pub const fn output(self) -> VerifiedCanonicalKernelIrIdentityV18 {
        self.output
    }
    pub const fn policy_version(self) -> u16 {
        self.policy
    }
    pub const fn execution_identity(self) -> [u8; 32] {
        self.execution
    }
    pub const fn statement_identity(self) -> [u8; 32] {
        self.statement
    }
    pub const fn modeled_functions(self) -> usize {
        self.functions
    }
    pub const fn modeled_blocks(self) -> usize {
        self.blocks
    }
}

enum BoundHandoff<'h, 'v, 's> {
    Worklist(&'h Handoff<'v, 's>),
    PureCse(&'h PureCseHandoff<'v, 's>),
}
struct Output<'a> {
    owner: &'a Owner,
    input_audit: &'a [u8],
    rows: Rows<'a>,
    execution: &'a [u8],
    policy: u16,
}
impl BoundHandoff<'_, '_, '_> {
    fn original(&self, source: &Source<'_>, budget: &mut Budget<'_>) -> Result<()> {
        let original = source.source_ssa(budget)?;
        self.original_owner(original, budget)
    }
    fn original_owner(
        &self,
        original: &fe2o3_pliron::ProductionSemanticSsaOwnerV1,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        match self {
            Self::Worklist(handoff) => handoff.check_original_source(original, budget)?,
            Self::PureCse(handoff) => handoff.check_original_source(original, budget)?,
        }
        Ok(())
    }
    fn storage(
        &self,
        required: usize,
        budget: &Budget<'_>,
    ) -> std::result::Result<(), SourceError> {
        match self {
            Self::Worklist(handoff) => handoff.observe_retained_storage_v18(required, budget),
            Self::PureCse(handoff) => handoff.observe_retained_storage_v18(required, budget),
        }
    }
    fn output(&self, budget: &Budget<'_>) -> Result<Output<'_>> {
        macro_rules! exact {
            ($handoff:expr, $policy:literal) => {{
                let owner = $handoff.output(budget)?;
                let witness = owner.execution();
                if witness.policy_version() != $policy || witness.graph_schema() != 18 {
                    return Err(Error::Statement("nominal mixed CFG policy and schema"));
                }
                Output {
                    owner: owner.owner(),
                    input_audit: owner.input_audit_bytes(),
                    rows: owner.occurrences().candidate(),
                    execution: witness.canonical_bytes(),
                    policy: $policy,
                }
            }};
        }
        Ok(match self {
            Self::Worklist(handoff) => exact!(handoff, 9),
            Self::PureCse(handoff) => exact!(handoff, 10),
        })
    }
}

struct Prepared<'h, 'v, 's> {
    source: &'h Source<'s>,
    handoff: BoundHandoff<'h, 'v, 's>,
    generated: CanonicalGeneratedVerusProofInputV3,
    subject: MixedOptimizerCfgSubjectV27,
    retained: usize,
    required: usize,
}
impl Prepared<'_, '_, '_> {
    fn check(&self, budget: &Budget<'_>) -> Result<()> {
        self.source.check_query_v18(budget)?;
        self.handoff.storage(self.required, budget)?;
        Ok(())
    }
    fn discard(self, budget: &mut Budget<'_>) -> Result<()> {
        let checked = self.check(budget);
        let custody = self.handoff.storage(self.required, budget);
        let Self {
            source,
            generated,
            retained,
            ..
        } = self;
        drop(generated);
        let settled = custody.and_then(|()| {
            budget
                .release_storage(retained)
                .map_err(|error| source.retain_query_resource_error_v18(error))
        });
        checked?;
        settled?;
        Ok(())
    }
}

macro_rules! request {
    ($name:ident, $prepare:ident, $handoff:ident, $variant:ident) => {
        /// Move-only request retaining its exact nominal source/optimized pair.
        /// It is neither a V26 block receipt nor an executed CFG theorem.
        #[must_use = "retain or explicitly settle the generated CFG request"]
        pub struct $name<'h, 'v, 's>(Prepared<'h, 'v, 's>);
        impl $name<'_, '_, '_> {
            pub fn subject(&self, budget: &Budget<'_>) -> Result<MixedOptimizerCfgSubjectV27> {
                self.0.check(budget)?;
                Ok(self.0.subject)
            }
            /// Replays association with the exact retained SSA owner, not an
            /// independently reconstructed equal-byte source.
            pub fn check_original_source(
                &self,
                original: &fe2o3_pliron::ProductionSemanticSsaOwnerV1,
                budget: &mut Budget<'_>,
            ) -> Result<()> {
                self.0.check(budget)?;
                self.0.handoff.original_owner(original, budget)
            }
            pub fn generated_source(&self, budget: &Budget<'_>) -> Result<&[u8]> {
                self.0.check(budget)?;
                Ok(self.0.generated.source())
            }
            pub fn retained_storage(&self, budget: &Budget<'_>) -> Result<usize> {
                self.0.check(budget)?;
                Ok(self.0.retained)
            }
            pub fn discard(self, budget: &mut Budget<'_>) -> Result<()> {
                self.0.discard(budget)
            }
            pub const fn authenticates_executed_proof(&self) -> bool {
                false
            }
            pub const fn proves_mir_to_native_lowering(&self) -> bool {
                false
            }
            pub const fn grants_artifact_or_launch_authority(&self) -> bool {
                false
            }
        }
        pub fn $prepare<'h, 'v, 's>(
            source: &'h Source<'s>,
            handoff: &'h $handoff<'v, 's>,
            budget: &mut Budget<'_>,
        ) -> Result<$name<'h, 'v, 's>> {
            prepare(source, BoundHandoff::$variant(handoff), budget).map($name)
        }
    };
}
request!(
    PreparedMixedWorklistCfgRefinementV27,
    prepare_mixed_worklist_cfg_refinement_v27,
    Handoff,
    Worklist
);
request!(
    PreparedMixedPureCseCfgRefinementV27,
    prepare_mixed_pure_cse_cfg_refinement_v27,
    PureCseHandoff,
    PureCse
);

fn prepare<'h, 'v, 's>(
    source: &'h Source<'s>,
    handoff: BoundHandoff<'h, 'v, 's>,
    budget: &mut Budget<'_>,
) -> Result<Prepared<'h, 'v, 's>> {
    source.check_query_v18(budget)?;
    handoff.original(source, budget)?;
    let floor = budget.storage();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let source_ssa = source.source_ssa(budget)?;
        let input_owner = source.canonical(budget)?;
        let optimized = handoff.output(budget)?;
        let bytes = input_owner.canonical_bytes();
        budget.charge_work(
            bytes
                .len()
                .checked_add(optimized.input_audit.len())
                .and_then(|count| count.checked_add(optimized.execution.len()))
                .ok_or(Resource::Accounting)?,
        )?;
        if bytes != optimized.input_audit {
            return Err(Error::Statement(
                "exact original mixed CFG optimizer audit bytes",
            ));
        }
        budget.reserve_storage(2 * SOURCE_LIMIT + size_of::<Prepared<'_, '_, '_>>())?;
        let (input, input_storage) = Inventory::derive_v18(input_owner, budget)?;
        budget.reserve_storage(input_storage.retained_storage())?;
        let (output, output_storage) = Inventory::derive_v18(optimized.owner, budget)?;
        budget.reserve_storage(output_storage.retained_storage())?;
        let (transition, checked_storage) =
            check_canonical_kir_transition_v18(&input, &output, optimized.rows, budget)?;
        budget.reserve_storage(checked_storage.retained_storage())?;
        budget.charge_work(input.functions().len())?;
        let functions = input
            .functions()
            .iter()
            .filter(|function| !function.blocks.is_empty())
            .count();
        if functions == 0 {
            return Err(Error::Statement("nonempty mixed CFG function census"));
        }
        let mut writer = Writer::new(budget)?;
        let blocks = semantics::generate_cfg_v27(&input, &output, transition.rows(), &mut writer)?;
        let text = writer.finish()?;
        budget.charge_work(text.len().checked_mul(3).ok_or(Resource::Accounting)?)?;
        let generated = CanonicalGeneratedVerusProofInputV3::new(text.into_bytes())?;
        let execution: [u8; 32] = Sha256::digest(optimized.execution).into();
        let mut digest = Sha256::new();
        digest.update(CFG_DOMAIN);
        digest.update(source_ssa.source_semantic_sha256());
        digest.update(source_ssa.identity().as_bytes());
        digest.update(optimized.policy.to_le_bytes());
        digest.update(execution);
        for owner in [input_owner, optimized.owner] {
            digest.update(owner.identity().canonical_length().to_le_bytes());
            digest.update(owner.identity().digest());
        }
        digest.update(generated.identity().as_bytes());
        digest.update(
            u64::try_from(functions)
                .map_err(|_| Resource::Arithmetic)?
                .to_le_bytes(),
        );
        digest.update(
            u64::try_from(blocks)
                .map_err(|_| Resource::Arithmetic)?
                .to_le_bytes(),
        );
        let subject = MixedOptimizerCfgSubjectV27 {
            source_semantic: *source_ssa.source_semantic_sha256(),
            source_ssa: *source_ssa.identity().as_bytes(),
            input: *input_owner.identity(),
            output: *optimized.owner.identity(),
            policy: optimized.policy,
            execution,
            statement: digest.finalize().into(),
            functions,
            blocks,
        };
        drop(transition);
        drop(output);
        drop(input);
        budget.release_storage(
            input_storage
                .retained_storage()
                .checked_add(output_storage.retained_storage())
                .and_then(|bytes| bytes.checked_add(checked_storage.retained_storage()))
                .ok_or(Resource::Accounting)?,
        )?;
        source.check_query_v18(budget)?;
        handoff.output(budget)?;
        Ok((generated, subject))
    }));
    match result {
        Ok(Ok((generated, subject))) => Ok(Prepared {
            source,
            handoff,
            generated,
            subject,
            retained: budget
                .storage()
                .checked_sub(floor)
                .ok_or(Resource::Accounting)?,
            required: budget.storage(),
        }),
        other => {
            if handoff.storage(floor, budget).is_ok() {
                let release = budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or(Resource::Accounting)?;
                let _ = budget
                    .release_storage(release)
                    .map_err(|error| source.retain_query_resource_error_v18(error));
            }
            match other {
                Ok(Err(error)) => {
                    let resource = match &error {
                        Error::Resource(error)
                        | Error::Inventory(CanonicalKirInventoryErrorV1::Resource(error))
                        | Error::Transition(CanonicalKirTransitionErrorV1::Resource(error)) => {
                            Some(*error)
                        }
                        _ => None,
                    };
                    Err(match resource {
                        Some(error) => source.retain_query_resource_error_v18(error).into(),
                        None => error,
                    })
                }
                Err(payload) => std::panic::resume_unwind(payload),
                _ => unreachable!(),
            }
        }
    }
}
