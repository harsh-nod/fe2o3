//! Native readiness binds the actual deployment family and expected child together.
macro_rules! supervisor_ready {
    ($schema:ident, $version:literal, $other:literal,
        $bytes:ident, $work:ident, $scratch:ident, $Owner:ident, $Identity:ident, $Failure:ident) => {
        pub const $bytes: usize = codec::BYTES;
        /// Fixed logical entry, validation, hashing and comparison quota, not elapsed time.
        pub const $work: usize = resources::ENTRY_WORK + 32 * codec::BYTES;
        const RETAINED: usize = size_of::<($Owner, Storage)>();
        /// Additional logical staging above prepaid inputs, not generated stack or RSS.
        pub const $scratch: usize = 4 * RETAINED + 4 * codec::BYTES + 2 * size_of::<sha2::Sha256>() + 4096;

        /// Inert family-specific readiness digest; not private-channel provenance.
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct $Identity([u8; 32]);
        impl $Identity {
            pub const fn as_bytes(&self) -> &[u8; 32] { &self.0 }
        }

        /// Move-only native bootstrap record for one exact supervisor and deployment.
        ///
        /// Decoding checks framing and the supplied context together, not merely an
        /// embedded digest. The trusted parent must still establish independently
        /// pinned deployment provenance, private bootstrap custody and pidfd liveness.
        /// Construction and decoding grant no process, signing or launch authority.
        /// Inputs remain prepaid on the original budget; reserve the returned FULL
        /// owner charge before retention, and retire it only after owner Drop.
        ///
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::CompilerExecutionSupervisorReadyV", $version, " as Ready;")]
        /// fn clone<T: Clone>() {}
        /// clone::<Ready>();
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::{CompilerExecutionSupervisorReadyV", $version, " as Ready, CompilerExecutionSupervisorReadyV", $other, " as Other};")]
        /// fn mix(ready: Ready) -> Other { ready.into() }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::{CompilerExecutionSupervisorReadyV", $version, " as Ready, CompilerExecutionSupervisorDeploymentV", $other, " as Deployment};")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn mix(d: &Deployment, b: &mut Budget<'_>) { let _ = Ready::new(42, d, b); }
        /// ```
        #[derive(Debug, Eq, PartialEq)]
        pub struct $Owner {
            pid: u32,
            deployment: DeploymentIdentity,
            bytes: [u8; codec::BYTES],
        }
        impl $Owner {
            pub fn new(pid: u32, deployment: &Deployment, budget: &mut Budget<'_>)
                -> Result<(Self, Storage)> {
                resources::fixed(budget, deployment.retained_storage(), $work, $scratch, || {
                    Ok((Self { pid, deployment: deployment.identity(),
                        bytes: codec::$schema.encode(pid, deployment.identity().as_bytes())? }, Storage(RETAINED)))
                })
            }

            /// Requires the exact expected child PID and actual same-family deployment.
            /// Restores entry storage and preserves all accepted work and first denials.
            pub fn decode(bytes: &[u8], expected_pid: u32, deployment: &Deployment,
                budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
                let floor = deployment.retained_storage()
                    .checked_add(resources::fixed_input_floor(bytes, codec::BYTES))
                    .ok_or(Resource::Arithmetic)?;
                resources::fixed(budget, floor, $work, $scratch, || {
                    let (pid, identity) = codec::$schema.decode(bytes)?;
                    if pid != expected_pid || identity != *deployment.identity().as_bytes() {
                        return Err($Failure::ContextMismatch);
                    }
                    Ok((Self { pid, deployment: deployment.identity(),
                        bytes: bytes.try_into().expect("validated fixed wire") }, Storage(RETAINED)))
                })
            }

            pub fn matches_deployment(&self, pid: u32, deployment: &Deployment,
                budget: &mut Budget<'_>) -> Result<bool> {
                let floor = RETAINED.checked_add(deployment.retained_storage())
                    .ok_or(Resource::Arithmetic)?;
                resources::fixed(budget, floor, $work, $scratch, || {
                    Ok(self.pid == pid && self.deployment == deployment.identity())
                })
            }
            pub const fn supervisor_pid(&self) -> u32 { self.pid }
            pub const fn deployment_identity(&self) -> DeploymentIdentity { self.deployment }
            pub fn identity(&self) -> $Identity {
                $Identity(self.bytes[56..].try_into().expect("fixed terminal digest"))
            }
            pub const fn canonical_bytes(&self) -> &[u8; codec::BYTES] { &self.bytes }
            pub const fn retained_storage(&self) -> usize { RETAINED }
        }

        #[derive(Debug)]
        pub enum $Failure {
            Framing(Framing),
            /// Exact PID/deployment comparison failed; no bootstrap fact is admitted.
            ContextMismatch,
            Resource(Resource),
        }
        type Result<T> = std::result::Result<T, $Failure>;
        impl From<Framing> for $Failure {
            fn from(e: Framing) -> Self { Self::Framing(e) }
        }
        impl From<Resource> for $Failure {
            fn from(e: Resource) -> Self { Self::Resource(e) }
        }
        impl fmt::Display for $Failure {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    Self::Framing(e) => e.fmt(f), Self::Resource(e) => e.fmt(f),
                    Self::ContextMismatch => f.write_str("native supervisor readiness context mismatch"),
                }
            }
        }
        impl Error for $Failure {
            fn source(&self) -> Option<&(dyn Error + 'static)> {
                match self { Self::Framing(e) => Some(e), Self::Resource(e) => Some(e),
                    Self::ContextMismatch => None }
            }
        }
        const _: () = {
            assert!(codec::BYTES == 88);
            assert!(8 * size_of::<$Failure>() + 64 * size_of::<usize>()
                + size_of::<Budget<'static>>() + 4 * size_of::<Storage>() <= 4096);
        };
    };
}
pub(crate) use supervisor_ready;
