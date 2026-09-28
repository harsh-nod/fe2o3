//! Shared native API; only nominal family names and wire schema vary.
macro_rules! external_anchor_deployment_adapter {
    ($schema:ident, $version:literal, $other:literal,
        $bytes:ident, $work:ident, $storage:ident, $max_executable:ident,
        $Identity:ident, $Owner:ident, $Failure:ident) => {
        /// Exact native wire length, including its terminal SHA-256 identity.
        pub const $bytes: usize = codec::BYTES;
        pub const $max_executable: u64 = codec::MAX_EXECUTABLE_BYTES;
        const RETAINED: usize = size_of::<($Owner, Storage)>();
        const LOCAL_WORK: usize = resources::ENTRY_WORK + 32 * codec::BYTES;
        const LOCAL_STORAGE: usize = 4 * RETAINED
            + 4 * size_of::<codec::Fields>()
            + 4 * codec::BYTES
            + 2 * size_of::<sha2::Sha256>()
            + 4096;
        /// Fixed total logical work, INCLUDING the actual supervisor's metered
        /// policy comparison on the original ledger. Not an instruction/time bound.
        pub const $work: usize = LOCAL_WORK + SUPERVISOR_WORK;
        /// Fixed additional peak above all prepaid live inputs, INCLUDING nested
        /// supervisor comparison staging. Not an allocator, stack or RSS bound.
        pub const $storage: usize = LOCAL_STORAGE + SUPERVISOR_STORAGE;

        /// Copyable nominal content identity, never authenticated provisioning evidence.
        ///
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::{CompilerExecutionExternalAnchorDeploymentIdentityV", $version, " as Identity, CompilerExecutionExternalAnchorDeploymentIdentityV", $other, " as Other};")]
        /// fn mix(value: Identity) -> Other { value }
        /// ```
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $Identity([u8; 32]);
        impl $Identity {
            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }

            /// Canonical identity AND actual same-family context matching. Malformed
            /// bytes or mismatched context return false; resource refusals remain errors.
            /// The complete borrowed input owner, supervisor and policy stay prepaid.
            pub fn matches_canonical_bytes(
                self,
                bytes: &[u8],
                supervisor: &Supervisor,
                policy: &Policy,
                budget: &mut Budget<'_>,
            ) -> Result<bool> {
                let floor = context_floor(supervisor, policy)?
                    .checked_add(resources::fixed_input_floor(bytes, codec::BYTES))
                    .ok_or(Resource::Arithmetic)?;
                metered(budget, floor, supervisor, policy, |policy_matches| {
                    Ok(policy_matches && codec::$schema.decode(bytes).is_ok_and(|record| {
                        record.identity == self.0 && fields_match(&record.fields, supervisor, policy)
                    }))
                })
            }
        }

        /// Move-only, inert configuration for an independently operated external anchor.
        ///
        /// Binds the complete native supervisor identity, exact anchor UID/GID and
        /// policy anchor key, and exact executable SHA-256 and nonzero length <=128 MiB.
        /// The actual same-family supervisor must match the actual native policy.
        /// Key equality to that already validated policy avoids repeated curve work.
        /// Public construction/decoding establish NO trusted provisioning origin,
        /// process custody, signing, compiler, publication, load or launch authority.
        /// Trusted callers must separately pin the configuration and executable.
        ///
        /// All working calls restore entry storage on success, error and unwind while
        /// retaining work, peak and first-denial history on the original Budget.
        /// Keep FULL borrowed owners prepaid, including the enclosing owner of a wire
        /// slice. The checked floor covers supervisor + policy, plus this owner for
        /// matching or the fixed wire for decoding. Wrong lengths require no wire floor.
        /// Returned Storage is the FULL unreserved size_of::<(Self, Storage)> charge;
        /// reserve it before retention and release retained_storage() after owner Drop.
        /// Accessors only return stored values and do not charge the ledger.
        ///
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::CompilerExecutionExternalAnchorDeploymentV", $version, " as Deployment;")]
        /// fn duplicate(value: Deployment) { let _ = value.clone(); }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::{CompilerExecutionExternalAnchorDeploymentV1, CompilerExecutionExternalAnchorDeploymentV", $version, " as Deployment};")]
        /// fn upgrade(value: CompilerExecutionExternalAnchorDeploymentV1) -> Deployment { value.into() }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::{CompilerExecutionExternalAnchorDeploymentV", $version, " as Deployment, CompilerExecutionExternalAnchorDeploymentV", $other, " as Other};")]
        /// fn mix(value: Deployment) -> Other { value.into() }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::{CompilerExecutionExternalAnchorDeploymentV", $version, " as Deployment, CompilerExecutionSupervisorDeploymentV", $other, " as Supervisor, CompilerExecutionIssuerPolicyV", $version, " as Policy};")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn mix(bytes: &[u8], s: &Supervisor, p: &Policy, b: &mut Budget<'_>) {
        ///     let _ = Deployment::decode(bytes, s, p, b);
        /// }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::{CompilerExecutionExternalAnchorDeploymentV", $version, " as Deployment, CompilerExecutionSupervisorDeploymentV", $version, " as Supervisor, CompilerExecutionIssuerPolicyV", $other, " as Policy};")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn mix(value: &Deployment, s: &Supervisor, p: &Policy, b: &mut Budget<'_>) {
        ///     let _ = value.matches_supervisor_and_policy(s, p, b);
        /// }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::{CompilerExecutionExternalAnchorDeploymentV", $version, " as Deployment, CompilerExecutionSupervisorDeploymentV1 as Supervisor, CompilerExecutionIssuerPolicyV1 as Policy};")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn legacy(bytes: &[u8], s: &Supervisor, p: &Policy, b: &mut Budget<'_>) {
        ///     let _ = Deployment::decode(bytes, s, p, b);
        /// }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::{CompilerExecutionExternalAnchorDeploymentV", $version, " as Deployment, CompilerExecutionSupervisorDeploymentIdentityV", $version, " as Identity, CompilerExecutionIssuerPolicyV", $version, " as Policy};")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn identity_only(bytes: &[u8], s: &Identity, p: &Policy, b: &mut Budget<'_>) {
        ///     let _ = Deployment::decode(bytes, s, p, b);
        /// }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::{CompilerExecutionExternalAnchorDeploymentV", $version, " as Deployment, CompilerExecutionSupervisorDeploymentV", $version, " as Supervisor, CompilerExecutionIssuerPolicyV", $version, " as Policy};")]
        /// fn unmetered(bytes: &[u8], s: &Supervisor, p: &Policy) {
        ///     let _ = Deployment::decode(bytes, s, p);
        /// }
        /// ```
        #[derive(Eq, PartialEq)]
        pub struct $Owner {
            service: Service,
            verifying_key: [u8; 32],
            supervisor: SupervisorIdentity,
            executable: Measurement,
            identity: $Identity,
            bytes: [u8; codec::BYTES],
        }
        impl $Owner {
            pub fn new(
                supervisor: &Supervisor,
                policy: &Policy,
                executable: Measurement,
                budget: &mut Budget<'_>,
            ) -> Result<(Self, Storage)> {
                metered(budget, context_floor(supervisor, policy)?, supervisor, policy, |matched| {
                    if !matched {
                        return Err($Failure::SupervisorPolicyMismatch);
                    }
                    let record = codec::$schema.encode(codec::Fields {
                        service: supervisor.external_anchor_service(),
                        verifying_key: *policy.external_anchor_verifying_key(),
                        supervisor: *supervisor.identity().as_bytes(),
                        executable,
                    })?;
                    Ok((Self::from_record(record, supervisor), Storage(RETAINED)))
                })
            }

            /// Strict canonical decode requires BOTH actual native owners. A rehashed
            /// identity, key or credential substitution still fails contextual matching.
            /// The executable is bounded configuration; use the exact-executable match
            /// against the caller's independently measured/pinned executable as needed.
            pub fn decode(
                bytes: &[u8],
                supervisor: &Supervisor,
                policy: &Policy,
                budget: &mut Budget<'_>,
            ) -> Result<(Self, Storage)> {
                let floor = context_floor(supervisor, policy)?
                    .checked_add(resources::fixed_input_floor(bytes, codec::BYTES))
                    .ok_or(Resource::Arithmetic)?;
                metered(budget, floor, supervisor, policy, |matched| {
                    if !matched {
                        return Err($Failure::SupervisorPolicyMismatch);
                    }
                    let record = codec::$schema.decode(bytes)?;
                    if !fields_match(&record.fields, supervisor, policy) {
                        return Err($Failure::ContextMismatch);
                    }
                    Ok((Self::from_record(record, supervisor), Storage(RETAINED)))
                })
            }

            pub fn matches_supervisor_and_policy(
                &self,
                supervisor: &Supervisor,
                policy: &Policy,
                budget: &mut Budget<'_>,
            ) -> Result<bool> {
                let floor = context_floor(supervisor, policy)?
                    .checked_add(RETAINED).ok_or(Resource::Arithmetic)?;
                metered(budget, floor, supervisor, policy, |matched| {
                    Ok(matched && self.matches_context(supervisor, policy))
                })
            }

            /// Includes exact digest AND length equality; no executable bytes are read.
            pub fn matches_supervisor_policy_and_executable(
                &self,
                supervisor: &Supervisor,
                policy: &Policy,
                executable: Measurement,
                budget: &mut Budget<'_>,
            ) -> Result<bool> {
                let floor = context_floor(supervisor, policy)?
                    .checked_add(RETAINED).ok_or(Resource::Arithmetic)?;
                metered(budget, floor, supervisor, policy, |matched| {
                    Ok(matched
                        && self.matches_context(supervisor, policy)
                        && self.executable == executable)
                })
            }

            fn matches_context(&self, supervisor: &Supervisor, policy: &Policy) -> bool {
                self.supervisor == supervisor.identity()
                    && self.service == supervisor.external_anchor_service()
                    && self.verifying_key == *policy.external_anchor_verifying_key()
            }

            fn from_record(record: codec::Record, supervisor: &Supervisor) -> Self {
                Self {
                    service: record.fields.service,
                    verifying_key: record.fields.verifying_key,
                    supervisor: supervisor.identity(),
                    executable: record.fields.executable,
                    identity: $Identity(record.identity),
                    bytes: record.bytes,
                }
            }
            pub const fn service(&self) -> Service { self.service }
            pub const fn verifying_key(&self) -> &[u8; 32] { &self.verifying_key }
            pub const fn supervisor_deployment_identity(&self) -> SupervisorIdentity { self.supervisor }
            pub const fn executable(&self) -> Measurement { self.executable }
            pub const fn identity(&self) -> $Identity { self.identity }
            pub const fn canonical_bytes(&self) -> &[u8; codec::BYTES] { &self.bytes }
            pub const fn retained_storage(&self) -> usize { RETAINED }
        }

        impl fmt::Debug for $Owner {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($Owner))
                    .field("authority", &"none")
                    .field("service", &self.service)
                    .field("supervisor_deployment_identity", &self.supervisor)
                    .field("identity", &self.identity)
                    .finish_non_exhaustive()
            }
        }

        /// Bounded typed diagnostics; reusing framing errors never invokes a V1 codec.
        #[derive(Debug)]
        pub enum $Failure {
            Framing(Framing),
            Supervisor(SupervisorError),
            SupervisorPolicyMismatch,
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
        impl From<SupervisorError> for $Failure {
            fn from(e: SupervisorError) -> Self {
                match e {
                    SupervisorError::Resource(e) => Self::Resource(e),
                    e => Self::Supervisor(e),
                }
            }
        }
        impl fmt::Display for $Failure {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    Self::Framing(e) => e.fmt(f),
                    Self::Supervisor(e) => e.fmt(f),
                    Self::Resource(e) => e.fmt(f),
                    Self::SupervisorPolicyMismatch => f.write_str("external-anchor supervisor policy mismatch"),
                    Self::ContextMismatch => f.write_str("external-anchor deployment context mismatch"),
                }
            }
        }
        impl Error for $Failure {
            fn source(&self) -> Option<&(dyn Error + 'static)> {
                match self {
                    Self::Framing(e) => Some(e),
                    Self::Supervisor(e) => Some(e),
                    Self::Resource(e) => Some(e),
                    Self::SupervisorPolicyMismatch | Self::ContextMismatch => None,
                }
            }
        }

        fn context_floor(supervisor: &Supervisor, policy: &Policy) -> Result<usize> {
            supervisor.retained_storage().checked_add(policy.retained_storage())
                .ok_or_else(|| Resource::Arithmetic.into())
        }
        fn fields_match(fields: &codec::Fields, supervisor: &Supervisor, policy: &Policy) -> bool {
            fields.supervisor == *supervisor.identity().as_bytes()
                && fields.service == supervisor.external_anchor_service()
                && fields.verifying_key == *policy.external_anchor_verifying_key()
        }
        fn metered<T>(
            budget: &mut Budget<'_>,
            floor: usize,
            supervisor: &Supervisor,
            policy: &Policy,
            operation: impl FnOnce(bool) -> Result<T>,
        ) -> Result<T> {
            // Admit the full borrowed-owner floor before our scratch can hide an
            // unpaid input. The nested check then uses this SAME protected ledger.
            resources::nested_fixed(budget, floor, LOCAL_WORK, LOCAL_STORAGE, |budget| {
                let matched = supervisor.matches_policy(policy, budget)?;
                operation(matched)
            })
        }
        const _: () = {
            type Output = ($Owner, Storage);
            assert!(codec::BYTES == 168 && codec::BYTES <= u32::MAX as usize);
            assert!(size_of::<codec::Record>() <= RETAINED);
            assert!(size_of::<Result<Output>>() >= RETAINED);
            assert!(size_of::<std::thread::Result<Result<Output>>>() >= RETAINED);
            assert!(size_of::<std::result::Result<codec::Record, Framing>>() >= size_of::<codec::Record>());
            assert!(8 * size_of::<$Failure>() + 64 * size_of::<usize>()
                + size_of::<Budget<'static>>() + 4 * size_of::<Storage>()
                + (size_of::<Result<Output>>() - RETAINED)
                + (size_of::<std::thread::Result<Result<Output>>>() - RETAINED)
                + (size_of::<std::result::Result<codec::Record, Framing>>() - size_of::<codec::Record>())
                <= 4096);
        };
    };
}
pub(crate) use external_anchor_deployment_adapter;
