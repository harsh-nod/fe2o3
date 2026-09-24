//! Native source custody through the shared artifact inspector and finalizer.

use std::{fmt, mem::size_of};

use fe2o3_compiler_ffi::CompilerDescriptorSourceV1;
use fe2o3_kernel_descriptor::{CanonicalCodeObjectDigest, DescriptorWireErrorV3};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use sha2::{Digest, Sha256};

use crate::{
    ContentIdentityV1, InertNativeFirstBuildWorkerEvidenceV1,
    NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V3, NativeFirstBuildWorkerErrorV1, NativeWorkerDiagnosticV1,
    NominalFinalizationErrorV3, WorkerV3HsacoPolicyV1, finalize_unfinalized_nominal_hsaco_v3,
    inspect_unfinalized_nominal_hsaco_v3,
    request_construction::decode_link_options,
    worker_v3_finalized_schema::DescriptorSchema,
    worker_v3_hsaco_admission::{
        SharedWorkerV3HsacoInspectionV1, WorkerV3LaunchContractV1,
        inspect_worker_v3_hsaco_preimage_v1, strict_kernel_launch_contract,
    },
    worker_v3_hsaco_finalization::finalize_worker_hsaco_preimage_v1,
};

const DOMAIN: &[u8] = b"FE2O3/NATIVE-WORKER-CANONICAL-FINALIZATION/V1\0";
const ENTRY_WORK: usize = 4096;
const ENTRY_STORAGE: usize = 2 * size_of::<PreparedFinalizedNativeWorkerHsacoV1>()
    + size_of::<NativeWorkerFinalizationErrorV1>()
    + size_of::<Sha256>()
    + 1024;

/// Additional unreserved native owner/header charge; artifact parsing and payload
/// storage retain the existing, separately bounded finalizer resource domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeWorkerFinalizationStorageV1(usize);
impl NativeWorkerFinalizationStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Domain-separated binding of retained native source/Worker evidence, independent
/// raw inspection and the exact finalized descriptor/artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeWorkerFinalizationIdentityV1([u8; 32]);
impl NativeWorkerFinalizationIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Structural finalization retaining the complete native source/F owner and its
/// fresh-consumption or independently recovered transcript custody distinction.
/// This supplies no protected origin, currentness, proof, publication or launch
/// authority. No V3 outer owner is manufactured from the native carrier.
///
/// ```compile_fail
/// use fe2o3_hsaco_finalize::PreparedFinalizedNativeWorkerHsacoV1 as Finalized;
/// fn duplicate(value: Finalized) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::PreparedFinalizedNativeWorkerHsacoV1 as Finalized;
/// fn forge() -> Finalized { Finalized::default() }
/// ```
pub struct PreparedFinalizedNativeWorkerHsacoV1 {
    source: InertNativeFirstBuildWorkerEvidenceV1,
    inspection: SharedWorkerV3HsacoInspectionV1,
    bytes: Vec<u8>,
    descriptor: Vec<u8>,
    digest: CanonicalCodeObjectDigest,
    identity: NativeWorkerFinalizationIdentityV1,
    output_identity: ContentIdentityV1,
    descriptor_identity: ContentIdentityV1,
    retained_storage: usize,
}
impl PreparedFinalizedNativeWorkerHsacoV1 {
    pub fn source_evidence(&self) -> &InertNativeFirstBuildWorkerEvidenceV1 {
        &self.source
    }
    pub fn exact_finalized_bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn descriptor_bytes(&self) -> &[u8] {
        &self.descriptor
    }
    pub const fn canonical_digest(&self) -> CanonicalCodeObjectDigest {
        self.digest
    }
    pub const fn identity(&self) -> NativeWorkerFinalizationIdentityV1 {
        self.identity
    }
    pub const fn output_identity(&self) -> ContentIdentityV1 {
        self.output_identity
    }
    pub const fn descriptor_identity(&self) -> ContentIdentityV1 {
        self.descriptor_identity
    }
    pub fn raw_policy(&self) -> &WorkerV3HsacoPolicyV1 {
        &self.inspection.policy
    }
    /// Complete native ledger floor, excluding the separately bounded artifact domain.
    pub const fn required_retained_storage(&self) -> usize {
        self.retained_storage
    }
    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }
    pub const fn grants_publication_authority(&self) -> bool {
        false
    }
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Debug)]
pub enum NativeWorkerFinalizationErrorV1 {
    Resource(Resource),
    Source(NativeFirstBuildWorkerErrorV1),
    Artifact {
        phase: &'static str,
        diagnostic: NativeWorkerDiagnosticV1,
    },
}
impl From<Resource> for NativeWorkerFinalizationErrorV1 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl From<NativeFirstBuildWorkerErrorV1> for NativeWorkerFinalizationErrorV1 {
    fn from(value: NativeFirstBuildWorkerErrorV1) -> Self {
        Self::Source(value)
    }
}
impl fmt::Display for NativeWorkerFinalizationErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Source(e) => e.fmt(f),
            Self::Artifact { phase, diagnostic } => {
                write!(f, "native finalizer {phase}: {diagnostic}")
            }
        }
    }
}
impl std::error::Error for NativeWorkerFinalizationErrorV1 {}

type Result<T> = std::result::Result<T, NativeWorkerFinalizationErrorV1>;

fn failure(phase: &'static str, value: impl fmt::Display) -> NativeWorkerFinalizationErrorV1 {
    NativeWorkerFinalizationErrorV1::Artifact {
        phase,
        diagnostic: NativeWorkerDiagnosticV1::from_display(value),
    }
}

fn nominal_failure(
    phase: &'static str,
    error: NominalFinalizationErrorV3<Resource>,
) -> NativeWorkerFinalizationErrorV1 {
    match error {
        NominalFinalizationErrorV3::Work(resource)
        | NominalFinalizationErrorV3::Wire(DescriptorWireErrorV3::Work(resource)) => {
            NativeWorkerFinalizationErrorV1::Resource(resource)
        }
        other => failure(phase, other),
    }
}

fn check_export_manifest(receipt: &[u8], manifest: &[u8], budget: &mut Budget<'_>) -> Result<()> {
    budget.charge_work(
        receipt
            .len()
            .max(manifest.len())
            .checked_add(1)
            .ok_or(Resource::Arithmetic)?,
    )?;
    if receipt != manifest {
        return Err(failure(
            "export manifest",
            "source receipt differs from compiler module",
        ));
    }
    Ok(())
}

/// Rebinds the exact retained native source, checks the same Worker/provider and
/// AMDHSA/ELF policies as the legacy adapter, then runs the existing canonical
/// finalizer for the descriptor schema in the retained ABI receipt.
///
/// The original source/preflight/Worker reservations remain paid on success or
/// refusal. The returned native header charge is additional and unreserved.
/// Native binding checks and fixed identity work use this caller ledger. Shared
/// Worker-wire decoding, artifact inspection/finalization, their payloads and
/// errors retain the existing bounded artifact domain, just as in nominal Worker
/// finalization; this is not aggregate artifact work/storage or an RSS bound.
/// Nominal descriptor traversal additionally uses its existing scratch/work API
/// on this ledger, preserving typed resource refusals. Artifact failures retain
/// only bounded text; no failure retains the source owner.
pub fn finalize_native_worker_hsaco_v1(
    source: InertNativeFirstBuildWorkerEvidenceV1,
    budget: &mut Budget<'_>,
) -> Result<(
    PreparedFinalizedNativeWorkerHsacoV1,
    NativeWorkerFinalizationStorageV1,
)> {
    let floor = source.required_retained_storage();
    budget.with_prepaid_scope(floor, 8, ENTRY_WORK, ENTRY_STORAGE, |budget| {
        source.revalidate_for_artifact(budget)?;
        source
            .artifact_lineage()
            .validate()
            .map_err(|e| failure("Worker lineage", e))?;
        let outer = source.recovered_handoff().handoff();
        let module = outer.module_handoff();
        let receipts = outer.capsule().base().receipts();
        check_export_manifest(
            receipts.export_manifest().canonical_preimage(),
            module.symbol_manifest().canonical_bytes(),
            budget,
        )?;
        let abi = receipts.abi().canonical_preimage();
        let schema =
            DescriptorSchema::from_abi(abi).map_err(|e| failure("descriptor schema", e))?;
        let launch = derive_launch(schema, abi, source.output_bytes(), budget)?;
        let (code_object, _) =
            decode_link_options(source.plan().options()).map_err(|e| failure("link options", e))?;
        let inspection = inspect_worker_v3_hsaco_preimage_v1(
            source.plan().target(),
            code_object,
            module.symbol_manifest().clone(),
            module.envelope().identity(),
            source.output_identity(),
            source.output_bytes(),
            launch,
        )
        .map_err(|e| failure("raw inspection", e))?;
        let (bytes, descriptor, digest) = match schema {
            DescriptorSchema::V1 => {
                let core = finalize_worker_hsaco_preimage_v1(
                    source.output_bytes(),
                    source.output_identity(),
                    &inspection.policy,
                    abi,
                )
                .map_err(|e| failure("canonical finalization", e))?;
                let digest = core.finalized.inspection().digest();
                (core.finalized.into_bytes(), core.descriptor_bytes, digest)
            }
            DescriptorSchema::NominalV3 => budget.with_prepaid_scope(
                floor,
                0,
                0,
                NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V3,
                |budget| {
                    let value = finalize_unfinalized_nominal_hsaco_v3(
                        source.output_bytes(),
                        abi,
                        NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V3,
                        &mut |work| budget.charge_work(work),
                    )
                    .map_err(|e| nominal_failure("nominal finalization", e))?;
                    let descriptor = value.descriptor_bytes().to_vec();
                    let digest = value.digest();
                    Ok::<_, NativeWorkerFinalizationErrorV1>((
                        value.into_bytes(),
                        descriptor,
                        digest,
                    ))
                },
            )?,
        };
        let output_identity = ContentIdentityV1::calculate(&bytes);
        let descriptor_identity = ContentIdentityV1::calculate(&descriptor);
        let identity = finalization_identity(
            &source,
            &inspection,
            output_identity,
            descriptor_identity,
            digest,
        );
        let storage =
            NativeWorkerFinalizationStorageV1(size_of::<PreparedFinalizedNativeWorkerHsacoV1>());
        let retained_storage = floor.checked_add(storage.0).ok_or(Resource::Arithmetic)?;
        Ok((
            PreparedFinalizedNativeWorkerHsacoV1 {
                source,
                inspection,
                bytes,
                descriptor,
                digest,
                identity,
                output_identity,
                descriptor_identity,
                retained_storage,
            },
            storage,
        ))
    })
}

fn derive_launch(
    schema: DescriptorSchema,
    abi: &[u8],
    raw: &[u8],
    budget: &mut Budget<'_>,
) -> Result<WorkerV3LaunchContractV1> {
    let mut launch = None;
    let mut join = |actual| -> Result<()> {
        if launch.is_some_and(|expected| expected != actual) {
            return Err(failure("launch", "heterogeneous per-kernel launch policy"));
        }
        launch = Some(actual);
        Ok(())
    };
    match schema {
        DescriptorSchema::V1 => {
            let source = CompilerDescriptorSourceV1::decode(abi)
                .map_err(|e| failure("descriptor source", e))?;
            for kernel in source.table().kernels() {
                join(
                    strict_kernel_launch_contract(kernel.launch())
                        .map_err(|e| failure("launch", e))?,
                )?;
            }
        }
        DescriptorSchema::NominalV3 => {
            let floor = budget.storage();
            budget.with_prepaid_scope(
                floor,
                0,
                0,
                NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V3,
                |budget| {
                    let inspected = inspect_unfinalized_nominal_hsaco_v3(
                        raw,
                        NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V3,
                        &mut |work| budget.charge_work(work),
                    )
                    .map_err(|e| nominal_failure("nominal descriptor", e))?;
                    let table = inspected.descriptor_table();
                    for index in 0..table.kernel_count() {
                        let kernel = table
                            .kernel(index, &mut |work| budget.charge_work(work))
                            .map_err(|e| {
                                nominal_failure(
                                    "nominal kernel",
                                    NominalFinalizationErrorV3::Wire(e),
                                )
                            })?;
                        join(
                            strict_kernel_launch_contract(kernel.launch())
                                .map_err(|e| failure("launch", e))?,
                        )?;
                    }
                    Ok::<_, NativeWorkerFinalizationErrorV1>(())
                },
            )?;
        }
    }
    launch.ok_or_else(|| failure("launch", "empty kernel set"))
}

fn finalization_identity(
    source: &InertNativeFirstBuildWorkerEvidenceV1,
    inspection: &SharedWorkerV3HsacoInspectionV1,
    output: ContentIdentityV1,
    descriptor: ContentIdentityV1,
    digest: CanonicalCodeObjectDigest,
) -> NativeWorkerFinalizationIdentityV1 {
    // The privately constructed source identity binds the complete native outer,
    // original occurrence, actual F and exact candidate/replay transcripts. This
    // fixed preimage composes those identities with independently checked output.
    let mut hash = Sha256::new();
    hash.update(DOMAIN);
    hash.update(source.identity().as_bytes());
    hash.update(source.binding().identity().as_bytes());
    hash.update(inspection.policy.identity().as_bytes());
    hash.update(inspection.descriptor_identity);
    hash.update(inspection.abi_identity);
    hash.update(inspection.resource_identity);
    for content in [output, descriptor] {
        hash.update(content.sha256());
        hash.update(content.byte_len().to_le_bytes());
    }
    hash.update(digest.as_bytes());
    NativeWorkerFinalizationIdentityV1(hash.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

    fn assert_resource(error: NativeWorkerFinalizationErrorV1, expected: Resource) {
        let NativeWorkerFinalizationErrorV1::Resource(actual) = error else {
            panic!("resource refusal was reclassified: {error:?}");
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn nominal_resource_mappings_preserve_exact_variants() {
        let mut work = Work::new(7);
        let mut budget = Budget::new(&mut work, 11);
        budget.charge_work(3).unwrap();
        budget.reserve_storage(5).unwrap();
        let resources = [
            budget.charge_work(5).unwrap_err(),
            budget.reserve_storage(7).unwrap_err(),
            Resource::Allocation,
            Resource::Accounting,
            Resource::Arithmetic,
        ];
        for phase in [
            "nominal finalization",
            "nominal descriptor",
            "nominal kernel",
        ] {
            for resource in resources {
                assert_resource(
                    nominal_failure(phase, NominalFinalizationErrorV3::Work(resource)),
                    resource,
                );
                assert_resource(
                    nominal_failure(
                        phase,
                        NominalFinalizationErrorV3::Wire(DescriptorWireErrorV3::Work(resource)),
                    ),
                    resource,
                );
            }
        }
        assert_eq!(budget.work(), 3);
        assert_eq!(budget.storage(), 5);
        assert_eq!(budget.failed_storage(), Some(12));
        assert_eq!(work.failed_work(), Some(8));
    }

    #[test]
    fn nominal_artifact_mappings_preserve_phase_and_bounded_diagnostic() {
        for phase in [
            "nominal finalization",
            "nominal descriptor",
            "nominal kernel",
        ] {
            let errors = [
                NominalFinalizationErrorV3::Artifact(
                    crate::FinalizationError::MissingDescriptorSection,
                ),
                NominalFinalizationErrorV3::DescriptorSourceMismatch,
                NominalFinalizationErrorV3::Scratch {
                    required: 2,
                    prepaid: 1,
                },
                NominalFinalizationErrorV3::Wire(DescriptorWireErrorV3::Index {
                    field: "kernel",
                    index: usize::MAX,
                    count: usize::MAX,
                }),
            ];
            for error in errors {
                let NativeWorkerFinalizationErrorV1::Artifact {
                    phase: actual,
                    diagnostic,
                } = nominal_failure(phase, error)
                else {
                    panic!("artifact failure was reclassified");
                };
                assert_eq!(actual, phase);
                let text = diagnostic.to_string();
                assert!(!text.is_empty());
                assert!(text.len() <= 80);
            }
        }
    }

    #[test]
    fn export_manifest_exact_budget_is_cumulative_and_length_dependent() {
        for len in [0, 1, ENTRY_WORK + 7] {
            let manifest = vec![b'x'; len];
            let mut work = Work::new(3 + len + 1);
            let mut budget = Budget::new(&mut work, 5);
            budget.charge_work(3).unwrap();
            budget.reserve_storage(5).unwrap();
            check_export_manifest(&manifest, &manifest, &mut budget).unwrap();
            assert_eq!(budget.work(), 3 + len + 1);
            assert_eq!(budget.storage(), 5);
            assert_eq!(budget.peak_storage(), 5);
            assert_eq!(work.failed_work(), None);
        }
    }

    #[test]
    fn export_manifest_one_short_refuses_before_comparison() {
        for (receipt, manifest) in [
            (&b""[..], &b""[..]),
            (&b"abc"[..], &b"abc"[..]),
            (&b"abc"[..], &b"abd"[..]),
            (&b"abcd"[..], &b"abc"[..]),
            (&b"abc"[..], &b"abcd"[..]),
        ] {
            let charge = receipt.len().max(manifest.len()) + 1;
            let mut work = Work::new(3 + charge - 1);
            let mut budget = Budget::new(&mut work, 5);
            budget.charge_work(3).unwrap();
            budget.reserve_storage(5).unwrap();
            assert!(matches!(
                check_export_manifest(receipt, manifest, &mut budget),
                Err(NativeWorkerFinalizationErrorV1::Resource(Resource::Work(_)))
            ));
            assert_eq!(budget.work(), 3);
            assert_eq!(budget.storage(), 5);
            assert_eq!(budget.peak_storage(), 5);
            assert_eq!(work.failed_work(), Some(3 + charge));
        }
    }

    #[test]
    fn funded_export_manifest_mismatch_remains_artifact_failure() {
        for (receipt, manifest) in [
            (&b"abc"[..], &b"abd"[..]),
            (&b"abcd"[..], &b"abc"[..]),
            (&b"abc"[..], &b"abcd"[..]),
        ] {
            let charge = receipt.len().max(manifest.len()) + 1;
            let mut work = Work::new(charge);
            let mut budget = Budget::new(&mut work, 0);
            assert!(matches!(
                check_export_manifest(receipt, manifest, &mut budget),
                Err(NativeWorkerFinalizationErrorV1::Artifact {
                    phase: "export manifest",
                    ..
                })
            ));
            assert_eq!(budget.work(), charge);
            assert_eq!(budget.storage(), 0);
            assert_eq!(budget.peak_storage(), 0);
            assert_eq!(work.failed_work(), None);
        }
    }
}
