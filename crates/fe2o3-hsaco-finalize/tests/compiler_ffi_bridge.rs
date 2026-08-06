use fe2o3_hsaco_finalize::{
    CompilerFfiBridgeError, CompilerFfiClosureV1, CompilerFfiContractIdentityV1,
    CompilerFfiDeclaredClaimsV1, CompilerFfiDefinitionV1, CompilerFfiDirectionV1,
    CompilerFfiFieldOriginV1, CompilerFfiPlanInputBindingV1, CompilerFfiPlanInputRoleV1,
    CompilerFfiProviderBindingV1, CompilerFfiSourceOwnerV1, CompilerFfiSymbolFieldV1,
    CompilerFfiSymbolV1, ContentIdentityV1, LinkInputV1, LinkOptionV1, LinkOutputV1,
    MAX_COMPILER_FFI_CRATE_NAME_BYTES_V1, MAX_COMPILER_FFI_PHYSICAL_ABI_BYTES_V1,
    MAX_COMPILER_FFI_SYMBOLS_V1, MultiInputLinkPlanV1, ProvenanceNodeV1, WorkerInputKindV1,
    bind_compiler_ffi_closure_v1,
};
use fe2o3_kernel_descriptor::{CodeObjectVersion, DeviceTargetV1};
use sha2::{Digest, Sha256};

const IMPORT_ABI: &str = "C(mut_ptr<global,u32>[size=8,align=8,as=global])->unit[size=0,align=1]";
const EXPORT_ABI: &str = "C(u32[size=4,align=4])->u32[size=4,align=4]";

#[derive(Clone)]
struct Fixture {
    plan: MultiInputLinkPlanV1,
    compiler: CompilerFfiClosureV1,
    plan_inputs: Vec<CompilerFfiPlanInputBindingV1>,
    providers: Vec<CompilerFfiProviderBindingV1>,
    rust_input: ContentIdentityV1,
    external_input: ContentIdentityV1,
    support_input: ContentIdentityV1,
}

fn target() -> DeviceTargetV1 {
    DeviceTargetV1::parse("gfx942:xnack-").unwrap()
}

fn other_target() -> DeviceTargetV1 {
    DeviceTargetV1::parse("gfx950").unwrap()
}

fn owner(index: u8, item: &str) -> CompilerFfiSourceOwnerV1 {
    CompilerFfiSourceOwnerV1::new(
        "ffi_crate",
        format!("ffi_crate::{item}"),
        [index; 16],
        format!("_RINvNtCs1234_9ffi_crate{item}"),
    )
    .unwrap()
}

fn claims(
    target: DeviceTargetV1,
    version: CodeObjectVersion,
    effects: &str,
    semantic_byte: u8,
) -> CompilerFfiDeclaredClaimsV1 {
    CompilerFfiDeclaredClaimsV1::new(target, version, effects, [semantic_byte; 32]).unwrap()
}

fn contract_identity(
    direction: CompilerFfiDirectionV1,
    symbol: &str,
    physical_abi: &str,
    declared: &CompilerFfiDeclaredClaimsV1,
) -> CompilerFfiContractIdentityV1 {
    let mut digest = Sha256::new();
    digest.update(b"fe2o3.device-ffi-contract.v1\0");
    digest.update(
        match direction {
            CompilerFfiDirectionV1::Import => 1_u16,
            CompilerFfiDirectionV1::Export => 2_u16,
        }
        .to_le_bytes(),
    );
    contract_field(&mut digest, symbol.as_bytes());
    contract_field(&mut digest, b"C");
    digest.update(
        match declared.code_object_version() {
            CodeObjectVersion::V4 => 4_u16,
            CodeObjectVersion::V5 => 5_u16,
            CodeObjectVersion::V6 => 6_u16,
        }
        .to_le_bytes(),
    );
    contract_field(&mut digest, declared.target().to_string().as_bytes());
    contract_field(&mut digest, physical_abi.as_bytes());
    contract_field(&mut digest, declared.effects().as_bytes());
    contract_field(&mut digest, hex(declared.semantic_claim()).as_bytes());
    contract_field(&mut digest, b"nounwind;nopanic");
    CompilerFfiContractIdentityV1::from_bytes(digest.finalize().into()).unwrap()
}

fn contract_field(digest: &mut Sha256, field: &[u8]) {
    digest.update((field.len() as u64).to_le_bytes());
    digest.update(field);
}

fn make_symbol(
    direction: CompilerFfiDirectionV1,
    symbol: &str,
    physical_abi: &str,
    source_owner: CompilerFfiSourceOwnerV1,
    declared: CompilerFfiDeclaredClaimsV1,
) -> CompilerFfiSymbolV1 {
    let definition = match direction {
        CompilerFfiDirectionV1::Import => CompilerFfiDefinitionV1::ExternalPlanInput,
        CompilerFfiDirectionV1::Export => CompilerFfiDefinitionV1::RustCompilerBitcode,
    };
    CompilerFfiSymbolV1::new(
        contract_identity(direction, symbol, physical_abi, &declared),
        direction,
        symbol,
        physical_abi,
        source_owner,
        definition,
        declared,
    )
    .unwrap()
}

fn plan(
    target: DeviceTargetV1,
    version: &str,
    identities: &[ContentIdentityV1],
) -> MultiInputLinkPlanV1 {
    let inputs: Vec<_> = identities
        .iter()
        .copied()
        .map(|identity| LinkInputV1::new(identity, target))
        .collect();
    let output = ContentIdentityV1::calculate(b"expected linked compiler FFI fixture");
    let mut provenance: Vec<_> = identities
        .iter()
        .copied()
        .map(|identity| ProvenanceNodeV1::new(identity, vec![]).unwrap())
        .collect();
    provenance.push(ProvenanceNodeV1::new(output, identities.to_vec()).unwrap());
    MultiInputLinkPlanV1::canonicalized(
        target,
        inputs,
        vec![LinkOptionV1::new("code-object-version", version).unwrap()],
        LinkOutputV1::new(output, target),
        provenance,
    )
    .unwrap()
}

fn fixture() -> Fixture {
    let rust_input = ContentIdentityV1::calculate(b"compiler Rust LLVM bitcode");
    let external_input = ContentIdentityV1::calculate(b"external AMDGPU object");
    let support_input = ContentIdentityV1::calculate(b"link support LLVM bitcode");
    let mut plan_inputs = vec![
        CompilerFfiPlanInputBindingV1::new(
            rust_input,
            WorkerInputKindV1::LlvmBitcode,
            CompilerFfiPlanInputRoleV1::RustCompilerBitcode,
        )
        .unwrap(),
        CompilerFfiPlanInputBindingV1::new(
            external_input,
            WorkerInputKindV1::AmdGpuRelocatable,
            CompilerFfiPlanInputRoleV1::ExternalDefinitionProvider,
        )
        .unwrap(),
        CompilerFfiPlanInputBindingV1::new(
            support_input,
            WorkerInputKindV1::LlvmBitcode,
            CompilerFfiPlanInputRoleV1::LinkSupport,
        )
        .unwrap(),
    ];
    plan_inputs.sort_by_key(|binding| binding.identity());
    let identities: Vec<_> = plan_inputs
        .iter()
        .map(|binding| binding.identity())
        .collect();
    let plan = plan(target(), "5", &identities);

    let mut symbols = vec![
        make_symbol(
            CompilerFfiDirectionV1::Import,
            "external_add",
            IMPORT_ABI,
            owner(1, "external_add"),
            claims(target(), CodeObjectVersion::V5, "read_global", 0x11),
        ),
        make_symbol(
            CompilerFfiDirectionV1::Export,
            "rust_helper",
            EXPORT_ABI,
            owner(2, "rust_helper"),
            claims(target(), CodeObjectVersion::V5, "none", 0x22),
        ),
    ];
    symbols.sort_by(|left, right| left.symbol().cmp(right.symbol()));
    let compiler = CompilerFfiClosureV1::new(
        target(),
        CodeObjectVersion::V5,
        strings(&["external_add", "kernel_main", "rust_helper"]),
        symbols,
    )
    .unwrap();
    let mut providers: Vec<_> = compiler
        .symbols()
        .iter()
        .map(|symbol| {
            let (identity, kind) = match symbol.definition() {
                CompilerFfiDefinitionV1::ExternalPlanInput => {
                    (external_input, WorkerInputKindV1::AmdGpuRelocatable)
                }
                CompilerFfiDefinitionV1::RustCompilerBitcode => {
                    (rust_input, WorkerInputKindV1::LlvmBitcode)
                }
            };
            CompilerFfiProviderBindingV1::new(
                symbol.contract_identity(),
                symbol.source_owner().identity(),
                identity,
                kind,
            )
        })
        .collect();
    providers.sort_by_key(|binding| binding.contract_identity());
    Fixture {
        plan,
        compiler,
        plan_inputs,
        providers,
        rust_input,
        external_input,
        support_input,
    }
}

#[test]
fn closes_exact_compiler_facts_over_the_canonical_g1_plan() {
    let fixture = fixture();
    let closed = bind_compiler_ffi_closure_v1(
        &fixture.plan,
        &fixture.compiler,
        fixture.plan_inputs.clone(),
        fixture.providers.clone(),
    )
    .unwrap();

    assert_eq!(closed.plan_identity(), fixture.plan.identity());
    assert_eq!(
        closed.compiler_closure_identity(),
        fixture.compiler.identity()
    );
    assert_eq!(closed.plan_inputs(), fixture.plan_inputs);
    assert_eq!(closed.provider_bindings(), fixture.providers);
    assert_eq!(
        closed.input_kinds().kinds(),
        fixture
            .plan_inputs
            .iter()
            .map(|binding| binding.kind())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        closed.symbols().required_symbols(),
        &strings(&["external_add", "kernel_main", "rust_helper"])
    );
    assert_eq!(
        closed.symbols().import_symbols(),
        &strings(&["external_add"])
    );
    assert_eq!(
        closed.symbols().export_symbols(),
        &strings(&["rust_helper"])
    );
    assert!(!closed.grants_link_authority());
    assert!(!closed.grants_load_authority());
    assert!(!closed.grants_launch_authority());
    assert!(!closed.effects_are_derived());
    assert!(!closed.semantics_are_verified());
    assert_eq!(
        fixture.compiler.required_symbols_origin(),
        CompilerFfiFieldOriginV1::CompilerDerived
    );
    assert_eq!(
        fixture.plan_inputs[0].origin(),
        CompilerFfiFieldOriginV1::CallerBindingClaim
    );
}

#[test]
fn canonical_bytes_and_identities_are_domain_separated_and_stable() {
    let fixture = fixture();
    let first = bind_compiler_ffi_closure_v1(
        &fixture.plan,
        &fixture.compiler,
        fixture.plan_inputs.clone(),
        fixture.providers.clone(),
    )
    .unwrap();
    let second = bind_compiler_ffi_closure_v1(
        &fixture.plan,
        &fixture.compiler,
        fixture.plan_inputs.clone(),
        fixture.providers.clone(),
    )
    .unwrap();

    assert_eq!(first, second);
    assert!(
        fixture
            .compiler
            .canonical_bytes()
            .starts_with(b"FE2O3/COMPILER-FFI-CLOSURE/V1\0")
    );
    assert!(
        first
            .canonical_bytes()
            .starts_with(b"FE2O3/PLAN-BOUND-COMPILER-FFI-CLOSURE/V1\0")
    );
    assert_eq!(
        fixture.compiler.identity().as_bytes(),
        &<[u8; 32]>::from(Sha256::digest(fixture.compiler.canonical_bytes()))
    );
    assert_eq!(
        first.identity().as_bytes(),
        &<[u8; 32]>::from(Sha256::digest(first.canonical_bytes()))
    );
    assert_eq!(
        hex(fixture.compiler.identity().as_bytes()),
        "b44a499036d31078cdb2425b75f8dbdcde3960c151e153f53ace3ec171709908"
    );
    assert_eq!(
        hex(first.identity().as_bytes()),
        "8738bb9b71aaf1fb1e6a5a9c8ee9f1a276fb3503809acc596c7b26d78a74a01e"
    );
}

#[test]
fn field_origins_do_not_upgrade_declaration_claims() {
    for field in [
        CompilerFfiSymbolFieldV1::ContractIdentity,
        CompilerFfiSymbolFieldV1::Direction,
        CompilerFfiSymbolFieldV1::Symbol,
        CompilerFfiSymbolFieldV1::PhysicalAbi,
        CompilerFfiSymbolFieldV1::SourceOwner,
        CompilerFfiSymbolFieldV1::Definition,
    ] {
        assert_eq!(
            CompilerFfiSymbolV1::field_origin(field),
            CompilerFfiFieldOriginV1::CompilerDerived
        );
    }
    for field in [
        CompilerFfiSymbolFieldV1::Target,
        CompilerFfiSymbolFieldV1::CodeObjectVersion,
        CompilerFfiSymbolFieldV1::Effects,
        CompilerFfiSymbolFieldV1::SemanticClaim,
    ] {
        assert_eq!(
            CompilerFfiSymbolV1::field_origin(field),
            CompilerFfiFieldOriginV1::DeclaredClaim
        );
    }
    let declared = claims(target(), CodeObjectVersion::V5, "none", 1);
    assert_eq!(declared.origin(), CompilerFfiFieldOriginV1::DeclaredClaim);
    assert!(!declared.effects_are_derived());
    assert!(!declared.semantics_are_verified());
}

#[test]
fn contract_identity_rejects_field_substitution() {
    let declared = claims(target(), CodeObjectVersion::V5, "none", 0x22);
    let identity = contract_identity(
        CompilerFfiDirectionV1::Export,
        "rust_helper",
        EXPORT_ABI,
        &declared,
    );
    let error = CompilerFfiSymbolV1::new(
        identity,
        CompilerFfiDirectionV1::Export,
        "rust_helper",
        "C(u64[size=8,align=8])->u64[size=8,align=8]",
        owner(2, "rust_helper"),
        CompilerFfiDefinitionV1::RustCompilerBitcode,
        declared,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        CompilerFfiBridgeError::ContractIdentityMismatch { .. }
    ));

    assert_eq!(
        CompilerFfiContractIdentityV1::from_bytes([0; 32]),
        Err(CompilerFfiBridgeError::ReservedIdentity(
            "compiler FFI contract"
        ))
    );
}

#[test]
fn compiler_closure_rejects_permutations_bounds_and_incomplete_symbols() {
    let fixture = fixture();
    let mut reversed = fixture.compiler.symbols().to_vec();
    reversed.reverse();
    assert_eq!(
        CompilerFfiClosureV1::new(
            target(),
            CodeObjectVersion::V5,
            fixture.compiler.required_symbols().to_vec(),
            reversed,
        ),
        Err(CompilerFfiBridgeError::NonCanonicalCompilerSymbols)
    );

    let only_export = fixture
        .compiler
        .symbols()
        .iter()
        .find(|symbol| symbol.direction() == CompilerFfiDirectionV1::Export)
        .unwrap()
        .clone();
    assert_eq!(
        CompilerFfiClosureV1::new(
            target(),
            CodeObjectVersion::V5,
            strings(&["kernel_main"]),
            vec![only_export],
        ),
        Err(CompilerFfiBridgeError::MissingRequiredSymbol(
            "rust_helper".to_owned()
        ))
    );

    let too_many = vec![fixture.compiler.symbols()[0].clone(); MAX_COMPILER_FFI_SYMBOLS_V1 + 1];
    assert_eq!(
        CompilerFfiClosureV1::new(
            target(),
            CodeObjectVersion::V5,
            fixture.compiler.required_symbols().to_vec(),
            too_many,
        ),
        Err(CompilerFfiBridgeError::TooManyCompilerFfiSymbols)
    );
}

#[test]
fn symbol_target_version_role_and_text_must_be_canonical() {
    let wrong_target_symbol = make_symbol(
        CompilerFfiDirectionV1::Export,
        "rust_helper",
        EXPORT_ABI,
        owner(2, "rust_helper"),
        claims(other_target(), CodeObjectVersion::V5, "none", 0x22),
    );
    assert_eq!(
        CompilerFfiClosureV1::new(
            target(),
            CodeObjectVersion::V5,
            strings(&["rust_helper"]),
            vec![wrong_target_symbol],
        ),
        Err(CompilerFfiBridgeError::SymbolTargetMismatch(
            "rust_helper".to_owned()
        ))
    );

    let wrong_version_symbol = make_symbol(
        CompilerFfiDirectionV1::Export,
        "rust_helper",
        EXPORT_ABI,
        owner(2, "rust_helper"),
        claims(target(), CodeObjectVersion::V6, "none", 0x22),
    );
    assert_eq!(
        CompilerFfiClosureV1::new(
            target(),
            CodeObjectVersion::V5,
            strings(&["rust_helper"]),
            vec![wrong_version_symbol],
        ),
        Err(CompilerFfiBridgeError::SymbolCodeObjectVersionMismatch(
            "rust_helper".to_owned()
        ))
    );

    let declared = claims(target(), CodeObjectVersion::V5, "none", 0x22);
    let identity = contract_identity(
        CompilerFfiDirectionV1::Export,
        "rust_helper",
        EXPORT_ABI,
        &declared,
    );
    assert!(matches!(
        CompilerFfiSymbolV1::new(
            identity,
            CompilerFfiDirectionV1::Export,
            "rust_helper",
            EXPORT_ABI,
            owner(2, "rust_helper"),
            CompilerFfiDefinitionV1::ExternalPlanInput,
            declared,
        ),
        Err(CompilerFfiBridgeError::DirectionDefinitionMismatch { .. })
    ));

    assert_eq!(
        CompilerFfiDeclaredClaimsV1::new(
            target(),
            CodeObjectVersion::V5,
            "write_global,read_global",
            [1; 32]
        ),
        Err(CompilerFfiBridgeError::InvalidEffects)
    );
    assert_eq!(
        CompilerFfiSourceOwnerV1::new(
            "x".repeat(MAX_COMPILER_FFI_CRATE_NAME_BYTES_V1 + 1),
            "item",
            [1; 16],
            "instance"
        ),
        Err(CompilerFfiBridgeError::InvalidText("source crate name"))
    );

    let oversized_abi = "x".repeat(MAX_COMPILER_FFI_PHYSICAL_ABI_BYTES_V1 + 1);
    let declared = claims(target(), CodeObjectVersion::V5, "none", 0x22);
    let identity = contract_identity(
        CompilerFfiDirectionV1::Export,
        "rust_helper",
        &oversized_abi,
        &declared,
    );
    assert_eq!(
        CompilerFfiSymbolV1::new(
            identity,
            CompilerFfiDirectionV1::Export,
            "rust_helper",
            oversized_abi,
            owner(2, "rust_helper"),
            CompilerFfiDefinitionV1::RustCompilerBitcode,
            declared,
        ),
        Err(CompilerFfiBridgeError::InvalidText("physical ABI"))
    );
}

#[test]
fn exact_plan_sequence_target_and_code_object_version_are_required() {
    let fixture = fixture();
    let mut permuted = fixture.plan_inputs.clone();
    permuted.swap(0, 1);
    assert!(matches!(
        bind_compiler_ffi_closure_v1(
            &fixture.plan,
            &fixture.compiler,
            permuted,
            fixture.providers.clone()
        ),
        Err(CompilerFfiBridgeError::PlanInputSequenceMismatch { index: 0, .. })
    ));

    let mut missing = fixture.plan_inputs.clone();
    missing.pop();
    assert_eq!(
        bind_compiler_ffi_closure_v1(
            &fixture.plan,
            &fixture.compiler,
            missing,
            fixture.providers.clone()
        ),
        Err(CompilerFfiBridgeError::PlanInputCountMismatch {
            plan: 3,
            bindings: 2,
        })
    );

    let identities: Vec<_> = fixture
        .plan_inputs
        .iter()
        .map(|binding| binding.identity())
        .collect();
    assert_eq!(
        bind_compiler_ffi_closure_v1(
            &plan(other_target(), "5", &identities),
            &fixture.compiler,
            fixture.plan_inputs.clone(),
            fixture.providers.clone()
        ),
        Err(CompilerFfiBridgeError::PlanTargetMismatch)
    );
    assert_eq!(
        bind_compiler_ffi_closure_v1(
            &plan(target(), "6", &identities),
            &fixture.compiler,
            fixture.plan_inputs.clone(),
            fixture.providers.clone()
        ),
        Err(CompilerFfiBridgeError::PlanCodeObjectVersionMismatch {
            plan: CodeObjectVersion::V6,
            compiler: CodeObjectVersion::V5,
        })
    );
    assert_eq!(
        bind_compiler_ffi_closure_v1(
            &plan(target(), "7", &identities),
            &fixture.compiler,
            fixture.plan_inputs.clone(),
            fixture.providers.clone()
        ),
        Err(CompilerFfiBridgeError::InvalidPlanCodeObjectVersion(
            "7".to_owned()
        ))
    );
}

#[test]
fn provider_bindings_reject_missing_duplicate_conflicting_and_unknown_contracts() {
    let fixture = fixture();
    let mut missing = fixture.providers.clone();
    let omitted = missing.pop().unwrap().contract_identity();
    assert_eq!(
        bind_compiler_ffi_closure_v1(
            &fixture.plan,
            &fixture.compiler,
            fixture.plan_inputs.clone(),
            missing
        ),
        Err(CompilerFfiBridgeError::MissingProviderBinding(omitted))
    );

    let mut duplicate = fixture.providers.clone();
    duplicate.insert(1, duplicate[0]);
    assert_eq!(
        bind_compiler_ffi_closure_v1(
            &fixture.plan,
            &fixture.compiler,
            fixture.plan_inputs.clone(),
            duplicate
        ),
        Err(CompilerFfiBridgeError::DuplicateProviderBinding(
            fixture.providers[0].contract_identity()
        ))
    );

    let mut conflicting = fixture.providers.clone();
    let first = conflicting[0];
    conflicting.insert(
        1,
        CompilerFfiProviderBindingV1::new(
            first.contract_identity(),
            first.source_owner_identity(),
            fixture.support_input,
            WorkerInputKindV1::LlvmBitcode,
        ),
    );
    assert_eq!(
        bind_compiler_ffi_closure_v1(
            &fixture.plan,
            &fixture.compiler,
            fixture.plan_inputs.clone(),
            conflicting
        ),
        Err(CompilerFfiBridgeError::ConflictingProviderBinding(
            first.contract_identity()
        ))
    );

    let unknown = CompilerFfiContractIdentityV1::from_bytes([0xfe; 32]).unwrap();
    let mut unreferenced = fixture.providers.clone();
    unreferenced.push(CompilerFfiProviderBindingV1::new(
        unknown,
        fixture.compiler.symbols()[0].source_owner().identity(),
        fixture.external_input,
        WorkerInputKindV1::AmdGpuRelocatable,
    ));
    unreferenced.sort_by_key(|binding| binding.contract_identity());
    assert_eq!(
        bind_compiler_ffi_closure_v1(
            &fixture.plan,
            &fixture.compiler,
            fixture.plan_inputs.clone(),
            unreferenced
        ),
        Err(CompilerFfiBridgeError::UnreferencedProviderBinding(unknown))
    );
}

#[test]
fn provider_permutations_owner_kind_role_and_substitution_fail_closed() {
    let fixture = fixture();
    let mut permuted = fixture.providers.clone();
    permuted.reverse();
    assert_eq!(
        bind_compiler_ffi_closure_v1(
            &fixture.plan,
            &fixture.compiler,
            fixture.plan_inputs.clone(),
            permuted
        ),
        Err(CompilerFfiBridgeError::NonCanonicalProviderBindings)
    );

    let mut wrong_owner = fixture.providers.clone();
    let binding = wrong_owner[0];
    wrong_owner[0] = CompilerFfiProviderBindingV1::new(
        binding.contract_identity(),
        owner(99, "substitute").identity(),
        binding.provider_input_identity(),
        binding.provider_input_kind(),
    );
    assert_eq!(
        bind_compiler_ffi_closure_v1(
            &fixture.plan,
            &fixture.compiler,
            fixture.plan_inputs.clone(),
            wrong_owner
        ),
        Err(CompilerFfiBridgeError::ProviderSourceOwnerMismatch(
            binding.contract_identity()
        ))
    );

    let external_index = fixture
        .providers
        .iter()
        .position(|binding| binding.provider_input_identity() == fixture.external_input)
        .unwrap();
    let external = fixture.providers[external_index];
    let mut wrong_kind = fixture.providers.clone();
    wrong_kind[external_index] = CompilerFfiProviderBindingV1::new(
        external.contract_identity(),
        external.source_owner_identity(),
        external.provider_input_identity(),
        WorkerInputKindV1::LlvmBitcode,
    );
    assert_eq!(
        bind_compiler_ffi_closure_v1(
            &fixture.plan,
            &fixture.compiler,
            fixture.plan_inputs.clone(),
            wrong_kind
        ),
        Err(CompilerFfiBridgeError::ProviderInputKindMismatch {
            contract: external.contract_identity(),
            declared: WorkerInputKindV1::LlvmBitcode,
            planned: WorkerInputKindV1::AmdGpuRelocatable,
        })
    );

    let mut wrong_role = fixture.providers.clone();
    wrong_role[external_index] = CompilerFfiProviderBindingV1::new(
        external.contract_identity(),
        external.source_owner_identity(),
        fixture.support_input,
        WorkerInputKindV1::LlvmBitcode,
    );
    assert_eq!(
        bind_compiler_ffi_closure_v1(
            &fixture.plan,
            &fixture.compiler,
            fixture.plan_inputs.clone(),
            wrong_role
        ),
        Err(CompilerFfiBridgeError::ProviderInputRoleMismatch {
            contract: external.contract_identity(),
            definition: CompilerFfiDefinitionV1::ExternalPlanInput,
            role: CompilerFfiPlanInputRoleV1::LinkSupport,
        })
    );

    let substitute = ContentIdentityV1::calculate(b"substituted provider bytes");
    let mut substituted = fixture.providers.clone();
    substituted[external_index] = CompilerFfiProviderBindingV1::new(
        external.contract_identity(),
        external.source_owner_identity(),
        substitute,
        WorkerInputKindV1::AmdGpuRelocatable,
    );
    assert_eq!(
        bind_compiler_ffi_closure_v1(
            &fixture.plan,
            &fixture.compiler,
            fixture.plan_inputs.clone(),
            substituted
        ),
        Err(CompilerFfiBridgeError::ProviderInputNotInPlan(substitute))
    );
}

#[test]
fn compiler_input_is_unique_typed_and_referenced() {
    let fixture = fixture();
    assert_eq!(
        CompilerFfiPlanInputBindingV1::new(
            fixture.rust_input,
            WorkerInputKindV1::AmdGpuRelocatable,
            CompilerFfiPlanInputRoleV1::RustCompilerBitcode,
        ),
        Err(CompilerFfiBridgeError::CompilerInputIsNotLlvmBitcode)
    );

    let mut duplicate_compiler = fixture.plan_inputs.clone();
    let support_index = duplicate_compiler
        .iter()
        .position(|binding| binding.identity() == fixture.support_input)
        .unwrap();
    duplicate_compiler[support_index] = CompilerFfiPlanInputBindingV1::new(
        fixture.support_input,
        WorkerInputKindV1::LlvmBitcode,
        CompilerFfiPlanInputRoleV1::RustCompilerBitcode,
    )
    .unwrap();
    assert_eq!(
        bind_compiler_ffi_closure_v1(
            &fixture.plan,
            &fixture.compiler,
            duplicate_compiler,
            fixture.providers.clone()
        ),
        Err(CompilerFfiBridgeError::MultipleRustCompilerInputs)
    );

    let mut unreferenced_external = fixture.plan_inputs.clone();
    let support_index = unreferenced_external
        .iter()
        .position(|binding| binding.identity() == fixture.support_input)
        .unwrap();
    unreferenced_external[support_index] = CompilerFfiPlanInputBindingV1::new(
        fixture.support_input,
        WorkerInputKindV1::LlvmBitcode,
        CompilerFfiPlanInputRoleV1::ExternalDefinitionProvider,
    )
    .unwrap();
    assert_eq!(
        bind_compiler_ffi_closure_v1(
            &fixture.plan,
            &fixture.compiler,
            unreferenced_external,
            fixture.providers.clone()
        ),
        Err(CompilerFfiBridgeError::UnreferencedProviderInput(
            fixture.support_input
        ))
    );
}

#[test]
fn binding_mutations_change_the_plan_bound_identity() {
    let fixture = fixture();
    let original = bind_compiler_ffi_closure_v1(
        &fixture.plan,
        &fixture.compiler,
        fixture.plan_inputs.clone(),
        fixture.providers.clone(),
    )
    .unwrap();

    let support_index = fixture
        .plan_inputs
        .iter()
        .position(|binding| binding.identity() == fixture.support_input)
        .unwrap();
    let mut changed_inputs = fixture.plan_inputs.clone();
    changed_inputs[support_index] = CompilerFfiPlanInputBindingV1::new(
        fixture.support_input,
        WorkerInputKindV1::AmdGpuRelocatable,
        CompilerFfiPlanInputRoleV1::LinkSupport,
    )
    .unwrap();
    let changed = bind_compiler_ffi_closure_v1(
        &fixture.plan,
        &fixture.compiler,
        changed_inputs,
        fixture.providers.clone(),
    )
    .unwrap();
    assert_ne!(original.identity(), changed.identity());
    assert_ne!(
        original.input_kinds().identity(),
        changed.input_kinds().identity()
    );
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}
