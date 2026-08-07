//! Deterministic differential-testing infrastructure for scalar GPU kernels.
//!
//! The crate generates and evaluates a deliberately small, bounded expression
//! language. It does not compile or execute GPU code and its results are not
//! parity, correctness, or safety evidence.

mod codec;
mod conformance;
mod eval;
mod generate;
mod model;
mod reduce;

pub use codec::{CodecError, MAX_CANONICAL_BYTES, decode_case_v1, encode_case_v1};
pub use conformance::{
    AccessKind, AtomicOperation, AtomicScope, AtomicSpec, BackendOutcome, CompileRejection,
    ConformanceOutcome, CopyNonoverlappingSpec, HardwareUnavailable, IntegerSwitchSpec, LayoutSpec,
    MAX_ATOMIC_STEPS, MAX_LAYOUT_MEMBERS, MAX_OBLIGATION_ACCESSES, MAX_SEMANTIC_WORDS,
    MAX_SWITCH_ARMS, MemoryAccess, MemoryOrdering, ObligationSpec, PointerDistanceSpec,
    ReferenceOutcome, ScalarLayout, SemanticCase, SemanticFeature, SemanticMismatch,
    SemanticModelError, SemanticObservation, SemanticSpec, VolatileOperation, VolatileSpec,
    classify_semantic_outcome, evaluate_semantic_case,
};
pub use eval::{
    LaneMismatch, MAX_REPORTED_MISMATCHES, MismatchReport, compare_outputs, evaluate_case,
    evaluate_lane,
};
pub use generate::{GenerateConfig, GenerateError, generate_case};
pub use model::{
    BinaryOp, Expr, KernelCase, MAX_EXPR_DEPTH, MAX_EXPR_NODES, MAX_INPUTS, MAX_WORK_ITEMS,
    ModelError, Program, UnaryOp,
};
pub use reduce::{
    CaseComplexity, MAX_REDUCTION_ATTEMPTS, ReduceError, ReductionResult, reduce_case,
};
