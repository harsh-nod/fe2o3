use crate::api::{ApiError, ExecutableApi, SymbolFacts};
use crate::environment::{AdapterCore, HsaRuntimeAdapterError, ReviewedHsaRuntimeAdapterV1};
use fe2o3_artifacts::PayloadDigest;
use fe2o3_host::{
    HsaCodeObjectLoadObservationV1, HsaDispatchObservationV1, HsaExecutableObjectIdentityV1,
    HsaKernelObjectIdentityV1, HsaKernelResolutionObservationV1, HsaLaunchGeometryV1,
    HsaUnloadObservationV1, ReviewedHsaExecutableLifecycleAdapterV1,
};
use sha2::{Digest, Sha256};

const HSA_SYMBOL_KIND_KERNEL: u32 = 1;

pub(crate) struct ExecutableState {
    pub(crate) bytes: Box<[u8]>,
    pub(crate) reader: u64,
    pub(crate) executable: u64,
    pub(crate) _loaded_code_object: u64,
    pub(crate) identity: HsaExecutableObjectIdentityV1,
}

/// Opaque private-handle ownership for one loaded HSA executable.
///
/// There is intentionally no public constructor or raw-handle accessor.
pub struct ReviewedHsaExecutableV1 {
    pub(crate) state: Option<ExecutableState>,
}

/// Opaque resolved kernel tied descriptively to its executable identity.
///
/// There is intentionally no public constructor or raw-handle accessor.
pub struct ReviewedHsaKernelV1 {
    pub(crate) symbol: u64,
    pub(crate) kernel_object: u64,
    pub(crate) executable_identity: HsaExecutableObjectIdentityV1,
    pub(crate) identity: HsaKernelObjectIdentityV1,
    pub(crate) kernarg_segment_size: u32,
    pub(crate) kernarg_segment_alignment: u32,
    pub(crate) group_segment_size: u32,
    pub(crate) private_segment_size: u32,
}

/// Linear ownership of a fixed set of distinct kernels from one executable.
///
/// The set borrows its executable so that safe Rust cannot unload the native
/// executable while any retained kernel remains accessible. Resolving a set
/// authenticates native identities only; it does not establish a typed kernarg
/// ABI for any kernel.
pub struct ReviewedHsaKernelSetV1<'executable, const N: usize> {
    _executable: &'executable ReviewedHsaExecutableV1,
    kernels: [ReviewedHsaKernelV1; N],
}

impl<const N: usize> ReviewedHsaKernelSetV1<'_, N> {
    /// Returns one retained kernel without transferring its linear ownership.
    pub fn get(&self, index: usize) -> Option<&ReviewedHsaKernelV1> {
        self.kernels.get(index)
    }

    /// Returns the number of retained kernels.
    pub const fn len(&self) -> usize {
        N
    }

    /// Returns whether this set has no kernels.
    pub const fn is_empty(&self) -> bool {
        N == 0
    }
}

impl Drop for ReviewedHsaKernelV1 {
    fn drop(&mut self) {
        // HSA symbols have no destroy operation. This explicit drop boundary
        // still makes the linear kernel token end before executable teardown.
    }
}

impl ReviewedHsaRuntimeAdapterV1 {
    /// Resolves and linearly retains distinct kernel symbols from one executable.
    ///
    /// # Safety
    ///
    /// `export_symbols` must identify kernels in the authenticated code object
    /// supplied when `executable` was loaded. This operation authenticates HSA
    /// symbol and executable identities, but callers must separately establish
    /// the exact reviewed kernarg ABI before dispatch.
    pub unsafe fn resolve_kernel_set<'executable, const N: usize>(
        &mut self,
        executable: &'executable ReviewedHsaExecutableV1,
        export_symbols: [&str; N],
    ) -> Result<
        (
            ReviewedHsaKernelSetV1<'executable, N>,
            [HsaKernelResolutionObservationV1; N],
        ),
        HsaRuntimeAdapterError,
    > {
        resolve_kernel_set(&mut self.core, executable, export_symbols)
    }
}

// SAFETY: construction measures and correlates one HIP/HSA physical device;
// the implementation below owns all native handles and validates every
// observation before returning it to the host authority state machine.
unsafe impl ReviewedHsaExecutableLifecycleAdapterV1 for ReviewedHsaRuntimeAdapterV1 {
    type Executable = ReviewedHsaExecutableV1;
    type Kernel = ReviewedHsaKernelV1;
    type Error = HsaRuntimeAdapterError;

    unsafe fn observe_environment(
        &mut self,
    ) -> Result<fe2o3_host::HsaEnvironmentObservationV1, Self::Error> {
        Ok(self.core.environment.clone())
    }

    unsafe fn load_executable(
        &mut self,
        bytes: &[u8],
        finalized_digest: PayloadDigest,
    ) -> Result<(Self::Executable, HsaCodeObjectLoadObservationV1), Self::Error> {
        load_executable(&mut self.core, bytes, finalized_digest)
    }

    unsafe fn resolve_kernel(
        &mut self,
        executable: &Self::Executable,
        export_symbol: &str,
    ) -> Result<(Self::Kernel, HsaKernelResolutionObservationV1), Self::Error> {
        resolve_kernel(&mut self.core, executable, export_symbol)
    }

    unsafe fn launch_and_wait(
        &mut self,
        executable: &Self::Executable,
        kernel: &Self::Kernel,
        geometry: HsaLaunchGeometryV1,
        kernarg: &mut [u8],
    ) -> Result<HsaDispatchObservationV1, Self::Error> {
        crate::dispatch::launch_and_wait(
            &mut self.core,
            &mut self.pending_dispatch,
            executable,
            kernel,
            geometry,
            kernarg,
        )
    }

    unsafe fn unload_executable(
        &mut self,
        executable: Self::Executable,
    ) -> Result<HsaUnloadObservationV1, Self::Error> {
        unload_executable(&mut self.core, executable)
    }
}

fn load_executable<A: ExecutableApi>(
    core: &mut AdapterCore<A>,
    bytes: &[u8],
    finalized_digest: PayloadDigest,
) -> Result<(ReviewedHsaExecutableV1, HsaCodeObjectLoadObservationV1), HsaRuntimeAdapterError> {
    finalized_digest
        .verify(bytes)
        .map_err(|_| HsaRuntimeAdapterError::InvalidExecutableObservation("finalized digest"))?;
    let generation = core.next_identity;
    let next_generation =
        generation
            .checked_add(1)
            .ok_or(HsaRuntimeAdapterError::InvalidExecutableObservation(
                "executable generation overflow",
            ))?;
    let owned = Vec::from(bytes).into_boxed_slice();
    let reader = core
        .api
        .reader_create(&owned)
        .map_err(HsaRuntimeAdapterError::api)?;
    let executable = match core.api.executable_create(core.profile) {
        Ok(executable) => executable,
        Err(primary) => {
            return Err(cleanup_reader_failure(
                &mut core.api,
                reader,
                primary,
                owned,
            ));
        }
    };
    let loaded_code_object = match core.api.executable_load(executable, core.agent, reader) {
        Ok(loaded) => loaded,
        Err(primary) => {
            return Err(cleanup_executable_failure(
                &mut core.api,
                executable,
                reader,
                primary,
                owned,
            ));
        }
    };
    if let Err(primary) = core.api.executable_freeze(executable) {
        return Err(cleanup_executable_failure(
            &mut core.api,
            executable,
            reader,
            primary,
            owned,
        ));
    }
    let identity = match executable_identity(
        core.environment.runtime().instance(),
        core.agent,
        reader,
        executable,
        loaded_code_object,
        generation,
        finalized_digest,
    ) {
        Ok(identity) => identity,
        Err(error) => {
            let primary = ApiError {
                operation: "derive unique HSA executable identity",
                status: -1,
            };
            let cleanup =
                cleanup_executable_failure(&mut core.api, executable, reader, primary, owned);
            return Err(match cleanup {
                HsaRuntimeAdapterError::RuntimeCall { .. } => error,
                ambiguous => ambiguous,
            });
        }
    };
    core.next_identity = next_generation;
    let byte_len = u64::try_from(owned.len()).map_err(|_| {
        HsaRuntimeAdapterError::InvalidExecutableObservation("code-object byte length")
    })?;
    let observation = HsaCodeObjectLoadObservationV1::new(
        finalized_digest,
        byte_len,
        core.environment.runtime().instance(),
        core.agent,
        identity,
    );
    Ok((
        ReviewedHsaExecutableV1 {
            state: Some(ExecutableState {
                bytes: owned,
                reader,
                executable,
                _loaded_code_object: loaded_code_object,
                identity,
            }),
        },
        observation,
    ))
}

fn resolve_kernel<A: ExecutableApi>(
    core: &mut AdapterCore<A>,
    executable: &ReviewedHsaExecutableV1,
    export_symbol: &str,
) -> Result<(ReviewedHsaKernelV1, HsaKernelResolutionObservationV1), HsaRuntimeAdapterError> {
    if export_symbol.is_empty() || export_symbol.as_bytes().contains(&0) {
        return Err(HsaRuntimeAdapterError::InvalidExecutableObservation(
            "export symbol",
        ));
    }
    let runtime_symbol = format!("{export_symbol}.kd");
    let state =
        executable
            .state
            .as_ref()
            .ok_or(HsaRuntimeAdapterError::InvalidExecutableObservation(
                "consumed executable",
            ))?;
    let symbol = core
        .api
        .resolve_symbol(state.executable, core.agent, &runtime_symbol)
        .map_err(HsaRuntimeAdapterError::api)?;
    validate_symbol(&symbol, &runtime_symbol)?;
    let identity = kernel_identity(state.identity, &symbol, export_symbol)?;
    let observation = HsaKernelResolutionObservationV1::new(
        state.identity,
        identity,
        export_symbol,
        u64::from(symbol.kernarg_size),
        u64::from(symbol.kernarg_alignment),
    )
    .map_err(|_| HsaRuntimeAdapterError::InvalidExecutableObservation("kernel ABI"))?;
    Ok((
        ReviewedHsaKernelV1 {
            symbol: symbol.handle,
            kernel_object: symbol.kernel_object,
            executable_identity: state.identity,
            identity,
            kernarg_segment_size: symbol.kernarg_size,
            kernarg_segment_alignment: symbol.kernarg_alignment,
            group_segment_size: symbol.group_segment_size,
            private_segment_size: symbol.private_segment_size,
        },
        observation,
    ))
}

fn resolve_kernel_set<'executable, A: ExecutableApi, const N: usize>(
    core: &mut AdapterCore<A>,
    executable: &'executable ReviewedHsaExecutableV1,
    export_symbols: [&str; N],
) -> Result<
    (
        ReviewedHsaKernelSetV1<'executable, N>,
        [HsaKernelResolutionObservationV1; N],
    ),
    HsaRuntimeAdapterError,
> {
    if N == 0 {
        return Err(HsaRuntimeAdapterError::InvalidExecutableObservation(
            "empty kernel set",
        ));
    }
    for (index, name) in export_symbols.iter().enumerate() {
        if export_symbols[..index].contains(name) {
            return Err(HsaRuntimeAdapterError::InvalidExecutableObservation(
                "duplicate export symbol",
            ));
        }
    }

    let mut kernels = Vec::with_capacity(N);
    let mut observations = Vec::with_capacity(N);
    for export_symbol in export_symbols {
        let (kernel, observation) = resolve_kernel(core, executable, export_symbol)?;
        if kernels.iter().any(|retained: &ReviewedHsaKernelV1| {
            retained.symbol == kernel.symbol
                || retained.kernel_object == kernel.kernel_object
                || retained.identity == kernel.identity
        }) {
            return Err(HsaRuntimeAdapterError::InvalidExecutableObservation(
                "distinct kernel symbol identities",
            ));
        }
        kernels.push(kernel);
        observations.push(observation);
    }

    Ok((
        ReviewedHsaKernelSetV1 {
            _executable: executable,
            kernels: exact_array(kernels),
        },
        exact_array(observations),
    ))
}

fn exact_array<T, const N: usize>(values: Vec<T>) -> [T; N] {
    values
        .try_into()
        .unwrap_or_else(|_| unreachable!("collected exactly N values"))
}

fn unload_executable<A: ExecutableApi>(
    core: &mut AdapterCore<A>,
    mut executable: ReviewedHsaExecutableV1,
) -> Result<HsaUnloadObservationV1, HsaRuntimeAdapterError> {
    let state =
        executable
            .state
            .take()
            .ok_or(HsaRuntimeAdapterError::InvalidExecutableObservation(
                "duplicate executable unload",
            ))?;
    let ExecutableState {
        bytes,
        reader,
        executable,
        _loaded_code_object: _,
        identity,
    } = state;
    if core.api.executable_destroy(executable).is_err() {
        retain_and_abort(bytes);
    }
    if core.api.reader_destroy(reader).is_err() {
        retain_and_abort(bytes);
    }
    Ok(HsaUnloadObservationV1::new(
        identity,
        core.environment.runtime().instance(),
        core.agent,
        true,
    ))
}

fn validate_symbol(
    symbol: &SymbolFacts,
    expected_name: &str,
) -> Result<(), HsaRuntimeAdapterError> {
    for (valid, field) in [
        (symbol.handle != 0, "symbol handle"),
        (symbol.kernel_object != 0, "kernel object"),
        (symbol.kind == HSA_SYMBOL_KIND_KERNEL, "symbol kind"),
        (symbol.name == expected_name, "exact symbol name"),
        (symbol.kernarg_size != 0, "kernarg size"),
        (
            symbol.kernarg_alignment != 0 && symbol.kernarg_alignment.is_power_of_two(),
            "kernarg alignment",
        ),
    ] {
        if !valid {
            return Err(HsaRuntimeAdapterError::InvalidExecutableObservation(field));
        }
    }
    Ok(())
}

fn cleanup_reader_failure<A: ExecutableApi>(
    api: &mut A,
    reader: u64,
    primary: ApiError,
    bytes: Box<[u8]>,
) -> HsaRuntimeAdapterError {
    match api.reader_destroy(reader) {
        Ok(()) => HsaRuntimeAdapterError::api(primary),
        Err(_) => retain_and_abort(bytes),
    }
}

fn cleanup_executable_failure<A: ExecutableApi>(
    api: &mut A,
    executable: u64,
    reader: u64,
    primary: ApiError,
    bytes: Box<[u8]>,
) -> HsaRuntimeAdapterError {
    if api.executable_destroy(executable).is_err() {
        retain_and_abort(bytes);
    }
    if api.reader_destroy(reader).is_err() {
        retain_and_abort(bytes);
    }
    HsaRuntimeAdapterError::api(primary)
}

fn retain_and_abort<T>(authority: T) -> ! {
    let _retained = std::mem::ManuallyDrop::new(authority);
    std::process::abort()
}

#[allow(clippy::too_many_arguments)]
fn executable_identity(
    runtime_instance: [u8; 16],
    agent: u64,
    reader: u64,
    executable: u64,
    loaded: u64,
    generation: u64,
    digest: PayloadDigest,
) -> Result<HsaExecutableObjectIdentityV1, HsaRuntimeAdapterError> {
    let mut hasher = Sha256::new();
    hasher.update(b"fe2o3-hsa-executable-object-v1\0");
    hasher.update(runtime_instance);
    hasher.update(agent.to_le_bytes());
    hasher.update(reader.to_le_bytes());
    hasher.update(executable.to_le_bytes());
    hasher.update(loaded.to_le_bytes());
    hasher.update(generation.to_le_bytes());
    hasher.update(digest.bytes().as_bytes());
    HsaExecutableObjectIdentityV1::new(hasher.finalize().into())
        .map_err(|_| HsaRuntimeAdapterError::InvalidExecutableObservation("executable identity"))
}

fn kernel_identity(
    executable: HsaExecutableObjectIdentityV1,
    symbol: &SymbolFacts,
    name: &str,
) -> Result<HsaKernelObjectIdentityV1, HsaRuntimeAdapterError> {
    let mut hasher = Sha256::new();
    hasher.update(b"fe2o3-hsa-kernel-object-v1\0");
    hasher.update(executable.as_bytes());
    hasher.update(symbol.handle.to_le_bytes());
    hasher.update(symbol.kernel_object.to_le_bytes());
    hasher.update(name.as_bytes());
    hasher.update(symbol.kernarg_size.to_le_bytes());
    hasher.update(symbol.kernarg_alignment.to_le_bytes());
    hasher.update(symbol.group_segment_size.to_le_bytes());
    hasher.update(symbol.private_segment_size.to_le_bytes());
    HsaKernelObjectIdentityV1::new(hasher.finalize().into())
        .map_err(|_| HsaRuntimeAdapterError::InvalidExecutableObservation("kernel identity"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{AgentFacts, EnvironmentApi, HipFacts, PoolFacts, RuntimeFacts};
    use fe2o3_amd_target::AmdTargetId;
    use fe2o3_artifacts::{DigestAlgorithm, PayloadDigest};
    use fe2o3_host::{
        HsaAgentIdentityV1, HsaEnvironmentObservationV1, HsaPhysicalDeviceIdentityV1,
        HsaRuntimeIdentityV1,
    };
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct MockApi {
        log: Vec<&'static str>,
        failures: BTreeMap<&'static str, i32>,
        symbol: Option<SymbolFacts>,
        resolved_name: Option<String>,
        symbols: BTreeMap<String, SymbolFacts>,
        resolved_names: Vec<String>,
    }

    impl MockApi {
        fn call(&mut self, operation: &'static str) -> Result<(), ApiError> {
            self.log.push(operation);
            match self.failures.get(operation) {
                Some(status) => Err(ApiError {
                    operation,
                    status: *status,
                }),
                None => Ok(()),
            }
        }
    }

    impl EnvironmentApi for MockApi {
        fn initialize(&mut self) -> Result<RuntimeFacts, ApiError> {
            unreachable!()
        }

        fn shut_down(&mut self) -> Result<(), ApiError> {
            self.call("shutdown")
        }

        fn observe_hip_device(&mut self, _ordinal: i32) -> Result<HipFacts, ApiError> {
            unreachable!()
        }

        fn collect_agents(&mut self) -> Result<Vec<AgentFacts>, ApiError> {
            unreachable!()
        }

        fn collect_kernarg_pools(&mut self) -> Result<Vec<PoolFacts>, ApiError> {
            unreachable!()
        }
    }

    impl ExecutableApi for MockApi {
        fn reader_create(&mut self, _bytes: &[u8]) -> Result<u64, ApiError> {
            self.call("reader_create")?;
            Ok(11)
        }

        fn reader_destroy(&mut self, _reader: u64) -> Result<(), ApiError> {
            self.call("reader_destroy")
        }

        fn executable_create(&mut self, _profile: u32) -> Result<u64, ApiError> {
            self.call("executable_create")?;
            Ok(12)
        }

        fn executable_load(
            &mut self,
            _executable: u64,
            _agent: u64,
            _reader: u64,
        ) -> Result<u64, ApiError> {
            self.call("executable_load")?;
            Ok(13)
        }

        fn executable_freeze(&mut self, _executable: u64) -> Result<(), ApiError> {
            self.call("executable_freeze")
        }

        fn executable_destroy(&mut self, _executable: u64) -> Result<(), ApiError> {
            self.call("executable_destroy")
        }

        fn resolve_symbol(
            &mut self,
            _executable: u64,
            _agent: u64,
            name: &str,
        ) -> Result<SymbolFacts, ApiError> {
            self.call("resolve_symbol")?;
            self.resolved_name = Some(name.to_owned());
            self.resolved_names.push(name.to_owned());
            Ok(self
                .symbols
                .get(name)
                .cloned()
                .or_else(|| self.symbol.clone())
                .unwrap_or_else(valid_symbol))
        }
    }

    fn environment() -> HsaEnvironmentObservationV1 {
        let digest = DigestAlgorithm::Sha256.calculate(b"runtime");
        let target = AmdTargetId::parse("gfx942").unwrap();
        let runtime = HsaRuntimeIdentityV1::new("ROCr", "1.18", digest, [1; 16]).unwrap();
        let physical = HsaPhysicalDeviceIdentityV1::new([2; 16], 2, 0, target).unwrap();
        let agent = HsaAgentIdentityV1::new([1; 16], 20, [2; 16], target).unwrap();
        HsaEnvironmentObservationV1::new(runtime, physical, agent).unwrap()
    }

    fn make_core(api: MockApi) -> AdapterCore<MockApi> {
        AdapterCore {
            api,
            environment: environment(),
            agent: 20,
            profile: 0,
            queue_min_size: 64,
            queue_max_size: 1024,
            kernarg_pool: 30,
            completion_timeout: crate::dispatch::COMPLETION_TIMEOUT,
            next_identity: 1,
            runtime_live: true,
            _context: None,
        }
    }

    fn valid_symbol() -> SymbolFacts {
        SymbolFacts {
            handle: 14,
            kernel_object: 15,
            kind: HSA_SYMBOL_KIND_KERNEL,
            kernarg_size: 304,
            kernarg_alignment: 16,
            group_segment_size: 32,
            private_segment_size: 64,
            name: "vecadd.kd".into(),
        }
    }

    fn digest(bytes: &[u8]) -> PayloadDigest {
        DigestAlgorithm::Sha256.calculate(bytes)
    }

    #[test]
    fn load_resolve_and_unload_follow_exact_reverse_cleanup_order() {
        let bytes = b"one exact code object";
        let mut core = make_core(MockApi::default());
        let (executable, load) = load_executable(&mut core, bytes, digest(bytes)).unwrap();
        let (kernel, resolution) = resolve_kernel(&mut core, &executable, "vecadd").unwrap();
        let unload = unload_executable(&mut core, executable).unwrap();

        assert_eq!(load.byte_len(), bytes.len() as u64);
        assert_eq!(resolution.export_symbol(), "vecadd");
        assert_eq!(kernel.kernarg_segment_size, 304);
        assert_eq!(kernel.kernarg_segment_alignment, 16);
        assert_eq!(core.api.resolved_name.as_deref(), Some("vecadd.kd"));
        assert!(unload.released());
        assert_eq!(
            core.api.log,
            [
                "reader_create",
                "executable_create",
                "executable_load",
                "executable_freeze",
                "resolve_symbol",
                "executable_destroy",
                "reader_destroy",
            ]
        );
    }

    #[test]
    fn one_executable_retains_two_distinct_kernel_symbols() {
        let bytes = b"one two-kernel code object";
        let mut first = valid_symbol();
        first.name = "first.kd".into();
        let mut second = valid_symbol();
        second.handle = 24;
        second.kernel_object = 25;
        second.name = "second.kd".into();
        let mut api = MockApi::default();
        api.symbols.insert(first.name.clone(), first);
        api.symbols.insert(second.name.clone(), second);
        let mut core = make_core(api);
        let (executable, load) = load_executable(&mut core, bytes, digest(bytes)).unwrap();

        let (kernels, resolutions) =
            resolve_kernel_set(&mut core, &executable, ["first", "second"]).unwrap();

        assert_eq!(kernels.len(), 2);
        assert!(!kernels.is_empty());
        let first = kernels.get(0).unwrap();
        let second = kernels.get(1).unwrap();
        assert_eq!(first.executable_identity, load.executable_object());
        assert_eq!(second.executable_identity, load.executable_object());
        assert_ne!(first.symbol, second.symbol);
        assert_ne!(first.kernel_object, second.kernel_object);
        assert_ne!(first.identity, second.identity);
        assert_eq!(resolutions[0].export_symbol(), "first");
        assert_eq!(resolutions[1].export_symbol(), "second");
        assert_eq!(
            core.api.resolved_names,
            ["first.kd".to_owned(), "second.kd".to_owned()]
        );

        drop(kernels);
        assert!(unload_executable(&mut core, executable).unwrap().released());
    }

    #[test]
    fn kernel_set_rejects_duplicate_requests_and_native_identity_aliases() {
        let bytes = b"one two-kernel code object";
        let mut core = make_core(MockApi::default());
        let (executable, _) = load_executable(&mut core, bytes, digest(bytes)).unwrap();
        assert!(matches!(
            resolve_kernel_set(&mut core, &executable, ["vecadd", "vecadd"]),
            Err(HsaRuntimeAdapterError::InvalidExecutableObservation(
                "duplicate export symbol"
            ))
        ));
        assert!(core.api.resolved_names.is_empty());

        let mut first = valid_symbol();
        first.name = "first.kd".into();
        let mut second = first.clone();
        second.name = "second.kd".into();
        core.api.symbols.insert(first.name.clone(), first);
        core.api.symbols.insert(second.name.clone(), second);
        assert!(matches!(
            resolve_kernel_set(&mut core, &executable, ["first", "second"]),
            Err(HsaRuntimeAdapterError::InvalidExecutableObservation(
                "distinct kernel symbol identities"
            ))
        ));
        assert_eq!(core.api.resolved_names, ["first.kd", "second.kd"]);

        unload_executable(&mut core, executable).unwrap();
    }

    #[test]
    fn each_load_failure_cleans_only_live_resources_in_reverse_order() {
        let cases = [
            ("reader_create", vec!["reader_create"]),
            (
                "executable_create",
                vec!["reader_create", "executable_create", "reader_destroy"],
            ),
            (
                "executable_load",
                vec![
                    "reader_create",
                    "executable_create",
                    "executable_load",
                    "executable_destroy",
                    "reader_destroy",
                ],
            ),
            (
                "executable_freeze",
                vec![
                    "reader_create",
                    "executable_create",
                    "executable_load",
                    "executable_freeze",
                    "executable_destroy",
                    "reader_destroy",
                ],
            ),
        ];
        for (failure, expected) in cases {
            let mut api = MockApi::default();
            api.failures.insert(failure, 91);
            let mut core = make_core(api);
            let error = match load_executable(&mut core, b"code", digest(b"code")) {
                Ok(_) => panic!("failure edge unexpectedly loaded"),
                Err(error) => error,
            };
            assert!(matches!(
                error,
                HsaRuntimeAdapterError::RuntimeCall { status: 91, .. }
            ));
            assert_eq!(core.api.log, expected, "failure edge {failure}");
        }
    }

    #[test]
    fn exact_symbol_type_name_object_and_abi_are_required() {
        for mutate in [
            |symbol: &mut SymbolFacts| symbol.handle = 0,
            |symbol: &mut SymbolFacts| symbol.kernel_object = 0,
            |symbol: &mut SymbolFacts| symbol.kind = 0,
            |symbol: &mut SymbolFacts| symbol.name = "other".into(),
            |symbol: &mut SymbolFacts| symbol.kernarg_size = 0,
            |symbol: &mut SymbolFacts| symbol.kernarg_alignment = 3,
        ] {
            let mut symbol = valid_symbol();
            mutate(&mut symbol);
            let mut api = MockApi {
                symbol: Some(symbol),
                ..MockApi::default()
            };
            let mut core = make_core(std::mem::take(&mut api));
            let (executable, _) = load_executable(&mut core, b"code", digest(b"code")).unwrap();
            assert!(matches!(
                resolve_kernel(&mut core, &executable, "vecadd"),
                Err(HsaRuntimeAdapterError::InvalidExecutableObservation(_))
            ));
            unload_executable(&mut core, executable).unwrap();
        }
    }

    #[test]
    fn kernel_identity_authenticates_every_runtime_resolved_abi_field() {
        let executable = HsaExecutableObjectIdentityV1::new([7; 32]).unwrap();
        let symbol = valid_symbol();
        let expected = kernel_identity(executable, &symbol, "vecadd").unwrap();
        for mutate in [
            |value: &mut SymbolFacts| value.kernarg_size += 8,
            |value: &mut SymbolFacts| value.kernarg_alignment *= 2,
            |value: &mut SymbolFacts| value.group_segment_size += 1,
            |value: &mut SymbolFacts| value.private_segment_size += 1,
        ] {
            let mut substituted = symbol.clone();
            mutate(&mut substituted);
            assert_ne!(
                kernel_identity(executable, &substituted, "vecadd").unwrap(),
                expected
            );
        }
    }

    #[test]
    #[cfg(unix)]
    fn ambiguous_executable_cleanup_is_terminal() {
        const CHILD: &str = "FE2O3_HSA_AMBIGUOUS_EXECUTABLE_CLEANUP_CHILD";
        if let Ok(case) = std::env::var(CHILD) {
            let mut api = MockApi::default();
            match case.as_str() {
                "load-executable" => {
                    api.failures.insert("executable_load", 71);
                    api.failures.insert("executable_destroy", 72);
                    let mut core = make_core(api);
                    let _ = load_executable(&mut core, b"code", digest(b"code"));
                }
                "load-reader" => {
                    api.failures.insert("executable_create", 73);
                    api.failures.insert("reader_destroy", 74);
                    let mut core = make_core(api);
                    let _ = load_executable(&mut core, b"code", digest(b"code"));
                }
                "unload-executable" | "unload-reader" => {
                    let failure = if case == "unload-executable" {
                        "executable_destroy"
                    } else {
                        "reader_destroy"
                    };
                    let mut core = make_core(api);
                    let (executable, _) =
                        load_executable(&mut core, b"code", digest(b"code")).unwrap();
                    core.api.failures.insert(failure, 88);
                    let _ = unload_executable(&mut core, executable);
                }
                _ => panic!("unknown executable cleanup case"),
            }
            std::process::exit(91);
        }

        use std::os::unix::process::ExitStatusExt;
        for case in [
            "load-executable",
            "load-reader",
            "unload-executable",
            "unload-reader",
        ] {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .arg("--exact")
                .arg("lifecycle::tests::ambiguous_executable_cleanup_is_terminal")
                .arg("--nocapture")
                .env(CHILD, case)
                .status()
                .unwrap();
            assert_eq!(status.signal(), Some(6), "cleanup case {case}: {status}");
        }
    }
}
