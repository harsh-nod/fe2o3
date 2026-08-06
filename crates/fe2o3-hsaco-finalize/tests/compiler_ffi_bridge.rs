use fe2o3_hsaco_finalize::{
    ContentIdentityV1, ExpectedFinalDefinedSymbolsClaimV1, FfiClaimOriginV1, FfiPlanInputClaimV1,
    FfiPlanInputRoleClaimV1, FfiSymbolProviderBindingClaimV1, FinalSymbolEvidenceSourceClaimV1,
    G4DeclarationOwnerClaimV1, G4DeclaredContractClaimsV1, G4FfiClaimEnvelopeAdapterV1,
    G4FfiClaimEnvelopeV1, G4FfiDirectionClaimV1, G4FfiSymbolClaimFieldV1, G4FfiSymbolClaimV1,
    G4SymbolProviderClassClaimV1, InputSymbolEvidenceCoverageClaimV1, LinkInputV1, LinkOptionV1,
    LinkOutputV1, MAX_G4_FFI_AGGREGATE_TEXT_BYTES_V1, MAX_G4_FFI_SYMBOL_CLAIMS_V1,
    MAX_G4_KERNEL_CLAIMS_V1, MAX_G4_RUST_DEFINITION_CLAIMS_V1, MultiInputLinkPlanV1,
    ProvenanceNodeV1, StagedFfiExecutionBlockerV1, StagedFfiLinkError, StagedFfiLinkPlanV1,
    UnauthenticatedProducerClaimV1, WorkerInputKindV1, stage_g4_ffi_link_plan_v1,
};
use fe2o3_kernel_descriptor::{CodeObjectVersion, DeviceTargetV1};
use reserved_fe2o3_symbols::{
    DEVICE_FFI_DIRECTION_EXPORT_V1, DEVICE_FFI_DIRECTION_IMPORT_V1, DeviceFfiContractFieldsV1,
    DeviceFfiContractIdV1, derive_device_ffi_contract_id_v1,
};
use sha2::{Digest, Sha256};

const IMPORT_ABI: &str = "C(mut_ptr<global,u32>[size=8,align=8,as=global])->unit[size=0,align=1]";
const EXPORT_ABI: &str = "C(u32[size=4,align=4])->u32[size=4,align=4]";

#[derive(Clone)]
struct Fixture {
    plan: MultiInputLinkPlanV1,
    envelope: G4FfiClaimEnvelopeV1,
    inputs: Vec<FfiPlanInputClaimV1>,
    providers: Vec<FfiSymbolProviderBindingClaimV1>,
    final_symbols: ExpectedFinalDefinedSymbolsClaimV1,
    compiler_module: ContentIdentityV1,
    external_input: ContentIdentityV1,
    support_input: ContentIdentityV1,
}

struct TestOnlyG4CompatibleAdapter {
    envelope: G4FfiClaimEnvelopeV1,
}

impl G4FfiClaimEnvelopeAdapterV1 for TestOnlyG4CompatibleAdapter {
    fn assertion_only_g4_ffi_claim_envelope_v1(
        &self,
    ) -> Result<G4FfiClaimEnvelopeV1, StagedFfiLinkError> {
        Ok(self.envelope.clone())
    }
}

fn target() -> DeviceTargetV1 {
    DeviceTargetV1::parse("gfx942:xnack-").unwrap()
}

fn other_target() -> DeviceTargetV1 {
    DeviceTargetV1::parse("gfx950").unwrap()
}

fn owner(index: u8, item: &str) -> G4DeclarationOwnerClaimV1 {
    G4DeclarationOwnerClaimV1::new(
        "ffi_crate",
        format!("ffi_crate::{item}"),
        [index; 16],
        format!("_RINvNtCs1234_9ffi_crate{item}"),
    )
    .unwrap()
}

fn producer(index: u8, name: &str) -> UnauthenticatedProducerClaimV1 {
    UnauthenticatedProducerClaimV1::new(name, format!("assertion-v{index}"), [index; 32]).unwrap()
}

fn declared(
    target: DeviceTargetV1,
    version: CodeObjectVersion,
    effects: &str,
    semantic_byte: u8,
) -> G4DeclaredContractClaimsV1 {
    G4DeclaredContractClaimsV1::new(target, version, effects, [semantic_byte; 32]).unwrap()
}

fn contract_identity(
    direction: G4FfiDirectionClaimV1,
    symbol: &str,
    physical_abi: &str,
    claim: &G4DeclaredContractClaimsV1,
) -> DeviceFfiContractIdV1 {
    derive_device_ffi_contract_id_v1(DeviceFfiContractFieldsV1 {
        direction: match direction {
            G4FfiDirectionClaimV1::Import => DEVICE_FFI_DIRECTION_IMPORT_V1,
            G4FfiDirectionClaimV1::Export => DEVICE_FFI_DIRECTION_EXPORT_V1,
        },
        symbol,
        calling_convention: "C",
        code_object_version: match claim.code_object_version() {
            CodeObjectVersion::V4 => 4,
            CodeObjectVersion::V5 => 5,
            CodeObjectVersion::V6 => 6,
        },
        target: &claim.target().to_string(),
        physical_abi,
        effects: claim.effects(),
        semantic_identity: &hex(claim.semantic_identity()),
    })
}

fn symbol(
    direction: G4FfiDirectionClaimV1,
    name: &str,
    abi: &str,
    owner: G4DeclarationOwnerClaimV1,
    declared: G4DeclaredContractClaimsV1,
) -> G4FfiSymbolClaimV1 {
    let provider_class = match direction {
        G4FfiDirectionClaimV1::Import => G4SymbolProviderClassClaimV1::ExternalPlanInput,
        G4FfiDirectionClaimV1::Export => G4SymbolProviderClassClaimV1::CompilerModuleInput,
    };
    G4FfiSymbolClaimV1::new(
        contract_identity(direction, name, abi, &declared),
        direction,
        name,
        abi,
        owner,
        provider_class,
        declared,
    )
    .unwrap()
}

fn build_plan(
    target: DeviceTargetV1,
    version: &str,
    inputs: &[FfiPlanInputClaimV1],
) -> MultiInputLinkPlanV1 {
    let link_inputs: Vec<_> = inputs
        .iter()
        .map(|input| LinkInputV1::new(input.identity(), target))
        .collect();
    let output = ContentIdentityV1::calculate(b"expected staged FFI output identity claim");
    let mut provenance: Vec<_> = link_inputs
        .iter()
        .map(|input| ProvenanceNodeV1::new(input.identity(), vec![]).unwrap())
        .collect();
    provenance.push(
        ProvenanceNodeV1::new(
            output,
            link_inputs.iter().map(|input| input.identity()).collect(),
        )
        .unwrap(),
    );
    MultiInputLinkPlanV1::canonicalized(
        target,
        link_inputs,
        vec![LinkOptionV1::new("code-object-version", version).unwrap()],
        LinkOutputV1::new(output, target),
        provenance,
    )
    .unwrap()
}

fn final_symbols_claim(
    symbols: Vec<String>,
    inputs: &[FfiPlanInputClaimV1],
) -> ExpectedFinalDefinedSymbolsClaimV1 {
    let coverage = inputs
        .iter()
        .enumerate()
        .map(|(index, input)| {
            InputSymbolEvidenceCoverageClaimV1::new(
                input.identity(),
                input.kind(),
                if index % 2 == 0 {
                    FinalSymbolEvidenceSourceClaimV1::BoundedInputInspection
                } else {
                    FinalSymbolEvidenceSourceClaimV1::AuthenticatedInputManifest
                },
                [0x40 + index as u8; 32],
            )
            .unwrap()
        })
        .collect();
    ExpectedFinalDefinedSymbolsClaimV1::new(symbols, coverage).unwrap()
}

fn fixture() -> Fixture {
    let compiler_module = ContentIdentityV1::calculate(b"future exact compiler module claim");
    let external_input = ContentIdentityV1::calculate(b"external AMDGPU provider object");
    let support_input = ContentIdentityV1::calculate(b"link support bitcode");
    let mut inputs = vec![
        FfiPlanInputClaimV1::new(
            compiler_module,
            WorkerInputKindV1::AmdGpuRelocatable,
            FfiPlanInputRoleClaimV1::CompilerModule,
            producer(1, "future-rustc-module-producer"),
        ),
        FfiPlanInputClaimV1::new(
            external_input,
            WorkerInputKindV1::AmdGpuRelocatable,
            FfiPlanInputRoleClaimV1::ExternalSymbolProvider,
            producer(2, "external-provider"),
        ),
        FfiPlanInputClaimV1::new(
            support_input,
            WorkerInputKindV1::LlvmBitcode,
            FfiPlanInputRoleClaimV1::LinkSupport,
            producer(3, "link-support"),
        ),
    ];
    inputs.sort_by_key(FfiPlanInputClaimV1::identity);
    let plan = build_plan(target(), "5", &inputs);
    let mut symbols = vec![
        symbol(
            G4FfiDirectionClaimV1::Import,
            "external_add",
            IMPORT_ABI,
            owner(1, "external_add"),
            declared(target(), CodeObjectVersion::V5, "read_global", 0x11),
        ),
        symbol(
            G4FfiDirectionClaimV1::Export,
            "rust_helper",
            EXPORT_ABI,
            owner(2, "rust_helper"),
            declared(target(), CodeObjectVersion::V5, "none", 0x22),
        ),
    ];
    symbols.sort_by(|left, right| left.symbol().cmp(right.symbol()));
    let envelope = G4FfiClaimEnvelopeV1::new(
        target(),
        CodeObjectVersion::V5,
        strings(&["external_add", "kernel_main", "rust_helper"]),
        1,
        1,
        symbols,
    )
    .unwrap();
    let mut providers: Vec<_> = envelope
        .symbols()
        .iter()
        .map(|symbol| {
            let input = match symbol.provider_class() {
                G4SymbolProviderClassClaimV1::ExternalPlanInput => inputs
                    .iter()
                    .find(|input| input.identity() == external_input)
                    .unwrap(),
                G4SymbolProviderClassClaimV1::CompilerModuleInput => inputs
                    .iter()
                    .find(|input| input.identity() == compiler_module)
                    .unwrap(),
            };
            FfiSymbolProviderBindingClaimV1::new(
                symbol.contract_identity(),
                symbol.declaration_owner().identity(),
                input.identity(),
                input.kind(),
                input.producer().identity(),
            )
        })
        .collect();
    providers.sort_by_key(FfiSymbolProviderBindingClaimV1::contract_identity);
    let final_symbols = final_symbols_claim(
        strings(&["external_add", "kernel_main", "rust_helper"]),
        &inputs,
    );
    Fixture {
        plan,
        envelope,
        inputs,
        providers,
        final_symbols,
        compiler_module,
        external_input,
        support_input,
    }
}

fn stage(fixture: &Fixture) -> StagedFfiLinkPlanV1 {
    stage_g4_ffi_link_plan_v1(
        &fixture.plan,
        &fixture.envelope,
        fixture.inputs.clone(),
        fixture.providers.clone(),
        Some(fixture.final_symbols.clone()),
    )
    .unwrap()
}

#[test]
fn adapter_contract_accepts_real_g4_compatible_contract_fields_without_attesting_them() {
    let fixture = fixture();
    let adapter = TestOnlyG4CompatibleAdapter {
        envelope: fixture.envelope.clone(),
    };
    let adapted = adapter.assertion_only_g4_ffi_claim_envelope_v1().unwrap();

    assert_eq!(adapted.identity(), fixture.envelope.identity());
    assert_eq!(adapted.symbols()[0].physical_abi(), IMPORT_ABI);
    assert_eq!(adapted.symbols()[1].physical_abi(), EXPORT_ABI);
    assert_eq!(adapted.claim_origin(), FfiClaimOriginV1::G4AssertionOnly);
    assert!(!adapted.is_actual_compiler_integration());
}

#[test]
fn complete_ffi_identity_is_staged_but_never_decomposed_into_worker_v1() {
    let fixture = fixture();
    let staged = stage(&fixture);
    let inspection = staged.inspection();

    assert_ne!(staged.identity().as_bytes(), &[0; 32]);
    assert_eq!(inspection.input_claim_count(), fixture.inputs.len());
    assert_eq!(
        inspection.provider_binding_claim_count(),
        fixture.providers.len()
    );
    assert!(inspection.has_expected_final_defined_symbols_claim());
    assert_eq!(
        inspection.execution_blocker(),
        StagedFfiExecutionBlockerV1::WorkerProtocolV1CannotBindCompleteFfiIdentity
    );
}

#[test]
fn missing_final_symbol_evidence_remains_explicitly_nonexecutable() {
    let fixture = fixture();
    let staged = stage_g4_ffi_link_plan_v1(
        &fixture.plan,
        &fixture.envelope,
        fixture.inputs.clone(),
        fixture.providers.clone(),
        None,
    )
    .unwrap();

    let inspection = staged.inspection();
    assert!(!inspection.has_expected_final_defined_symbols_claim());
    assert_eq!(
        inspection.execution_blocker(),
        StagedFfiExecutionBlockerV1::MissingExpectedFinalDefinedSymbolsClaim
    );
}

#[test]
fn every_public_adapter_field_remains_an_assertion_only_claim() {
    let fixture = fixture();
    assert_eq!(
        fixture.envelope.claim_origin(),
        FfiClaimOriginV1::G4AssertionOnly
    );
    assert!(!fixture.envelope.is_actual_compiler_integration());
    for field in [
        G4FfiSymbolClaimFieldV1::ContractIdentity,
        G4FfiSymbolClaimFieldV1::Direction,
        G4FfiSymbolClaimFieldV1::Symbol,
        G4FfiSymbolClaimFieldV1::PhysicalAbi,
        G4FfiSymbolClaimFieldV1::DeclarationOwner,
        G4FfiSymbolClaimFieldV1::ProviderClass,
        G4FfiSymbolClaimFieldV1::Target,
        G4FfiSymbolClaimFieldV1::CodeObjectVersion,
        G4FfiSymbolClaimFieldV1::Effects,
        G4FfiSymbolClaimFieldV1::SemanticIdentity,
    ] {
        assert_eq!(
            G4FfiSymbolClaimV1::field_claim_origin(field),
            FfiClaimOriginV1::G4AssertionOnly
        );
    }
    assert_eq!(
        fixture.inputs[0].claim_origin(),
        FfiClaimOriginV1::CallerBindingAssertionOnly
    );
    assert_eq!(
        fixture.inputs[0].producer().claim_origin(),
        FfiClaimOriginV1::UnauthenticatedProducerClaim
    );
    assert!(!fixture.inputs[0].producer().is_authenticated());
    assert_eq!(
        fixture.final_symbols.claim_origin(),
        FfiClaimOriginV1::UnauthenticatedEvidenceClaim
    );
    assert!(!fixture.final_symbols.is_authenticated());
}

#[test]
fn declaration_owner_and_unauthenticated_producer_are_separate_identities() {
    let first = owner(7, "same_item");
    let relabeled = G4DeclarationOwnerClaimV1::new(
        "new_label",
        "new_label::path",
        *first.def_path_hash(),
        first.concrete_instance_symbol(),
    )
    .unwrap();
    let producer = producer(7, "producer-is-not-owner");

    assert_eq!(first.identity(), relabeled.identity());
    assert_ne!(first.identity().as_bytes(), producer.identity().as_bytes());
    assert_eq!(first.claim_origin(), FfiClaimOriginV1::G4AssertionOnly);
    assert_eq!(
        producer.claim_origin(),
        FfiClaimOriginV1::UnauthenticatedProducerClaim
    );
}

#[test]
fn neutral_compiler_module_claim_does_not_imply_llvm_bitcode_or_actual_emission() {
    let fixture = fixture();
    let module = fixture
        .inputs
        .iter()
        .find(|input| input.identity() == fixture.compiler_module)
        .unwrap();
    assert_eq!(module.role(), FfiPlanInputRoleClaimV1::CompilerModule);
    assert_eq!(module.kind(), WorkerInputKindV1::AmdGpuRelocatable);
    assert!(!module.producer().is_authenticated());
}

#[test]
fn import_only_and_kernel_only_claims_still_require_one_compiler_module() {
    let fixture = fixture();
    let import = fixture
        .envelope
        .symbols()
        .iter()
        .find(|symbol| symbol.direction() == G4FfiDirectionClaimV1::Import)
        .unwrap()
        .clone();
    let import_only = G4FfiClaimEnvelopeV1::new(
        target(),
        CodeObjectVersion::V5,
        strings(&["external_add", "kernel_main"]),
        0,
        1,
        vec![import],
    )
    .unwrap();
    let without_module: Vec<_> = fixture
        .inputs
        .iter()
        .filter(|input| input.identity() != fixture.compiler_module)
        .cloned()
        .collect();
    let plan_without_module = build_plan(target(), "5", &without_module);
    let external_binding = fixture
        .providers
        .iter()
        .find(|binding| binding.provider_input_identity() == fixture.external_input)
        .copied()
        .unwrap();
    assert_eq!(
        stage_g4_ffi_link_plan_v1(
            &plan_without_module,
            &import_only,
            without_module,
            vec![external_binding],
            None,
        ),
        Err(StagedFfiLinkError::MissingCompilerModuleInputClaim)
    );

    let module_input = fixture
        .inputs
        .iter()
        .find(|input| input.identity() == fixture.compiler_module)
        .unwrap()
        .clone();
    let kernel_inputs = vec![module_input];
    let kernel_plan = build_plan(target(), "5", &kernel_inputs);
    let kernel_only = G4FfiClaimEnvelopeV1::new(
        target(),
        CodeObjectVersion::V5,
        strings(&["kernel_main"]),
        0,
        1,
        vec![],
    )
    .unwrap();
    let staged =
        stage_g4_ffi_link_plan_v1(&kernel_plan, &kernel_only, kernel_inputs, vec![], None).unwrap();
    assert_eq!(
        staged.inspection().execution_blocker(),
        StagedFfiExecutionBlockerV1::MissingExpectedFinalDefinedSymbolsClaim
    );
}

#[test]
fn compiler_module_cardinality_matches_rust_definition_and_kernel_claims() {
    let fixture = fixture();
    let mut no_module = fixture.inputs.clone();
    let index = no_module
        .iter()
        .position(|input| input.identity() == fixture.compiler_module)
        .unwrap();
    let original = no_module[index].clone();
    no_module[index] = FfiPlanInputClaimV1::new(
        original.identity(),
        original.kind(),
        FfiPlanInputRoleClaimV1::LinkSupport,
        original.producer().clone(),
    );
    assert_eq!(
        stage_g4_ffi_link_plan_v1(
            &fixture.plan,
            &fixture.envelope,
            no_module,
            fixture.providers.clone(),
            None,
        ),
        Err(StagedFfiLinkError::MissingCompilerModuleInputClaim)
    );

    let mut two_modules = fixture.inputs.clone();
    let support = two_modules
        .iter()
        .position(|input| input.identity() == fixture.support_input)
        .unwrap();
    let original = two_modules[support].clone();
    two_modules[support] = FfiPlanInputClaimV1::new(
        original.identity(),
        original.kind(),
        FfiPlanInputRoleClaimV1::CompilerModule,
        original.producer().clone(),
    );
    assert_eq!(
        stage_g4_ffi_link_plan_v1(
            &fixture.plan,
            &fixture.envelope,
            two_modules,
            fixture.providers.clone(),
            None,
        ),
        Err(StagedFfiLinkError::MultipleCompilerModuleInputClaims)
    );

    let empty_envelope =
        G4FfiClaimEnvelopeV1::new(target(), CodeObjectVersion::V5, vec![], 0, 0, vec![]).unwrap();
    assert_eq!(
        stage_g4_ffi_link_plan_v1(
            &fixture.plan,
            &empty_envelope,
            fixture.inputs.clone(),
            vec![],
            None,
        ),
        Err(StagedFfiLinkError::UnexpectedCompilerModuleInputClaim)
    );
}

#[test]
fn compiler_required_symbols_are_not_final_defined_symbol_expectations() {
    let fixture = fixture();
    let staged_without_final = stage_g4_ffi_link_plan_v1(
        &fixture.plan,
        &fixture.envelope,
        fixture.inputs.clone(),
        fixture.providers.clone(),
        None,
    )
    .unwrap();
    assert!(
        !staged_without_final
            .inspection()
            .has_expected_final_defined_symbols_claim()
    );

    let incomplete =
        final_symbols_claim(strings(&["external_add", "rust_helper"]), &fixture.inputs);
    assert_eq!(
        stage_g4_ffi_link_plan_v1(
            &fixture.plan,
            &fixture.envelope,
            fixture.inputs.clone(),
            fixture.providers.clone(),
            Some(incomplete),
        ),
        Err(
            StagedFfiLinkError::CompilerRequiredSymbolAbsentFromFinalExpectation(
                "kernel_main".to_owned()
            )
        )
    );
}

#[test]
fn final_symbol_expectations_require_exact_all_input_evidence_coverage() {
    let fixture = fixture();
    let mut short_coverage = fixture.final_symbols.coverage().to_vec();
    short_coverage.pop();
    let short = ExpectedFinalDefinedSymbolsClaimV1::new(
        fixture.final_symbols.symbols().to_vec(),
        short_coverage,
    )
    .unwrap();
    assert_eq!(
        stage_g4_ffi_link_plan_v1(
            &fixture.plan,
            &fixture.envelope,
            fixture.inputs.clone(),
            fixture.providers.clone(),
            Some(short),
        ),
        Err(StagedFfiLinkError::SymbolEvidenceCoverageCountMismatch {
            inputs: 3,
            coverage: 2,
        })
    );

    let mut wrong_kind = fixture.final_symbols.coverage().to_vec();
    let first = wrong_kind[0];
    wrong_kind[0] = InputSymbolEvidenceCoverageClaimV1::new(
        first.input_identity(),
        opposite_kind(first.input_kind()),
        first.source(),
        *first.evidence_identity_claim(),
    )
    .unwrap();
    let wrong = ExpectedFinalDefinedSymbolsClaimV1::new(
        fixture.final_symbols.symbols().to_vec(),
        wrong_kind,
    )
    .unwrap();
    assert_eq!(
        stage_g4_ffi_link_plan_v1(
            &fixture.plan,
            &fixture.envelope,
            fixture.inputs.clone(),
            fixture.providers.clone(),
            Some(wrong),
        ),
        Err(StagedFfiLinkError::SymbolEvidenceCoverageMismatch { index: 0 })
    );

    let mut permuted = fixture.final_symbols.coverage().to_vec();
    permuted.swap(0, 1);
    assert_eq!(
        ExpectedFinalDefinedSymbolsClaimV1::new(fixture.final_symbols.symbols().to_vec(), permuted,),
        Err(StagedFfiLinkError::NonCanonicalSymbolEvidenceCoverage)
    );
}

#[test]
fn authoritative_contract_derivation_and_exact_g4_grammar_fail_closed() {
    let claim = declared(target(), CodeObjectVersion::V5, "read_global", 0x11);
    let valid = contract_identity(
        G4FfiDirectionClaimV1::Import,
        "external_add",
        IMPORT_ABI,
        &claim,
    );
    assert!(
        G4FfiSymbolClaimV1::new(
            valid,
            G4FfiDirectionClaimV1::Import,
            "external_add",
            IMPORT_ABI,
            owner(1, "external_add"),
            G4SymbolProviderClassClaimV1::ExternalPlanInput,
            claim.clone(),
        )
        .is_ok()
    );
    assert!(matches!(
        G4FfiSymbolClaimV1::new(
            DeviceFfiContractIdV1::from_bytes([0xfe; 32]),
            G4FfiDirectionClaimV1::Import,
            "external_add",
            IMPORT_ABI,
            owner(1, "external_add"),
            G4SymbolProviderClassClaimV1::ExternalPlanInput,
            claim.clone(),
        ),
        Err(StagedFfiLinkError::ContractIdentityMismatch { .. })
    ));
    assert_eq!(
        G4FfiSymbolClaimV1::new(
            valid,
            G4FfiDirectionClaimV1::Import,
            "bad symbol",
            IMPORT_ABI,
            owner(1, "external_add"),
            G4SymbolProviderClassClaimV1::ExternalPlanInput,
            claim.clone(),
        ),
        Err(StagedFfiLinkError::InvalidFfiSymbol)
    );
    assert_eq!(
        G4DeclaredContractClaimsV1::new(
            target(),
            CodeObjectVersion::V5,
            "write_global,read_global",
            [1; 32],
        ),
        Err(StagedFfiLinkError::InvalidEffects)
    );
    assert_eq!(
        G4FfiSymbolClaimV1::new(
            valid,
            G4FfiDirectionClaimV1::Import,
            "external_add",
            IMPORT_ABI,
            owner(1, "external_add"),
            G4SymbolProviderClassClaimV1::CompilerModuleInput,
            claim.clone(),
        ),
        Err(StagedFfiLinkError::DirectionProviderClassMismatch {
            symbol: "external_add".to_owned(),
            direction: G4FfiDirectionClaimV1::Import,
            provider_class: G4SymbolProviderClassClaimV1::CompilerModuleInput,
        })
    );

    for bad_abi in [
        "C( u32[size=4,align=4])->unit[size=0,align=1]",
        "C(u32[size=8,align=8])->unit[size=0,align=1]",
        "C(f16[size=2,align=2])->unit[size=0,align=1]",
        "C(mut_ptr<constant,u32>[size=8,align=8,as=constant])->unit[size=0,align=1]",
        "C(mut_ptr<global,ptr>[size=8,align=8,as=global])->unit[size=0,align=1]",
    ] {
        let identity = contract_identity(
            G4FfiDirectionClaimV1::Import,
            "external_add",
            bad_abi,
            &claim,
        );
        assert_eq!(
            G4FfiSymbolClaimV1::new(
                identity,
                G4FfiDirectionClaimV1::Import,
                "external_add",
                bad_abi,
                owner(1, "external_add"),
                G4SymbolProviderClassClaimV1::ExternalPlanInput,
                claim.clone(),
            ),
            Err(StagedFfiLinkError::InvalidPhysicalAbi)
        );
    }

    let arguments = std::iter::repeat_n("u8[size=1,align=1]", 33)
        .collect::<Vec<_>>()
        .join(",");
    let oversized_arguments = format!("C({arguments})->unit[size=0,align=1]");
    let identity = contract_identity(
        G4FfiDirectionClaimV1::Import,
        "external_add",
        &oversized_arguments,
        &claim,
    );
    assert_eq!(
        G4FfiSymbolClaimV1::new(
            identity,
            G4FfiDirectionClaimV1::Import,
            "external_add",
            oversized_arguments,
            owner(1, "external_add"),
            G4SymbolProviderClassClaimV1::ExternalPlanInput,
            claim,
        ),
        Err(StagedFfiLinkError::TooManyPhysicalAbiArguments)
    );
}

#[test]
fn provider_claims_reject_permutation_cardinality_and_substitution() {
    let fixture = fixture();
    let mut reversed = fixture.providers.clone();
    reversed.reverse();
    assert_eq!(
        stage_g4_ffi_link_plan_v1(
            &fixture.plan,
            &fixture.envelope,
            fixture.inputs.clone(),
            reversed,
            None,
        ),
        Err(StagedFfiLinkError::NonCanonicalProviderBindingClaims)
    );

    let mut missing = fixture.providers.clone();
    let omitted = missing.pop().unwrap().contract_identity();
    assert_eq!(
        stage_g4_ffi_link_plan_v1(
            &fixture.plan,
            &fixture.envelope,
            fixture.inputs.clone(),
            missing,
            None,
        ),
        Err(StagedFfiLinkError::MissingProviderBindingClaim(omitted))
    );

    let mut duplicate = fixture.providers.clone();
    duplicate.insert(1, duplicate[0]);
    assert_eq!(
        stage_g4_ffi_link_plan_v1(
            &fixture.plan,
            &fixture.envelope,
            fixture.inputs.clone(),
            duplicate,
            None,
        ),
        Err(StagedFfiLinkError::DuplicateProviderBindingClaim(
            fixture.providers[0].contract_identity()
        ))
    );

    let external_index = fixture
        .providers
        .iter()
        .position(|binding| binding.provider_input_identity() == fixture.external_input)
        .unwrap();
    let external = fixture.providers[external_index];
    let substitute = ContentIdentityV1::calculate(b"substituted provider");
    let mut substituted = fixture.providers.clone();
    substituted[external_index] = FfiSymbolProviderBindingClaimV1::new(
        external.contract_identity(),
        external.declaration_owner_identity(),
        substitute,
        external.provider_input_kind(),
        external.producer_claim_identity(),
    );
    assert_eq!(
        stage_g4_ffi_link_plan_v1(
            &fixture.plan,
            &fixture.envelope,
            fixture.inputs.clone(),
            substituted,
            None,
        ),
        Err(StagedFfiLinkError::ProviderInputAbsent(substitute))
    );

    let mut wrong_producer = fixture.providers.clone();
    wrong_producer[external_index] = FfiSymbolProviderBindingClaimV1::new(
        external.contract_identity(),
        external.declaration_owner_identity(),
        external.provider_input_identity(),
        external.provider_input_kind(),
        producer(99, "substitute-producer").identity(),
    );
    assert_eq!(
        stage_g4_ffi_link_plan_v1(
            &fixture.plan,
            &fixture.envelope,
            fixture.inputs.clone(),
            wrong_producer,
            None,
        ),
        Err(StagedFfiLinkError::ProviderProducerClaimMismatch(
            external.contract_identity()
        ))
    );
}

#[test]
fn exact_plan_input_sequence_target_and_code_object_version_are_required() {
    let fixture = fixture();
    let mut permuted = fixture.inputs.clone();
    permuted.swap(0, 1);
    assert!(matches!(
        stage_g4_ffi_link_plan_v1(
            &fixture.plan,
            &fixture.envelope,
            permuted,
            fixture.providers.clone(),
            None,
        ),
        Err(StagedFfiLinkError::PlanInputClaimSequenceMismatch { index: 0, .. })
    ));

    let mut short = fixture.inputs.clone();
    short.pop();
    assert_eq!(
        stage_g4_ffi_link_plan_v1(
            &fixture.plan,
            &fixture.envelope,
            short,
            fixture.providers.clone(),
            None,
        ),
        Err(StagedFfiLinkError::PlanInputClaimCountMismatch { plan: 3, claims: 2 })
    );

    let other_plan = build_plan(other_target(), "5", &fixture.inputs);
    assert_eq!(
        stage_g4_ffi_link_plan_v1(
            &other_plan,
            &fixture.envelope,
            fixture.inputs.clone(),
            fixture.providers.clone(),
            None,
        ),
        Err(StagedFfiLinkError::PlanTargetMismatch)
    );
    let v6_plan = build_plan(target(), "6", &fixture.inputs);
    assert_eq!(
        stage_g4_ffi_link_plan_v1(
            &v6_plan,
            &fixture.envelope,
            fixture.inputs.clone(),
            fixture.providers.clone(),
            None,
        ),
        Err(StagedFfiLinkError::PlanCodeObjectVersionMismatch {
            plan: CodeObjectVersion::V6,
            g4_claim: CodeObjectVersion::V5,
        })
    );
}

#[test]
fn exact_cardinality_and_aggregate_bounds_fail_before_staging() {
    let fixture = fixture();
    assert_eq!(
        G4FfiClaimEnvelopeV1::new(
            target(),
            CodeObjectVersion::V5,
            fixture.envelope.compiler_required_symbols().to_vec(),
            MAX_G4_RUST_DEFINITION_CLAIMS_V1 + 1,
            0,
            vec![],
        ),
        Err(StagedFfiLinkError::TooManyRustDefinitionClaims)
    );
    assert_eq!(
        G4FfiClaimEnvelopeV1::new(
            target(),
            CodeObjectVersion::V5,
            fixture.envelope.compiler_required_symbols().to_vec(),
            0,
            MAX_G4_KERNEL_CLAIMS_V1 + 1,
            vec![],
        ),
        Err(StagedFfiLinkError::TooManyKernelClaims)
    );
    assert_eq!(
        G4FfiClaimEnvelopeV1::new(
            target(),
            CodeObjectVersion::V5,
            fixture.envelope.compiler_required_symbols().to_vec(),
            MAX_G4_FFI_SYMBOL_CLAIMS_V1 as u32 + 1,
            0,
            vec![fixture.envelope.symbols()[1].clone(); MAX_G4_FFI_SYMBOL_CLAIMS_V1 + 1],
        ),
        Err(StagedFfiLinkError::TooManyFfiSymbolClaims)
    );
    assert_eq!(
        G4FfiClaimEnvelopeV1::new(
            target(),
            CodeObjectVersion::V5,
            fixture.envelope.compiler_required_symbols().to_vec(),
            0,
            0,
            vec![fixture.envelope.symbols()[1].clone()],
        ),
        Err(StagedFfiLinkError::RustDefinitionCountTooSmall {
            claimed: 0,
            exports: 1,
        })
    );

    let long_symbols: Vec<_> = (0..1_900)
        .map(|index| format!("s{index:04}_{}", "x".repeat(210)))
        .collect();
    assert!(
        long_symbols.iter().map(String::len).sum::<usize>() > MAX_G4_FFI_AGGREGATE_TEXT_BYTES_V1
    );
    assert_eq!(
        G4FfiClaimEnvelopeV1::new(
            target(),
            CodeObjectVersion::V5,
            long_symbols.clone(),
            1,
            0,
            vec![],
        ),
        Err(StagedFfiLinkError::AggregateTextBoundExceeded)
    );
    assert_eq!(
        ExpectedFinalDefinedSymbolsClaimV1::new(
            long_symbols,
            vec![
                InputSymbolEvidenceCoverageClaimV1::new(
                    fixture.inputs[0].identity(),
                    fixture.inputs[0].kind(),
                    FinalSymbolEvidenceSourceClaimV1::BoundedInputInspection,
                    [1; 32],
                )
                .unwrap()
            ],
        ),
        Err(StagedFfiLinkError::AggregateTextBoundExceeded)
    );
}

#[test]
fn abi_owner_provider_effect_and_semantic_changes_alter_the_opaque_identity() {
    let fixture = fixture();
    let original = stage(&fixture);

    let import_changed_effect = symbol(
        G4FfiDirectionClaimV1::Import,
        "external_add",
        IMPORT_ABI,
        owner(1, "external_add"),
        declared(target(), CodeObjectVersion::V5, "write_global", 0x11),
    );
    let export_changed_abi = symbol(
        G4FfiDirectionClaimV1::Export,
        "rust_helper",
        "C(u64[size=8,align=8])->u64[size=8,align=8]",
        owner(9, "rust_helper"),
        declared(target(), CodeObjectVersion::V5, "none", 0x33),
    );
    let mut changed_symbols = vec![import_changed_effect, export_changed_abi];
    changed_symbols.sort_by(|left, right| left.symbol().cmp(right.symbol()));
    let changed_envelope = G4FfiClaimEnvelopeV1::new(
        target(),
        CodeObjectVersion::V5,
        fixture.envelope.compiler_required_symbols().to_vec(),
        1,
        1,
        changed_symbols,
    )
    .unwrap();
    assert_ne!(fixture.envelope.identity(), changed_envelope.identity());

    let mut changed_providers: Vec<_> = changed_envelope
        .symbols()
        .iter()
        .map(|symbol| {
            let input = match symbol.provider_class() {
                G4SymbolProviderClassClaimV1::ExternalPlanInput => fixture
                    .inputs
                    .iter()
                    .find(|input| input.identity() == fixture.external_input)
                    .unwrap(),
                G4SymbolProviderClassClaimV1::CompilerModuleInput => fixture
                    .inputs
                    .iter()
                    .find(|input| input.identity() == fixture.compiler_module)
                    .unwrap(),
            };
            FfiSymbolProviderBindingClaimV1::new(
                symbol.contract_identity(),
                symbol.declaration_owner().identity(),
                input.identity(),
                input.kind(),
                input.producer().identity(),
            )
        })
        .collect();
    changed_providers.sort_by_key(FfiSymbolProviderBindingClaimV1::contract_identity);
    let changed = stage_g4_ffi_link_plan_v1(
        &fixture.plan,
        &changed_envelope,
        fixture.inputs.clone(),
        changed_providers,
        Some(fixture.final_symbols.clone()),
    )
    .unwrap();
    assert_ne!(original.identity(), changed.identity());

    let mut changed_inputs = fixture.inputs.clone();
    let external_index = changed_inputs
        .iter()
        .position(|input| input.identity() == fixture.external_input)
        .unwrap();
    let external = changed_inputs[external_index].clone();
    changed_inputs[external_index] = FfiPlanInputClaimV1::new(
        external.identity(),
        external.kind(),
        external.role(),
        producer(88, "different-producer-claim"),
    );
    let mut reciprocal = fixture.providers.clone();
    let provider_index = reciprocal
        .iter()
        .position(|binding| binding.provider_input_identity() == fixture.external_input)
        .unwrap();
    let binding = reciprocal[provider_index];
    reciprocal[provider_index] = FfiSymbolProviderBindingClaimV1::new(
        binding.contract_identity(),
        binding.declaration_owner_identity(),
        binding.provider_input_identity(),
        binding.provider_input_kind(),
        changed_inputs[external_index].producer().identity(),
    );
    let producer_changed = stage_g4_ffi_link_plan_v1(
        &fixture.plan,
        &fixture.envelope,
        changed_inputs,
        reciprocal,
        Some(fixture.final_symbols.clone()),
    )
    .unwrap();
    assert_ne!(original.identity(), producer_changed.identity());
}

#[test]
fn canonical_staging_bytes_have_stable_golden_identities() {
    let fixture = fixture();
    let staged = stage(&fixture);
    assert!(
        fixture
            .envelope
            .canonical_bytes()
            .starts_with(b"FE2O3/G4-FFI-ASSERTION-ONLY-ENVELOPE/V1\0")
    );
    assert!(
        fixture
            .final_symbols
            .canonical_bytes()
            .starts_with(b"FE2O3/EXPECTED-FINAL-DEFINED-SYMBOLS-CLAIM/V1\0")
    );
    assert_eq!(
        fixture.envelope.identity().as_bytes(),
        &<[u8; 32]>::from(Sha256::digest(fixture.envelope.canonical_bytes()))
    );
    assert_eq!(
        fixture.final_symbols.identity().as_bytes(),
        &<[u8; 32]>::from(Sha256::digest(fixture.final_symbols.canonical_bytes()))
    );
    assert_eq!(
        hex(fixture.envelope.identity().as_bytes()),
        "ce28a414534a24a46a0d1b32abd059af6751748ab7adf5ffb592a81af5a9cfbf"
    );
    assert_eq!(
        hex(fixture.final_symbols.identity().as_bytes()),
        "481cc54741209b26cd37fa43aa45a280f698930bb1443cd53e61601370749bfc"
    );
    assert_eq!(
        hex(staged.identity().as_bytes()),
        "094118e3859aacaf4a6c18d7388e39ca12a803fe1dd40568100f828a244b654a"
    );
}

fn opposite_kind(kind: WorkerInputKindV1) -> WorkerInputKindV1 {
    match kind {
        WorkerInputKindV1::LlvmBitcode => WorkerInputKindV1::AmdGpuRelocatable,
        WorkerInputKindV1::AmdGpuRelocatable => WorkerInputKindV1::LlvmBitcode,
    }
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
