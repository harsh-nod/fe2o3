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

    pub(crate) fn execute_generated_rust_verify(
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
pub(crate) struct FunctionalRefinementRuntimeProcessOutputV1 {
    pub(crate) exit_code: Option<i32>,
    pub(crate) signal: Option<i32>,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
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

#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
mod tests {
    use std::time::Duration;

    use super::*;

    const PROTECTED_RUNTIME_ROOT: &str =
        "/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5";

    fn execute_protected_proof(assertion: &str) -> FunctionalRefinementRuntimeProcessOutputV1 {
        let runtime = FunctionalRefinementVerusRuntimeLeaseV1::open(PROTECTED_RUNTIME_ROOT)
            .expect("the public lease must admit the installed protected runtime");
        runtime
            .revalidate()
            .expect("revalidate before proof execution");
        let source = CanonicalGeneratedVerusProofInputV3::new(
            format!(
                "use vstd::prelude::*;\nverus! {{\n    pub proof fn protected_runtime_sample(value: int) {{\n        assert({assertion});\n    }}\n}}\n"
            )
            .into_bytes(),
        )
        .expect("admit canonical generated proof source");
        let output = runtime
            .execute_generated_rust_verify(
                &source,
                Instant::now() + Duration::from_secs(120),
                64 * 1024,
            )
            .expect("execute the proof through the retained sealed-source path");
        runtime
            .revalidate()
            .expect("revalidate after proof execution");
        output
    }

    #[test]
    #[ignore = "requires the installed root-owned pinned functional-refinement runtime"]
    fn protected_public_lease_executes_real_verus() {
        let output = execute_protected_proof("value + 1 > value");
        assert_eq!((output.exit_code, output.signal), (Some(0), None));
        assert_eq!(
            output.stdout.as_slice(),
            b"verification results:: 1 verified, 0 errors\n",
        );
        assert!(
            output.stderr.is_empty(),
            "unexpected Verus stderr: {}",
            String::from_utf8_lossy(&output.stderr),
        );
    }

    #[test]
    #[ignore = "requires the installed root-owned pinned functional-refinement runtime"]
    fn protected_public_lease_rejects_false_proof() {
        let output = execute_protected_proof("value + 1 > value + 1");
        assert_eq!((output.exit_code, output.signal), (Some(1), None));
        assert_eq!(
            output.stdout.as_slice(),
            b"verification results:: 0 verified, 1 errors\n",
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("assertion failed"),
            "expected a Verus assertion failure, got: {}",
            String::from_utf8_lossy(&output.stderr),
        );
    }
}
