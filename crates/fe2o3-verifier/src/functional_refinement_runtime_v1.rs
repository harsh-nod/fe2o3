//! Workload-neutral retained runtime for compiler-generated functional-refinement proofs.
//!
//! The public identity covers only the pinned verifier, solver, Rust toolchain, and runtime
//! dependencies. Reviewed workload proof sources are deliberately excluded: generated proofs
//! receive their source through a sealed descriptor and cannot inherit the retained proof tree.

use std::{error::Error, fmt, path::Path, time::Instant};

use crate::CanonicalGeneratedVerusProofInputV3;
use crate::retained_functional_refinement_runtime_v1::{
    RetainedFunctionalRefinementRuntimeErrorV1, RetainedFunctionalRefinementRuntimeOutputV1,
    RetainedGeneratedVerusRuntimeBackendV1, open_retained_generated_verus_runtime_v1,
};

/// Domain-separated identity of the exact workload-neutral verifier runtime.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct FunctionalRefinementVerusRuntimeIdentityV1([u8; 32]);

impl FunctionalRefinementVerusRuntimeIdentityV1 {
    /// Returns the exact identity bytes.
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Non-copyable lease over the retained workload-neutral generated-proof runtime.
///
/// Opening or revalidating the runtime does not establish a proof or grant compiler authority.
pub struct FunctionalRefinementVerusRuntimeLeaseV1 {
    identity: FunctionalRefinementVerusRuntimeIdentityV1,
    backend: RetainedGeneratedVerusRuntimeBackendV1,
}

impl fmt::Debug for FunctionalRefinementVerusRuntimeLeaseV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FunctionalRefinementVerusRuntimeLeaseV1")
            .field("root", &self.backend.root())
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

impl FunctionalRefinementVerusRuntimeLeaseV1 {
    /// Opens and retains the exact no-follow runtime closure.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, FunctionalRefinementRuntimeErrorV1> {
        let root = root.as_ref();
        let backend =
            open_retained_generated_verus_runtime_v1(root).map_err(runtime_error_from_backend)?;
        Ok(Self {
            identity: FunctionalRefinementVerusRuntimeIdentityV1(backend.identity()),
            backend,
        })
    }

    /// Returns the diagnostic path supplied when this lease was opened.
    pub fn root(&self) -> &Path {
        self.backend.root()
    }

    /// Returns the exact workload-neutral runtime identity.
    pub const fn identity(&self) -> FunctionalRefinementVerusRuntimeIdentityV1 {
        self.identity
    }

    /// Revalidates the retained runtime objects and path edges.
    pub fn revalidate(&self) -> Result<(), FunctionalRefinementRuntimeErrorV1> {
        self.backend
            .revalidate()
            .map_err(runtime_error_from_backend)
    }

    /// Executes one already-canonical generated proof through the retained runtime.
    ///
    /// The bounded process output is observational evidence only and grants no
    /// compiler, artifact, publication, load, or launch authority.
    pub fn execute_generated_rust_verify(
        &self,
        source: &CanonicalGeneratedVerusProofInputV3,
        deadline: Instant,
        output_limit: usize,
    ) -> Result<FunctionalRefinementRuntimeProcessOutputV1, FunctionalRefinementRuntimeErrorV1>
    {
        self.backend
            .execute_generated_rust_verify(source, deadline, output_limit)
            .map(FunctionalRefinementRuntimeProcessOutputV1::from)
            .map_err(runtime_error_from_backend)
    }
}

/// Bounded output from one retained generated-proof process.
pub struct FunctionalRefinementRuntimeProcessOutputV1 {
    exit_code: Option<i32>,
    signal: Option<i32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl FunctionalRefinementRuntimeProcessOutputV1 {
    /// Constructs an authority-free observed process result.
    pub fn from_observed_process(
        exit_code: Option<i32>,
        signal: Option<i32>,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    ) -> Self {
        Self {
            exit_code,
            signal,
            stdout,
            stderr,
        }
    }

    /// Returns the process exit code when it exited normally.
    pub const fn exit_code(&self) -> Option<i32> {
        self.exit_code
    }

    /// Returns the terminating signal when the process was signalled.
    pub const fn signal(&self) -> Option<i32> {
        self.signal
    }

    /// Returns bounded standard output bytes.
    pub fn stdout(&self) -> &[u8] {
        &self.stdout
    }

    /// Returns bounded standard error bytes.
    pub fn stderr(&self) -> &[u8] {
        &self.stderr
    }
}

impl From<RetainedFunctionalRefinementRuntimeOutputV1>
    for FunctionalRefinementRuntimeProcessOutputV1
{
    fn from(output: RetainedFunctionalRefinementRuntimeOutputV1) -> Self {
        Self {
            exit_code: output.exit_code,
            signal: output.signal,
            stdout: output.stdout,
            stderr: output.stderr,
        }
    }
}

/// Runtime admission, revalidation, or execution failure.
#[derive(Debug)]
pub struct FunctionalRefinementRuntimeErrorV1 {
    detail: String,
}

impl fmt::Display for FunctionalRefinementRuntimeErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.detail)
    }
}

impl Error for FunctionalRefinementRuntimeErrorV1 {}

fn runtime_error_from_backend(
    error: RetainedFunctionalRefinementRuntimeErrorV1,
) -> FunctionalRefinementRuntimeErrorV1 {
    FunctionalRefinementRuntimeErrorV1 {
        detail: format!(
            "retained generated-proof runtime failed: {:?}",
            error.kind()
        ),
    }
}
