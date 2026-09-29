//! Source-bound conditional Worker inputs, before signed/refinement admission.
use super::super::{Budget, Resource};
use super::{ConditionalMixedTargetLlvmV26, SourceError};
use fe2o3_kernel_descriptor::{
    CodeObjectVersion, DescriptorWireErrorV3, DeviceDescriptorTableV3, DeviceTargetV1,
    mixed_conditional_v26::MAX_MIXED_CONTRACT_BYTES_V26,
};
use fe2o3_lower_mir_kernel::ProductionKernelArgumentAbiInputV18 as Abi;
use std::cell::Cell;
use std::mem::{align_of, size_of};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

#[derive(Debug)]
pub(crate) enum MixedWorkerInputErrorV26 {
    Source(SourceError),
    Target(super::super::ClosedScalarTargetLlvmErrorV29),
    Descriptor(DescriptorWireErrorV3<Resource>),
    Mismatch(&'static str),
}
impl From<SourceError> for MixedWorkerInputErrorV26 {
    fn from(error: SourceError) -> Self {
        Self::Source(error)
    }
}
impl From<Resource> for MixedWorkerInputErrorV26 {
    fn from(error: Resource) -> Self {
        Self::Source(error.into())
    }
}
impl From<super::super::ClosedScalarTargetLlvmErrorV29> for MixedWorkerInputErrorV26 {
    fn from(error: super::super::ClosedScalarTargetLlvmErrorV29) -> Self {
        Self::Target(error)
    }
}
impl std::fmt::Display for MixedWorkerInputErrorV26 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "conditional mixed Worker input: {self:?}")
    }
}
impl std::error::Error for MixedWorkerInputErrorV26 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Target(error) => Some(error),
            Self::Descriptor(error) => Some(error),
            Self::Mismatch(_) => None,
        }
    }
}
type Result<T> = std::result::Result<T, MixedWorkerInputErrorV26>;

/// Required producers are absent from this intermediate input. This is not a
/// caller-selectable option or a gate that can be cleared by supplying a digest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MixedWorkerInputOpenGateV26 {
    SignedV18SourceRefinementReceipt,
    ProtectedCompilerExecutionJoin,
    VersionedWorkerAndFinalizerReplay,
    ConcreteRuntimePremiseDischarge,
}

/// Move-only, inseparable views of actual V18 target text and source contracts.
/// This is not a Worker request, authenticated compiler-module handoff, signed
/// proof or artifact authority. All contract bytes are freshly emitted from the
/// genuine retained handoff; a caller cannot install serialized row claims.
#[must_use = "discard the prepared inputs before their borrowed target owner"]
pub(crate) struct PreparedMixedWorkerInputV26<'native, 'handoff, 'view, 'source, 'table, 'wire> {
    native: &'native ConditionalMixedTargetLlvmV26<'handoff, 'view, 'source>,
    table: &'table DeviceDescriptorTableV3<'wire>,
    contracts: Vec<Vec<u8>>,
    retained: usize,
    required: usize,
}
impl PreparedMixedWorkerInputV26<'_, '_, '_, '_, '_, '_> {
    fn custody(&self, budget: &Budget<'_>) -> std::result::Result<(), SourceError> {
        self.native
            .handoff
            .observe_retained_storage_v18(self.required, budget)
    }
    fn check(&self, budget: &Budget<'_>) -> std::result::Result<(), SourceError> {
        let custody = self.custody(budget);
        self.native.check(budget).and(custody)
    }
    pub(crate) fn llvm_ir(&self, budget: &Budget<'_>) -> Result<&str> {
        self.check(budget)?;
        Ok(self.native.llvm_ir(budget)?)
    }
    pub(crate) fn descriptor(&self, budget: &Budget<'_>) -> Result<&DeviceDescriptorTableV3<'_>> {
        self.check(budget)?;
        Ok(self.table)
    }
    pub(crate) fn contract(&self, original_root: usize, budget: &Budget<'_>) -> Result<&[u8]> {
        self.check(budget)?;
        self.contracts
            .get(original_root)
            .map(Vec::as_slice)
            .ok_or(MixedWorkerInputErrorV26::Mismatch("original root ordinal"))
    }
    pub(crate) fn root_count(&self, budget: &Budget<'_>) -> Result<usize> {
        self.check(budget)?;
        Ok(self.contracts.len())
    }
    pub(crate) fn retained_storage(&self, budget: &Budget<'_>) -> Result<usize> {
        self.check(budget)?;
        Ok(self.retained)
    }
    pub(crate) const fn open_gates(&self) -> &'static [MixedWorkerInputOpenGateV26] {
        use MixedWorkerInputOpenGateV26::*;
        &[
            SignedV18SourceRefinementReceipt,
            ProtectedCompilerExecutionJoin,
            VersionedWorkerAndFinalizerReplay,
            ConcreteRuntimePremiseDischarge,
        ]
    }
    pub(crate) const fn grants_worker_or_artifact_authority(&self) -> bool {
        false
    }
    pub(crate) fn discard(self, budget: &mut Budget<'_>) -> Result<()> {
        let selected = self.check(budget);
        let custody = self.custody(budget);
        let Self {
            native,
            contracts,
            retained,
            ..
        } = self;
        drop(contracts);
        let settled = custody.and_then(|()| {
            budget
                .release_storage(retained)
                .map_err(|error| native.source.retain_query_resource_error_v18(error))
        });
        selected?;
        settled?;
        Ok(())
    }
}

fn sum(parts: &[usize]) -> std::result::Result<usize, Resource> {
    parts.iter().try_fold(0usize, |n, part| {
        n.checked_add(*part).ok_or(Resource::Arithmetic)
    })
}
fn reserve(n: usize, accepted: &Cell<usize>, budget: &mut Budget<'_>) -> Result<()> {
    let next = sum(&[accepted.get(), n])?;
    budget.reserve_storage(n)?;
    accepted.set(next);
    Ok(())
}
fn vector<T>(count: usize, accepted: &Cell<usize>, budget: &mut Budget<'_>) -> Result<Vec<T>> {
    let bytes = count
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    budget.charge_work(count)?;
    reserve(bytes, accepted, budget)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    reserve(
        rows.capacity()
            .checked_sub(count)
            .and_then(|n| n.checked_mul(size_of::<T>()))
            .ok_or(Resource::Arithmetic)?,
        accepted,
        budget,
    )?;
    Ok(rows)
}

fn descriptor_ordinal(
    table: &DeviceDescriptorTableV3<'_>,
    binding: &[u8; 32],
    budget: &mut Budget<'_>,
) -> Result<usize> {
    // V3 admission requires a strictly sorted, unique kernel-id roster. Search
    // the canonical structured view without assuming semantic-root order.
    let (mut low, mut high) = (0, table.kernel_count());
    while low < high {
        budget.charge_work(40)?;
        let middle = low + (high - low) / 2;
        let kernel = table
            .kernel(middle, &mut |n| budget.charge_work(n))
            .map_err(MixedWorkerInputErrorV26::Descriptor)?;
        match kernel.kernel_id().as_bytes().cmp(binding) {
            std::cmp::Ordering::Less => low = middle + 1,
            std::cmp::Ordering::Greater => high = middle,
            std::cmp::Ordering::Equal => return Ok(middle),
        }
    }
    Err(MixedWorkerInputErrorV26::Mismatch(
        "complete source/descriptor root roster",
    ))
}

fn headers() -> std::result::Result<usize, Resource> {
    type Owner<'a> = PreparedMixedWorkerInputV26<'a, 'a, 'a, 'a, 'a, 'a>;
    type Frame<'a> = (
        &'a ConditionalMixedTargetLlvmV26<'a, 'a, 'a>,
        &'a DeviceDescriptorTableV3<'a>,
        Abi<'a>,
        &'a mut Budget<'a>,
        Cell<usize>,
        &'a Cell<usize>,
        Vec<Vec<u8>>,
        Vec<u8>,
        Vec<u8>,
        [usize; 12],
        [u8; 32],
        Result<Vec<Vec<u8>>>,
        std::thread::Result<Result<Vec<Vec<u8>>>>,
        fe2o3_kernel_descriptor::KernelDescriptorRefV3<'a, 'a>,
        std::slice::Iter<'a, fe2o3_lower_mir_kernel::ProductionKernelArgumentAbiRootV18<'a>>,
    );
    sum(&[
        size_of::<Owner<'_>>(),
        align_of::<Owner<'_>>(),
        size_of::<Frame<'_>>(),
        align_of::<Frame<'_>>(),
        size_of::<AssertUnwindSafe<Frame<'_>>>(),
        size_of::<Result<Owner<'_>>>(),
    ])
}

/// Replay complete original ABI and freshly emit all root contracts against the
/// exact target's decoded V3 descriptor candidate. Borrowed compiler identity
/// labels and build-evidence fields remain claims, not protected compiler origin.
/// The target owner must already be retained on this same source ledger.
/// Descriptor decoding and target lowering keep their existing bounded domains;
/// this wrapper prepays its frames, borrowed wire and all vector capacities.
pub(crate) fn prepare_mixed_worker_input_v26<'native, 'handoff, 'view, 'source, 'table, 'wire>(
    native: &'native ConditionalMixedTargetLlvmV26<'handoff, 'view, 'source>,
    abi: Abi<'_>,
    table: &'table DeviceDescriptorTableV3<'wire>,
    budget: &mut Budget<'_>,
) -> Result<PreparedMixedWorkerInputV26<'native, 'handoff, 'view, 'source, 'table, 'wire>> {
    native.check(budget)?;
    let floor = budget.storage();
    let accepted = Cell::new(0usize);
    let caught = catch_unwind(AssertUnwindSafe(|| -> Result<Vec<Vec<u8>>> {
        reserve(
            sum(&[headers()?, table.canonical_bytes().len()])?,
            &accepted,
            budget,
        )?;
        native
            .handoff
            .check_original_argument_abi_v26(Abi { roots: abi.roots }, budget)?;
        let profile = native.target(budget)?;
        budget.charge_work(sum(&[profile.device_target().len(), 8])?)?;
        let expected = DeviceTargetV1::parse(profile.device_target())
            .map_err(|_| MixedWorkerInputErrorV26::Mismatch("retained target profile"))?;
        if table.device_target() != expected || table.code_object_version() != CodeObjectVersion::V6
        {
            return Err(MixedWorkerInputErrorV26::Mismatch(
                "descriptor target or code object version",
            ));
        }
        budget.charge_work(4)?;
        if abi.roots.is_empty()
            || abi.roots.len() != table.kernel_count()
            || abi.roots.len() != native.source.root_count(budget)?
        {
            return Err(MixedWorkerInputErrorV26::Mismatch(
                "complete source/descriptor root count",
            ));
        }
        let mut contracts = vector::<Vec<u8>>(abi.roots.len(), &accepted, budget)?;
        let mut scratch = vector::<u8>(MAX_MIXED_CONTRACT_BYTES_V26, &accepted, budget)?;
        scratch.resize(MAX_MIXED_CONTRACT_BYTES_V26, 0);
        for (original_root, root) in abi.roots.iter().enumerate() {
            budget.charge_work(2)?;
            let ordinal = descriptor_ordinal(table, root.kernel_binding, budget)?;
            let length = native.handoff.emit_mixed_contract_v26(
                original_root,
                Abi { roots: abi.roots },
                table,
                ordinal,
                &mut scratch,
                budget,
            )?;
            let mut contract = vector::<u8>(length, &accepted, budget)?;
            contract.extend_from_slice(&scratch[..length]);
            if contracts.len() == contracts.capacity() {
                return Err(Resource::Accounting.into());
            }
            contracts.push(contract);
        }
        drop(scratch);
        native.check(budget)?;
        Ok(contracts)
    }));
    let required = floor
        .checked_add(accepted.get())
        .expect("accepted storage is representable");
    let custody = native
        .handoff
        .observe_retained_storage_v18(required, budget);
    match caught {
        Ok(Ok(contracts)) if custody.is_ok() => Ok(PreparedMixedWorkerInputV26 {
            native,
            table,
            contracts,
            retained: accepted.get(),
            required: budget.storage(),
        }),
        Ok(Ok(contracts)) => {
            drop(contracts);
            Err(native
                .source
                .retain_query_resource_error_v18(Resource::Accounting)
                .into())
        }
        Ok(Err(error)) => {
            if custody.is_ok() {
                let _ = budget
                    .release_storage(accepted.get())
                    .map_err(|error| native.source.retain_query_resource_error_v18(error));
            }
            if let MixedWorkerInputErrorV26::Source(SourceError::Resource(resource)) = error {
                return Err(native
                    .source
                    .retain_query_resource_error_v18(resource)
                    .into());
            }
            Err(error)
        }
        Err(payload) => {
            if custody.is_ok() {
                let _ = budget
                    .release_storage(accepted.get())
                    .map_err(|error| native.source.retain_query_resource_error_v18(error));
            }
            resume_unwind(payload)
        }
    }
}

#[cfg(test)]
#[path = "production_worker_mixed_input_v26_tests.rs"]
pub(crate) mod tests;
