//! One typed source/prefix/LICM/forwarding statement on the actual native owner.
use super::super::super::{Inventory, SOURCE_LIMIT, Writer, semantics};
use super::*;
use crate::CanonicalGeneratedVerusProofInputV3;
use fe2o3_kernel_ir::{EndiannessV2, ExplicitLaunchExtent, FormalIndexWidth};

#[path = "mixed_optimizer_typed_source_receipt_v50.rs"]
mod receipt;
pub use receipt::{ExecutedTypedSourceTailV50, PreparedTypedSourceTailExecutionV50};

const DOMAIN: &[u8] = b"FE2O3/ORIGINAL-MIR/POLICY11/LICM/STORE-CONSENSUS/TYPED/V50\0";
type Native<'n, 'p, 'v, 's> = Handoff<'n, 'p, 'v, 's, Policy11<'v, 's>>;

/// Inert identities for the complete typed statement, not a proof certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TypedSourceTailSubjectV50 {
    semantic: [u8; 32],
    ssa: [u8; 32],
    graphs: [Identity; 4],
    prefix_execution: [u8; 32],
    runtime: [u8; 32],
    statement: [u8; 32],
    census: [usize; 6],
}
impl TypedSourceTailSubjectV50 {
    /// Exact independently admitted original semantic MIR identity.
    pub const fn source_semantic_identity(self) -> [u8; 32] {
        self.semantic
    }
    /// Exact original SSA identity.
    pub const fn source_ssa_identity(self) -> [u8; 32] {
        self.ssa
    }
    /// Original canonical, Policy11, LICM and final forwarding identities.
    pub const fn graph_identities(self) -> [Identity; 4] {
        self.graphs
    }
    /// Complete retained fixed-policy execution, including every round.
    pub const fn prefix_execution_identity(self) -> [u8; 32] {
        self.prefix_execution
    }
    /// Concrete launch, INDEX width and endian identity.
    pub const fn runtime_identity(self) -> [u8; 32] {
        self.runtime
    }
    /// Distinct typed statement and generated-source identity.
    pub const fn statement_identity(self) -> [u8; 32] {
        self.statement
    }
    /// Roots, instances, statements, assignments, operations and definitions.
    pub const fn census(self) -> [usize; 6] {
        self.census
    }
}

/// Retains the genuine source, fixed transformation chain and final native
/// handoff. Neither old integer CFG requests nor V36 requests convert to it.
///
/// ```compile_fail
/// use fe2o3_verifier::{PreparedOriginalSemanticMirRefinementV36, PreparedTypedSourceTailV50};
/// fn substitute<'a>(old: PreparedOriginalSemanticMirRefinementV36<'a, 'a>)
///     -> PreparedTypedSourceTailV50<'a, 'a, 'a, 'a, 'a> { old }
/// ```
#[must_use = "discard on the original funded ledger before the native handoff"]
pub struct PreparedTypedSourceTailV50<'h, 'n, 'p, 'v, 's> {
    source: &'h Source<'s>,
    handoff: &'h Native<'n, 'p, 'v, 's>,
    generated: CanonicalGeneratedVerusProofInputV3,
    subject: TypedSourceTailSubjectV50,
    endianness: EndiannessV2,
    retained: usize,
    required: usize,
}
type Request<'h, 'n, 'p, 'v, 's> = PreparedTypedSourceTailV50<'h, 'n, 'p, 'v, 's>;
type Capture<'h, 'n, 'p, 'v, 's> = (
    &'h Source<'s>,
    &'h Native<'n, 'p, 'v, 's>,
    EndiannessV2,
    usize,
);

fn charge_hash(budget: &mut Budget<'_>, hash: &mut Sha256, bytes: &[u8]) -> Result<()> {
    budget.charge_work(bytes.len().checked_add(8).ok_or(Resource::Arithmetic)?)?;
    hash.update(
        u64::try_from(bytes.len())
            .map_err(|_| Resource::Arithmetic)?
            .to_le_bytes(),
    );
    hash.update(bytes);
    Ok(())
}

fn runtime_identity(
    roots: usize,
    launches: &[ExplicitLaunchExtent],
    width: FormalIndexWidth,
    endian: EndiannessV2,
    budget: &mut Budget<'_>,
) -> Result<[u8; 32]> {
    budget.charge_work(8)?;
    let bits = match width {
        FormalIndexWidth::Bits32 => 32u8,
        FormalIndexWidth::Bits64 => 64u8,
        FormalIndexWidth::Unknown => return Err(Error::Binding("typed tail INDEX width")),
    };
    if roots == 0 || launches.len() != roots {
        return Err(Error::Binding("typed tail complete native launch census"));
    }
    let mut hash = Sha256::new();
    charge_hash(budget, &mut hash, b"FE2O3/TYPED-SOURCE-TAIL/RUNTIME/V50\0")?;
    charge_hash(budget, &mut hash, &[bits, endian as u8])?;
    for launch in launches {
        budget.charge_work(8)?;
        let ExplicitLaunchExtent::Exact { rank, extents } = launch else {
            return Err(Error::Binding("typed tail exact native launch"));
        };
        if !(1..=3).contains(rank)
            || extents.contains(&0)
            || (*rank < 2 && extents[1] != 1)
            || (*rank < 3 && extents[2] != 1)
            || (bits == 32 && extents.iter().any(|n| *n > u64::from(u32::MAX)))
        {
            return Err(Error::Binding("typed tail native launch extent"));
        }
        charge_hash(budget, &mut hash, &[*rank])?;
        for value in extents {
            charge_hash(budget, &mut hash, &value.to_le_bytes())?;
        }
    }
    Ok(hash.finalize().into())
}

fn retain(error: Error, source: &Source<'_>) -> Error {
    let mut cause: &(dyn std::error::Error + 'static) = &error;
    loop {
        if let Some(resource) = cause.downcast_ref::<Resource>() {
            return source.retain_query_resource_error_v18(*resource).into();
        }
        match cause.source() {
            Some(next) => cause = next,
            None => return error,
        }
    }
}

fn headers() -> Result<usize> {
    type S = Request<'static, 'static, 'static, 'static, 'static>;
    type C = Capture<'static, 'static, 'static, 'static, 'static>;
    type Forward = fe2o3_kernel_analysis::CheckedCanonicalKirCrossBlockForwardingV18<'static>;
    type ForwardStorage = fe2o3_kernel_analysis::CanonicalKirCrossBlockForwardingStorageV1;
    [
        2 * SOURCE_LIMIT,
        size_of::<S>(),
        align_of::<S>(),
        2 * size_of::<Result<S>>(),
        size_of::<std::thread::Result<Result<S>>>(),
        size_of::<C>(),
        align_of::<C>(),
        size_of::<std::panic::AssertUnwindSafe<C>>(),
        PrefixView::inspection_storage_v29()?,
        4 * size_of::<Inventory<'_>>(),
        4 * size_of::<fe2o3_kernel_analysis::CanonicalKirInventoryStorageV1>(),
        size_of::<fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV18<'_, '_, '_, '_>>(),
        size_of::<Pair<'_>>(),
        size_of::<Forward>(),
        size_of::<ForwardStorage>(),
        size_of::<fe2o3_kernel_analysis::CanonicalKirLicmStorageV1>(),
        size_of::<Writer<'_, '_>>(),
        size_of::<Result<Writer<'_, '_>>>(),
        size_of::<(String, [usize; 6])>(),
        size_of::<Result<(String, [usize; 6])>>(),
        size_of::<TypedSourceTailSubjectV50>(),
        2 * size_of::<Sha256>(),
        size_of::<[Identity; 4]>(),
        6 * size_of::<usize>(),
        8 * size_of::<&()>(),
    ]
    .into_iter()
    .try_fold(0usize, |n, v| {
        n.checked_add(v).ok_or(Resource::Arithmetic.into())
    })
}

/// Generate the complete typed original-source to final-byte statement. The
/// native handoff must retain the fixed Policy11, LICM and forwarding chain.
/// This reserves its own retained credit and does not execute a solver.
pub fn prepare_typed_source_tail_v50<'h, 'n, 'p, 'v, 's>(
    source: &'h Source<'s>,
    handoff: &'h Native<'n, 'p, 'v, 's>,
    endianness: EndiannessV2,
    budget: &mut Budget<'_>,
) -> Result<Request<'h, 'n, 'p, 'v, 's>> {
    source.check_query_v18(budget)?;
    handoff.check_original_source(source.source_ssa(budget)?, budget)?;
    let floor = budget.storage();
    let capture: Capture<'_, '_, '_, '_, '_> = (source, handoff, endianness, floor);
    let produce = move |budget: &mut Budget<'_>| {
        let (source, handoff, endianness, floor) = std::convert::identity(capture);
        produce(source, handoff, endianness, floor, budget).map_err(|e| retain(e, source))
    };
    #[cfg(test)]
    {
        assert_eq!(
            std::mem::size_of_val(&produce),
            size_of::<Capture<'_, '_, '_, '_, '_>>()
        );
        assert_eq!(
            std::mem::align_of_val(&produce),
            align_of::<Capture<'_, '_, '_, '_, '_>>()
        );
    }
    let request = budget
        .with_prepaid_scope(floor, 1, 1, headers()?, produce)
        .map_err(|e| retain(e, source))?;
    budget
        .reserve_storage(request.retained)
        .map_err(|e| source.retain_query_resource_error_v18(e))?;
    request.check(budget)?;
    Ok(request)
}

fn produce<'h, 'n, 'p, 'v, 's>(
    source: &'h Source<'s>,
    handoff: &'h Native<'n, 'p, 'v, 's>,
    endianness: EndiannessV2,
    floor: usize,
    budget: &mut Budget<'_>,
) -> Result<Request<'h, 'n, 'p, 'v, 's>> {
    let forwarding_owner = handoff.store_consensus_v46(budget)?.ok_or(Error::Binding(
        "typed request requires the actual forwarding native handoff",
    ))?;
    let relocation = handoff.relocation(budget)?;
    if !std::ptr::eq(relocation, forwarding_owner.relocation(budget)?) {
        return Err(Error::Binding("typed request exact shared LICM owner"));
    }
    relocation.replay(budget)?;
    let prefix = relocation.prefix(budget)?.checked_prefix_v29(budget)?;
    let execution = prefix.execution();
    if execution.policy_version() != 11 || !(1..=32).contains(&execution.rounds()) {
        return Err(Error::Binding(
            "typed request complete fixed Policy11 execution",
        ));
    }
    let original_owner = source.canonical(budget)?;
    let licm_owner = relocation.tail(budget)?.output();
    let final_owner = handoff.output(budget)?;
    if !std::ptr::eq(final_owner, forwarding_owner.output(budget)?) {
        return Err(Error::Binding("typed request actual final native owner"));
    }
    let (original, receipt) =
        Inventory::derive_v18(original_owner, budget).map_err(Error::Inventory)?;
    budget.reserve_storage(receipt.retained_storage())?;
    let (middle, receipt) =
        Inventory::derive_v18(prefix.owner(), budget).map_err(Error::Inventory)?;
    budget.reserve_storage(receipt.retained_storage())?;
    let (motion, receipt) = Inventory::derive_v18(licm_owner, budget).map_err(Error::Inventory)?;
    budget.reserve_storage(receipt.retained_storage())?;
    let (final_graph, receipt) =
        Inventory::derive_v18(final_owner, budget).map_err(Error::Inventory)?;
    budget.reserve_storage(receipt.retained_storage())?;
    let (transition, receipt) = fe2o3_kernel_analysis::check_canonical_kir_transition_v18(
        &original,
        &middle,
        prefix.occurrences().candidate(),
        budget,
    )
    .map_err(super::super::super::Error::from)?;
    budget.reserve_storage(receipt.retained_storage())?;
    let (licm, receipt) = relocation
        .tail(budget)?
        .replay_against(prefix.owner(), budget)?;
    budget.reserve_storage(receipt.retained_storage())?;
    let (forwarding, receipt) = forwarding_owner.replay(budget)?;
    budget.reserve_storage(receipt.retained_storage())?;
    let (launches, width) = handoff.launch_context(budget)?;
    let runtime = runtime_identity(
        source.root_count(budget)?,
        launches,
        width,
        endianness,
        budget,
    )?;
    let (text, census) =
        source.with_ranked_correspondence_v18(&original, budget, |relation, budget| {
            let mut out = Writer::new(budget)?;
            let census = semantics::original_scalar_v30::generate_invocations_typed_v49(
                relation,
                &transition,
                &licm,
                &motion,
                &forwarding,
                &final_graph,
                launches,
                width,
                endianness,
                &mut out,
            )?;
            Ok::<_, Error>((out.finish()?, census))
        })?;
    budget.charge_work(text.len().checked_mul(3).ok_or(Resource::Arithmetic)?)?;
    let generated = CanonicalGeneratedVerusProofInputV3::new(text.into_bytes())
        .map_err(super::super::super::Error::from)?;
    let original_source = source.source_ssa(budget)?;
    let semantic = *original_source.source_semantic_sha256();
    let ssa = *original_source.identity().as_bytes();
    let graphs = [
        *original_owner.identity(),
        *prefix.owner().identity(),
        *licm_owner.identity(),
        *final_owner.identity(),
    ];
    budget.charge_work(execution.canonical_bytes().len())?;
    let prefix_execution = Sha256::digest(execution.canonical_bytes()).into();
    let mut hash = Sha256::new();
    for bytes in [DOMAIN, &semantic, &ssa, &prefix_execution, &runtime] {
        charge_hash(budget, &mut hash, bytes)?;
    }
    for graph in graphs {
        charge_hash(budget, &mut hash, graph.digest())?;
        charge_hash(budget, &mut hash, &graph.canonical_length().to_le_bytes())?;
    }
    for n in census {
        charge_hash(
            budget,
            &mut hash,
            &u64::try_from(n)
                .map_err(|_| Resource::Arithmetic)?
                .to_le_bytes(),
        )?;
    }
    charge_hash(budget, &mut hash, &generated.identity().as_bytes())?;
    let subject = TypedSourceTailSubjectV50 {
        semantic,
        ssa,
        graphs,
        prefix_execution,
        runtime,
        statement: hash.finalize().into(),
        census,
    };
    let retained = size_of::<Request<'_, '_, '_, '_, '_>>()
        .checked_add(generated.source().len())
        .ok_or(Resource::Arithmetic)?;
    handoff.observe_retained_storage_v28(floor, budget)?;
    Ok(Request {
        source,
        handoff,
        generated,
        subject,
        endianness,
        retained,
        required: floor.checked_add(retained).ok_or(Resource::Arithmetic)?,
    })
}

impl Request<'_, '_, '_, '_, '_> {
    fn check(&self, budget: &Budget<'_>) -> Result<()> {
        self.source.check_query_v18(budget)?;
        self.handoff
            .observe_retained_storage_v28(self.required, budget)?;
        Ok(())
    }
    /// Exact typed subject, not interchangeable with historical proof subjects.
    pub fn subject(&self, budget: &Budget<'_>) -> Result<TypedSourceTailSubjectV50> {
        self.check(budget)?;
        Ok(self.subject)
    }
    /// Complete generated typed interpreter/trace obligations.
    pub fn generated_source(&self, budget: &Budget<'_>) -> Result<&[u8]> {
        self.check(budget)?;
        Ok(self.generated.source())
    }
    /// Exact reserved request credit, excluding all borrowed owners.
    pub fn retained_storage(&self, budget: &Budget<'_>) -> Result<usize> {
        self.check(budget)?;
        Ok(self.retained)
    }
    /// Rejects equal-byte replacement source owners.
    pub fn check_original_source(&self, source: &Source<'_>, budget: &Budget<'_>) -> Result<()> {
        self.check(budget)?;
        if !std::ptr::eq(self.source, source) {
            return Err(Error::Binding("typed tail foreign source owner"));
        }
        Ok(())
    }
    /// Regenerates the exact typed statement through fresh actual graph checks.
    /// This checks compiler custody, not solver success.
    pub fn replay(&self, budget: &mut Budget<'_>) -> Result<()> {
        self.check(budget)?;
        let refreshed =
            prepare_typed_source_tail_v50(self.source, self.handoff, self.endianness, budget)?;
        let compare = (|| {
            budget.charge_work(
                self.generated
                    .source()
                    .len()
                    .checked_add(32)
                    .ok_or(Resource::Arithmetic)?,
            )?;
            if self.subject != refreshed.subject
                || self.generated.source() != refreshed.generated.source()
            {
                return Err(Error::Binding("typed source tail replay changed"));
            }
            Ok(())
        })();
        let settled = refreshed.discard(budget);
        compare.map_err(|error| retain(error, self.source))?;
        settled
    }
    /// Drop all request bytes before refunding the original native ledger.
    pub fn discard(self, budget: &mut Budget<'_>) -> Result<()> {
        let checked = self.check(budget);
        let custody = self
            .handoff
            .observe_retained_storage_v28(self.required, budget);
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
                .map_err(|e| source.retain_query_resource_error_v18(e))
        });
        checked?;
        settled.map_err(Into::into)
    }
    /// Always false until a distinct admitted runtime executes this statement.
    pub const fn authenticates_executed_proof(&self) -> bool {
        false
    }
    /// Generated statements never authorize publication or device launch.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

#[cfg(test)]
#[path = "mixed_optimizer_typed_source_request_v50_tests.rs"]
mod tests;
