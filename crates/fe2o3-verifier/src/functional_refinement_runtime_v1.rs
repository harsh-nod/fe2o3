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

/// Private execution profiles selected by the verifier-owned proof boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GeneratedVerusExecutionProfileV1 {
    Ranked,
    ClosedFill,
}

impl GeneratedVerusExecutionProfileV1 {
    pub(crate) const fn solver_processes(self) -> usize {
        match self {
            Self::Ranked => 1,
            Self::ClosedFill => 12,
        }
    }

    pub(crate) const fn verifier_configuration(self) -> &'static [u8] {
        match self {
            Self::Ranked => b"sealed-generated-source-fd;fixed-env",
            Self::ClosedFill => {
                b"sealed-generated-source-fd;fixed-env;closed-fill-v1;no-bv-simplify"
            }
        }
    }

    pub(crate) const fn solver_configuration(self) -> &'static [u8] {
        match self {
            Self::Ranked => b"rust_verify-managed-z3;fixed-env",
            Self::ClosedFill => {
                b"rust_verify-managed-z3;fixed-env;closed-fill-v1;exact-processes=12"
            }
        }
    }
}

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
    backend: RuntimeBackend,
}

enum RuntimeBackend {
    Local(RetainedGeneratedVerusRuntimeBackendV1),
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    Brokered {
        local: RetainedGeneratedVerusRuntimeBackendV1,
        session: Box<crate::compiler_proof_broker_v1::AuthenticatedCompilerProofSessionV1>,
    },
}

impl RuntimeBackend {
    fn local(&self) -> &RetainedGeneratedVerusRuntimeBackendV1 {
        match self {
            Self::Local(local) => local,
            #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
            Self::Brokered { local, .. } => local,
        }
    }
}

impl fmt::Debug for FunctionalRefinementVerusRuntimeLeaseV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FunctionalRefinementVerusRuntimeLeaseV1")
            .field("root", &self.backend.local().root())
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
            backend: RuntimeBackend::Local(backend),
        })
    }

    /// Returns the diagnostic path supplied when this lease was opened.
    pub fn root(&self) -> &Path {
        self.backend.local().root()
    }

    /// Returns the exact workload-neutral runtime identity.
    pub const fn identity(&self) -> FunctionalRefinementVerusRuntimeIdentityV1 {
        self.identity
    }

    /// Revalidates the retained runtime objects and path edges.
    pub fn revalidate(&self) -> Result<(), FunctionalRefinementRuntimeErrorV1> {
        self.revalidate_until(Instant::now() + std::time::Duration::from_secs(30))
    }

    pub(crate) fn revalidate_until(
        &self,
        deadline: Instant,
    ) -> Result<(), FunctionalRefinementRuntimeErrorV1> {
        if let Err(error) = self.backend.local().revalidate() {
            #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
            if let RuntimeBackend::Brokered { session, .. } = &self.backend {
                session.poison();
            }
            return Err(runtime_error_from_backend(error));
        }
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        if let RuntimeBackend::Brokered { session, .. } = &self.backend {
            session
                .revalidate_until(deadline)
                .map_err(runtime_error_from_broker)?;
        }
        #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
        let _ = deadline;
        Ok(())
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    pub(crate) fn with_compiler_broker(
        self,
        session: crate::compiler_proof_broker_v1::AuthenticatedCompilerProofSessionV1,
        deadline: Instant,
    ) -> Result<Self, FunctionalRefinementRuntimeErrorV1> {
        let RuntimeBackend::Local(local) = self.backend else {
            return Err(runtime_error_from_broker(std::io::Error::other(
                "compiler-proof runtime is already brokered",
            )));
        };
        let value = Self {
            identity: self.identity,
            backend: RuntimeBackend::Brokered {
                local,
                session: Box::new(session),
            },
        };
        value.revalidate_until(deadline)?;
        Ok(value)
    }

    pub(crate) fn execute_generated_rust_verify(
        &self,
        source: &CanonicalGeneratedVerusProofInputV3,
        deadline: Instant,
        output_limit: usize,
        profile: GeneratedVerusExecutionProfileV1,
    ) -> Result<FunctionalRefinementRuntimeProcessOutputV1, FunctionalRefinementRuntimeErrorV1>
    {
        match &self.backend {
            RuntimeBackend::Local(local) => local
                .execute_generated_rust_verify(source, deadline, output_limit, profile)
                .map(FunctionalRefinementRuntimeProcessOutputV1::from)
                .map_err(runtime_error_from_backend),
            #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
            RuntimeBackend::Brokered { local, session } => {
                if let Err(error) = local.revalidate() {
                    session.poison();
                    return Err(runtime_error_from_backend(error));
                }
                let output = session.execute(source, deadline, output_limit, profile);
                if let Err(error) = local.revalidate() {
                    session.poison();
                    return Err(runtime_error_from_backend(error));
                }
                output.map_err(runtime_error_from_broker)
            }
        }
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

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn runtime_error_from_broker(error: std::io::Error) -> FunctionalRefinementRuntimeErrorV1 {
    FunctionalRefinementRuntimeErrorV1 {
        detail: format!("authenticated compiler-proof broker failed: {error}"),
    }
}

fn runtime_error_from_backend(
    error: RetainedFunctionalRefinementRuntimeErrorV1,
) -> FunctionalRefinementRuntimeErrorV1 {
    #[cfg(test)]
    eprintln!("retained runtime diagnostic: {error}");
    FunctionalRefinementRuntimeErrorV1 {
        detail: format!(
            "retained generated-proof runtime failed: {:?}",
            error.kind()
        ),
    }
}
