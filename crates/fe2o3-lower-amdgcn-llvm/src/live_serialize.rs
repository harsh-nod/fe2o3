use core::{error::Error, fmt};

use fe2o3_llvm_handoff::HandoffIdentityV2;
use fe2o3_llvm_text::{
    Gfx942LlvmAssemblyV2, LlvmAssemblySha256V2, SerializeErrorV2, serialize_gfx942_handoff_v2,
};
use fe2o3_llvm_worker_handoff::{
    AdmittedWorkerRequestV2, MeasuredLlvmLldBuildV1, WorkerAdmissionErrorV2,
    WorkerAdmissionIdentityV2, WorkerAdmissionRequestV2,
};
use sha2::{Digest as _, Sha256};

use crate::{
    CanonicalLoweringReceiptV1, CanonicalPlironLlvmGraphExportV1, GraphExportErrorV1,
    GraphExportIdentityV1, GraphExportRequestV1, LiveGraphInspectionV1, LoweredAmdgcnPlironLlvmV1,
    LoweringReceiptIdentityV1, NonGraphEnvelopeIdentityV1,
};

const SERIALIZATION_IDENTITY_DOMAIN_V1: &[u8] =
    b"fe2o3.lower-amdgcn-llvm.live-worker-serialization.identity.v1\0";

/// Untrusted identities presented when acquiring a live graph serialization token.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveGraphSerializationRequestV1 {
    receipt_identity: LoweringReceiptIdentityV1,
    non_graph_envelope_identity: NonGraphEnvelopeIdentityV1,
}

impl LiveGraphSerializationRequestV1 {
    /// Constructs one untrusted live serialization request.
    pub const fn new(
        receipt_identity: LoweringReceiptIdentityV1,
        non_graph_envelope_identity: NonGraphEnvelopeIdentityV1,
    ) -> Self {
        Self {
            receipt_identity,
            non_graph_envelope_identity,
        }
    }
}

/// Failure while freshly traversing, serializing, or admitting one live graph.
#[derive(Debug)]
pub enum LiveGraphSerializationErrorV1 {
    /// Fresh bounded graph traversal or correspondence admission failed.
    Graph(GraphExportErrorV1),
    /// Graph-derived Handoff V2 could not be represented as bounded LLVM assembly.
    Assembly(SerializeErrorV2),
    /// Exact canonical graph bytes or measured worker policy failed admission.
    Worker(WorkerAdmissionErrorV2),
    /// A retained export was paired with a different process-local graph owner.
    RetainedGraphOwnerMismatch,
    /// Fresh traversal of the retained owner no longer reproduces the retained export.
    RetainedGraphExportMismatch,
}

impl fmt::Display for LiveGraphSerializationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "live Pliron LLVM serialization failed: {self:?}")
    }
}

impl Error for LiveGraphSerializationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Graph(error) => Some(error),
            Self::Assembly(error) => Some(error),
            Self::Worker(error) => Some(error),
            Self::RetainedGraphOwnerMismatch | Self::RetainedGraphExportMismatch => None,
        }
    }
}

/// Domain-separated identity of one exact live graph serialization and admission.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LiveGraphSerializationIdentityV1([u8; 32]);

impl LiveGraphSerializationIdentityV1 {
    /// Returns the exact identity bytes.
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Durable evidence produced while the exact owner-bound graph was live.
///
/// This receipt binds identities only. It does not claim that a process-local
/// Pliron context crosses the worker boundary and grants no artifact authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveGraphSerializationReceiptV1 {
    graph_export_identity: GraphExportIdentityV1,
    graph_handoff_identity: HandoffIdentityV2,
    graph_receipt_identity: LoweringReceiptIdentityV1,
    non_graph_envelope_identity: NonGraphEnvelopeIdentityV1,
    graph_inspection: LiveGraphInspectionV1,
    assembly_sha256: LlvmAssemblySha256V2,
    worker_admission_identity: WorkerAdmissionIdentityV2,
    identity: LiveGraphSerializationIdentityV1,
}

impl LiveGraphSerializationReceiptV1 {
    /// Returns the fresh bounded graph-export identity.
    pub const fn graph_export_identity(self) -> GraphExportIdentityV1 {
        self.graph_export_identity
    }

    /// Returns the canonical identity reconstructed from the live graph.
    pub const fn graph_handoff_identity(self) -> HandoffIdentityV2 {
        self.graph_handoff_identity
    }

    /// Returns the fresh graph-derived receipt identity.
    pub const fn graph_receipt_identity(self) -> LoweringReceiptIdentityV1 {
        self.graph_receipt_identity
    }

    /// Returns the independently bound non-graph envelope identity.
    pub const fn non_graph_envelope_identity(self) -> NonGraphEnvelopeIdentityV1 {
        self.non_graph_envelope_identity
    }

    /// Returns facts recovered by the traversal that immediately preceded serialization.
    pub const fn graph_inspection(self) -> LiveGraphInspectionV1 {
        self.graph_inspection
    }

    /// Returns the digest of exact graph-derived LLVM assembly bytes.
    pub const fn assembly_sha256(self) -> LlvmAssemblySha256V2 {
        self.assembly_sha256
    }

    /// Returns the exact graph-Handoff and measured-build admission identity.
    pub const fn worker_admission_identity(self) -> WorkerAdmissionIdentityV2 {
        self.worker_admission_identity
    }

    /// Returns the identity binding all live serialization receipt fields.
    pub const fn identity(self) -> LiveGraphSerializationIdentityV1 {
        self.identity
    }
}

/// Move-only concrete export retained from one exact live graph traversal.
///
/// Unlike the identity-only serialization receipt, this value owns the
/// canonical graph-derived handoff. It also remembers process-local owner
/// provenance so an equivalent graph in another Pliron context cannot be
/// substituted during fresh revalidation. Neither identity grants artifact or
/// runtime authority.
///
/// The retained export cannot be cloned:
///
/// ```compile_fail
/// use fe2o3_lower_amdgcn_llvm::RetainedLiveGraphExportV1;
///
/// fn clone_export(export: RetainedLiveGraphExportV1) {
///     let _ = export.clone();
/// }
/// ```
#[derive(Debug)]
pub struct RetainedLiveGraphExportV1 {
    owner: fe2o3_pliron::ContextIdentity,
    export: CanonicalPlironLlvmGraphExportV1,
}

impl RetainedLiveGraphExportV1 {
    /// Returns the concrete canonical handoff reconstructed from the live graph.
    pub const fn canonical_handoff(&self) -> &fe2o3_llvm_handoff::Gfx942HandoffV2 {
        self.export.graph_handoff()
    }

    /// Returns the identity of the retained concrete canonical handoff.
    pub fn canonical_handoff_identity(&self) -> HandoffIdentityV2 {
        self.export.graph_handoff_identity()
    }

    /// Returns the identity binding the retained graph export and envelope.
    pub const fn graph_export_identity(&self) -> GraphExportIdentityV1 {
        self.export.identity()
    }

    /// Returns the canonical graph-derived lowering receipt.
    pub const fn graph_receipt(&self) -> &CanonicalLoweringReceiptV1 {
        self.export.graph_receipt()
    }

    /// Returns graph facts recovered by the retained traversal.
    pub const fn graph_inspection(&self) -> LiveGraphInspectionV1 {
        self.export.graph_inspection()
    }

    /// Freshly traverses the exact process-local owner and compares the complete export.
    pub fn revalidate_against(
        &self,
        owner: &LoweredAmdgcnPlironLlvmV1,
    ) -> Result<(), LiveGraphSerializationErrorV1> {
        if owner.context_identity() != self.owner {
            return Err(LiveGraphSerializationErrorV1::RetainedGraphOwnerMismatch);
        }
        let fresh = crate::lower::export_graph(
            owner,
            GraphExportRequestV1::new(
                owner.receipt().identity(),
                owner.non_graph_envelope().identity(),
            ),
        )
        .map_err(LiveGraphSerializationErrorV1::Graph)?;
        if fresh != self.export {
            return Err(LiveGraphSerializationErrorV1::RetainedGraphExportMismatch);
        }
        Ok(())
    }

    /// This retained compiler export grants no artifact or runtime authority.
    pub const fn grants_artifact_authority(&self) -> bool {
        false
    }
}

/// Inert output of one owner-borrowing graph serialization and worker admission.
///
/// This value is deliberately not cloneable. Consuming it separates the exact
/// assembly, worker admission, retained concrete graph export, and serialization receipt.
#[derive(Debug)]
pub struct AdmittedLiveGraphSerializationV1 {
    graph_export: RetainedLiveGraphExportV1,
    receipt: LiveGraphSerializationReceiptV1,
    assembly: Gfx942LlvmAssemblyV2,
    worker_admission: AdmittedWorkerRequestV2,
}

impl AdmittedLiveGraphSerializationV1 {
    /// Returns evidence binding this output to fresh traversal of the live graph.
    pub const fn receipt(&self) -> LiveGraphSerializationReceiptV1 {
        self.receipt
    }

    /// Returns the retained concrete graph-derived handoff owner.
    pub const fn retained_graph_export(&self) -> &RetainedLiveGraphExportV1 {
        &self.graph_export
    }

    /// Returns the exact graph-derived LLVM assembly.
    pub const fn assembly(&self) -> &Gfx942LlvmAssemblyV2 {
        &self.assembly
    }

    /// Returns the exact graph-derived worker admission.
    pub const fn worker_admission(&self) -> &AdmittedWorkerRequestV2 {
        &self.worker_admission
    }

    /// Consumes this inert boundary result into its exact components.
    pub fn into_parts(
        self,
    ) -> (
        LiveGraphSerializationReceiptV1,
        Gfx942LlvmAssemblyV2,
        AdmittedWorkerRequestV2,
    ) {
        (self.receipt, self.assembly, self.worker_admission)
    }

    /// Consumes this result without discarding its concrete graph-derived handoff.
    pub fn into_retained_parts(
        self,
    ) -> (
        RetainedLiveGraphExportV1,
        LiveGraphSerializationReceiptV1,
        Gfx942LlvmAssemblyV2,
        AdmittedWorkerRequestV2,
    ) {
        (
            self.graph_export,
            self.receipt,
            self.assembly,
            self.worker_admission,
        )
    }
}

/// Non-cloneable capability borrowing the exact live graph owner through serialization.
///
/// The capability cannot outlive or be detached from its owner:
///
/// ```compile_fail
/// use fe2o3_llvm_worker_handoff::MeasuredLlvmLldBuildV1;
/// use fe2o3_lower_amdgcn_llvm::{
///     LiveGraphSerializationRequestV1, LoweredAmdgcnPlironLlvmV1,
/// };
///
/// fn cannot_drop_owner(owner: LoweredAmdgcnPlironLlvmV1) {
///     let request = LiveGraphSerializationRequestV1::new(
///         owner.receipt().identity(),
///         owner.non_graph_envelope().identity(),
///     );
///     let token = owner.acquire_worker_serialization_v1(
///         request,
///         MeasuredLlvmLldBuildV1::exact(),
///     );
///     drop(owner);
///     let _ = token.serialize_and_admit_v1();
/// }
/// ```
///
/// A detached Handoff cannot acquire this capability:
///
/// ```compile_fail
/// use fe2o3_llvm_handoff::Gfx942HandoffV2;
/// use fe2o3_llvm_worker_handoff::MeasuredLlvmLldBuildV1;
/// use fe2o3_lower_amdgcn_llvm::LiveGraphSerializationRequestV1;
///
/// fn detached_cannot_serialize(
///     handoff: Gfx942HandoffV2,
///     request: LiveGraphSerializationRequestV1,
/// ) {
///     let _ = handoff.acquire_worker_serialization_v1(
///         request,
///         MeasuredLlvmLldBuildV1::exact(),
///     );
/// }
/// ```
pub struct LiveGraphSerializationTokenV1<'owner, 'build> {
    owner: &'owner LoweredAmdgcnPlironLlvmV1,
    request: LiveGraphSerializationRequestV1,
    measured_build: MeasuredLlvmLldBuildV1<'build>,
}

impl LoweredAmdgcnPlironLlvmV1 {
    /// Borrows this owner through the exact fresh serialization/admission call.
    pub const fn acquire_worker_serialization_v1<'owner, 'build>(
        &'owner self,
        request: LiveGraphSerializationRequestV1,
        measured_build: MeasuredLlvmLldBuildV1<'build>,
    ) -> LiveGraphSerializationTokenV1<'owner, 'build> {
        LiveGraphSerializationTokenV1 {
            owner: self,
            request,
            measured_build,
        }
    }
}

impl LiveGraphSerializationTokenV1<'_, '_> {
    /// Freshly traverses the borrowed graph, serializes it, and admits exact worker input.
    pub fn serialize_and_admit_v1(
        self,
    ) -> Result<AdmittedLiveGraphSerializationV1, LiveGraphSerializationErrorV1> {
        let export = crate::lower::export_graph(
            self.owner,
            GraphExportRequestV1::new(
                self.request.receipt_identity,
                self.request.non_graph_envelope_identity,
            ),
        )
        .map_err(LiveGraphSerializationErrorV1::Graph)?;
        let assembly = serialize_gfx942_handoff_v2(export.graph_handoff())
            .map_err(LiveGraphSerializationErrorV1::Assembly)?;
        let canonical_handoff = export.graph_handoff().encode_canonical();
        let worker_admission = WorkerAdmissionRequestV2::new(
            canonical_handoff.as_bytes(),
            *export.graph_handoff_identity().as_bytes(),
            self.measured_build,
        )
        .admit()
        .map_err(LiveGraphSerializationErrorV1::Worker)?;

        let graph_export_identity = export.identity();
        let graph_handoff_identity = export.graph_handoff_identity();
        let graph_receipt_identity = export.graph_receipt().identity();
        let non_graph_envelope_identity = export.non_graph_envelope_identity();
        let graph_inspection = export.graph_inspection();
        let assembly_sha256 = assembly.sha256();
        let worker_admission_identity = worker_admission.admission_identity();
        let identity = LiveGraphSerializationIdentityV1(
            Sha256::new()
                .chain_update(SERIALIZATION_IDENTITY_DOMAIN_V1)
                .chain_update(graph_export_identity.as_bytes())
                .chain_update(graph_handoff_identity.as_bytes())
                .chain_update(graph_receipt_identity.as_bytes())
                .chain_update(non_graph_envelope_identity.as_bytes())
                .chain_update(graph_inspection.graph_sha256())
                .chain_update(assembly_sha256.as_bytes())
                .chain_update(worker_admission_identity.as_bytes())
                .finalize()
                .into(),
        );
        Ok(AdmittedLiveGraphSerializationV1 {
            graph_export: RetainedLiveGraphExportV1 {
                owner: self.owner.context_identity(),
                export,
            },
            receipt: LiveGraphSerializationReceiptV1 {
                graph_export_identity,
                graph_handoff_identity,
                graph_receipt_identity,
                non_graph_envelope_identity,
                graph_inspection,
                assembly_sha256,
                worker_admission_identity,
                identity,
            },
            assembly,
            worker_admission,
        })
    }
}
