//! Workload-neutral retained runtime for compiler-generated functional-refinement proofs.
//!
//! The public identity covers only the pinned verifier, solver, Rust toolchain, and runtime
//! dependencies. Reviewed workload proof sources are deliberately excluded: generated proofs
//! receive their source through a sealed descriptor and cannot inherit the retained proof tree.

use std::{error::Error, fmt, path::Path, time::Instant};

use crate::CanonicalGeneratedVerusProofInputV3;
use crate::retained_functional_refinement_runtime_v1::{
    GeneratedProofProcessPolicyV2, RetainedFunctionalRefinementRuntimeErrorV1,
    RetainedFunctionalRefinementRuntimeOutputV1, RetainedGeneratedVerusRuntimeBackendV1,
    RuntimeAttemptV1, open_retained_generated_verus_context_runtime_v2,
    open_retained_generated_verus_context_runtime_v3, open_retained_generated_verus_runtime_v1,
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

pub(crate) struct FunctionalRefinementAttemptV1(RuntimeAttemptV1);

impl FunctionalRefinementAttemptV1 {
    #[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
    pub(crate) fn before_publication(&self) {
        self.0.before_publication();
    }
    pub(crate) fn complete(&self) -> Result<(), FunctionalRefinementRuntimeErrorV1> {
        self.0.complete().map_err(runtime_error_from_backend)
    }
}

impl fmt::Debug for FunctionalRefinementVerusRuntimeLeaseV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FunctionalRefinementVerusRuntimeLeaseV1")
            .field("root", &self.backend.root())
            .field("identity", &self.identity)
            .field("process_policy", &self.process_policy())
            .finish_non_exhaustive()
    }
}

impl FunctionalRefinementVerusRuntimeLeaseV1 {
    pub(crate) fn begin_attempt(
        &self,
    ) -> Result<FunctionalRefinementAttemptV1, FunctionalRefinementRuntimeErrorV1> {
        self.backend
            .begin_attempt()
            .map(FunctionalRefinementAttemptV1)
            .map_err(runtime_error_from_backend)
    }
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

    /// Opens the same exact pinned tools under the explicit bounded V2 context policy.
    ///
    /// This permits at most 4,096 authenticated solver lifetimes, with at most two
    /// live solver groups for the fixed single-threaded Verus invocation. It does
    /// not grant proof authority, change an accepted receipt policy, or fall back
    /// to a different process policy. Legacy `open` retains its original behavior.
    pub fn open_pinned_contexts_v2(
        root: impl AsRef<Path>,
    ) -> Result<Self, FunctionalRefinementRuntimeErrorV1> {
        let backend = open_retained_generated_verus_context_runtime_v2(root.as_ref())
            .map_err(runtime_error_from_backend)?;
        Ok(Self {
            identity: FunctionalRefinementVerusRuntimeIdentityV1(backend.identity()),
            backend,
        })
    }

    /// Opens the exact pinned tools under the explicit V3 context/stack policy.
    ///
    /// V3 retains the V2 solver census and permits the pinned interpreter's at most
    /// 1 GiB clone3 stack only for authenticated verifier threads. Other thread and
    /// process stacks retain the 32 MiB limit. No environment or request selects
    /// these bounds, and this resource policy grants no proof or signer authority.
    pub fn open_pinned_contexts_v3(
        root: impl AsRef<Path>,
    ) -> Result<Self, FunctionalRefinementRuntimeErrorV1> {
        let backend = open_retained_generated_verus_context_runtime_v3(root.as_ref())
            .map_err(runtime_error_from_backend)?;
        Ok(Self {
            identity: FunctionalRefinementVerusRuntimeIdentityV1(backend.identity()),
            backend,
        })
    }

    pub(crate) const fn process_policy(&self) -> GeneratedProofProcessPolicyV2 {
        self.backend.process_policy()
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
        attempt: &mut FunctionalRefinementAttemptV1,
        source: &CanonicalGeneratedVerusProofInputV3,
        deadline: Instant,
        output_limit: usize,
    ) -> Result<FunctionalRefinementRuntimeProcessOutputV1, FunctionalRefinementRuntimeErrorV1>
    {
        let output = self
            .backend
            .execute_generated_rust_verify(&mut attempt.0, source, deadline, output_limit)
            .map_err(runtime_error_from_backend)?;
        check_output_policy(self.process_policy(), output.policy)?;
        Ok(FunctionalRefinementRuntimeProcessOutputV1::from(output))
    }
}

fn check_output_policy(
    expected: GeneratedProofProcessPolicyV2,
    actual: GeneratedProofProcessPolicyV2,
) -> Result<(), FunctionalRefinementRuntimeErrorV1> {
    if expected != actual {
        return Err(runtime_error_from_backend(RetainedFunctionalRefinementRuntimeErrorV1::new(
            crate::retained_functional_refinement_runtime_v1::RetainedFunctionalRefinementRuntimeErrorKindV1::Process,
            "generated proof output process policy differs from its immutable runtime lease",
        )));
    }
    Ok(())
}

/// Bounded output from one retained generated-proof process.
pub(crate) struct FunctionalRefinementRuntimeProcessOutputV1 {
    pub(crate) policy: GeneratedProofProcessPolicyV2,
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
            policy: output.policy,
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

const MAX_BACKEND_DIAGNOSTIC_BYTES: usize = 2048;

fn runtime_error_from_backend(
    error: RetainedFunctionalRefinementRuntimeErrorV1,
) -> FunctionalRefinementRuntimeErrorV1 {
    let mut detail = format!(
        "retained generated-proof runtime failed: {:?}: \"",
        error.kind()
    );
    let cause_start = detail.len();
    let mut truncated = false;
    // Bound both scanning and escaped output without copying the full backend cause.
    for character in error.detail().chars() {
        let escaped = character.escape_default();
        if detail.len() - cause_start + escaped.clone().count() > MAX_BACKEND_DIAGNOSTIC_BYTES {
            truncated = true;
            break;
        }
        detail.extend(escaped);
    }
    detail.push('"');
    if truncated {
        detail.push_str(" (truncated)");
    }
    FunctionalRefinementRuntimeErrorV1 { detail }
}

#[cfg(test)]
#[path = "functional_refinement_runtime_v1_diagnostic_tests.rs"]
mod diagnostic_tests;

#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
mod tests {
    use std::time::Duration;

    use super::*;

    const PROTECTED_RUNTIME_ROOT: &str =
        "/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5";

    #[test]
    fn immutable_runtime_output_policy_rejects_cross_policy_substitution() {
        use GeneratedProofProcessPolicyV2::{
            LegacySingleSolverV1, PinnedSingleThreadContextsV2, PinnedSingleThreadContextsV3,
        };
        for expected in [
            LegacySingleSolverV1,
            PinnedSingleThreadContextsV2,
            PinnedSingleThreadContextsV3,
        ] {
            for actual in [
                LegacySingleSolverV1,
                PinnedSingleThreadContextsV2,
                PinnedSingleThreadContextsV3,
            ] {
                assert_eq!(
                    check_output_policy(expected, actual).is_ok(),
                    expected == actual
                );
            }
        }
    }

    #[test]
    #[ignore = "requires the installed root-owned pinned functional-refinement runtime"]
    fn protected_pinned_context_policy_accepts_stock_modules_and_spinoffs() {
        let runtime = FunctionalRefinementVerusRuntimeLeaseV1::open_pinned_contexts_v3(
            PROTECTED_RUNTIME_ROOT,
        )
        .expect("admit exact pinned tools under explicit context policy");
        let source = CanonicalGeneratedVerusProofInputV3::new(
            br#"use vstd::prelude::*;
verus! {
    pub proof fn ordinary(value: int) { assert(value + 1 > value); }
    pub proof fn spinoff(value: u32) { assert((value ^ value) == 0u32) by(bit_vector); }
}
mod separate_bucket {
    use vstd::prelude::*;
    verus! { pub proof fn ordinary(value: int) { assert(value + 2 > value); } }
}
"#
            .to_vec(),
        )
        .unwrap();
        let mut attempt = runtime.begin_attempt().unwrap();
        let output = runtime
            .execute_generated_rust_verify(
                &mut attempt,
                &source,
                Instant::now() + Duration::from_secs(120),
                16 * 1024,
            )
            .expect("stock modules and bit-vector spinoff retain every solver terminal");
        assert_eq!(
            output.policy,
            GeneratedProofProcessPolicyV2::PinnedSingleThreadContextsV3
        );
        crate::functional_refinement_receipt_v2::validate_proved_output(&output).unwrap();
        let (born, executed, completed, peak) =
            crate::retained_functional_refinement_runtime_v1::finished_solver_contexts_for_test()
                .expect("this attempt finished an authenticated context census");
        assert!(
            born >= 3,
            "root, child module and bit-vector contexts: {born}"
        );
        assert_eq!((executed, completed), (born, born));
        assert_eq!(peak, 2);
        runtime.revalidate().unwrap();
        attempt.complete().unwrap();
    }

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
        let mut attempt = runtime.begin_attempt().expect("acquire diagnostic attempt");
        let output = runtime
            .execute_generated_rust_verify(
                &mut attempt,
                &source,
                Instant::now() + Duration::from_secs(120),
                64 * 1024,
            )
            .expect("execute the proof through the retained sealed-source path");
        runtime
            .revalidate()
            .expect("revalidate after proof execution");
        attempt.complete().expect("complete diagnostic attempt");
        output
    }

    #[test]
    #[ignore = "requires the installed root-owned pinned functional-refinement runtime"]
    fn protected_public_lease_audits_installed_closure() {
        let runtime = FunctionalRefinementVerusRuntimeLeaseV1::open(PROTECTED_RUNTIME_ROOT)
            .expect("the public lease must admit the installed protected runtime");
        runtime
            .revalidate()
            .expect("revalidate the installed closure");
        eprintln!(
            "protected runtime audit identity={:?}",
            runtime.identity().as_bytes()
        );
        // Deliberately no execute call: an installed-closure audit is not a proof.
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
