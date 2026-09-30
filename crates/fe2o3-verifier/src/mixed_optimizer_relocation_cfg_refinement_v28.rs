//! Distinct generated prefix-to-final relocation CFG obligations.
//! This retains the original source and final-native owner but does not claim
//! execution or compose an old V27 receipt by matching endpoint digests.
//!
//! ```compile_fail
//! use fe2o3_verifier::{PreparedMixedPureCseCfgRefinementV27, PreparedMixedRelocationCfgRefinementV28};
//! fn relabel<'h, 'n, 'p, 'v, 's>(old: PreparedMixedPureCseCfgRefinementV27<'h, 'v, 's>)
//!     -> PreparedMixedRelocationCfgRefinementV28<'h, 'n, 'p, 'v, 's> { old }
//! ```
use super::super::super::{Inventory, SOURCE_LIMIT, Writer, semantics};
use super::*;
use crate::CanonicalGeneratedVerusProofInputV3;

const DOMAIN: &[u8] = b"FE2O3/V18/POLICY10-PREFIX/LICM-EXPRESSION-CFG/V28\0";
const COMPOSED_DOMAIN: &[u8] = b"FE2O3/V18/ORIGINAL-POLICY10-LICM/COMPOSED-EXPRESSION-CFG/V28\0";

/// Inert identity and graph census for generated relocation CFG obligations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MixedOptimizerRelocationCfgSubjectV28 {
    expressions: MixedOptimizerRelocationSubjectV28,
    statement: [u8; 32],
    functions: usize,
    blocks: usize,
    composed: bool,
}
impl MixedOptimizerRelocationCfgSubjectV28 {
    /// Observe the source-bound expression subject retained by this request.
    pub const fn expressions(self) -> MixedOptimizerRelocationSubjectV28 {
        self.expressions
    }
    /// Domain-separated identity of the exact generated statement and owners.
    pub const fn statement_identity(self) -> [u8; 32] {
        self.statement
    }
    /// Number of defined functions represented by the generated obligations.
    pub const fn modeled_functions(self) -> usize {
        self.functions
    }
    /// Number of blocks represented by the generated obligations.
    pub const fn modeled_blocks(self) -> usize {
        self.blocks
    }
    /// Whether the generated source models both the prefix and LICM tail.
    /// This is a scope observation, not evidence of an executed theorem.
    pub const fn models_original_to_final_composition(self) -> bool {
        self.composed
    }
}

/// Owns the actual source/final-native expression request and exact generated
/// bytes. Neither a V27 receipt nor an external digest constructs this owner.
#[must_use = "retain or discard the exact relocation CFG request"]
pub struct PreparedMixedRelocationCfgRefinementV28<'h, 'n, 'p, 'v, 's> {
    expressions: PreparedMixedRelocationExpressionsV28<'h, 'n, 'p, 'v, 's>,
    generated: CanonicalGeneratedVerusProofInputV3,
    subject: MixedOptimizerRelocationCfgSubjectV28,
    retained: usize,
    required: usize,
}
type Prepared<'h, 'n, 'p, 'v, 's> = PreparedMixedRelocationCfgRefinementV28<'h, 'n, 'p, 'v, 's>;
/// A freshly generated composed request, not a converted tail-only receipt.
/// The private common storage retains the original, intermediate and final
/// owner chain; only this constructor emits the original-to-final theorem.
///
/// ```compile_fail
/// use fe2o3_verifier::{PreparedMixedRelocationCfgRefinementV28, PreparedMixedComposedRelocationCfgRefinementV28};
/// fn relabel<'h, 'n, 'p, 'v, 's>(old: PreparedMixedRelocationCfgRefinementV28<'h, 'n, 'p, 'v, 's>)
///     -> PreparedMixedComposedRelocationCfgRefinementV28<'h, 'n, 'p, 'v, 's> { old }
/// ```
#[must_use = "retain or discard the exact composed relocation CFG request"]
pub struct PreparedMixedComposedRelocationCfgRefinementV28<'h, 'n, 'p, 'v, 's>(
    Prepared<'h, 'n, 'p, 'v, 's>,
);
impl PreparedMixedComposedRelocationCfgRefinementV28<'_, '_, '_, '_, '_> {
    /// Observe the composed subject after checking the original custody ledger.
    pub fn subject(&self, budget: &Budget<'_>) -> Result<MixedOptimizerRelocationCfgSubjectV28> {
        self.0.subject(budget)
    }
    /// Borrow the exact composed generated source after checking custody.
    pub fn generated_source(&self, budget: &Budget<'_>) -> Result<&[u8]> {
        self.0.generated_source(budget)
    }
    /// Require the exact original mixed-SSA owner retained by the handoff.
    pub fn check_original_source(
        &self,
        source: &fe2o3_pliron::ProductionSemanticSsaOwnerV1,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        self.0.check_original_source(source, budget)
    }
    /// Replay the original source and exact native relocation chain, not Verus.
    pub fn replay(&self, budget: &mut Budget<'_>) -> Result<()> {
        self.0.replay(budget)
    }
    /// Return retained expression and composed-source storage after custody checks.
    pub fn retained_storage(&self, budget: &Budget<'_>) -> Result<usize> {
        self.0.retained_storage(budget)
    }
    /// Drop all composed request backing before refunding its original ledger.
    pub fn discard(self, budget: &mut Budget<'_>) -> Result<()> {
        self.0.discard(budget)
    }
    /// Always false: generated obligations are not an executed proof.
    pub const fn authenticates_executed_proof(&self) -> bool {
        false
    }
    /// Always false: modeling composition does not authenticate its execution.
    pub const fn proves_original_to_final_composition(&self) -> bool {
        false
    }
    /// Always false: MIR-to-native semantics remain outside this request.
    pub const fn proves_mir_to_native_lowering(&self) -> bool {
        false
    }
    /// Always false: generated source grants no artifact or launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}
type Capture<'a, 'h, 'n, 'p, 'v, 's, 'w> = (
    &'a PreparedMixedRelocationExpressionsV28<'h, 'n, 'p, 'v, 's>,
    &'a mut Budget<'w>,
    usize,
    bool,
);
type Built = (
    CanonicalGeneratedVerusProofInputV3,
    MixedOptimizerRelocationCfgSubjectV28,
    usize,
);

fn headers() -> Result<usize> {
    let owner = size_of::<Prepared<'_, '_, '_, '_, '_>>()
        .checked_sub(size_of::<
            PreparedMixedRelocationExpressionsV28<'_, '_, '_, '_, '_>,
        >())
        .ok_or(Resource::Arithmetic)?;
    [
        owner,
        align_of::<Prepared<'_, '_, '_, '_, '_>>(),
        size_of::<Capture<'_, '_, '_, '_, '_, '_, '_>>(),
        align_of::<Capture<'_, '_, '_, '_, '_, '_, '_>>(),
        size_of::<std::panic::AssertUnwindSafe<Capture<'_, '_, '_, '_, '_, '_, '_>>>(),
        size_of::<Result<Built>>(),
        size_of::<std::thread::Result<Result<Built>>>(),
        size_of::<Writer<'_, '_>>(),
        3 * size_of::<Inventory<'_>>(),
        size_of::<fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV18<'_, '_, '_, '_>>(),
        size_of::<Sha256>(),
        size_of::<[usize; 12]>(),
        2 * SOURCE_LIMIT,
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| {
        sum.checked_add(bytes).ok_or(Resource::Arithmetic.into())
    })
}

impl Prepared<'_, '_, '_, '_, '_> {
    fn custody(&self, budget: &Budget<'_>) -> Result<()> {
        self.expressions
            .handoff
            .observe_retained_storage_v28(self.required, budget)?;
        self.expressions.custody(budget)
    }
    fn check(&self, budget: &Budget<'_>) -> Result<()> {
        let custody = self.custody(budget);
        self.expressions.check(budget)?;
        custody
    }
    /// Observe the inert subject after checking the original custody ledger.
    pub fn subject(&self, budget: &Budget<'_>) -> Result<MixedOptimizerRelocationCfgSubjectV28> {
        self.check(budget)?;
        Ok(self.subject)
    }
    /// Borrow the exact canonical generated source after checking custody.
    pub fn generated_source(&self, budget: &Budget<'_>) -> Result<&[u8]> {
        self.check(budget)?;
        Ok(self.generated.source())
    }
    /// Require the exact original mixed-SSA owner retained by the handoff.
    pub fn check_original_source(
        &self,
        source: &fe2o3_pliron::ProductionSemanticSsaOwnerV1,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        self.check(budget)?;
        self.expressions.check_original_source(source, budget)
    }
    /// Replay source, native, LICM, and expression bindings without executing Verus.
    pub fn replay(&self, budget: &mut Budget<'_>) -> Result<()> {
        self.check(budget)?;
        self.expressions.replay(budget)
    }
    /// Return the combined expression and generated-source retained storage.
    pub fn retained_storage(&self, budget: &Budget<'_>) -> Result<usize> {
        self.check(budget)?;
        self.expressions
            .retained
            .checked_add(self.retained)
            .ok_or(Resource::Arithmetic.into())
    }
    /// Drop generated bytes and expression backing before refunding storage.
    pub fn discard(self, budget: &mut Budget<'_>) -> Result<()> {
        let checked = self.check(budget);
        let custody = self.custody(budget);
        let Self {
            expressions,
            generated,
            retained,
            ..
        } = self;
        drop(generated);
        let released = custody.and_then(|()| {
            budget.release_storage(retained).map_err(|error| {
                Error::Source(expressions.source.retain_query_resource_error_v18(error))
            })
        });
        let settled = expressions.discard(budget);
        checked?;
        released?;
        settled
    }
    /// Always false: generated obligations are not an executed proof.
    pub const fn authenticates_executed_proof(&self) -> bool {
        false
    }
    /// Always false: this request does not authenticate a composition theorem.
    pub const fn proves_original_to_final_composition(&self) -> bool {
        false
    }
    /// Always false: MIR-to-native semantics are outside this request.
    pub const fn proves_mir_to_native_lowering(&self) -> bool {
        false
    }
    /// Always false: generated source grants no artifact or launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

impl<'h, 'n, 'p, 'v, 's> PreparedMixedRelocationExpressionsV28<'h, 'n, 'p, 'v, 's> {
    /// Generate prefix-to-final CFG obligations while retaining the exact
    /// source-bound expression request; this does not run Verus.
    pub fn prepare_cfg_refinement(
        self,
        budget: &mut Budget<'_>,
    ) -> Result<Prepared<'h, 'n, 'p, 'v, 's>> {
        self.prepare_cfg_profile(budget, false)
    }
    /// Generate original-to-Policy10 and LICM CFG obligations with an exact
    /// intermediate interpreter bridge; this does not execute Verus.
    pub fn prepare_composed_cfg_refinement(
        self,
        budget: &mut Budget<'_>,
    ) -> Result<PreparedMixedComposedRelocationCfgRefinementV28<'h, 'n, 'p, 'v, 's>> {
        self.prepare_cfg_profile(budget, true)
            .map(PreparedMixedComposedRelocationCfgRefinementV28)
    }
    fn prepare_cfg_profile(
        self,
        budget: &mut Budget<'_>,
        composed: bool,
    ) -> Result<Prepared<'h, 'n, 'p, 'v, 's>> {
        let floor = budget.storage();
        let capture: Capture<'_, '_, '_, '_, '_, '_, '_> = (&self, &mut *budget, floor, composed);
        let construct = move || -> Result<Built> {
            let (expressions, budget, floor, composed) = std::convert::identity(capture);
            expressions.replay(budget)?;
            budget.reserve_storage(headers()?)?;
            let pair = &expressions.pair;
            let (input, input_storage) =
                Inventory::derive_v18(pair.input(), budget).map_err(Error::Inventory)?;
            budget.reserve_storage(input_storage.retained_storage())?;
            let (output, output_storage) =
                Inventory::derive_v18(pair.output(), budget).map_err(Error::Inventory)?;
            budget.reserve_storage(output_storage.retained_storage())?;
            budget.charge_work(input.functions().len())?;
            let functions = input
                .functions()
                .iter()
                .filter(|f| !f.blocks.is_empty())
                .count();
            if functions == 0 {
                return Err(Error::Binding("nonempty relocation CFG function census"));
            }
            let mut writer = Writer::new(budget)?;
            let blocks = if composed {
                let original_owner = expressions.source.canonical(writer.budget)?;
                let (original, original_storage) =
                    Inventory::derive_v18(original_owner, writer.budget)
                        .map_err(Error::Inventory)?;
                writer
                    .budget
                    .reserve_storage(original_storage.retained_storage())?;
                let prefix = expressions
                    .handoff
                    .relocation(writer.budget)?
                    .prefix(writer.budget)?
                    .output(writer.budget)?;
                if !std::ptr::eq(prefix.owner(), pair.input()) {
                    return Err(Error::Binding("composed exact Policy10 intermediate owner"));
                }
                let (transition, storage) =
                    fe2o3_kernel_analysis::check_canonical_kir_transition_v18(
                        &original,
                        &input,
                        prefix.occurrences().candidate(),
                        writer.budget,
                    )
                    .map_err(super::super::super::Error::from)?;
                writer.budget.reserve_storage(storage.retained_storage())?;
                let blocks = semantics::generate_composed_relocation_cfg_v28(
                    &transition,
                    &output,
                    pair,
                    &mut writer,
                )?;
                drop(transition);
                drop(original);
                writer.budget.release_storage(
                    original_storage
                        .retained_storage()
                        .checked_add(storage.retained_storage())
                        .ok_or(Resource::Arithmetic)?,
                )?;
                blocks
            } else {
                semantics::generate_relocation_cfg_v28(&input, &output, pair, &mut writer)?
            };
            let text = writer.finish()?;
            let domain = if composed { COMPOSED_DOMAIN } else { DOMAIN };
            budget.charge_work(
                text.len()
                    .checked_mul(3)
                    .and_then(|work| work.checked_add(domain.len() + 264))
                    .ok_or(Resource::Arithmetic)?,
            )?;
            let generated = CanonicalGeneratedVerusProofInputV3::new(text.into_bytes())
                .map_err(super::super::super::Error::from)?;
            let mut digest = Sha256::new();
            digest.update(domain);
            digest.update(expressions.subject.source_semantic);
            digest.update(expressions.subject.source_ssa);
            digest.update(expressions.subject.prefix_execution);
            for owner in [
                expressions.subject.input,
                expressions.subject.prefix,
                expressions.subject.output,
            ] {
                digest.update(owner.canonical_length().to_le_bytes());
                digest.update(owner.digest());
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
            let subject = MixedOptimizerRelocationCfgSubjectV28 {
                expressions: expressions.subject,
                statement: digest.finalize().into(),
                functions,
                blocks,
                composed,
            };
            drop((input, output));
            budget.release_storage(
                input_storage
                    .retained_storage()
                    .checked_add(output_storage.retained_storage())
                    .ok_or(Resource::Arithmetic)?,
            )?;
            expressions.check(budget)?;
            let retained = budget
                .storage()
                .checked_sub(floor)
                .ok_or(Resource::Accounting)?;
            Ok((generated, subject, retained))
        };
        #[cfg(test)]
        {
            assert_eq!(
                std::mem::size_of_val(&construct),
                size_of::<Capture<'_, '_, '_, '_, '_, '_, '_>>()
            );
            assert_eq!(
                std::mem::align_of_val(&construct),
                align_of::<Capture<'_, '_, '_, '_, '_, '_, '_>>()
            );
        }
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(construct)) {
            Ok(Ok((generated, subject, retained))) => Ok(Prepared {
                expressions: self,
                generated,
                subject,
                retained,
                required: budget.storage(),
            }),
            rejected => {
                if self
                    .handoff
                    .observe_retained_storage_v28(floor, budget)
                    .is_ok()
                {
                    if let Some(credit) = budget.storage().checked_sub(floor) {
                        let _ = budget
                            .release_storage(credit)
                            .map_err(|error| self.source.retain_query_resource_error_v18(error));
                    }
                }
                let _ = self.discard(budget);
                match rejected {
                    Ok(Err(error)) => Err(error),
                    Err(payload) => std::panic::resume_unwind(payload),
                    Ok(Ok(_)) => unreachable!(),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relocation_cfg_request_headers_cover_the_actual_named_capture_and_owner_fields() {
        type CaptureFields<'a> = (&'a (), &'a mut (), usize, bool);
        let owner_fields = size_of::<CanonicalGeneratedVerusProofInputV3>()
            + size_of::<MixedOptimizerRelocationCfgSubjectV28>()
            + 2 * size_of::<usize>();
        assert_eq!(
            size_of::<Prepared<'_, '_, '_, '_, '_>>(),
            size_of::<PreparedMixedRelocationExpressionsV28<'_, '_, '_, '_, '_>>() + owner_fields
        );
        assert_eq!(
            size_of::<PreparedMixedComposedRelocationCfgRefinementV28<'_, '_, '_, '_, '_>>(),
            size_of::<Prepared<'_, '_, '_, '_, '_>>()
        );
        assert_eq!(
            align_of::<PreparedMixedComposedRelocationCfgRefinementV28<'_, '_, '_, '_, '_>>(),
            align_of::<Prepared<'_, '_, '_, '_, '_>>()
        );
        let expected = owner_fields
            + align_of::<Prepared<'_, '_, '_, '_, '_>>()
            + size_of::<CaptureFields<'_>>()
            + align_of::<CaptureFields<'_>>()
            + size_of::<std::panic::AssertUnwindSafe<CaptureFields<'_>>>()
            + size_of::<Result<Built>>()
            + size_of::<std::thread::Result<Result<Built>>>()
            + size_of::<Writer<'_, '_>>()
            + 3 * size_of::<Inventory<'_>>()
            + size_of::<fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV18<'_, '_, '_, '_>>()
            + size_of::<Sha256>()
            + 12 * size_of::<usize>()
            + 2 * SOURCE_LIMIT;
        assert_eq!(headers().unwrap(), expected);
    }
}
