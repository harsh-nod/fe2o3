//! Complete production history serialization with unchanged genuine custody.
//! No optimizer owner is reconstructed from bytes; signing/default remain separate.
#![allow(clippy::drop_non_drop, reason = "Drop dependent views before refunds.")]
#![allow(
    clippy::result_large_err,
    reason = "Keep exact typed child causes inline."
)]
use super::super::{Policy7ExecutionWitnessV1, PreparedRefinedForwardingHistoryClaimsV1};
use super::*;
use fe2o3_kernel_opt::{
    ExpandedHistoryErrorV1, InertExpandedHistoryBytesV1 as HistoryBytes, LoopUnrollHistoryInputsV1,
    LoopUnrollHistoryWireErrorV1, RefinedForwardingHistoryWireErrorV1,
    ScalarFixedPointHistoryErrorV1, encode_expanded_history_v1, encode_loop_unroll_history_v1,
    encode_refined_forwarding_history_v1, encode_scalar_fixed_point_history_v1,
    materialize_expanded_history_v1, read_expanded_history_v1, read_loop_unroll_history_v1,
    read_refined_forwarding_history_v1, read_scalar_fixed_point_history_v1,
};
use fe2o3_lower_mir_kernel::{
    CheckedDecodedExpandedSourceV1, DecodedExpandedSourceErrorV1, ProductionExpandedHistoryV1,
    ProductionExpandedPrefixV1, ProductionOwnedExpandedContinuationV1 as Expanded,
    with_checked_decoded_expanded_source_v1,
};

#[derive(Debug)]
pub(crate) enum ExpandedHistoryTransportErrorV1 {
    Resource(Resource),
    Native(ExpandedNativeTransportErrorV3),
    Nominal(descriptor::E),
    Forwarding(RefinedForwardingHistoryWireErrorV1),
    Unroll(LoopUnrollHistoryWireErrorV1),
    Scalar(ScalarFixedPointHistoryErrorV1),
    History(ExpandedHistoryErrorV1),
    Source(DecodedExpandedSourceErrorV1<ExpandedNativeTransportErrorV3>),
    Mismatch(&'static str),
    Panicked,
}
type HError = ExpandedHistoryTransportErrorV1;
type HResult<T> = std::result::Result<T, HError>;
impl From<Resource> for HError {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl fmt::Display for HError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "expanded history transport: {self:?}")
    }
}
impl std::error::Error for HError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Native(e) => Some(e),
            Self::Nominal(e) => Some(e),
            Self::Forwarding(e) => Some(e),
            Self::Unroll(e) => Some(e),
            Self::Scalar(e) => Some(e),
            Self::History(e) => Some(e),
            Self::Source(e) => Some(e),
            Self::Mismatch(_) | Self::Panicked => None,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ExpandedHistoryTransportStorageV1(usize);
impl ExpandedHistoryTransportStorageV1 {
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Moves genuine custody once and retains exact inert complete history bytes.
/// Neither those bytes nor the independently checked source relation authenticates
/// observed execution, signing or a caller-provided source file identity.
pub(crate) struct PreparedExpandedHistoryTransportV1 {
    transport: Output,
    history: HistoryBytes,
    input_floor: usize,
    retained_floor: usize,
}
type Prepared = PreparedExpandedHistoryTransportV1;
fn add(a: usize, b: usize) -> HResult<usize> {
    a.checked_add(b).ok_or(Resource::Arithmetic.into())
}
fn header() -> HResult<usize> {
    size_of::<Prepared>()
        .checked_sub(size_of::<Output>())
        .and_then(|n| n.checked_sub(size_of::<HistoryBytes>()))
        .ok_or(Resource::Accounting.into())
}
fn added(history: &HistoryBytes) -> HResult<usize> {
    add(header()?, history.storage().retained_storage())
}
fn scope<'w, T>(b: &mut Budget<'w>, f: impl FnOnce(&mut Budget<'w>) -> HResult<T>) -> HResult<T> {
    descriptor::scoped(b, || HError::Panicked, f)
}
fn equal(a: &[u8], b: &[u8], budget: &mut Budget<'_>) -> HResult<()> {
    budget.charge_work(add(a.len(), b.len())?)?;
    if a != b {
        return Err(HError::Mismatch("complete canonical history bytes"));
    }
    Ok(())
}

/// Crate-private serialization helper; inputs are genuine source-owned history.
/// Returned bytes remain inert and their complete receipt is unreserved.
pub(crate) fn encode_live_history(
    owner: &Expanded,
    execution: &Policy7ExecutionWitnessV1,
    claims: &PreparedRefinedForwardingHistoryClaimsV1,
    budget: &mut Budget<'_>,
) -> HResult<HistoryBytes> {
    scope(budget, |budget| {
        budget.charge_work(5)?;
        owner
            .verify_equivalence(budget)
            .map_err(|e| HError::Nominal(descriptor::E::Expanded(e)))?;
        budget.reserve_storage(
            size_of::<super::super::FinalSource<'_>>()
                .checked_add(size_of::<ProductionExpandedPrefixV1<'_>>())
                .and_then(|n| n.checked_add(size_of::<ProductionExpandedHistoryV1<'_>>()))
                .ok_or(Resource::Arithmetic)?,
        )?;
        let source = super::super::final_f(owner);
        claims
            .check_source_v1(source, execution, budget)
            .map_err(|e| HError::Nominal(descriptor::pipeline(e)))?;
        budget.reserve_storage(
            size_of::<fe2o3_kernel_opt::CanonicalRefinedForwardingHistoryInputsV1<'_>>()
                .checked_add(size_of::<LoopUnrollHistoryInputsV1<'_, '_>>())
                .ok_or(Resource::Arithmetic)?,
        )?;
        let inputs = claims.source_inputs_v1(source, execution);
        let f = encode_refined_forwarding_history_v1(inputs, budget).map_err(HError::Forwarding)?;
        budget.reserve_storage(f.storage().retained_storage())?;
        let ff = read_refined_forwarding_history_v1(f.canonical_bytes(), budget)
            .map_err(HError::Forwarding)?;
        budget.reserve_storage(ff.storage().retained_storage())?;
        let (output, origins, limits) = match owner.prefix() {
            ProductionExpandedPrefixV1::Direct(v) => {
                (v.output(), v.continuation().origins(), v.limits())
            }
            ProductionExpandedPrefixV1::Erased(v) => {
                (v.output(), v.continuation().origins(), v.limits())
            }
        };
        let u = encode_loop_unroll_history_v1(
            LoopUnrollHistoryInputsV1 {
                prefix: &ff,
                output,
                origins,
                limits,
            },
            budget,
        )
        .map_err(HError::Unroll)?;
        budget.reserve_storage(u.storage().retained_storage())?;
        let uf =
            read_loop_unroll_history_v1(u.canonical_bytes(), budget).map_err(HError::Unroll)?;
        budget.reserve_storage(uf.storage().retained_storage())?;
        let ProductionExpandedHistoryV1::ScalarCleanup(scalar) = owner.history();
        let s =
            encode_scalar_fixed_point_history_v1(output, scalar, budget).map_err(HError::Scalar)?;
        budget.reserve_storage(s.storage().retained_storage())?;
        let sf = read_scalar_fixed_point_history_v1(s.canonical_bytes(), budget)
            .map_err(HError::Scalar)?;
        budget.reserve_storage(sf.storage().retained_storage())?;
        let result = encode_expanded_history_v1(&uf, &sf, budget).map_err(HError::History)?;
        budget.reserve_storage(result.storage().retained_storage())?;
        drop(sf);
        drop(s);
        drop(uf);
        drop(u);
        drop(ff);
        drop(f);
        Ok(result)
    })
}

impl Output {
    /// The old receipt remains paid on both branches. On success reserve only
    /// the returned addition; after consuming failure retire the old receipt once.
    pub(crate) fn into_serialized_expanded_history_v1(
        self,
        budget: &mut Budget<'_>,
    ) -> HResult<(Prepared, ExpandedHistoryTransportStorageV1)> {
        let incoming = budget.storage();
        scope(budget, move |budget| {
            budget.charge_work(3)?;
            if incoming < self.retained_floor {
                return Err(Resource::Accounting.into());
            }
            self.verify_equivalence(budget).map_err(HError::Native)?;
            budget.reserve_storage(header()?)?;
            let history = encode_live_history(
                &self.custody.owner,
                &self.custody.prefix_execution,
                &self.custody.history,
                budget,
            )?;
            budget.reserve_storage(history.storage().retained_storage())?;
            let additional = added(&history)?;
            let result = Prepared {
                transport: self,
                history,
                input_floor: incoming,
                retained_floor: add(incoming, additional)?,
            };
            result.verify_equivalence(budget)?;
            Ok((result, ExpandedHistoryTransportStorageV1(additional)))
        })
    }
}
impl Prepared {
    pub(crate) fn history_bytes(&self) -> &[u8] {
        self.history.canonical_bytes()
    }
    pub(crate) fn output(&self) -> &Graph {
        self.transport.output()
    }
    pub(crate) fn module(&self) -> &Module {
        self.transport.module()
    }
    pub(crate) fn descriptor_source(&self) -> &Source {
        self.transport.descriptor_source()
    }
    pub(crate) const fn retained_storage_floor_v1(&self) -> usize {
        self.retained_floor
    }
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    pub(crate) const fn authenticates_execution(&self) -> bool {
        false
    }

    pub(crate) fn verify_equivalence(&self, budget: &mut Budget<'_>) -> HResult<()> {
        if budget.storage() < self.retained_floor {
            return Err(Resource::Accounting.into());
        }
        scope(budget, |budget| {
            budget.charge_work(4)?;
            if self.input_floor < self.transport.retained_floor
                || add(self.input_floor, added(&self.history)?)? != self.retained_floor
            {
                return Err(HError::Mismatch("exact cumulative history receipt"));
            }
            self.transport
                .verify_equivalence(budget)
                .map_err(HError::Native)?;
            let expected = encode_live_history(
                &self.transport.custody.owner,
                &self.transport.custody.prefix_execution,
                &self.transport.custody.history,
                budget,
            )?;
            let expected_storage = expected.storage().retained_storage();
            budget.reserve_storage(expected_storage)?;
            equal(expected.canonical_bytes(), self.history_bytes(), budget)?;
            drop(expected);
            budget.release_storage(expected_storage)?;
            check_decoded_transport(&self.transport, self.history_bytes(), budget)
        })
    }
}

fn check_decoded_transport(
    transport: &Output,
    bytes: &[u8],
    budget: &mut Budget<'_>,
) -> HResult<()> {
    let frame = read_expanded_history_v1(bytes, budget).map_err(HError::History)?;
    budget.reserve_storage(frame.storage().retained_storage())?;
    let decoded = materialize_expanded_history_v1(&frame, budget).map_err(HError::History)?;
    budget.reserve_storage(decoded.storage().retained_storage())?;
    with_checked_decoded_expanded_source_v1(
        transport.custody.owner.source_anchor(),
        &decoded,
        budget,
        |view, budget| check_decoded_final(transport, &view, budget),
    )
    .map_err(HError::Source)?;
    drop(decoded);
    drop(frame);
    Ok(())
}
fn check_decoded_final(
    transport: &Output,
    view: &CheckedDecodedExpandedSourceV1<'_>,
    budget: &mut Budget<'_>,
) -> std::result::Result<(), ExpandedNativeTransportErrorV3> {
    let actual = view.output(budget)?;
    let owner = &transport.custody.owner;
    let origins = view.origins(budget)?;
    let kernels = view.kernels(budget)?;
    budget.charge_work(
        actual
            .canonical()
            .canonical_bytes()
            .len()
            .checked_add(owner.output().canonical().canonical_bytes().len())
            .and_then(|n| {
                n.checked_add(origins.len().checked_mul(size_of::<
                    fe2o3_lower_mir_kernel::ProductionExpandedSourceOriginV1,
                >())?)
            })
            .and_then(|n| n.checked_add(kernels.len()))
            .ok_or(Resource::Arithmetic)?,
    )?;
    if actual.canonical().canonical_bytes() != owner.output().canonical().canonical_bytes()
        || origins != owner.origins()
        || kernels != owner.kernels()
    {
        return Err(E::Mismatch("complete decoded final graph/source/report"));
    }
    let fresh = descriptor::decoded::produce(
        view,
        &transport.custody.bindings.typed_descriptor_roots,
        &transport.custody.bindings.rustc_target,
        budget,
    )
    .map_err(E::Expanded)?;
    let fresh_storage = fresh
        .capacity()
        .checked_add(size_of::<Vec<u8>>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(fresh_storage)?;
    budget.charge_work(
        fresh
            .len()
            .checked_add(transport.source.canonical_bytes().len())
            .ok_or(Resource::Arithmetic)?,
    )?;
    if fresh != transport.source.canonical_bytes() {
        return Err(E::Mismatch(
            "fresh decoded final source/file/nominal descriptor",
        ));
    }
    drop(fresh);
    budget.release_storage(fresh_storage)?;
    module::check_nominal_compiler_module_metadata_v3(
        actual,
        &transport.module,
        &transport.source,
        budget,
    )
    .map_err(E::Module)?;
    budget.reserve_storage(COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V3)?;
    let table = transport
        .source
        .table(
            transport
                .source
                .storage()
                .retained_storage()
                .checked_add(COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V3)
                .ok_or(Resource::Arithmetic)?,
            &mut |n| budget.charge_work(n),
        )
        .map_err(E::Source)?;
    budget.release_storage(DESCRIPTOR_READER_SCRATCH_STORAGE_V3)?;
    let anchor = view.source_anchor(budget)?;
    with_checked_source_pipeline_catalog_v1(anchor, budget, |catalog, budget| {
        let relation = check_native_v12_text_descriptor_relation_v3(
            actual,
            catalog.catalog(budget).map_err(NativeError::Resource)?,
            actual.canonical().canonical_bytes(),
            transport.custody.profile,
            &table,
            transport.module.llvm_ir(),
            budget,
        )?;
        budget
            .reserve_storage(relation.storage().retained_storage())
            .map_err(NativeError::Resource)?;
        budget
            .charge_work(
                relation
                    .pre_descriptor_llvm()
                    .len()
                    .checked_add(transport.custody.llvm.len())
                    .ok_or(Resource::Arithmetic)?,
            )
            .map_err(NativeError::Resource)?;
        if relation.pre_descriptor_llvm() != transport.custody.llvm {
            return Err(NativeError::Invalid(
                "actual expanded-final native prefix after full decode",
            ));
        }
        drop(relation);
        Ok::<_, NativeError>(())
    })
    .map_err(E::Catalog)?;
    drop(table);
    budget.release_storage(DESCRIPTOR_TABLE_VIEW_STORAGE_V3)?;
    Ok(())
}
