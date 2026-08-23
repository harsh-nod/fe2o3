//! Owned production implementations of the fe2o3 Worker V3 verifier boundary.
//!
//! The current scalar GEMM implementation authenticates exact request bytes, final HSACO
//! structure, and retained request-bound Verus execution. It remains fail-closed until compiler
//! provenance, source/MIR, IEEE-754, emitted-machine, ABI, and complete effect refinements are
//! mechanically established.

#![forbid(unsafe_code)]

mod scalar_gemm_gfx942_v1;

pub use scalar_gemm_gfx942_v1::{
    PRODUCTION_SCALAR_GEMM_WORKER_V3_OPEN_OBLIGATIONS_V1,
    PreparedProductionScalarGemmWorkerV3ProofV1, ProductionScalarGemmWorkerV3AuditV1,
    ProductionScalarGemmWorkerV3OpenObligationV1, ProductionScalarGemmWorkerV3RequestAuditorV1,
    ProductionScalarGemmWorkerV3VerifierErrorV1, ProductionScalarGemmWorkerV3VerifierV1,
};
