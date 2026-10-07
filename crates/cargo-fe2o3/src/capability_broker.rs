//! Descriptor transport from the `cargo-fe2o3` parent to managed rustc wrappers.
//!
//! Cargo receives a strict per-instance route and build-session binding. Both peers check Linux
//! credentials, exact executable identity, the prepared profile/config identity, and a
//! challenge-response bound to a separate 256-bit broker secret before transferring a
//! sealed backend image and a read-only artifact-directory descriptor with `SCM_RIGHTS`. A
//! protected release also receives one sealed descriptor carrying the complete admitted
//! compiler-closure preimage.
//! Receivers validate the exact profile-specific descriptor count and positional types before
//! installing capabilities in the caller-selected compiler process for a compile-shaped wrapper
//! invocation.
//!
//! An independent seccomp exec boundary additionally grants a one-use broker permit only to a
//! direct Cargo child stopped while requesting the pinned wrapper image. Inherited route material
//! therefore cannot authorize build-script or procedural-macro replay. A procedural macro still
//! executes inside an already-authorized rustc process and can observe that compilation's
//! descriptors. The directory is opened `O_RDONLY`, but still grants descriptor-relative namespace
//! mutation. The
//! receiver treats that route as untrusted: before connecting, it independently observes its own
//! running `cargo-fe2o3` image and requires the advertised broker to have the same uid, executable
//! object, and bytes. This closes a self-consistent route redirected to an arbitrary mock
//! executable. A substitute running the same executable object and bytes remains inside the
//! executable-authentication boundary, but it has no public broker-server entry point and must
//! still possess a kernel-observed one-use invocation permit. This is not a sandbox against hostile
//! same-user code that can ptrace or inject into another process; untrusted build dependencies
//! require a separate process sandbox.
//!
//! The private V4 transport binds a fresh V3 client-profile identity and keeps
//! the same four protected descriptor roles. V3/V1 and V4/V3 readers select an
//! exact family before authentication, with no retry, upgrade or fallback.

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod platform {
    use std::collections::BTreeMap;
    use std::fs::{self, File};
    use std::io::{self, IoSlice, IoSliceMut, Read, Write};
    use std::mem::MaybeUninit;
    use std::net::Shutdown;
    use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd};
    use std::os::linux::net::SocketAddrExt;
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::net::{SocketAddr, UnixListener, UnixStream};
    use std::path::PathBuf;
    use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};
    use std::thread::{self, JoinHandle};
    use std::time::{Duration, Instant};

    use fe2o3_artifact_transaction::{
        BROKERED_INVOCATION_ADMITTED_V1, BROKERED_INVOCATION_PREPARED_V1,
        BROKERED_INVOCATION_REQUEST_BYTES_V1, BROKERED_INVOCATION_REQUEST_BYTES_V2,
        BrokeredInvocationCapabilityRequestV1, BrokeredInvocationCapabilityRequestV2, BuildAttempt,
        BuildSession,
    };
    use fe2o3_process_identity::LinuxObjectIdentityV3;
    use fe2o3_verifier::{
        CompilerProofBrokerV1, PROTECTED_FUNCTIONAL_REFINEMENT_RUNTIME_ROOT_V1,
        PendingCompilerProofDelegationV1,
    };
    use rustix::net::{
        RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags, SendAncillaryBuffer,
        SendAncillaryMessage, SendFlags, recvmsg, sendmsg,
    };
    use sha2::{Digest, Sha256};

    use crate::authority_release::profile::{ClientProfileAccountV3, FundedClientProfileV3};
    use crate::build_config::{
        ProductionSourceIsaObservationKindV1, ProductionSourceIsaObserverPolicyV1,
    };
    use crate::cargo_invocation_boundary::{InvocationAuthorizationRegistryV1, ProcessIdentityV1};
    use crate::pinned_codegen_backend::PinnedCodegenBackend;
    use crate::pinned_executable::{PinExecutableError, PinnedExecutable};
    use crate::project::PinnedDirectory;
    use fe2o3_compiler_closure_capability::{
        CompilerClosureCapabilityV1, CompilerExecutionClientProfileCapabilityV1,
    };
    use fe2o3_source_isa_observation::characteristic_v1::{
        InertSourceIsaCharacteristicCollectionV1,
        MAX_SOURCE_ISA_CHARACTERISTIC_COLLECTION_BYTES_V1, SourceIsaCharacteristicCollectionV1,
        SourceIsaCharacteristicTargetProfileV1,
    };
    use fe2o3_source_isa_observation::wire_v1::{
        SOURCE_ISA_OBSERVATION_FRAME_BYTES_V1, SourceIsaObservationCollectionV1,
        SourceIsaObservationFrameV1, SourceIsaObservationOutcomeV1,
        SourceIsaObservationTargetProfileV1, SourceIsaObservationTransportFailureV1,
    };

    mod profile_transport {
        include!("capability_broker_profile_transport.rs");
    }
    #[cfg(test)]
    use profile_transport::response_bytes;
    use profile_transport::{
        BrokerProfileRef, RetainedBrokerProfile, authenticate_request, request_bytes,
        response_bytes_for,
    };
    pub(crate) use profile_transport::{
        CompilerExecutionProfileFamily, broker_route_family_from_environment,
    };

    pub(crate) const CAPABILITY_BROKER_ENV: &str = "FE2O3_CAPABILITY_BROKER_V1";
    const REQUEST_MAGIC: &[u8] = b"FE2O3-CARGO-CAPABILITY-BROKER-V3\0";
    const ROUTE_PREFIX: &str = "fe2o3-capability-route-v3";
    const REQUEST_MAGIC_V4: &[u8] = b"FE2O3-CARGO-CAPABILITY-BROKER-V4\0";
    const ROUTE_PREFIX_V4: &str = "fe2o3-capability-route-v4";
    const REQUEST_AUTH_DOMAIN_V4: &[u8] = b"FE2O3/CAPABILITY-BROKER/REQUEST-AUTH/V4\0";
    const RESPONSE_AUTH_DOMAIN_V4: &[u8] = b"FE2O3/CAPABILITY-BROKER/RESPONSE-AUTH/V4\0";
    const ENDPOINT_BYTES: usize = 32;
    const ENDPOINT_HEX_BYTES: usize = ENDPOINT_BYTES * 2;
    const SECRET_BYTES: usize = 32;
    const CHALLENGE_BYTES: usize = 32;
    const CONFIG_ID_BYTES: usize = 32;
    const COMPILER_CLOSURE_ID_BYTES: usize = 32;
    const RUSTC_EXECUTABLE_ID_BYTES: usize = 32;
    const RETAINED_OBJECT_BINDING_BYTES: usize = 32;
    const REQUEST_AUTH_BYTES: usize = 32;
    const REQUEST_BYTES: usize = REQUEST_MAGIC.len()
        + 16
        + 1
        + CONFIG_ID_BYTES
        + 1
        + COMPILER_CLOSURE_ID_BYTES
        + RUSTC_EXECUTABLE_ID_BYTES
        + RETAINED_OBJECT_BINDING_BYTES
        + CHALLENGE_BYTES
        + REQUEST_AUTH_BYTES;
    const RESPONSE_BYTES: usize = 1 + REQUEST_AUTH_BYTES;
    const REQUEST_AUTH_DOMAIN: &[u8] = b"FE2O3/CAPABILITY-BROKER/REQUEST-AUTH/V3\0";
    const RESPONSE_AUTH_DOMAIN: &[u8] = b"FE2O3/CAPABILITY-BROKER/RESPONSE-AUTH/V3\0";
    const MAX_PROC_STAT_BYTES: usize = 4096;
    const EXECUTABLE_PIN_ATTEMPTS: usize = 8;
    const RECEIVED_DESCRIPTOR_FLOOR: i32 = 226;
    #[cfg(test)]
    const _: () = assert!(
        RECEIVED_DESCRIPTOR_FLOOR > fe2o3_verifier::COMPILER_PROOF_ENDPOINT_CHILD_FD_V1
            && RECEIVED_DESCRIPTOR_FLOOR > fe2o3_verifier::COMPILER_PROOF_BROKER_CHILD_FD_V1
    );
    const BROKER_AUTHENTICATION_TIMEOUT: Duration = Duration::from_secs(30);
    const BROKER_CLIENT_RESPONSE_TIMEOUT: Duration = Duration::from_secs(60);
    const _: () = assert!(
        BROKER_CLIENT_RESPONSE_TIMEOUT.as_secs()
            >= BROKER_AUTHENTICATION_TIMEOUT.as_secs().saturating_mul(2)
    );
    const BROKER_INVOCATION_FRAME_TIMEOUT: Duration = Duration::from_secs(30);
    const BROKER_INVOCATION_LIFETIME: Duration = Duration::from_secs(6 * 60 * 60);
    const MAX_ACTIVE_CONNECTIONS: usize = 64;
    const MAX_CONCURRENT_AUTHENTICATIONS: usize = 8;
    const MAX_SOURCE_ISA_OBSERVATION_UNITS_V1: usize = 1024;
    const MAX_SOURCE_ISA_OBSERVATION_AGGREGATE_BYTES_V1: usize = 4 * 1024 * 1024;
    const _: () = assert!(
        fe2o3_source_isa_observation::wire_v1::MAX_SOURCE_ISA_OBSERVATION_COLLECTION_BYTES_V1
            <= MAX_SOURCE_ISA_OBSERVATION_AGGREGATE_BYTES_V1
    );
    const BROKERED_INVOCATION_REQUEST_MAGIC_V1: &[u8; 8] = b"F2BRKIV1";
    const BROKERED_INVOCATION_REQUEST_MAGIC_V2: &[u8; 8] = b"F2BRKIV2";
    const BROKERED_SOURCE_ISA_PREPARED_V1: &[u8; 16] = b"F2SI-PREPARED-V1";
    const PROOF_PREPARE_MAGIC: &[u8; 8] = b"F3CPRPV1";
    const PROOF_PREPARE_BYTES: usize = 192;
    const PROOF_RESPONSE_MAGIC: &[u8; 8] = b"F3CPRSV1";
    const PROOF_RESPONSE_BYTES: usize = 72;
    const PROOF_PREPARE_DOMAIN: &[u8] = b"FE2O3/COMPILER-PROOF/PREPARE/V1\0";
    const PROOF_RESPONSE_DOMAIN: &[u8] = b"FE2O3/COMPILER-PROOF/PREPARED/V1\0";

    struct ProofPreparationRequest {
        attempt: BuildAttempt,
        challenge: [u8; 32],
        authentication: [u8; 32],
    }

    impl ProofPreparationRequest {
        fn encode(&self, secret: &[u8; 32]) -> [u8; PROOF_PREPARE_BYTES] {
            let mut bytes = [0; PROOF_PREPARE_BYTES];
            let attempt = self.attempt.to_env_value();
            bytes[..8].copy_from_slice(PROOF_PREPARE_MAGIC);
            bytes[8..40].copy_from_slice(&self.challenge);
            bytes[40..42].copy_from_slice(&(attempt.len() as u16).to_le_bytes());
            bytes[42..42 + attempt.len()].copy_from_slice(attempt.as_bytes());
            let auth = keyed_digest(PROOF_PREPARE_DOMAIN, secret, &[&bytes[..160]]);
            bytes[160..].copy_from_slice(&auth);
            bytes
        }

        fn decode(bytes: &[u8; PROOF_PREPARE_BYTES]) -> io::Result<Self> {
            let length = u16::from_le_bytes(bytes[40..42].try_into().unwrap()) as usize;
            if &bytes[..8] != PROOF_PREPARE_MAGIC
                || !(1..=118).contains(&length)
                || bytes[42 + length..160].iter().any(|byte| *byte != 0)
                || bytes[8..40] == [0; 32]
            {
                return Err(io::Error::other("noncanonical compiler-proof preparation"));
            }
            let attempt = std::str::from_utf8(&bytes[42..42 + length]).map_err(io::Error::other)?;
            let attempt = BuildAttempt::from_env_value(attempt).map_err(io::Error::other)?;
            Ok(Self {
                attempt,
                challenge: bytes[8..40].try_into().unwrap(),
                authentication: bytes[160..].try_into().unwrap(),
            })
        }

        fn require_authenticated(
            &self,
            secret: &[u8; 32],
            session: BuildSession,
        ) -> io::Result<()> {
            if self.attempt.session() == BuildSession::DIRECT
                || self.attempt.session() != session
                || self.encode(secret)[160..] != self.authentication
            {
                return Err(io::Error::other(
                    "compiler-proof preparation is not authenticated to this build session",
                ));
            }
            Ok(())
        }

        fn response(&self, secret: &[u8; 32], session: [u8; 32]) -> [u8; PROOF_RESPONSE_BYTES] {
            let mut bytes = [0; PROOF_RESPONSE_BYTES];
            bytes[..8].copy_from_slice(PROOF_RESPONSE_MAGIC);
            bytes[8..40].copy_from_slice(&session);
            let auth = keyed_digest(
                PROOF_RESPONSE_DOMAIN,
                secret,
                &[&self.encode(secret), &bytes[..40]],
            );
            bytes[40..].copy_from_slice(&auth);
            bytes
        }
    }

    #[derive(Clone, Copy)]
    struct BrokerLimits {
        max_active_connections: usize,
        authentication_timeout: Duration,
        invocation_frame_timeout: Duration,
        invocation_lifetime: Duration,
    }

    const PRODUCTION_BROKER_LIMITS: BrokerLimits = BrokerLimits {
        max_active_connections: MAX_ACTIVE_CONNECTIONS,
        authentication_timeout: BROKER_AUTHENTICATION_TIMEOUT,
        invocation_frame_timeout: BROKER_INVOCATION_FRAME_TIMEOUT,
        invocation_lifetime: BROKER_INVOCATION_LIFETIME,
    };

    struct BrokerCompilerCapabilities<'profile> {
        closure: Option<fe2o3_build_authority::CompilerClosureV2>,
        execution_profile: Option<BrokerProfileRef<'profile>>,
        proof: Option<Arc<CompilerProofBrokerV1>>,
    }

    #[derive(Clone)]
    struct BrokerSourceIsaObserverV1 {
        config_identity: [u8; 32],
        session: BuildSession,
        selected_units: Vec<[u8; 32]>,
        kind: ProductionSourceIsaObservationKindV1,
        collector: Arc<Mutex<SourceIsaObservationCollectorStateV1>>,
    }

    impl BrokerSourceIsaObserverV1 {
        fn from_policy(
            policy: &ProductionSourceIsaObserverPolicyV1,
            session: BuildSession,
        ) -> Result<Self, String> {
            if session == BuildSession::DIRECT {
                return Err("source/ISA observer requires a managed build session".to_owned());
            }
            let unit_count = policy.selected_units().len();
            if unit_count == 0 || unit_count > MAX_SOURCE_ISA_OBSERVATION_UNITS_V1 {
                return Err(format!(
                    "source/ISA observer requires 1..={MAX_SOURCE_ISA_OBSERVATION_UNITS_V1} exact units"
                ));
            }
            if policy.kind() == ProductionSourceIsaObservationKindV1::Characteristic
                && unit_count != 1
            {
                return Err(
                    "source/ISA characteristic observer requires exactly one unit".to_owned(),
                );
            }
            let mut selected_units = Vec::new();
            selected_units.try_reserve_exact(unit_count).map_err(|_| {
                "cannot allocate the bounded source/ISA observer broker policy".to_owned()
            })?;
            selected_units.extend(
                policy
                    .selected_units()
                    .iter()
                    .map(|identity| *identity.as_bytes()),
            );
            if selected_units.contains(&[0; 32])
                || selected_units.windows(2).any(|pair| pair[0] >= pair[1])
            {
                return Err("source/ISA observer broker policy is not canonical".to_owned());
            }
            let config_identity = *policy.config_identity().as_bytes();
            let collector = SourceIsaObservationCollectorStateV1::with_expected_context(
                config_identity,
                session,
                &selected_units,
                policy.kind(),
            )?;
            Ok(Self {
                config_identity,
                session,
                selected_units,
                kind: policy.kind(),
                collector: Arc::new(Mutex::new(collector)),
            })
        }

        fn accepts(&self, request: BrokeredInvocationCapabilityRequestV2) -> bool {
            request.config_identity() == self.config_identity
                && self
                    .selected_units
                    .binary_search(&request.unit_identity())
                    .is_ok()
        }

        fn collect(
            &self,
            frame: SourceIsaObservationFrameV1,
            characteristic: Option<Vec<u8>>,
        ) -> io::Result<()> {
            let context = frame.context();
            if context.config() != self.config_identity
                || context.attempt().session()
                    != crate::source_isa_observation::inert_source_isa_session_v1(self.session)
                || self.selected_units.binary_search(&context.unit()).is_err()
            {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "source/ISA observation frame is not bound to the configured unit",
                ));
            }
            self.collector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(frame, characteristic)
                .map_err(io::Error::other)
        }

        fn fail(&self, reason: SourceIsaObservationTransportFailureV1) {
            self.collector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .fail(reason);
        }
    }

    struct SourceIsaObservationCollectorStateV1 {
        config_identity: [u8; 32],
        session: BuildSession,
        frames: Vec<([u8; 32], SourceIsaObservationFrameV1)>,
        expected_units: Vec<[u8; 32]>,
        aggregate_bytes: usize,
        failure: Option<SourceIsaObservationTransportFailureV1>,
        kind: ProductionSourceIsaObservationKindV1,
        characteristic: Option<([u8; 32], Vec<u8>)>,
        census: Vec<([u8; 32], Vec<u8>)>,
    }

    impl SourceIsaObservationCollectorStateV1 {
        fn with_expected_context(
            config_identity: [u8; 32],
            session: BuildSession,
            expected: &[[u8; 32]],
            kind: ProductionSourceIsaObservationKindV1,
        ) -> Result<Self, String> {
            if config_identity == [0; 32] || session == BuildSession::DIRECT {
                return Err("source/ISA collector requires exact nonzero context".to_owned());
            }
            let mut frames = Vec::new();
            frames.try_reserve_exact(expected.len()).map_err(|_| {
                "cannot allocate the bounded source/ISA observation collector".to_owned()
            })?;
            let mut expected_units = Vec::new();
            expected_units
                .try_reserve_exact(expected.len())
                .map_err(|_| {
                    "cannot allocate the bounded source/ISA expected-unit set".to_owned()
                })?;
            expected_units.extend_from_slice(expected);
            let mut census = Vec::new();
            if kind == ProductionSourceIsaObservationKindV1::ProductionCensusV91 {
                census
                    .try_reserve_exact(expected.len())
                    .map_err(|_| "cannot allocate bounded production census".to_owned())?;
            }
            Ok(Self {
                config_identity,
                session,
                frames,
                expected_units,
                aggregate_bytes: 0,
                failure: None,
                kind,
                characteristic: None,
                census,
            })
        }

        fn insert(
            &mut self,
            frame: SourceIsaObservationFrameV1,
            characteristic: Option<Vec<u8>>,
        ) -> Result<(), SourceIsaObservationTransportFailureV1> {
            if frame.context().config() != self.config_identity
                || frame.context().attempt().session()
                    != crate::source_isa_observation::inert_source_isa_session_v1(self.session)
            {
                self.fail(SourceIsaObservationTransportFailureV1::RejectedFrame);
                return Err(SourceIsaObservationTransportFailureV1::RejectedFrame);
            }
            let unit = frame.context().unit();
            match (self.kind, characteristic.as_ref()) {
                (ProductionSourceIsaObservationKindV1::Summary, None) => {}
                (ProductionSourceIsaObservationKindV1::Characteristic, Some(bytes))
                    if !bytes.is_empty()
                        && bytes.len() <= MAX_SOURCE_ISA_CHARACTERISTIC_COLLECTION_BYTES_V1 => {}
                (ProductionSourceIsaObservationKindV1::ProductionCensusV91, Some(bytes))
                    if crate::production_census_v91::Census::decode(bytes)
                        .and_then(|row| row.check_frame(&frame))
                        .is_ok() => {}
                _ => {
                    self.fail(SourceIsaObservationTransportFailureV1::RejectedFrame);
                    return Err(SourceIsaObservationTransportFailureV1::RejectedFrame);
                }
            }
            let insertion = match self.frames.binary_search_by_key(&unit, |(unit, _)| *unit) {
                Ok(index) => {
                    let characteristic_matches = match (
                        self.kind,
                        self.characteristic.as_ref(),
                        characteristic.as_ref(),
                    ) {
                        (ProductionSourceIsaObservationKindV1::Summary, None, None) => true,
                        (
                            ProductionSourceIsaObservationKindV1::ProductionCensusV91,
                            None,
                            Some(candidate),
                        ) => self
                            .census
                            .get(index)
                            .is_some_and(|(found, bytes)| *found == unit && bytes == candidate),
                        (
                            ProductionSourceIsaObservationKindV1::Characteristic,
                            Some((observed_unit, observed)),
                            Some(candidate),
                        ) => *observed_unit == unit && observed == candidate,
                        _ => false,
                    };
                    if self.frames[index].1 == frame && characteristic_matches {
                        return Ok(());
                    }
                    self.fail(SourceIsaObservationTransportFailureV1::ConflictingDuplicate);
                    return Err(SourceIsaObservationTransportFailureV1::ConflictingDuplicate);
                }
                Err(insertion) => insertion,
            };
            if self.frames.len() >= MAX_SOURCE_ISA_OBSERVATION_UNITS_V1 {
                self.fail(SourceIsaObservationTransportFailureV1::UnitBound);
                return Err(SourceIsaObservationTransportFailureV1::UnitBound);
            }
            let payload_bytes = SOURCE_ISA_OBSERVATION_FRAME_BYTES_V1
                .checked_add(characteristic.as_ref().map_or(0, |bytes| bytes.len() + 8))
                .ok_or(SourceIsaObservationTransportFailureV1::AggregateByteBound)?;
            let aggregate_limit = match self.kind {
                ProductionSourceIsaObservationKindV1::Summary => {
                    MAX_SOURCE_ISA_OBSERVATION_AGGREGATE_BYTES_V1
                }
                ProductionSourceIsaObservationKindV1::ProductionCensusV91 => {
                    crate::production_census_v91::MAX_AGGREGATE
                }
                ProductionSourceIsaObservationKindV1::Characteristic => {
                    MAX_SOURCE_ISA_CHARACTERISTIC_COLLECTION_BYTES_V1
                        + SOURCE_ISA_OBSERVATION_FRAME_BYTES_V1
                        + 8
                }
            };
            let Some(aggregate_bytes) = self
                .aggregate_bytes
                .checked_add(payload_bytes)
                .filter(|bytes| *bytes <= aggregate_limit)
            else {
                self.fail(SourceIsaObservationTransportFailureV1::AggregateByteBound);
                return Err(SourceIsaObservationTransportFailureV1::AggregateByteBound);
            };
            self.frames.insert(insertion, (unit, frame));
            if let Some(bytes) = characteristic {
                if self.kind == ProductionSourceIsaObservationKindV1::ProductionCensusV91 {
                    self.census.insert(insertion, (unit, bytes));
                    self.aggregate_bytes = aggregate_bytes;
                    return Ok(());
                }
                if self.characteristic.is_some() {
                    self.fail(SourceIsaObservationTransportFailureV1::ConflictingDuplicate);
                    return Err(SourceIsaObservationTransportFailureV1::ConflictingDuplicate);
                }
                self.characteristic = Some((unit, bytes));
            }
            self.aggregate_bytes = aggregate_bytes;
            Ok(())
        }

        fn fail(&mut self, reason: SourceIsaObservationTransportFailureV1) {
            self.failure.get_or_insert(reason);
        }

        fn finish(mut self) -> CompletedSourceIsaObservationsV1 {
            self.expected_units.retain(|unit| {
                self.frames
                    .binary_search_by_key(unit, |(observed, _)| *observed)
                    .is_err()
            });
            if !self.expected_units.is_empty() && self.failure.is_none() {
                self.failure = Some(SourceIsaObservationTransportFailureV1::MissingSelectedUnits);
            }
            let summary = SourceIsaObservationCollectionV1::from_collected(
                self.config_identity,
                crate::source_isa_observation::inert_source_isa_session_v1(self.session),
                self.frames,
                self.expected_units,
                self.failure,
            );
            CompletedSourceIsaObservationsV1 {
                config: self.config_identity,
                summary,
                characteristic: self.characteristic,
                census: self.census,
            }
        }
    }

    pub(crate) struct CompletedSourceIsaObservationsV1 {
        pub(crate) config: [u8; 32],
        pub(crate) summary: SourceIsaObservationCollectionV1,
        pub(crate) characteristic: Option<([u8; 32], Vec<u8>)>,
        pub(crate) census: Vec<([u8; 32], Vec<u8>)>,
    }

    impl<'profile> BrokerCompilerCapabilities<'profile> {
        const fn ordinary() -> Self {
            Self {
                closure: None,
                execution_profile: None,
                proof: None,
            }
        }

        fn protected(
            closure: fe2o3_build_authority::CompilerClosureV2,
            execution_profile: &'profile CompilerExecutionClientProfileCapabilityV1,
        ) -> Result<Self, String> {
            execution_profile.revalidate()?;
            let proof = CompilerProofBrokerV1::open(
                closure,
                PROTECTED_FUNCTIONAL_REFINEMENT_RUNTIME_ROOT_V1,
            )
            .map_err(|error| format!("cannot open protected compiler proof executor: {error}"))?;
            Ok(Self {
                closure: Some(closure),
                execution_profile: Some(BrokerProfileRef::V1(execution_profile)),
                proof: Some(Arc::new(proof)),
            })
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) enum CapabilityProfileV1 {
        Ordinary,
    }

    impl CapabilityProfileV1 {
        const fn request_magic(self) -> &'static [u8] {
            match self {
                Self::Ordinary => REQUEST_MAGIC,
            }
        }

        const fn descriptor_count(self) -> usize {
            match self {
                Self::Ordinary => 2,
            }
        }

        const fn name(self) -> &'static str {
            match self {
                Self::Ordinary => "ordinary",
            }
        }

        const fn route_name(self) -> &'static str {
            match self {
                Self::Ordinary => "ordinary",
            }
        }

        fn parse_route_name(value: &str) -> Option<Self> {
            match value {
                "ordinary" => Some(Self::Ordinary),
                _ => None,
            }
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) struct CapabilityBindingV3 {
        profile: CapabilityProfileV1,
        config_identity: Option<[u8; CONFIG_ID_BYTES]>,
        protected_compiler_closure_v2: bool,
        compiler_closure_sha256: [u8; COMPILER_CLOSURE_ID_BYTES],
        rustc_executable_sha256: [u8; RUSTC_EXECUTABLE_ID_BYTES],
        retained_object_binding_sha256: [u8; RETAINED_OBJECT_BINDING_BYTES],
        // None preserves the frozen V3 bytes; Some is exclusively V4/V3-profile.
        native_profile_identity: Option<[u8; 32]>,
    }

    impl CapabilityBindingV3 {
        pub(crate) fn new(
            profile: CapabilityProfileV1,
            config_identity: Option<[u8; CONFIG_ID_BYTES]>,
            compiler_closure_sha256: [u8; COMPILER_CLOSURE_ID_BYTES],
            rustc_executable_sha256: [u8; RUSTC_EXECUTABLE_ID_BYTES],
            retained_object_binding_sha256: [u8; RETAINED_OBJECT_BINDING_BYTES],
        ) -> Result<Self, String> {
            if compiler_closure_sha256 == [0; COMPILER_CLOSURE_ID_BYTES]
                || rustc_executable_sha256 == [0; RUSTC_EXECUTABLE_ID_BYTES]
                || retained_object_binding_sha256 == [0; RETAINED_OBJECT_BINDING_BYTES]
            {
                return Err("capability binding identities must be nonzero".into());
            }
            Ok(Self {
                profile,
                config_identity,
                protected_compiler_closure_v2: false,
                compiler_closure_sha256,
                rustc_executable_sha256,
                retained_object_binding_sha256,
                native_profile_identity: None,
            })
        }

        pub(crate) fn new_protected(
            profile: CapabilityProfileV1,
            config_identity: Option<[u8; CONFIG_ID_BYTES]>,
            compiler_closure: fe2o3_build_authority::CompilerClosureV2,
            retained_object_binding_sha256: [u8; RETAINED_OBJECT_BINDING_BYTES],
        ) -> Result<Self, String> {
            let mut binding = Self::new(
                profile,
                config_identity,
                compiler_closure.identity_sha256(),
                compiler_closure.rustc_executable_sha256(),
                retained_object_binding_sha256,
            )?;
            binding.protected_compiler_closure_v2 = true;
            Ok(binding)
        }

        /// Reads an untrusted routing claim only. `receive` must authenticate it
        /// before its configuration identity can authorize manifest preparation.
        pub(crate) fn from_environment_for_client(
            profile: CapabilityProfileV1,
        ) -> Result<Self, String> {
            let encoded_route = std::env::var(CAPABILITY_BROKER_ENV).map_err(|_| {
                format!("managed rustc invocation is missing {CAPABILITY_BROKER_ENV}")
            })?;
            let route = BrokerRouteV3::parse(&encoded_route)?;
            if route.binding.profile != profile {
                return Err("capability broker route has the wrong profile".into());
            }
            Ok(route.binding)
        }

        /// A routing claim until `receive` authenticates this exact binding.
        pub(crate) const fn config_identity(self) -> Option<[u8; CONFIG_ID_BYTES]> {
            self.config_identity
        }

        pub(crate) const fn compiler_closure_sha256(self) -> [u8; 32] {
            self.compiler_closure_sha256
        }

        pub(crate) const fn requires_compiler_closure_v2(self) -> bool {
            self.protected_compiler_closure_v2
        }

        const fn descriptor_count(self) -> usize {
            self.profile.descriptor_count()
                + if self.protected_compiler_closure_v2 {
                    2
                } else {
                    0
                }
        }

        pub(crate) const fn rustc_executable_sha256(self) -> [u8; 32] {
            self.rustc_executable_sha256
        }

        pub(crate) const fn retained_object_binding_sha256(self) -> [u8; 32] {
            self.retained_object_binding_sha256
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct BrokerPeerIdentityV2 {
        uid: u32,
        pid: u32,
        start_time_ticks: u64,
        device: u64,
        inode: u64,
        mode: u32,
        executable_sha256: [u8; 32],
    }

    #[derive(Clone, Copy)]
    struct CurrentExecutableObservation {
        device: u64,
        inode: u64,
        mode: u32,
        executable_sha256: [u8; 32],
    }

    static CURRENT_EXECUTABLE_OBSERVATION: OnceLock<Result<CurrentExecutableObservation, String>> =
        OnceLock::new();
    fn current_executable_observation() -> Result<CurrentExecutableObservation, String> {
        CURRENT_EXECUTABLE_OBSERVATION
            .get_or_init(|| {
                let pid = std::process::id();
                let initial_start = process_start_time_ticks(pid)?;
                let (pinned, metadata) = pin_process_executable(pid)?;
                if process_start_time_ticks(pid)? != initial_start {
                    return Err("current broker process identity changed while pinning".to_owned());
                }
                Ok(CurrentExecutableObservation {
                    device: metadata.dev(),
                    inode: metadata.ino(),
                    mode: metadata.mode(),
                    executable_sha256: *pinned.sha256(),
                })
            })
            .clone()
    }

    impl BrokerPeerIdentityV2 {
        fn current() -> Result<Self, String> {
            let pid = std::process::id();
            let start_time_ticks = process_start_time_ticks(pid)?;
            let executable = current_executable_observation()?;
            let identity = Self {
                uid: rustix::process::geteuid().as_raw(),
                pid,
                start_time_ticks,
                device: executable.device,
                inode: executable.inode,
                mode: executable.mode,
                executable_sha256: executable.executable_sha256,
            };
            if process_start_time_ticks(pid)? != start_time_ticks {
                return Err("current broker process identity changed while pinning".into());
            }
            Ok(identity)
        }

        fn require_current_executable(self) -> Result<(), String> {
            let current = current_executable_observation()?;
            if self.uid != rustix::process::geteuid().as_raw()
                || self.object_identity()
                    != LinuxObjectIdentityV3::from_linux_stat(
                        current.device,
                        current.inode,
                        current.mode,
                    )
                || self.executable_sha256 != current.executable_sha256
            {
                return Err(
                    "capability broker route does not name the current cargo-fe2o3 executable"
                        .into(),
                );
            }
            Ok(())
        }

        const fn object_identity(self) -> LinuxObjectIdentityV3 {
            LinuxObjectIdentityV3::from_linux_stat(self.device, self.inode, self.mode)
        }

        fn authenticate(self, stream: &UnixStream) -> Result<(), String> {
            let credentials = rustix::net::sockopt::socket_peercred(stream)
                .map_err(|error| format!("cannot inspect capability broker peer: {error}"))?;
            let peer_pid = u32::try_from(credentials.pid.as_raw_nonzero().get())
                .map_err(|_| "capability broker peer PID is negative".to_owned())?;
            let current_uid = rustix::process::geteuid().as_raw();
            if credentials.uid.as_raw() != current_uid || credentials.uid.as_raw() != self.uid {
                return Err("capability broker peer uid does not match the current user".into());
            }
            if peer_pid != self.pid {
                return Err("capability broker peer PID does not match the prepared route".into());
            }
            let initial_start = process_start_time_ticks(peer_pid)?;
            if initial_start != self.start_time_ticks {
                return Err(
                    "capability broker peer start time does not match the prepared route".into(),
                );
            }
            let path = PathBuf::from(format!("/proc/{peer_pid}/exe"));
            let executable = File::open(&path).map_err(|error| {
                format!(
                    "cannot open capability broker peer executable {}: {error}",
                    path.display()
                )
            })?;
            let metadata = executable.metadata().map_err(|error| {
                format!(
                    "cannot inspect capability broker peer executable {}: {error}",
                    path.display()
                )
            })?;
            let final_start = process_start_time_ticks(peer_pid)?;
            if final_start != initial_start {
                return Err("capability broker peer PID was reused while authenticating".into());
            }
            if LinuxObjectIdentityV3::from_linux_stat(
                metadata.dev(),
                metadata.ino(),
                metadata.mode(),
            ) != self.object_identity()
            {
                return Err("capability broker peer executable does not match the prepared object and bytes".into());
            }
            Ok(())
        }

        fn authenticate_client(self, stream: &UnixStream) -> Result<ProcessIdentityV1, String> {
            let credentials = rustix::net::sockopt::socket_peercred(stream)
                .map_err(|error| format!("cannot inspect capability broker client: {error}"))?;
            let client_pid = u32::try_from(credentials.pid.as_raw_nonzero().get())
                .map_err(|_| "capability broker client PID is negative".to_owned())?;
            if credentials.uid.as_raw() != self.uid {
                return Err("capability broker client uid does not match the broker".into());
            }
            let initial_start = process_start_time_ticks(client_pid)?;
            let (executable, metadata) = pin_process_executable(client_pid)?;
            if process_start_time_ticks(client_pid)? != initial_start {
                return Err("capability broker client PID was reused while authenticating".into());
            }
            if LinuxObjectIdentityV3::from_linux_stat(
                metadata.dev(),
                metadata.ino(),
                metadata.mode(),
            ) != self.object_identity()
                || executable.sha256() != &self.executable_sha256
            {
                return Err(
                    "capability broker client is not the exact pinned cargo-fe2o3 object and bytes"
                        .into(),
                );
            }
            ProcessIdentityV1::observe(client_pid)
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct BrokerRouteV3 {
        endpoint: String,
        secret: [u8; SECRET_BYTES],
        binding: CapabilityBindingV3,
        peer: BrokerPeerIdentityV2,
    }

    impl BrokerRouteV3 {
        fn encode(&self) -> String {
            let prefix = if self.binding.native_profile_identity.is_some() {
                ROUTE_PREFIX_V4
            } else {
                ROUTE_PREFIX
            };
            let mut route = format!(
                "{prefix}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{:x}:{:x}:{:x}:{}",
                self.endpoint,
                hex(&self.secret),
                self.binding.profile.route_name(),
                self.binding
                    .config_identity
                    .map(|identity| hex(&identity))
                    .unwrap_or_else(|| "-".to_owned()),
                if self.binding.protected_compiler_closure_v2 {
                    "v2"
                } else {
                    "-"
                },
                hex(&self.binding.compiler_closure_sha256),
                hex(&self.binding.rustc_executable_sha256),
                hex(&self.binding.retained_object_binding_sha256),
                self.peer.uid,
                self.peer.pid,
                self.peer.start_time_ticks,
                self.peer.device,
                self.peer.inode,
                self.peer.mode,
                hex(&self.peer.executable_sha256),
            );
            if let Some(identity) = self.binding.native_profile_identity {
                route.push_str(":profile-v3:");
                route.push_str(&hex(&identity));
            }
            route
        }

        fn parse(value: &str) -> Result<Self, String> {
            Self::parse_for(value, false)
        }

        fn parse_v4(value: &str) -> Result<Self, String> {
            Self::parse_for(value, true)
        }

        fn parse_for(value: &str, native: bool) -> Result<Self, String> {
            let fields = value.split(':').collect::<Vec<_>>();
            let (count, prefix) = if native {
                (18, ROUTE_PREFIX_V4)
            } else {
                (16, ROUTE_PREFIX)
            };
            if fields.len() != count || fields[0] != prefix {
                return Err(
                    "capability broker route is not the exact selected transport family".into(),
                );
            }
            let endpoint = fields[1].to_owned();
            endpoint_address(&endpoint)?;
            let secret = decode_fixed_hex(fields[2], "broker secret")?;
            let profile = CapabilityProfileV1::parse_route_name(fields[3])
                .ok_or_else(|| "capability broker route has an unknown profile".to_owned())?;
            let config_identity = if fields[4] == "-" {
                None
            } else {
                Some(decode_fixed_hex(fields[4], "config identity")?)
            };
            let protected_compiler_closure_v2 = match fields[5] {
                "-" => false,
                "v2" => true,
                _ => return Err("capability broker route has an unknown closure schema".into()),
            };
            let compiler_closure_sha256 = decode_fixed_hex(fields[6], "compiler closure digest")?;
            let rustc_executable_sha256 = decode_fixed_hex(fields[7], "rustc executable digest")?;
            let retained_object_binding_sha256 =
                decode_fixed_hex(fields[8], "retained object binding digest")?;
            let mut binding = CapabilityBindingV3::new(
                profile,
                config_identity,
                compiler_closure_sha256,
                rustc_executable_sha256,
                retained_object_binding_sha256,
            )?;
            binding.protected_compiler_closure_v2 = protected_compiler_closure_v2;
            if native {
                if !protected_compiler_closure_v2
                    || fields[16] != "profile-v3"
                    || config_identity.is_none_or(|identity| identity == [0; 32])
                {
                    return Err(
                        "native broker route has mixed family or missing protected configuration"
                            .into(),
                    );
                }
                let identity = decode_fixed_hex(fields[17], "native profile identity")?;
                if identity == [0; 32] {
                    return Err("native broker route has zero profile identity".into());
                }
                binding.native_profile_identity = Some(identity);
            }
            let peer = BrokerPeerIdentityV2 {
                uid: u32::try_from(parse_canonical_decimal(fields[9], "peer uid", true)?)
                    .map_err(|_| "capability broker peer uid exceeds u32".to_owned())?,
                pid: u32::try_from(parse_canonical_decimal(fields[10], "peer pid", false)?)
                    .map_err(|_| "capability broker peer pid exceeds u32".to_owned())?,
                start_time_ticks: parse_canonical_decimal(fields[11], "peer start time", false)?,
                device: parse_canonical_hex(fields[12], "peer device")?,
                inode: parse_canonical_hex(fields[13], "peer inode")?,
                mode: u32::try_from(parse_canonical_hex(fields[14], "peer mode")?)
                    .map_err(|_| "capability broker peer mode exceeds u32".to_owned())?,
                executable_sha256: decode_fixed_hex(fields[15], "peer executable digest")?,
            };
            let route = Self {
                endpoint,
                secret,
                binding,
                peer,
            };
            if route.encode() != value {
                return Err("capability broker route is not canonically encoded".into());
            }
            Ok(route)
        }
    }

    pub(crate) struct CapabilityBroker {
        route: String,
        invocation_authorization: InvocationAuthorizationRegistryV1,
        source_isa_observer: Option<Arc<Mutex<SourceIsaObservationCollectorStateV1>>>,
        shutdown: Arc<BrokerShutdown>,
        worker: Option<JoinHandle<()>>,
    }

    #[derive(Default)]
    struct BrokerShutdownState {
        stopping: bool,
        next_connection_id: u64,
        active: BTreeMap<u64, Arc<UnixStream>>,
        active_authentications: usize,
    }

    struct BrokerShutdown {
        // This mutex is the shutdown/SCM_RIGHTS linearization point and owns the wakeup socket.
        state: Mutex<BrokerShutdownState>,
        authentication_available: Condvar,
        max_concurrent_authentications: usize,
        max_active_connections: usize,
        compiler_proof: Option<Arc<CompilerProofBrokerV1>>,
    }

    impl BrokerShutdown {
        fn new(max_active_connections: usize) -> Self {
            assert!(max_active_connections != 0);
            Self {
                state: Mutex::new(BrokerShutdownState::default()),
                authentication_available: Condvar::new(),
                max_concurrent_authentications: max_active_connections
                    .min(MAX_CONCURRENT_AUTHENTICATIONS),
                max_active_connections,
                compiler_proof: None,
            }
        }

        fn state(&self) -> MutexGuard<'_, BrokerShutdownState> {
            self.state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        }

        fn is_stopping(&self) -> bool {
            self.state().stopping
        }

        fn register(
            self: &Arc<Self>,
            stream: &Arc<UnixStream>,
            deadline: BrokerDeadline,
        ) -> io::Result<Option<ConnectionRegistryGuard>> {
            let mut state = self.state();
            if state.stopping {
                let _ = stream.shutdown(Shutdown::Both);
                return Ok(None);
            }
            if state.active.len() >= self.max_active_connections {
                let _ = stream.shutdown(Shutdown::Both);
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "capability broker is at active connection capacity",
                ));
            }
            if let Err(error) = deadline.require_remaining() {
                let _ = stream.shutdown(Shutdown::Both);
                return Err(error);
            }
            let connection_id = state.next_connection_id;
            state.next_connection_id =
                state.next_connection_id.checked_add(1).ok_or_else(|| {
                    io::Error::other("capability broker exhausted connection identifiers")
                })?;
            state.active.insert(connection_id, Arc::clone(stream));
            Ok(Some(ConnectionRegistryGuard {
                shutdown: Arc::clone(self),
                connection_id,
            }))
        }

        fn finish(&self, connection_id: u64) {
            self.state().active.remove(&connection_id);
        }

        fn begin_authentication(
            self: &Arc<Self>,
            deadline: BrokerDeadline,
        ) -> io::Result<AuthenticationRegistryGuard> {
            let mut state = self.state();
            while !state.stopping
                && state.active_authentications >= self.max_concurrent_authentications
            {
                let remaining = deadline.remaining()?;
                let (next_state, _) = self
                    .authentication_available
                    .wait_timeout(state, remaining)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                state = next_state;
            }
            if state.stopping {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "capability broker is shutting down",
                ));
            }
            deadline.require_remaining()?;
            state.active_authentications += 1;
            Ok(AuthenticationRegistryGuard {
                shutdown: Arc::clone(self),
            })
        }

        fn finish_authentication(&self) {
            let mut state = self.state();
            state.active_authentications = state
                .active_authentications
                .checked_sub(1)
                .expect("authentication registry guard must own an active slot");
            drop(state);
            self.authentication_available.notify_one();
        }

        fn begin(&self) {
            if let Some(proof) = &self.compiler_proof {
                proof.stop();
            }
            let mut state = self.state();
            state.stopping = true;
            for active in state.active.values() {
                let _ = active.shutdown(Shutdown::Both);
            }
            state.active.clear();
            drop(state);
            self.authentication_available.notify_all();
        }

        fn send_response(
            &self,
            stream: &UnixStream,
            response: &[u8],
            descriptors: &[BorrowedFd<'_>],
            deadline: BrokerDeadline,
        ) -> io::Result<()> {
            let state = self.state();
            if state.stopping {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "capability broker is shutting down",
                ));
            }
            deadline.require_remaining()?;
            let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(4))];
            let mut ancillary = SendAncillaryBuffer::new(&mut space);
            if !ancillary.push(SendAncillaryMessage::ScmRights(descriptors)) {
                return Err(io::Error::other("capability control buffer is too small"));
            }
            let sent = sendmsg(
                stream,
                &[IoSlice::new(response)],
                &mut ancillary,
                SendFlags::NOSIGNAL | SendFlags::DONTWAIT,
            )
            .map_err(io::Error::from)?;
            if sent != response.len() {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "capability broker response was truncated",
                ));
            }
            drop(state);
            Ok(())
        }
    }

    struct ConnectionRegistryGuard {
        shutdown: Arc<BrokerShutdown>,
        connection_id: u64,
    }

    impl Drop for ConnectionRegistryGuard {
        fn drop(&mut self) {
            self.shutdown.finish(self.connection_id);
        }
    }

    struct AuthenticationRegistryGuard {
        shutdown: Arc<BrokerShutdown>,
    }

    impl Drop for AuthenticationRegistryGuard {
        fn drop(&mut self) {
            self.shutdown.finish_authentication();
        }
    }

    impl CapabilityBroker {
        pub(crate) fn start(
            session: BuildSession,
            binding: CapabilityBindingV3,
            backend: &PinnedCodegenBackend,
            artifact: &PinnedDirectory,
            pinned_cargo_image: &PinnedExecutable,
        ) -> Result<Self, String> {
            Self::start_with_limits(
                session,
                binding,
                backend,
                artifact,
                pinned_cargo_image,
                PRODUCTION_BROKER_LIMITS,
            )
        }

        pub(crate) fn start_protected(
            session: BuildSession,
            binding: CapabilityBindingV3,
            compiler_closure: fe2o3_build_authority::CompilerClosureV2,
            compiler_execution_profile: &CompilerExecutionClientProfileCapabilityV1,
            backend: &PinnedCodegenBackend,
            artifact: &PinnedDirectory,
            pinned_cargo_image: &PinnedExecutable,
        ) -> Result<Self, String> {
            Self::start_with_compiler_capabilities(
                session,
                binding,
                BrokerCompilerCapabilities::protected(
                    compiler_closure,
                    compiler_execution_profile,
                )?,
                backend,
                artifact,
                pinned_cargo_image,
                None,
                PRODUCTION_BROKER_LIMITS,
            )
        }

        // Keep each retained authority and observer policy explicit at this security boundary.
        #[allow(clippy::too_many_arguments)]
        pub(crate) fn start_protected_with_source_isa_observer(
            session: BuildSession,
            binding: CapabilityBindingV3,
            compiler_closure: fe2o3_build_authority::CompilerClosureV2,
            compiler_execution_profile: &CompilerExecutionClientProfileCapabilityV1,
            backend: &PinnedCodegenBackend,
            artifact: &PinnedDirectory,
            pinned_cargo_image: &PinnedExecutable,
            observer_policy: &ProductionSourceIsaObserverPolicyV1,
        ) -> Result<Self, String> {
            Self::start_with_compiler_capabilities(
                session,
                binding,
                BrokerCompilerCapabilities::protected(
                    compiler_closure,
                    compiler_execution_profile,
                )?,
                backend,
                artifact,
                pinned_cargo_image,
                Some(observer_policy),
                PRODUCTION_BROKER_LIMITS,
            )
        }

        /// Starts only from the fresh capability retained by a V4 release. The
        /// shared engine checks the exact profile identity before thread transfer.
        #[allow(clippy::too_many_arguments)]
        pub(crate) fn start_protected_v4(
            session: BuildSession,
            binding: CapabilityBindingV3,
            compiler_closure: fe2o3_build_authority::CompilerClosureV2,
            compiler_execution_profile: &FundedClientProfileV3,
            backend: &PinnedCodegenBackend,
            artifact: &PinnedDirectory,
            pinned_cargo_image: &PinnedExecutable,
            observer_policy: Option<&ProductionSourceIsaObserverPolicyV1>,
        ) -> Result<Self, String> {
            Self::start_with_compiler_capabilities(
                session,
                binding,
                BrokerCompilerCapabilities {
                    closure: Some(compiler_closure),
                    execution_profile: Some(BrokerProfileRef::V3(compiler_execution_profile)),
                    proof: None,
                },
                backend,
                artifact,
                pinned_cargo_image,
                observer_policy,
                PRODUCTION_BROKER_LIMITS,
            )
        }

        fn start_with_limits(
            session: BuildSession,
            binding: CapabilityBindingV3,
            backend: &PinnedCodegenBackend,
            artifact: &PinnedDirectory,
            pinned_cargo_image: &PinnedExecutable,
            limits: BrokerLimits,
        ) -> Result<Self, String> {
            Self::start_with_compiler_capabilities(
                session,
                binding,
                BrokerCompilerCapabilities::ordinary(),
                backend,
                artifact,
                pinned_cargo_image,
                None,
                limits,
            )
        }

        // Keep each retained authority, policy, and test-injected limit explicit.
        #[allow(clippy::too_many_arguments)]
        fn start_with_compiler_capabilities(
            session: BuildSession,
            binding: CapabilityBindingV3,
            compiler: BrokerCompilerCapabilities<'_>,
            backend: &PinnedCodegenBackend,
            artifact: &PinnedDirectory,
            pinned_cargo_image: &PinnedExecutable,
            observer_policy: Option<&ProductionSourceIsaObserverPolicyV1>,
            limits: BrokerLimits,
        ) -> Result<Self, String> {
            if limits.max_active_connections == 0
                || limits.authentication_timeout.is_zero()
                || limits.invocation_frame_timeout.is_zero()
                || limits.invocation_lifetime.is_zero()
            {
                return Err("capability broker limits must be nonzero".to_owned());
            }
            if binding.requires_compiler_closure_v2() != compiler.closure.is_some()
                || compiler.closure.is_some() != compiler.execution_profile.is_some()
                || matches!(compiler.execution_profile, Some(BrokerProfileRef::V1(_)))
                    != compiler.proof.is_some()
            {
                return Err(
                    "capability binding and protected compiler capability presence differ"
                        .to_owned(),
                );
            }
            match (binding.native_profile_identity, compiler.execution_profile) {
                (None, None | Some(BrokerProfileRef::V1(_))) => {}
                (Some(identity), Some(BrokerProfileRef::V3(profile)))
                    if identity == *profile.profile().identity().as_bytes() => {}
                _ => {
                    return Err(
                        "capability binding and retained profile family/identity differ".to_owned(),
                    );
                }
            }
            let source_isa_observer = observer_policy
                .map(|policy| BrokerSourceIsaObserverV1::from_policy(policy, session))
                .transpose()?;
            if let Some(observer) = &source_isa_observer
                && (!binding.requires_compiler_closure_v2()
                    || binding.config_identity != Some(observer.config_identity))
            {
                return Err(
                    "source/ISA observer policy requires the exact protected V2 config binding"
                        .to_owned(),
                );
            }
            let compiler_closure = compiler
                .closure
                .map(|closure| {
                    if closure.identity_sha256() != binding.compiler_closure_sha256()
                        || closure.rustc_executable_sha256() != binding.rustc_executable_sha256()
                        || closure.codegen_backend_sha256() != *backend.sha256()
                        || closure.cargo_executable_sha256() != *pinned_cargo_image.sha256()
                    {
                        return Err(
                            "compiler-closure descriptor differs from broker-retained images"
                                .to_owned(),
                        );
                    }
                    CompilerClosureCapabilityV1::create(closure)
                })
                .transpose()?;
            let compiler_execution_profile = compiler
                .execution_profile
                .map(|profile| match profile {
                    BrokerProfileRef::V1(profile) => {
                        profile.revalidate()?;
                        CompilerExecutionClientProfileCapabilityV1::from_file(
                            profile.try_clone_for_transfer()?,
                        )
                        .map(RetainedBrokerProfile::V1)
                    }
                    BrokerProfileRef::V3(profile) => {
                        profile.try_clone_retained().map(RetainedBrokerProfile::V3)
                    }
                })
                .transpose()?;
            let endpoint = random_endpoint().map_err(|error| {
                format!("failed to allocate capability broker endpoint: {error}")
            })?;
            let address = endpoint_address(&endpoint)?;
            let listener = UnixListener::bind_addr(&address)
                .map_err(|error| format!("failed to bind capability broker: {error}"))?;
            listener
                .set_nonblocking(true)
                .map_err(|error| format!("failed to configure capability broker: {error}"))?;
            let backend = backend
                .try_clone_for_transfer()
                .map_err(|error| format!("failed to retain broker backend: {error}"))?;
            let artifact = artifact
                .try_clone_for_transfer()
                .map_err(|error| format!("failed to retain broker artifact directory: {error}"))?;
            let executable = BrokerPeerIdentityV2::current().map_err(|error| {
                format!("failed to identify capability broker executable: {error}")
            })?;
            let secret = random_bytes()
                .map_err(|error| format!("failed to allocate capability broker secret: {error}"))?;
            let route = BrokerRouteV3 {
                endpoint,
                secret,
                binding,
                peer: executable,
            }
            .encode();
            let mut shutdown = BrokerShutdown::new(limits.max_active_connections);
            shutdown.compiler_proof = compiler.proof.clone();
            let shutdown = Arc::new(shutdown);
            let invocation_authorization = InvocationAuthorizationRegistryV1::new();
            let worker_invocation_authorization = invocation_authorization.clone();
            let worker_shutdown = Arc::clone(&shutdown);
            let returned_source_isa_observer = source_isa_observer
                .as_ref()
                .map(|observer| Arc::clone(&observer.collector));
            let worker = thread::Builder::new()
                .name("fe2o3-capability-broker".to_string())
                .spawn(move || {
                    BrokerServer {
                        listener,
                        session,
                        binding,
                        secret,
                        executable,
                        backend,
                        artifact,
                        compiler_closure,
                        compiler_execution_profile,
                        compiler_proof: compiler.proof,
                        source_isa_observer,
                        authentication_timeout: limits.authentication_timeout,
                        invocation_frame_timeout: limits.invocation_frame_timeout,
                        invocation_lifetime: limits.invocation_lifetime,
                        invocation_authorization: worker_invocation_authorization,
                        shutdown: worker_shutdown,
                    }
                    .serve();
                })
                .map_err(|error| format!("failed to start capability broker: {error}"))?;
            Ok(Self {
                route,
                invocation_authorization,
                source_isa_observer: returned_source_isa_observer,
                shutdown,
                worker: Some(worker),
            })
        }

        pub(crate) fn route(&self) -> &str {
            &self.route
        }

        pub(crate) fn invocation_authorization(&self) -> InvocationAuthorizationRegistryV1 {
            self.invocation_authorization.clone()
        }

        pub(crate) fn finish_source_isa_observations(
            mut self,
        ) -> Result<CompletedSourceIsaObservationsV1, String> {
            let collector = self.source_isa_observer.take().ok_or_else(|| {
                "capability broker has no source/ISA observer collector".to_owned()
            })?;
            self.shutdown.begin();
            let worker_panicked = self
                .worker
                .take()
                .is_some_and(|worker| worker.join().is_err());
            let collector = Arc::try_unwrap(collector).map_err(|_| {
                "source/ISA observer collector still has a live broker owner".to_owned()
            })?;
            let mut collector = collector
                .into_inner()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if worker_panicked {
                collector.fail(SourceIsaObservationTransportFailureV1::BrokerWorkerPanic);
            }
            Ok(collector.finish())
        }
    }

    impl Drop for CapabilityBroker {
        fn drop(&mut self) {
            self.shutdown.begin();
            if let Some(worker) = self.worker.take() {
                let _ = worker.join();
            }
        }
    }

    pub(crate) struct BrokeredCapabilities {
        pub(crate) backend: PinnedCodegenBackend,
        pub(crate) artifact: PinnedDirectory,
        pub(crate) compiler_closure: Option<CompilerClosureCapabilityV1>,
        pub(crate) compiler_execution_profile: Option<CompilerExecutionClientProfileCapabilityV1>,
        compiler_execution_profile_v3: Option<FundedClientProfileV3>,
        authenticated_binding: Option<CapabilityBindingV3>,
        pub(crate) invocation_authority: Option<BrokeredInvocationAuthorityV1>,
    }

    pub(crate) struct BrokeredInvocationAuthorityV1 {
        stream: UnixStream,
        profile_account: Option<ClientProfileAccountV3>,
        proof: Option<ProofPreparationContext>,
    }

    struct ProofPreparationContext {
        session: BuildSession,
        closure: fe2o3_build_authority::CompilerClosureV2,
        peer: BrokerPeerIdentityV2,
        secret: [u8; 32],
    }

    pub(crate) struct SourceIsaObservationSinkV1 {
        stream: UnixStream,
        _profile_account: Option<ClientProfileAccountV3>,
        config_identity: [u8; 32],
        unit_identity: [u8; 32],
        attempt: BuildAttempt,
    }

    impl BrokeredInvocationAuthorityV1 {
        fn from_authenticated_stream_with_account(
            stream: UnixStream,
            profile_account: Option<ClientProfileAccountV3>,
            proof: Option<ProofPreparationContext>,
        ) -> Result<Self, String> {
            let normalized = rustix::io::fcntl_dupfd_cloexec(&stream, RECEIVED_DESCRIPTOR_FLOOR)
                .map_err(|error| {
                    format!("failed to retain authenticated invocation capability: {error}")
                })?;
            Ok(Self {
                stream: UnixStream::from(normalized),
                profile_account,
                proof,
            })
        }

        pub(crate) fn prepare_compiler_proof(
            &mut self,
            attempt: BuildAttempt,
        ) -> Result<PendingCompilerProofDelegationV1, String> {
            let context = self.proof.take().ok_or_else(|| {
                "compiler proof preparation is unavailable or already consumed".to_owned()
            })?;
            let result = (|| -> io::Result<_> {
                if attempt.session() != context.session || attempt.session() == BuildSession::DIRECT
                {
                    return Err(io::Error::other(
                        "compiler proof attempt is outside authenticated session",
                    ));
                }
                context
                    .peer
                    .authenticate(&self.stream)
                    .map_err(io::Error::other)?;
                let deadline = BrokerDeadline::new(Instant::now(), BROKER_CLIENT_RESPONSE_TIMEOUT);
                let request = ProofPreparationRequest {
                    attempt,
                    challenge: random_bytes()?,
                    authentication: [0; 32],
                };
                let mut stream = &self.stream;
                stream.set_write_timeout(Some(deadline.remaining()?))?;
                stream.write_all(&request.encode(&context.secret))?;
                stream.set_read_timeout(Some(deadline.remaining()?))?;
                let mut response = [0; PROOF_RESPONSE_BYTES];
                let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2))];
                let mut ancillary = RecvAncillaryBuffer::new(&mut space);
                let message = recvmsg(
                    stream,
                    &mut [IoSliceMut::new(&mut response)],
                    &mut ancillary,
                    RecvFlags::CMSG_CLOEXEC,
                )?;
                let mut descriptors = Vec::new();
                let mut rights_messages = 0;
                let mut unexpected = false;
                for item in ancillary.drain() {
                    match item {
                        RecvAncillaryMessage::ScmRights(rights) => {
                            rights_messages += 1;
                            descriptors.extend(rights);
                        }
                        _ => unexpected = true,
                    }
                }
                let session: [u8; 32] = response[8..40].try_into().unwrap();
                if message.bytes != PROOF_RESPONSE_BYTES
                    || !(message.flags - ReturnFlags::CMSG_CLOEXEC).is_empty()
                    || unexpected
                    || rights_messages != 1
                    || descriptors.len() != 2
                    || session == [0; 32]
                    || response != request.response(&context.secret, session)
                {
                    return Err(io::Error::other(
                        "malformed authenticated compiler-proof preparation response",
                    ));
                }
                context
                    .peer
                    .authenticate(stream)
                    .map_err(io::Error::other)?;
                deadline.require_remaining()?;
                let descriptors: [OwnedFd; 2] = descriptors
                    .try_into()
                    .map_err(|_| io::Error::other("compiler-proof descriptor roster"))?;
                PendingCompilerProofDelegationV1::from_authenticated_transfer(
                    session,
                    descriptors,
                    &context.closure,
                )
            })();
            if result.is_err() {
                let _ = self.stream.shutdown(Shutdown::Both);
            }
            result.map_err(|error| format!("compiler proof preparation failed: {error}"))
        }

        pub(crate) fn release(self) -> Result<(), String> {
            self.exchange(
                BrokeredInvocationCapabilityRequestV1::Release,
                BROKERED_INVOCATION_PREPARED_V1,
            )
        }

        pub(crate) fn release_with_source_isa_observer(
            self,
            config_identity: [u8; 32],
            unit_identity: [u8; 32],
            attempt: BuildAttempt,
        ) -> Result<SourceIsaObservationSinkV1, String> {
            let request = BrokeredInvocationCapabilityRequestV2::release_with_source_isa_observer(
                config_identity,
                unit_identity,
                attempt,
            )
            .map_err(|error| error.to_string())?;
            let mut stream = self.stream;
            stream.write_all(&request.encode()).map_err(|error| {
                format!("failed to write observer invocation capability: {error}")
            })?;
            let mut response = [0; BROKERED_SOURCE_ISA_PREPARED_V1.len()];
            stream.read_exact(&mut response).map_err(|error| {
                format!("failed to read observer invocation preparation: {error}")
            })?;
            if &response != BROKERED_SOURCE_ISA_PREPARED_V1 {
                return Err(
                    "observer invocation capability returned a malformed response".to_owned(),
                );
            }
            Ok(SourceIsaObservationSinkV1 {
                stream,
                _profile_account: self.profile_account,
                config_identity,
                unit_identity,
                attempt,
            })
        }

        fn exchange(
            &self,
            request: BrokeredInvocationCapabilityRequestV1,
            expected: &[u8; 16],
        ) -> Result<(), String> {
            let mut stream = &self.stream;
            stream
                .write_all(&request.encode())
                .map_err(|error| format!("failed to write invocation capability: {error}"))?;
            let mut response = [0_u8; 16];
            stream
                .read_exact(&mut response)
                .map_err(|error| format!("failed to read invocation capability: {error}"))?;
            if &response != expected {
                return Err("invocation capability returned a malformed response".to_owned());
            }
            Ok(())
        }
    }

    impl SourceIsaObservationSinkV1 {
        pub(crate) fn submit_census(
            mut self,
            frame: &SourceIsaObservationFrameV1,
            census: &crate::production_census_v91::Census,
        ) -> Result<(), String> {
            census.check_frame(frame)?;
            self.submit_inner(frame, Some(&census.encode()?))
        }

        pub(crate) fn submit(mut self, frame: &SourceIsaObservationFrameV1) -> Result<(), String> {
            self.submit_inner(frame, None)
        }

        pub(crate) fn submit_characteristic(
            mut self,
            frame: &SourceIsaObservationFrameV1,
            characteristic: &SourceIsaCharacteristicCollectionV1,
        ) -> Result<(), String> {
            let encoded = characteristic
                .encode_canonical()
                .map_err(|error| format!("failed to encode Source/ISA characteristics: {error}"))?;
            self.submit_inner(frame, Some(&encoded))
        }

        fn submit_inner(
            &mut self,
            frame: &SourceIsaObservationFrameV1,
            characteristic: Option<&[u8]>,
        ) -> Result<(), String> {
            let context = frame.context();
            let observation_attempt =
                crate::source_isa_observation::inert_source_isa_attempt_v1(self.attempt)
                    .map_err(|error| format!("invalid source/ISA observation attempt: {error}"))?;
            if context.config() != self.config_identity
                || context.unit() != self.unit_identity
                || context.attempt() != observation_attempt
            {
                return Err(
                    "source/ISA observation frame differs from its authenticated sink".to_owned(),
                );
            }
            self.stream.write_all(&frame.encode()).map_err(|error| {
                format!("failed to write source/ISA observation frame: {error}")
            })?;
            if let Some(characteristic) = characteristic {
                let length = u64::try_from(characteristic.len())
                    .map_err(|_| "Source/ISA characteristic length exceeds u64".to_owned())?;
                self.stream
                    .write_all(&length.to_le_bytes())
                    .map_err(|error| {
                        format!("failed to write Source/ISA characteristic length: {error}")
                    })?;
                self.stream.write_all(characteristic).map_err(|error| {
                    format!("failed to write Source/ISA characteristics: {error}")
                })?;
            }
            self.stream
                .shutdown(Shutdown::Write)
                .map_err(|error| format!("failed to close source/ISA observation frame: {error}"))
        }
    }

    pub(crate) fn receive(
        session: BuildSession,
        binding: CapabilityBindingV3,
    ) -> Result<BrokeredCapabilities, String> {
        if binding.native_profile_identity.is_some() {
            return Err("legacy broker reader rejects a native profile binding".to_owned());
        }
        let encoded_route = std::env::var(CAPABILITY_BROKER_ENV)
            .map_err(|_| format!("managed rustc invocation is missing {CAPABILITY_BROKER_ENV}"))?;
        let route = BrokerRouteV3::parse(&encoded_route)?;
        receive_from(&route, session, binding)
    }

    pub(crate) fn receive_v4(
        session: BuildSession,
        binding: CapabilityBindingV3,
        account: ClientProfileAccountV3,
    ) -> Result<BrokeredCapabilities, String> {
        if binding.native_profile_identity.is_none() {
            return Err("native broker reader rejects a legacy profile binding".to_owned());
        }
        let encoded_route = std::env::var(CAPABILITY_BROKER_ENV)
            .map_err(|_| format!("managed rustc invocation is missing {CAPABILITY_BROKER_ENV}"))?;
        let route = BrokerRouteV3::parse_v4(&encoded_route)?;
        receive_from_with_account(&route, session, binding, Some(account))
    }

    fn receive_from(
        route: &BrokerRouteV3,
        session: BuildSession,
        binding: CapabilityBindingV3,
    ) -> Result<BrokeredCapabilities, String> {
        receive_from_with_account(route, session, binding, None)
    }

    fn receive_from_with_account(
        route: &BrokerRouteV3,
        session: BuildSession,
        binding: CapabilityBindingV3,
        account: Option<ClientProfileAccountV3>,
    ) -> Result<BrokeredCapabilities, String> {
        if binding.native_profile_identity.is_some() != account.is_some() {
            return Err("broker reader account/profile family differs".to_owned());
        }
        if route.binding != binding {
            return Err(
                "capability broker route does not match the requested profile/config/rustc identity"
                    .into(),
            );
        }
        if let Some(account) = &account {
            // Prepay all incoming descriptor owners before recvmsg can create them.
            // This is monotone: a malformed frame never refunds a live descriptor.
            let storage = fe2o3_compiler_closure_capability::CompilerExecutionClientProfileCapabilityV3::FILE_STORAGE
                .checked_mul(binding.descriptor_count() + 2).ok_or("native broker descriptor quota overflow")?;
            account
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .with_budget(|budget| budget.reserve_storage(storage))
                .map_err(|error| error.to_string())?;
        }
        route.peer.require_current_executable()?;
        let address = endpoint_address(&route.endpoint)?;
        let mut stream = UnixStream::connect_addr(&address)
            .map_err(|error| format!("failed to connect to capability broker: {error}"))?;
        stream
            .set_read_timeout(Some(BROKER_CLIENT_RESPONSE_TIMEOUT))
            .map_err(|error| format!("failed to bound capability broker read: {error}"))?;
        route.peer.authenticate(&stream)?;
        let challenge = random_bytes()
            .map_err(|error| format!("failed to allocate broker client challenge: {error}"))?;
        let request = request_bytes(session, binding, challenge, &route.secret);
        let request_auth: [u8; REQUEST_AUTH_BYTES] = request[request.len() - REQUEST_AUTH_BYTES..]
            .try_into()
            .expect("request authentication field has a fixed size");
        stream
            .write_all(&request)
            .map_err(|error| format!("failed to authenticate to capability broker: {error}"))?;

        let descriptors =
            receive_response(&stream, binding, &route.secret, challenge, request_auth)?;
        let invocation_account = account.as_ref().map(Arc::clone);
        let mut capabilities =
            decode_received_descriptors_with_account(descriptors, binding, account)?;
        let proof = if capabilities.compiler_execution_profile.is_some() {
            Some(ProofPreparationContext {
                session,
                closure: capabilities
                    .compiler_closure
                    .as_ref()
                    .ok_or_else(|| {
                        "compiler proof profile requires its authenticated closure".to_owned()
                    })?
                    .closure(),
                peer: route.peer,
                secret: route.secret,
            })
        } else {
            None
        };
        capabilities.invocation_authority = Some(
            BrokeredInvocationAuthorityV1::from_authenticated_stream_with_account(
                stream,
                invocation_account,
                proof,
            )?,
        );
        capabilities.authenticated_binding = Some(binding);
        Ok(capabilities)
    }

    fn receive_response(
        stream: &UnixStream,
        binding: CapabilityBindingV3,
        secret: &[u8; SECRET_BYTES],
        challenge: [u8; CHALLENGE_BYTES],
        request_auth: [u8; REQUEST_AUTH_BYTES],
    ) -> Result<Vec<OwnedFd>, String> {
        let mut response = [0_u8; RESPONSE_BYTES];
        let mut iov = [IoSliceMut::new(&mut response)];
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(4))];
        let mut ancillary = RecvAncillaryBuffer::new(&mut space);
        let message = recvmsg(stream, &mut iov, &mut ancillary, RecvFlags::CMSG_CLOEXEC)
            .map_err(|error| format!("failed to receive brokered capabilities: {error}"))?;
        let mut descriptors = Vec::new();
        let mut unexpected = false;
        for control in ancillary.drain() {
            match control {
                RecvAncillaryMessage::ScmRights(received) => descriptors.extend(received),
                _ => unexpected = true,
            }
        }
        if message.flags.contains(ReturnFlags::CTRUNC) {
            return Err("capability broker descriptor response was truncated".to_string());
        }
        let expected_response = response_bytes_for(binding, secret, challenge, request_auth);
        if message.bytes != RESPONSE_BYTES || response != expected_response {
            return Err("capability broker returned a malformed response".to_string());
        }
        if unexpected || descriptors.len() != binding.descriptor_count() {
            return Err(
                "capability broker returned an unexpected control message or descriptor count"
                    .to_owned(),
            );
        }
        Ok(descriptors)
    }

    #[cfg(test)]
    fn decode_received_descriptors(
        descriptors: Vec<OwnedFd>,
        binding: CapabilityBindingV3,
    ) -> Result<BrokeredCapabilities, String> {
        decode_received_descriptors_with_account(descriptors, binding, None)
    }

    fn decode_received_descriptors_with_account(
        mut descriptors: Vec<OwnedFd>,
        binding: CapabilityBindingV3,
        account: Option<ClientProfileAccountV3>,
    ) -> Result<BrokeredCapabilities, String> {
        if binding.native_profile_identity.is_some() != account.is_some() {
            return Err("descriptor decoder and profile family differ".to_owned());
        }
        if descriptors.len() != binding.descriptor_count() {
            return Err(format!(
                "capability broker returned {} descriptors instead of {} for the {} profile",
                descriptors.len(),
                binding.descriptor_count(),
                binding.profile.name(),
            ));
        }
        // Each roster position names a different retained object. Reject aliases
        // before any positional decoder can consume a duplicate of another role.
        for (index, descriptor) in descriptors.iter().enumerate() {
            let object = rustix::fs::fstat(descriptor).map_err(|error| error.to_string())?;
            for previous in &descriptors[..index] {
                let other = rustix::fs::fstat(previous).map_err(|error| error.to_string())?;
                if (object.st_dev, object.st_ino) == (other.st_dev, other.st_ino) {
                    return Err(
                        "capability broker returned duplicate descriptor objects".to_owned()
                    );
                }
            }
        }
        let mut compiler_execution_profile_v3 = None;
        let compiler_execution_profile = if binding.requires_compiler_closure_v2() {
            let descriptor = descriptors
                .pop()
                .expect("compiler-execution profile descriptor count checked");
            if let Some(identity) = binding.native_profile_identity {
                let profile = FundedClientProfileV3::from_received_with(
                    account.expect("native account presence checked"),
                    || {
                        normalize_received_descriptor(
                            descriptor,
                            "compiler-execution client profile V3",
                        )
                    },
                )?;
                if *profile.profile().identity().as_bytes() != identity {
                    return Err(
                        "brokered native profile differs from authenticated profile identity"
                            .to_owned(),
                    );
                }
                compiler_execution_profile_v3 = Some(profile);
                None
            } else {
                let image =
                    normalize_received_descriptor(descriptor, "compiler-execution client profile")?;
                Some(CompilerExecutionClientProfileCapabilityV1::from_file(
                    image,
                )?)
            }
        } else {
            None
        };
        let compiler_closure = if binding.requires_compiler_closure_v2() {
            let image = normalize_received_descriptor(
                descriptors
                    .pop()
                    .expect("compiler-closure descriptor count checked"),
                "compiler closure",
            )?;
            let capability = CompilerClosureCapabilityV1::from_file(image)?;
            if capability.closure().identity_sha256() != binding.compiler_closure_sha256()
                || capability.closure().rustc_executable_sha256()
                    != binding.rustc_executable_sha256()
            {
                return Err(
                    "brokered compiler closure differs from the authenticated binding".to_owned(),
                );
            }
            Some(capability)
        } else {
            None
        };
        let artifact = normalize_received_descriptor(
            descriptors.pop().expect("descriptor count checked"),
            "artifact directory",
        )?;
        let artifact =
            PinnedDirectory::from_transferred_file(artifact, "artifact output directory")
                .map_err(|error| format!("invalid brokered artifact directory: {error}"))?;
        let backend = normalize_received_descriptor(
            descriptors.pop().expect("descriptor count checked"),
            "codegen backend",
        )?;
        let backend = PinnedCodegenBackend::from_transferred_file(backend)
            .map_err(|error| format!("invalid brokered codegen backend: {error}"))?;
        Ok(BrokeredCapabilities {
            backend,
            artifact,
            compiler_closure,
            compiler_execution_profile,
            compiler_execution_profile_v3,
            authenticated_binding: None,
            invocation_authority: None,
        })
    }

    fn normalize_received_descriptor(descriptor: OwnedFd, kind: &str) -> Result<File, String> {
        let normalized = rustix::io::fcntl_dupfd_cloexec(&descriptor, RECEIVED_DESCRIPTOR_FLOOR)
            .map_err(|error| format!("failed to normalize brokered {kind} descriptor: {error}"))?;
        let file = File::from(normalized);
        if file.as_raw_fd() < RECEIVED_DESCRIPTOR_FLOOR {
            return Err(format!(
                "brokered {kind} descriptor overlaps reserved child descriptors"
            ));
        }
        Ok(file)
    }

    struct BrokerServer {
        listener: UnixListener,
        session: BuildSession,
        binding: CapabilityBindingV3,
        secret: [u8; SECRET_BYTES],
        executable: BrokerPeerIdentityV2,
        backend: File,
        artifact: File,
        compiler_closure: Option<CompilerClosureCapabilityV1>,
        compiler_execution_profile: Option<RetainedBrokerProfile>,
        compiler_proof: Option<Arc<CompilerProofBrokerV1>>,
        source_isa_observer: Option<BrokerSourceIsaObserverV1>,
        authentication_timeout: Duration,
        invocation_frame_timeout: Duration,
        invocation_lifetime: Duration,
        invocation_authorization: InvocationAuthorizationRegistryV1,
        shutdown: Arc<BrokerShutdown>,
    }

    impl BrokerServer {
        fn serve(self) {
            thread::scope(|scope| {
                while !self.shutdown.is_stopping() {
                    match self.listener.accept() {
                        Ok((stream, _)) => {
                            let stream = Arc::new(stream);
                            let accepted_at = Instant::now();
                            let deadline =
                                BrokerDeadline::new(accepted_at, self.authentication_timeout);
                            match self.shutdown.register(&stream, deadline) {
                                Ok(Some(registry_guard)) => {
                                    let server = &self;
                                    let worker = move || {
                                        let _registry_guard = registry_guard;
                                        let _ =
                                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                                                || server.serve_one(&stream, deadline),
                                            ));
                                    };
                                    let spawned =
                                        thread::Builder::new().spawn_scoped(scope, worker);
                                    if spawned.is_err() {
                                        continue;
                                    }
                                }
                                Ok(None) => break,
                                Err(_) => continue,
                            }
                        }
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(2));
                        }
                        Err(_) => {
                            // Descriptor exhaustion and transient listener failures must reject
                            // work without permanently disabling the broker.
                            thread::sleep(Duration::from_millis(2));
                        }
                    }
                }
            });
        }

        fn serve_one(&self, stream: &UnixStream, deadline: BrokerDeadline) -> io::Result<()> {
            deadline.require_remaining()?;
            let authentication = self.shutdown.begin_authentication(deadline)?;
            let deadline = BrokerDeadline::new(Instant::now(), self.authentication_timeout);
            let client = self
                .executable
                .authenticate_client(stream)
                .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error))?;
            drop(authentication);
            let original_wrapper = if self.compiler_proof.is_some() {
                Some(
                    self.invocation_authorization
                        .consume_original(client)
                        .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error))?,
                )
            } else {
                self.invocation_authorization
                    .consume(client)
                    .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error))?;
                None
            };
            deadline.require_remaining()?;
            let mut request = vec![0_u8; self.binding.request_bytes()];
            deadline.read_exact(stream, &mut request)?;
            let (challenge, request_auth) =
                authenticate_request(&request, self.session, self.binding, &self.secret)?;
            deadline.require_remaining()?;
            let mut descriptors = vec![self.backend.as_fd(), self.artifact.as_fd()];
            let compiler_closure = self
                .compiler_closure
                .as_ref()
                .map(CompilerClosureCapabilityV1::try_clone_for_transfer)
                .transpose()
                .map_err(io::Error::other)?;
            if let Some(compiler_closure) = &compiler_closure {
                descriptors.push(compiler_closure.as_fd());
            }
            let compiler_execution_profile = self
                .compiler_execution_profile
                .as_ref()
                .map(RetainedBrokerProfile::try_clone_for_transfer)
                .transpose()
                .map_err(io::Error::other)?;
            if let Some(compiler_execution_profile) = &compiler_execution_profile {
                descriptors.push(compiler_execution_profile.file().as_fd());
            }
            let response = response_bytes_for(self.binding, &self.secret, challenge, request_auth);
            self.shutdown
                .send_response(stream, &response, &descriptors, deadline)?;
            self.serve_invocation_authority(stream, client, original_wrapper)
        }

        fn serve_invocation_authority(
            &self,
            stream: &UnixStream,
            client: ProcessIdentityV1,
            original_wrapper: Option<File>,
        ) -> io::Result<()> {
            let liveness = InvocationLiveness {
                client,
                started_at: Instant::now(),
                frame_timeout: self.invocation_frame_timeout,
                lifetime: self.invocation_lifetime,
            };
            let request = read_invocation_request(liveness, stream)?;
            let BrokerInvocationRequest::PrepareProof(preparation) = request else {
                return self.serve_invocation_request(stream, liveness, request, None);
            };
            preparation.require_authenticated(&self.secret, self.session)?;
            if self
                .executable
                .authenticate_client(stream)
                .map_err(io::Error::other)?
                != client
            {
                return Err(io::Error::other("compiler-proof wrapper identity changed"));
            }
            let credentials = rustix::net::sockopt::socket_peercred(stream)?;
            let broker = self
                .compiler_proof
                .as_ref()
                .ok_or_else(|| io::Error::other("compiler proof is unavailable for this broker"))?;
            let original_wrapper = original_wrapper.ok_or_else(|| {
                io::Error::other("compiler proof requires the original wrapper exec permit")
            })?;
            let deadline = Instant::now() + self.invocation_frame_timeout;
            let (server, bootstrap) = broker
                .prepare(
                    original_wrapper.into(),
                    (
                        client.pid(),
                        credentials.uid.as_raw(),
                        credentials.gid.as_raw(),
                    ),
                    client.start_time_ticks(),
                    preparation.attempt,
                    deadline,
                )
                .map_err(io::Error::other)?;
            let response = preparation.response(&self.secret, bootstrap.session());
            let descriptors = bootstrap.into_descriptors();
            self.shutdown.send_response(
                stream,
                &response,
                &[descriptors[0].as_fd(), descriptors[1].as_fd()],
                BrokerDeadline::new(Instant::now(), self.invocation_frame_timeout),
            )?;
            drop(descriptors);

            thread::scope(|scope| {
                // Cancel before the scoped join on both errors and unwinding.
                let cancellation = server.cancellation();
                let worker = thread::Builder::new().spawn_scoped(scope, move || {
                    server.serve(liveness.started_at + liveness.lifetime)
                })?;
                let result = read_invocation_request(liveness, stream).and_then(|request| {
                    self.serve_invocation_request(
                        stream,
                        liveness,
                        request,
                        Some(preparation.attempt),
                    )
                });
                if result.is_err() {
                    cancellation.cancel();
                }
                // EOF or compiler exit ends the proof channel normally. Proof failures are
                // delivered to the compiler, which must revalidate before publication.
                let _ = worker
                    .join()
                    .map_err(|_| io::Error::other("compiler proof worker panicked"))?;
                result
            })
        }

        fn serve_invocation_request(
            &self,
            stream: &UnixStream,
            liveness: InvocationLiveness,
            request: BrokerInvocationRequest,
            proof_attempt: Option<BuildAttempt>,
        ) -> io::Result<()> {
            if let BrokerInvocationRequest::V2(request) = request {
                if proof_attempt.is_some_and(|attempt| attempt != request.attempt()) {
                    return Err(io::Error::other(
                        "source/ISA attempt differs from compiler proof",
                    ));
                }
                return self.receive_source_isa_observation(stream, liveness, request);
            }
            let BrokerInvocationRequest::V1(request) = request else {
                return Err(io::Error::other("duplicate compiler proof preparation"));
            };
            let claim = match request {
                BrokeredInvocationCapabilityRequestV1::Release => {
                    let mut stream = stream;
                    stream.write_all(BROKERED_INVOCATION_PREPARED_V1)?;
                    return Ok(());
                }
                BrokeredInvocationCapabilityRequestV1::Prepare(claim)
                    if claim.attempt().session() == self.session
                        && proof_attempt.is_none_or(|attempt| attempt == claim.attempt()) =>
                {
                    claim
                }
                BrokeredInvocationCapabilityRequestV1::Prepare(_)
                | BrokeredInvocationCapabilityRequestV1::Consume(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "invocation capability preparation is not bound to this build session",
                    ));
                }
            };
            let mut stream = stream;
            stream.write_all(BROKERED_INVOCATION_PREPARED_V1)?;

            let mut encoded = [0_u8; BROKERED_INVOCATION_REQUEST_BYTES_V1];
            liveness.read_frame(stream, &mut encoded)?;
            if BrokeredInvocationCapabilityRequestV1::decode(&encoded)
                != Ok(BrokeredInvocationCapabilityRequestV1::Consume(claim))
            {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "rustc did not consume the exact wrapper-prepared invocation claim",
                ));
            }
            stream.write_all(BROKERED_INVOCATION_ADMITTED_V1)
        }

        fn receive_source_isa_observation(
            &self,
            stream: &UnixStream,
            liveness: InvocationLiveness,
            request: BrokeredInvocationCapabilityRequestV2,
        ) -> io::Result<()> {
            let observer = self.source_isa_observer.as_ref().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "source/ISA observer request is unavailable for this broker",
                )
            })?;
            let result = (|| {
                prepare_source_isa_observer_request(observer, self.session, stream, request)?;
                let mut encoded = [0; SOURCE_ISA_OBSERVATION_FRAME_BYTES_V1];
                liveness.read_frame(stream, &mut encoded)?;
                let characteristic = match observer.kind {
                    ProductionSourceIsaObservationKindV1::Summary => None,
                    ProductionSourceIsaObservationKindV1::Characteristic
                    | ProductionSourceIsaObservationKindV1::ProductionCensusV91 => {
                        let mut encoded_length = [0; 8];
                        liveness.read_frame(stream, &mut encoded_length)?;
                        let length = usize::try_from(u64::from_le_bytes(encoded_length))
                            .ok()
                            .filter(|length| {
                                *length != 0
                                    && *length <= if observer.kind == ProductionSourceIsaObservationKindV1::ProductionCensusV91 {
                                        crate::production_census_v91::MAX_BYTES
                                    } else { MAX_SOURCE_ISA_CHARACTERISTIC_COLLECTION_BYTES_V1 }
                            })
                            .ok_or_else(|| {
                                io::Error::new(
                                    io::ErrorKind::InvalidData,
                                    "Source/ISA characteristic length exceeds its bound",
                                )
                            })?;
                        let mut body = Vec::new();
                        body.try_reserve_exact(length).map_err(|_| {
                            io::Error::new(
                                io::ErrorKind::OutOfMemory,
                                "cannot allocate bounded Source/ISA characteristic body",
                            )
                        })?;
                        body.resize(length, 0);
                        liveness.read_frame(stream, &mut body)?;
                        Some(body)
                    }
                };
                liveness.require_eof(stream)?;
                let frame = SourceIsaObservationFrameV1::decode(&encoded)
                    .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error))?;
                validate_source_isa_observer_frame_binding(self.session, request, &frame)?;
                if let Some(body) = characteristic.as_ref() {
                    if observer.kind == ProductionSourceIsaObservationKindV1::ProductionCensusV91 {
                        crate::production_census_v91::Census::decode(body)
                            .and_then(|row| row.check_frame(&frame))
                            .map_err(io::Error::other)?;
                    } else {
                        validate_source_isa_characteristic_binding(&frame, body)?;
                    }
                }
                observer.collect(frame, characteristic)
            })();
            if result.is_err() {
                observer.fail(SourceIsaObservationTransportFailureV1::RejectedFrame);
            }
            result
        }
    }

    fn prepare_source_isa_observer_request(
        observer: &BrokerSourceIsaObserverV1,
        broker_session: BuildSession,
        stream: &UnixStream,
        request: BrokeredInvocationCapabilityRequestV2,
    ) -> io::Result<()> {
        if !observer.accepts(request) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "source/ISA observer request is not bound to the exact configured unit",
            ));
        }
        if request.attempt().session() == BuildSession::DIRECT
            || request.attempt().session() != broker_session
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "source/ISA observer request is not bound to this build session",
            ));
        }
        let mut stream = stream;
        stream.write_all(BROKERED_SOURCE_ISA_PREPARED_V1)
    }

    fn validate_source_isa_observer_frame_binding(
        broker_session: BuildSession,
        request: BrokeredInvocationCapabilityRequestV2,
        frame: &SourceIsaObservationFrameV1,
    ) -> io::Result<()> {
        let context = frame.context();
        let observation_attempt =
            crate::source_isa_observation::inert_source_isa_attempt_v1(request.attempt())
                .map_err(|error| io::Error::other(error.to_string()))?;
        if request.attempt().session() != broker_session
            || context.config() != request.config_identity()
            || context.unit() != request.unit_identity()
            || context.attempt() != observation_attempt
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "source/ISA observation frame differs from its exact authenticated request",
            ));
        }
        Ok(())
    }

    fn validate_source_isa_characteristic_binding(
        frame: &SourceIsaObservationFrameV1,
        encoded: &[u8],
    ) -> io::Result<()> {
        let inert = InertSourceIsaCharacteristicCollectionV1::decode_canonical(encoded)
            .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error))?;
        let binding = inert.claimed_binding();
        let SourceIsaObservationOutcomeV1::Admitted(summary) = frame.outcome() else {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Source/ISA characteristic body requires an admitted summary",
            ));
        };
        let structural = summary.structural();
        let target_matches = matches!(
            (structural.target_profile(), binding.target_profile()),
            (
                SourceIsaObservationTargetProfileV1::Gfx942,
                SourceIsaCharacteristicTargetProfileV1::Gfx942
            ) | (
                SourceIsaObservationTargetProfileV1::Gfx950,
                SourceIsaCharacteristicTargetProfileV1::Gfx950
            )
        );
        let summary_artifact = summary.artifact();
        let characteristic_artifact = binding.artifact();
        let summary_target_kir = structural.target_kir();
        let characteristic_target_kir = binding.target_kir();
        if !target_matches
            || structural.identity() != binding.structural_identity()
            || summary.correlation() != binding.correlation_identity()
            || summary_artifact.sha256() != characteristic_artifact.sha256()
            || summary_artifact.byte_len() != characteristic_artifact.byte_len()
            || summary_target_kir.sha256() != characteristic_target_kir.sha256()
            || summary_target_kir.byte_len() != characteristic_target_kir.byte_len()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Source/ISA characteristic body differs from its authenticated summary",
            ));
        }
        Ok(())
    }

    enum BrokerInvocationRequest {
        V1(BrokeredInvocationCapabilityRequestV1),
        V2(BrokeredInvocationCapabilityRequestV2),
        PrepareProof(ProofPreparationRequest),
    }

    fn read_invocation_request(
        liveness: InvocationLiveness,
        stream: &UnixStream,
    ) -> io::Result<BrokerInvocationRequest> {
        let mut magic = [0; 8];
        liveness.read_frame(stream, &mut magic)?;
        if &magic == PROOF_PREPARE_MAGIC {
            let mut encoded = [0; PROOF_PREPARE_BYTES];
            encoded[..8].copy_from_slice(&magic);
            liveness.read_frame(stream, &mut encoded[8..])?;
            return ProofPreparationRequest::decode(&encoded)
                .map(BrokerInvocationRequest::PrepareProof);
        }
        if &magic == BROKERED_INVOCATION_REQUEST_MAGIC_V1 {
            let mut encoded = [0; BROKERED_INVOCATION_REQUEST_BYTES_V1];
            encoded[..magic.len()].copy_from_slice(&magic);
            liveness.read_frame(stream, &mut encoded[magic.len()..])?;
            return BrokeredInvocationCapabilityRequestV1::decode(&encoded)
                .map(BrokerInvocationRequest::V1)
                .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error));
        }
        if &magic == BROKERED_INVOCATION_REQUEST_MAGIC_V2 {
            let mut encoded = [0; BROKERED_INVOCATION_REQUEST_BYTES_V2];
            encoded[..magic.len()].copy_from_slice(&magic);
            liveness.read_frame(stream, &mut encoded[magic.len()..])?;
            return BrokeredInvocationCapabilityRequestV2::decode(&encoded)
                .map(BrokerInvocationRequest::V2)
                .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error));
        }
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "brokered invocation request has an unknown version",
        ))
    }

    #[derive(Clone, Copy)]
    struct InvocationLiveness {
        client: ProcessIdentityV1,
        started_at: Instant,
        frame_timeout: Duration,
        lifetime: Duration,
    }

    impl InvocationLiveness {
        fn read_frame(self, stream: &UnixStream, buffer: &mut [u8]) -> io::Result<()> {
            let mut stream = stream;
            let mut offset = 0;
            let mut frame_deadline = None;
            while offset < buffer.len() {
                let now = Instant::now();
                if now.duration_since(self.started_at) >= self.lifetime {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "invocation capability exceeded its total lifetime",
                    ));
                }
                let deadline = frame_deadline
                    .get_or_insert_with(|| now + self.frame_timeout)
                    .to_owned();
                stream.set_read_timeout(Some(
                    deadline
                        .checked_duration_since(now)
                        .filter(|remaining| !remaining.is_zero())
                        .unwrap_or(Duration::from_millis(1)),
                ))?;
                match stream.read(&mut buffer[offset..]) {
                    Ok(0) => {
                        return Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "invocation capability frame ended early",
                        ));
                    }
                    Ok(read) => offset += read,
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                        ) && offset == 0 =>
                    {
                        self.client.require_current().map_err(|error| {
                            io::Error::new(io::ErrorKind::PermissionDenied, error)
                        })?;
                        frame_deadline = None;
                    }
                    Err(error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                        ) =>
                    {
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "invocation capability frame deadline expired",
                        ));
                    }
                    Err(error) => return Err(error),
                }
            }
            Ok(())
        }

        fn require_eof(self, stream: &UnixStream) -> io::Result<()> {
            let mut trailing = [0; 1];
            let mut stream = stream;
            loop {
                let remaining = self
                    .lifetime
                    .checked_sub(self.started_at.elapsed())
                    .filter(|remaining| !remaining.is_zero())
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::TimedOut,
                            "invocation capability exceeded its total lifetime",
                        )
                    })?;
                stream.set_read_timeout(Some(self.frame_timeout.min(remaining)))?;
                match stream.read(&mut trailing) {
                    Ok(0) => return Ok(()),
                    Ok(_) => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "source/ISA observation contains trailing bytes",
                        ));
                    }
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error) => return Err(error),
                }
            }
        }
    }

    #[derive(Clone, Copy)]
    struct BrokerDeadline {
        expires_at: Instant,
    }

    impl BrokerDeadline {
        fn new(accepted_at: Instant, timeout: Duration) -> Self {
            Self {
                expires_at: accepted_at + timeout,
            }
        }

        fn remaining(self) -> io::Result<Duration> {
            self.expires_at
                .checked_duration_since(Instant::now())
                .filter(|remaining| !remaining.is_zero())
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::TimedOut,
                        "capability broker connection deadline expired",
                    )
                })
        }

        fn require_remaining(self) -> io::Result<()> {
            self.remaining().map(|_| ())
        }

        fn read_exact(self, stream: &UnixStream, mut buffer: &mut [u8]) -> io::Result<()> {
            let mut stream = stream;
            while !buffer.is_empty() {
                stream.set_read_timeout(Some(self.remaining()?))?;
                match stream.read(buffer) {
                    Ok(0) => {
                        return Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "capability broker request ended early",
                        ));
                    }
                    Ok(read) => buffer = &mut buffer[read..],
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                        ) =>
                    {
                        self.require_remaining()?;
                    }
                    Err(error) => return Err(error),
                }
            }
            Ok(())
        }
    }

    fn endpoint_address(endpoint: &str) -> Result<SocketAddr, String> {
        if endpoint.len() != ENDPOINT_HEX_BYTES
            || endpoint
                .bytes()
                .any(|byte| !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase())
        {
            return Err("capability broker endpoint is not canonical lowercase hexadecimal".into());
        }
        SocketAddr::from_abstract_name(format!("fe2o3-cap-v2-{endpoint}").as_bytes())
            .map_err(|error| format!("invalid capability broker endpoint: {error}"))
    }

    fn random_endpoint() -> io::Result<String> {
        let bytes = random_bytes()?;
        Ok(hex(&bytes))
    }

    fn random_bytes() -> io::Result<[u8; ENDPOINT_BYTES]> {
        let mut bytes = [0_u8; ENDPOINT_BYTES];
        File::open("/dev/urandom")?.read_exact(&mut bytes)?;
        Ok(bytes)
    }

    fn keyed_digest(domain: &[u8], secret: &[u8; SECRET_BYTES], fields: &[&[u8]]) -> [u8; 32] {
        let mut digest = Sha256::new();
        digest.update((domain.len() as u64).to_le_bytes());
        digest.update(domain);
        digest.update((secret.len() as u64).to_le_bytes());
        digest.update(secret);
        for field in fields {
            digest.update((field.len() as u64).to_le_bytes());
            digest.update(field);
        }
        digest.finalize().into()
    }

    fn process_start_time_ticks(pid: u32) -> Result<u64, String> {
        let path = PathBuf::from(format!("/proc/{pid}/stat"));
        let bytes = fs::read(&path)
            .map_err(|error| format!("cannot read broker process {}: {error}", path.display()))?;
        if bytes.is_empty() || bytes.len() > MAX_PROC_STAT_BYTES {
            return Err(format!(
                "broker process {} must contain 1 through {MAX_PROC_STAT_BYTES} bytes",
                path.display()
            ));
        }
        let close = bytes
            .iter()
            .rposition(|byte| *byte == b')')
            .ok_or_else(|| "broker process stat has no command terminator".to_owned())?;
        let recorded_pid = bytes[..close]
            .split(|byte| *byte == b' ')
            .next()
            .and_then(|value| std::str::from_utf8(value).ok())
            .and_then(|value| value.parse::<u32>().ok());
        if recorded_pid != Some(pid) {
            return Err("broker process stat PID does not match its proc entry".into());
        }
        bytes[close + 1..]
            .split(u8::is_ascii_whitespace)
            .filter(|field| !field.is_empty())
            .nth(19)
            .and_then(|value| std::str::from_utf8(value).ok())
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|value| *value != 0)
            .ok_or_else(|| "broker process stat has no valid start-time field".to_owned())
    }

    fn pin_process_executable(pid: u32) -> Result<(PinnedExecutable, fs::Metadata), String> {
        let path = PathBuf::from(format!("/proc/{pid}/exe"));
        for attempt in 0..EXECUTABLE_PIN_ATTEMPTS {
            let file = File::open(&path).map_err(|error| {
                format!(
                    "cannot open broker process executable {}: {error}",
                    path.display()
                )
            })?;
            let metadata = file.metadata().map_err(|error| {
                format!(
                    "cannot inspect broker process executable {}: {error}",
                    path.display()
                )
            })?;
            match PinnedExecutable::from_transferred_file(file, path.clone()) {
                Ok(pinned) => {
                    if pinned.object_identity()
                        != LinuxObjectIdentityV3::from_linux_stat(
                            metadata.dev(),
                            metadata.ino(),
                            metadata.mode(),
                        )
                    {
                        return Err("broker process executable object changed while pinning".into());
                    }
                    return Ok((pinned, metadata));
                }
                Err(PinExecutableError::ChangedDuringRead { .. })
                    if attempt + 1 < EXECUTABLE_PIN_ATTEMPTS =>
                {
                    thread::yield_now();
                }
                Err(error) => {
                    return Err(format!(
                        "cannot pin broker process executable {}: {error}",
                        path.display()
                    ));
                }
            }
        }
        unreachable!("executable pin retries either return or report their final error")
    }

    fn decode_fixed_hex<const N: usize>(value: &str, label: &str) -> Result<[u8; N], String> {
        if value.len() != N * 2
            || value
                .bytes()
                .any(|byte| !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase())
        {
            return Err(format!(
                "capability broker {label} is not canonical lowercase hex"
            ));
        }
        let mut decoded = [0_u8; N];
        for (index, output) in decoded.iter_mut().enumerate() {
            let offset = index * 2;
            *output = u8::from_str_radix(&value[offset..offset + 2], 16)
                .map_err(|_| format!("capability broker {label} is invalid"))?;
        }
        Ok(decoded)
    }

    fn parse_canonical_decimal(value: &str, label: &str, allow_zero: bool) -> Result<u64, String> {
        let parsed = value
            .parse::<u64>()
            .map_err(|_| format!("capability broker {label} is not decimal"))?;
        if (!allow_zero && parsed == 0) || parsed.to_string() != value {
            return Err(format!("capability broker {label} is not canonical"));
        }
        Ok(parsed)
    }

    fn parse_canonical_hex(value: &str, label: &str) -> Result<u64, String> {
        let parsed = u64::from_str_radix(value, 16)
            .map_err(|_| format!("capability broker {label} is not hexadecimal"))?;
        if parsed == 0 || format!("{parsed:x}") != value {
            return Err(format!("capability broker {label} is not canonical"));
        }
        Ok(parsed)
    }

    fn hex(bytes: &[u8]) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut endpoint = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            endpoint.push(char::from(HEX[(byte >> 4) as usize]));
            endpoint.push(char::from(HEX[(byte & 0x0f) as usize]));
        }
        endpoint
    }

    #[cfg(test)]
    mod tests;
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub(crate) use platform::*;

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
mod unsupported {
    use fe2o3_artifact_transaction::{
        BrokeredInvocationCapabilityClaimV1, BuildAttempt, BuildSession,
    };

    use crate::authority_release::profile::{ClientProfileAccountV3, FundedClientProfileV3};
    use crate::build_config::ProductionSourceIsaObserverPolicyV1;
    use crate::cargo_invocation_boundary::InvocationAuthorizationRegistryV1;
    use crate::pinned_codegen_backend::PinnedCodegenBackend;
    use crate::pinned_executable::PinnedExecutable;
    use crate::project::PinnedDirectory;
    use fe2o3_compiler_closure_capability::{
        CompilerClosureCapabilityV1, CompilerExecutionClientProfileCapabilityV1,
    };
    use fe2o3_source_isa_observation::characteristic_v1::SourceIsaCharacteristicCollectionV1;
    use fe2o3_source_isa_observation::wire_v1::{
        SourceIsaObservationCollectionV1, SourceIsaObservationFrameV1,
    };

    pub(crate) const CAPABILITY_BROKER_ENV: &str = "FE2O3_CAPABILITY_BROKER_V1";

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) enum CompilerExecutionProfileFamily {
        LegacyV1,
        NativeV3,
    }

    pub(crate) fn broker_route_family_from_environment()
    -> Result<CompilerExecutionProfileFamily, String> {
        Err("Cargo capability transport requires Linux x86_64".to_owned())
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) enum CapabilityProfileV1 {
        Ordinary,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) struct CapabilityBindingV3;

    impl CapabilityBindingV3 {
        pub(crate) fn new_protected_v4(
            _profile: CapabilityProfileV1,
            _config_identity: Option<[u8; 32]>,
            _compiler_closure: fe2o3_build_authority::CompilerClosureV2,
            _retained_object_binding_sha256: [u8; 32],
            _native_profile_identity: [u8; 32],
        ) -> Result<Self, String> {
            Err("Cargo capability transport requires Linux x86_64".to_owned())
        }

        pub(crate) fn from_environment_for_client_v4(
            _profile: CapabilityProfileV1,
        ) -> Result<Self, String> {
            Err("Cargo capability transport requires Linux x86_64".to_owned())
        }
        pub(crate) fn new(
            _profile: CapabilityProfileV1,
            _config_identity: Option<[u8; 32]>,
            _compiler_closure_sha256: [u8; 32],
            _rustc_executable_sha256: [u8; 32],
            _retained_object_binding_sha256: [u8; 32],
        ) -> Result<Self, String> {
            Ok(Self)
        }

        pub(crate) fn from_environment_for_client(
            _profile: CapabilityProfileV1,
        ) -> Result<Self, String> {
            Err("Cargo capability transport requires Linux".to_owned())
        }

        pub(crate) const fn config_identity(self) -> Option<[u8; 32]> {
            None
        }

        pub(crate) fn new_protected(
            _profile: CapabilityProfileV1,
            _config_identity: Option<[u8; 32]>,
            _compiler_closure: fe2o3_build_authority::CompilerClosureV2,
            _retained_object_binding_sha256: [u8; 32],
        ) -> Result<Self, String> {
            Ok(Self)
        }

        pub(crate) const fn compiler_closure_sha256(self) -> [u8; 32] {
            [0; 32]
        }

        pub(crate) const fn requires_compiler_closure_v2(self) -> bool {
            false
        }

        pub(crate) const fn rustc_executable_sha256(self) -> [u8; 32] {
            [0; 32]
        }

        pub(crate) const fn retained_object_binding_sha256(self) -> [u8; 32] {
            [0; 32]
        }
    }

    pub(crate) struct CapabilityBroker;

    pub(crate) struct CompletedSourceIsaObservationsV1 {
        pub(crate) config: [u8; 32],
        pub(crate) summary: SourceIsaObservationCollectionV1,
        pub(crate) characteristic: Option<([u8; 32], Vec<u8>)>,
        pub(crate) census: Vec<([u8; 32], Vec<u8>)>,
    }

    impl CapabilityBroker {
        #[allow(clippy::too_many_arguments)]
        pub(crate) fn start_protected_v4(
            _session: BuildSession,
            _binding: CapabilityBindingV3,
            _compiler_closure: fe2o3_build_authority::CompilerClosureV2,
            _compiler_execution_profile: &FundedClientProfileV3,
            _backend: &PinnedCodegenBackend,
            _artifact: &PinnedDirectory,
            _pinned_cargo_image: &PinnedExecutable,
            _observer_policy: Option<&ProductionSourceIsaObserverPolicyV1>,
        ) -> Result<Self, String> {
            Err("Cargo capability transport requires Linux x86_64".to_owned())
        }
        pub(crate) fn start(
            _session: BuildSession,
            _binding: CapabilityBindingV3,
            _backend: &PinnedCodegenBackend,
            _artifact: &PinnedDirectory,
            _pinned_cargo_image: &PinnedExecutable,
        ) -> Result<Self, String> {
            Err("Cargo capability transport requires Linux".to_string())
        }

        pub(crate) fn start_protected(
            _session: BuildSession,
            _binding: CapabilityBindingV3,
            _compiler_closure: fe2o3_build_authority::CompilerClosureV2,
            _compiler_execution_profile: &CompilerExecutionClientProfileCapabilityV1,
            _backend: &PinnedCodegenBackend,
            _artifact: &PinnedDirectory,
            _pinned_cargo_image: &PinnedExecutable,
        ) -> Result<Self, String> {
            Err("Cargo capability transport requires Linux".to_string())
        }

        pub(crate) fn start_protected_with_source_isa_observer(
            _session: BuildSession,
            _binding: CapabilityBindingV3,
            _compiler_closure: fe2o3_build_authority::CompilerClosureV2,
            _compiler_execution_profile: &CompilerExecutionClientProfileCapabilityV1,
            _backend: &PinnedCodegenBackend,
            _artifact: &PinnedDirectory,
            _pinned_cargo_image: &PinnedExecutable,
            _observer_policy: &ProductionSourceIsaObserverPolicyV1,
        ) -> Result<Self, String> {
            Err("Cargo capability transport requires Linux".to_string())
        }

        pub(crate) fn route(&self) -> &str {
            ""
        }

        pub(crate) fn invocation_authorization(&self) -> InvocationAuthorizationRegistryV1 {
            InvocationAuthorizationRegistryV1::new()
        }

        pub(crate) fn finish_source_isa_observations(
            self,
        ) -> Result<CompletedSourceIsaObservationsV1, String> {
            Err("Cargo capability transport requires Linux".to_string())
        }
    }

    pub(crate) struct BrokeredInvocationAuthorityV1;

    impl BrokeredInvocationAuthorityV1 {
        pub(crate) fn release(self) -> Result<(), String> {
            Err("Cargo capability transport requires Linux".to_owned())
        }

        pub(crate) fn release_with_source_isa_observer(
            self,
            _config_identity: [u8; 32],
            _unit_identity: [u8; 32],
            _attempt: BuildAttempt,
        ) -> Result<SourceIsaObservationSinkV1, String> {
            Err("Cargo capability transport requires Linux".to_owned())
        }

        pub(crate) fn prepare(
            &self,
            _claim: BrokeredInvocationCapabilityClaimV1,
        ) -> Result<(), String> {
            Err("Cargo capability transport requires Linux".to_owned())
        }
    }

    pub(crate) struct SourceIsaObservationSinkV1;

    impl SourceIsaObservationSinkV1 {
        pub(crate) fn submit_census(
            self,
            _frame: &SourceIsaObservationFrameV1,
            _census: &crate::production_census_v91::Census,
        ) -> Result<(), String> {
            Err("Cargo capability transport requires Linux".to_owned())
        }
        pub(crate) fn submit(self, _frame: &SourceIsaObservationFrameV1) -> Result<(), String> {
            Err("Cargo capability transport requires Linux".to_owned())
        }

        pub(crate) fn submit_characteristic(
            self,
            _frame: &SourceIsaObservationFrameV1,
            _characteristic: &SourceIsaCharacteristicCollectionV1,
        ) -> Result<(), String> {
            Err("Cargo capability transport requires Linux".to_owned())
        }
    }

    pub(crate) struct BrokeredCapabilities {
        pub(crate) backend: PinnedCodegenBackend,
        pub(crate) artifact: PinnedDirectory,
        pub(crate) compiler_closure: Option<CompilerClosureCapabilityV1>,
        pub(crate) compiler_execution_profile: Option<CompilerExecutionClientProfileCapabilityV1>,
        pub(crate) invocation_authority: Option<BrokeredInvocationAuthorityV1>,
    }

    impl BrokeredCapabilities {
        pub(crate) fn authenticated_client_profile_v3_identity(&self) -> Option<[u8; 32]> {
            None
        }
        pub(crate) fn compiler_execution_profile_v3(
            &self,
        ) -> Result<&FundedClientProfileV3, String> {
            Err("Cargo capability transport requires Linux x86_64".to_owned())
        }
        pub(crate) fn take_compiler_execution_profile_v3(
            &mut self,
        ) -> Result<FundedClientProfileV3, String> {
            Err("Cargo capability transport requires Linux x86_64".to_owned())
        }
    }

    pub(crate) fn receive_v4(
        _session: BuildSession,
        _binding: CapabilityBindingV3,
        _account: ClientProfileAccountV3,
    ) -> Result<BrokeredCapabilities, String> {
        Err("Cargo capability transport requires Linux x86_64".to_owned())
    }

    pub(crate) fn receive(
        _session: BuildSession,
        _binding: CapabilityBindingV3,
    ) -> Result<BrokeredCapabilities, String> {
        Err("Cargo capability transport requires Linux".to_string())
    }
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
pub(crate) use unsupported::*;
