const PRE_HARDWARE_EXPORT_SCHEMA: &str = "fe2o3-tutorial-pre-hardware-transaction-export-v1";
const PRE_HARDWARE_RECORD_SCHEMA: &str = "fe2o3-tutorial-pre-hardware-record-v1";
const PRE_HARDWARE_RESULT_NAME: &str = "pre-hardware-transaction-export-v1.json";
const PRE_HARDWARE_RECORD_DOMAIN: &[u8] = b"fe2o3-tutorial-pre-hardware-record-v1\0";

// These bytes all come from the admitted repository, the protected execution envelope, or the
// recovered V5 result. Hardware and semantic-suite observations belong to later phases.
const COMPILER_EVIDENCE_KINDS: &[&str] = &[
    "artifact",
    "artifact-inspection",
    "capability-analysis",
    "capability-closure",
    "compiler-input",
    "compiler-policy",
    "host-admission",
    "launch-contract",
    "llvm-module",
    "lowering",
    "machine-refinement",
    "numerical-policy",
    "optimized-kir-v13",
    "proof-checker",
    "proof-evidence",
    "proof-obligation-set",
    "sealed-production-receipt",
    "semantic-mir",
    "simulation-bundle-v8",
    "source-closure",
    "source-mir-to-kir-refinement",
    "target-capability-decision",
    "target-identity",
];

const PENDING_EVIDENCE_KINDS: &[&str] = &[
    "driver-identity",
    "hardware",
    "negative-fixture-set",
    "runtime-identity",
];

fn assemble_pre_hardware_transaction_v1(
    context: &RequestContext,
    recovered: &RecoveredProtectedFixtureResultV1,
    output_directory: &Path,
) -> ResultV1<TutorialProductionTransactionReceiptV1> {
    recovered.revalidate_currentness()?;
    let (objects, ordinal) = compiler_evidence_objects_v1(context, recovered)?;
    let record = pre_hardware_record_v1(context, recovered, &objects, ordinal)?;
    validate_pre_hardware_record_v1(context, recovered.production_result(), &record, &objects)?;

    let request = object(&context.document, "request")?;
    let envelope = serde_json::json!({
        "candidate": required(request, "candidate", "request")?,
        "fixtureId": context.fixture_id,
        "record": record,
        "requestBindingSha256": context.request_binding_sha256,
        "schema": PRE_HARDWARE_EXPORT_SCHEMA,
    });
    let export_bytes = canonical_document(&envelope)?;
    publish_transaction_export(
        output_directory,
        PRE_HARDWARE_RESULT_NAME,
        &export_bytes,
        &objects,
        None,
    )?;

    Ok(TutorialProductionTransactionReceiptV1 {
        fixture_id: context.fixture_id.clone(),
        request_binding_sha256: context.request_binding_sha256.clone(),
        transaction_sha256: hex32(
            recovered
                .production_result()
                .transaction()
                .identity()
                .sha256(),
        ),
        export_sha256: hex_sha256(&export_bytes),
        output_directory: output_directory.to_path_buf(),
    })
}

fn compiler_evidence_objects_v1(
    context: &RequestContext,
    recovered: &RecoveredProtectedFixtureResultV1,
) -> ResultV1<(BTreeMap<String, Vec<u8>>, usize)> {
    let result = recovered.production_result();
    let handoff = result.handoff();
    let bundle = VerifiedSimulationBundleV8::from_canonical_bytes(
        result.simulation_bundle().canonical_bytes().to_vec(),
    )
    .map_err(|error| {
        TutorialProductionTransactionErrorV1::new(
            TutorialProductionTransactionErrorCodeV1::InvalidProductionResult,
            format!("simulation Bundle V8 failed typed decoding: {error}"),
        )
    })?;
    bundle.revalidate().map_err(|error| {
        TutorialProductionTransactionErrorV1::new(
            TutorialProductionTransactionErrorCodeV1::InvalidProductionResult,
            format!("simulation Bundle V8 failed revalidation: {error}"),
        )
    })?;
    let (_, module) = VerifiedCanonicalKernelIrV13::from_canonical_bytes_with_module(
        handoff.executable_kir().canonical_preimage().to_vec(),
    )
    .map_err(|error| {
        TutorialProductionTransactionErrorV1::new(
            TutorialProductionTransactionErrorCodeV1::InvalidProductionResult,
            format!("optimized KIR V13 failed typed decoding: {error}"),
        )
    })?;
    let ordinal = module
        .kernels
        .iter()
        .position(|kernel| {
            kernel.id.as_str() == context.kernel_symbol
                || kernel.entry.as_str() == context.kernel_symbol
        })
        .ok_or_else(|| {
            TutorialProductionTransactionErrorV1::new(
                TutorialProductionTransactionErrorCodeV1::EvidenceMismatch,
                "requested kernel symbol is absent from exact KIR V13",
            )
        })?;
    if bundle.target() != context.target
        || bundle.semantic_mir()
            != handoff
                .legacy_handoff()
                .capsule()
                .receipts()
                .semantic_mir()
                .canonical_preimage()
        || handoff.subjects().len() != module.kernels.len()
        || handoff.obligation_roster().len() != module.kernels.len()
        || result.capability_associations().entries().len() != module.kernels.len()
    {
        return fail(
            TutorialProductionTransactionErrorCodeV1::EvidenceMismatch,
            "V5 result, Bundle V8, KIR V13, and kernel-root rosters differ",
        );
    }

    let policy = recovered
        .envelope
        .wire()
        .compiler_execution_receipt()
        .policy();
    if policy.identity().as_bytes() != &handoff.inputs().compiler_policy() {
        return fail(
            TutorialProductionTransactionErrorCodeV1::EvidenceMismatch,
            "protected compiler policy differs from the V5 handoff",
        );
    }
    require_exact_measurement(
        result.object_output().output_sha256(),
        result.object_output().output_bytes(),
        recovered.ancillary.object_bytes(),
        "object/HSACO",
    )?;
    let checker = recovered
        .ancillary
        .checker_evidence_identity()
        .map_err(|error| {
            TutorialProductionTransactionErrorV1::new(
                TutorialProductionTransactionErrorCodeV1::EvidenceMismatch,
                format!("authenticated checker evidence is invalid: {error}"),
            )
        })?;
    if checker.sha256()
        != authenticated_compiler_capability_evidence_identity_v5(
            recovered.ancillary.checker_evidence_bytes(),
        )
        .map_err(|error| {
            TutorialProductionTransactionErrorV1::new(
                TutorialProductionTransactionErrorCodeV1::EvidenceMismatch,
                format!("authenticated checker evidence cannot be measured: {error}"),
            )
        })?
        .sha256()
    {
        return fail(
            TutorialProductionTransactionErrorCodeV1::EvidenceMismatch,
            "authenticated checker evidence identity changed",
        );
    }

    let target_closure = handoff.target_closure().canonical_bytes().to_vec();
    let receipts = handoff.legacy_handoff().capsule().receipts();
    let objects = BTreeMap::from([
        (
            "artifact".to_owned(),
            recovered.ancillary.object_bytes().to_vec(),
        ),
        (
            "artifact-inspection".to_owned(),
            result.object_output().canonical_bytes().to_vec(),
        ),
        (
            "capability-analysis".to_owned(),
            handoff.final_graph_report().canonical_results().to_vec(),
        ),
        ("capability-closure".to_owned(), target_closure.clone()),
        (
            "compiler-input".to_owned(),
            compiler_input_preimage_v1(context)?,
        ),
        (
            "compiler-policy".to_owned(),
            policy.canonical_bytes().to_vec(),
        ),
        (
            "host-admission".to_owned(),
            recovered
                .envelope
                .canonical_evidence_view()
                .exact_canonical_bytes()
                .to_vec(),
        ),
        ("launch-contract".to_owned(), target_closure.clone()),
        (
            "llvm-module".to_owned(),
            handoff
                .legacy_handoff()
                .module_handoff()
                .module_bytes()
                .to_vec(),
        ),
        (
            "lowering".to_owned(),
            receipts.amdgpu_lowering().canonical_preimage().to_vec(),
        ),
        (
            "machine-refinement".to_owned(),
            result.machine_refinement().canonical_preimage().to_vec(),
        ),
        (
            "numerical-policy".to_owned(),
            result.proof_owner().canonical_bytes().to_vec(),
        ),
        (
            "optimized-kir-v13".to_owned(),
            handoff.executable_kir().canonical_preimage().to_vec(),
        ),
        (
            "proof-checker".to_owned(),
            recovered.ancillary.checker_evidence_bytes().to_vec(),
        ),
        (
            "proof-evidence".to_owned(),
            result.capability_associations().entries()[ordinal]
                .result_set_bytes()
                .to_vec(),
        ),
        (
            "proof-obligation-set".to_owned(),
            handoff.obligation_roster()[ordinal]
                .canonical_bytes()
                .to_vec(),
        ),
        (
            "sealed-production-receipt".to_owned(),
            recovered.ancillary.production_result_bytes().to_vec(),
        ),
        ("semantic-mir".to_owned(), bundle.semantic_mir().to_vec()),
        (
            "simulation-bundle-v8".to_owned(),
            result.simulation_bundle().canonical_bytes().to_vec(),
        ),
        (
            "source-closure".to_owned(),
            context.source_closure_preimage.clone(),
        ),
        (
            "source-mir-to-kir-refinement".to_owned(),
            handoff.source_refinement().canonical_preimage().to_vec(),
        ),
        (
            "target-capability-decision".to_owned(),
            target_closure.clone(),
        ),
        ("target-identity".to_owned(), target_closure),
    ]);
    validate_compiler_evidence_roster_v1(&objects)?;
    Ok((objects, ordinal))
}

fn compiler_input_preimage_v1(context: &RequestContext) -> ResultV1<Vec<u8>> {
    let manifest_bytes = read_repository_file(&context.repository, MANIFEST_PATH, MAX_JSON_BYTES)?;
    let manifest = parse_repository_document(&manifest_bytes, "tutorial manifest")?;
    let manifest = object(&manifest, "tutorial manifest")?;
    let fixture = find_fixture(
        required(manifest, "compilerFixtures", "tutorial manifest")?,
        &context.fixture_id,
        "tutorial manifest.compilerFixtures",
    )?;
    let mut compiler_input = object(
        required(fixture, "compilerInput", "tutorial fixture")?,
        "tutorial fixture.compilerInput",
    )?
    .clone();
    compiler_input.remove("contractSha256");
    let subject = serde_json::json!({
        "compilerInput": compiler_input,
        "fixtureId": required(fixture, "fixtureId", "tutorial fixture")?,
        "matrix": required(fixture, "matrix", "tutorial fixture")?,
        "target": required(fixture, "target", "tutorial fixture")?,
    });
    let mut preimage = FIXTURE_INPUT_DOMAIN.to_vec();
    preimage.extend_from_slice(&canonical_json(&subject)?);
    let request = object(&context.document, "request")?;
    let input = object(
        required(
            object(required(request, "fixture", "request")?, "request.fixture")?,
            "compilerInput",
            "request.fixture",
        )?,
        "request.fixture.compilerInput",
    )?;
    if input.get("contractSha256").and_then(Value::as_str) != Some(hex_sha256(&preimage).as_str()) {
        return fail(
            TutorialProductionTransactionErrorCodeV1::SourceMismatch,
            "compiler-input preimage differs from the admitted fixture contract",
        );
    }
    Ok(preimage)
}

fn pre_hardware_record_v1(
    context: &RequestContext,
    recovered: &RecoveredProtectedFixtureResultV1,
    objects: &BTreeMap<String, Vec<u8>>,
    ordinal: usize,
) -> ResultV1<Value> {
    let request = object(&context.document, "request")?;
    let fixture = object(required(request, "fixture", "request")?, "request.fixture")?;
    let input = object(
        required(fixture, "compilerInput", "request.fixture")?,
        "request.fixture.compilerInput",
    )?;
    let kernel = object(
        required(request, "capabilityKernel", "request")?,
        "request.capabilityKernel",
    )?;
    let result = recovered.production_result();
    let handoff = result.handoff();
    let bundle = VerifiedSimulationBundleV8::from_canonical_bytes(
        result.simulation_bundle().canonical_bytes().to_vec(),
    )
    .map_err(|error| {
        TutorialProductionTransactionErrorV1::new(
            TutorialProductionTransactionErrorCodeV1::InvalidProductionResult,
            error.to_string(),
        )
    })?;
    let association = &result.capability_associations().entries()[ordinal];
    let checker = recovered
        .ancillary
        .checker_evidence_identity()
        .map_err(|error| {
            TutorialProductionTransactionErrorV1::new(
                TutorialProductionTransactionErrorCodeV1::EvidenceMismatch,
                error.to_string(),
            )
        })?;
    let subject = handoff.subjects()[ordinal];
    let mut references = objects
        .iter()
        .map(|(kind, payload)| (kind.clone(), object_reference(payload)))
        .collect::<Map<_, _>>();
    references.insert(
        "simulator".to_owned(),
        required(request, "simulatorEvidence", "request")?.clone(),
    );
    let compiler_input = [
        "cargoLockSha256",
        "contractSha256",
        "packageManifestSha256",
        "sourceClosureSha256",
    ]
    .into_iter()
    .map(|field| {
        Ok((
            field.to_owned(),
            required(input, field, "request.fixture.compilerInput")?.clone(),
        ))
    })
    .collect::<Result<Map<_, _>, TutorialProductionTransactionErrorV1>>()?;
    let graph = serde_json::json!({
        "bundleContentIdentitySha256": hex32(*bundle.identity().as_bytes()),
        "finalGraphEpoch": bundle.final_graph_epoch(),
        "kernelCount": bundle.kernel_count(),
        "productionKirIdentitySha256": hex32(*bundle.canonical_kir_v13_digest()),
        "semanticMirIdentitySha256": hex32(bundle.semantic_mir_identity()),
    });
    let mut record = serde_json::json!({
        "capabilityClosure": {
            "requirements": required(
                object(required(kernel, "capabilityClosure", "request.capabilityKernel")?, "request.capabilityKernel.capabilityClosure")?,
                "requirements",
                "request.capabilityKernel.capabilityClosure",
            )?,
            "sha256": hex32(handoff.target_closure().closure_identity()),
            "status": "compiler-complete",
        },
        "compilerInput": compiler_input,
        "evidenceFiles": references,
        "fixtureId": context.fixture_id,
        "graph": graph,
        "hardware": {
            "commandSha256": context.hardware_command_sha256,
            "lane": context.hardware_lane,
            "status": "pending-authenticated-observation",
            "target": context.target,
            "timeoutSeconds": context.hardware_timeout_seconds,
        },
        "kernelSymbol": context.kernel_symbol,
        "lessonIds": required(kernel, "lessonIds", "request.capabilityKernel")?,
        "pendingEvidence": PENDING_EVIDENCE_KINDS,
        "preHardwareBindingSha256": "0000000000000000000000000000000000000000000000000000000000000000",
        "productionEvidence": {
            "artifactInspectionSha256": hex_sha256(evidence(objects, "artifact-inspection")?),
            "artifactSha256": hex32(result.object_output().output_sha256()),
            "capabilityAnalysisSha256": hex_sha256(evidence(objects, "capability-analysis")?),
            "capabilityClosureSha256": hex32(handoff.target_closure().closure_identity()),
            "compilerPolicySha256": hex32(handoff.inputs().compiler_policy()),
            "finalOptimizedKirSha256": hex32(*bundle.canonical_kir_v13_digest()),
            "launchContractSha256": hex32(*subject.launch_contract().digest().as_bytes()),
            "loweringIdentitySha256": hex32(*handoff.legacy_handoff().capsule().receipts().amdgpu_lowering().identity().sha256()),
            "machineRefinementSha256": hex32(result.machine_refinement().identity().sha256()),
            "numericalPolicyEvidenceSha256": hex_sha256(evidence(objects, "numerical-policy")?),
            "proofCheckerSha256": hex32(checker.sha256()),
            "proofEvidenceSha256": hex32(*association.result_set_identity().digest().as_bytes()),
            "proofObligationSetSha256": hex32(*handoff.obligation_roster()[ordinal].identity().digest().as_bytes()),
            "sealedResultSha256": hex32(result.identity().sha256()),
            "sourceMirIdentitySha256": hex32(bundle.semantic_mir_identity()),
            "sourceMirToKirRefinementSha256": hex32(handoff.source_refinement().identity().sha256()),
            "targetIdentitySha256": hex32(*subject.target_model().digest().as_bytes()),
        },
        "productionTransaction": {
            "allowsFallback": false,
            "allowsPipelineSelection": false,
            "pipelineEntry": PIPELINE_ENTRY,
            "policyVersion": 4,
            "status": "sealed-production-complete",
            "transactionSha256": hex32(result.transaction().identity().sha256()),
        },
        "proof": {
            "checkerSha256": hex32(checker.sha256()),
            "evidenceSha256": hex32(*association.result_set_identity().digest().as_bytes()),
            "obligationSetSha256": hex32(*handoff.obligation_roster()[ordinal].identity().digest().as_bytes()),
            "properties": required(kernel, "requiredProperties", "request.capabilityKernel")?,
            "status": "compiler-complete",
        },
        "schema": PRE_HARDWARE_RECORD_SCHEMA,
        "simulator": {
            "commandSha256": context.simulator_command_sha256,
            "evidenceSha256": sha_field(required(request, "simulatorEvidence", "request")?, "sha256", "request.simulatorEvidence")?,
            "status": "request-bound-external-observation",
            "subjectSha256": hex32(*bundle.canonical_kir_v13_digest()),
        },
        "target": context.target,
        "targetDecision": {
            "capabilityClosureSha256": hex32(handoff.target_closure().closure_identity()),
            "status": "compiler-complete",
            "targetIdentitySha256": hex32(*subject.target_model().digest().as_bytes()),
        },
    });
    bind_pre_hardware_record_v1(&mut record)?;
    Ok(record)
}

fn bind_pre_hardware_record_v1(record: &mut Value) -> ResultV1<()> {
    let record_object = record.as_object_mut().ok_or_else(|| {
        TutorialProductionTransactionErrorV1::new(
            TutorialProductionTransactionErrorCodeV1::RequestSchema,
            "pre-hardware record must be an object",
        )
    })?;
    record_object.remove("preHardwareBindingSha256");
    let binding = domain_sha256(PRE_HARDWARE_RECORD_DOMAIN, record)?;
    record
        .as_object_mut()
        .expect("record object was checked")
        .insert(
            "preHardwareBindingSha256".to_owned(),
            Value::String(binding),
        );
    Ok(())
}

fn validate_pre_hardware_record_v1(
    context: &RequestContext,
    result: &InertProductionCapabilityResultV5,
    record: &Value,
    objects: &BTreeMap<String, Vec<u8>>,
) -> ResultV1<()> {
    validate_compiler_evidence_roster_v1(objects)?;
    validate_evidence_references_v1(
        required(
            object(record, "pre-hardware record")?,
            "evidenceFiles",
            "pre-hardware record",
        )?,
        objects,
        required(
            object(&context.document, "request")?,
            "simulatorEvidence",
            "request",
        )?,
    )?;
    let claimed = sha_field(record, "preHardwareBindingSha256", "pre-hardware record")?;
    let mut subject = object(record, "pre-hardware record")?.clone();
    subject.remove("preHardwareBindingSha256");
    if claimed != domain_sha256(PRE_HARDWARE_RECORD_DOMAIN, &Value::Object(subject))?
        || evidence(objects, "sealed-production-receipt")? != result.canonical_bytes()
    {
        return fail(
            TutorialProductionTransactionErrorCodeV1::EvidenceMismatch,
            "pre-hardware record binding or sealed compiler evidence differs",
        );
    }
    Ok(())
}

fn validate_compiler_evidence_roster_v1(objects: &BTreeMap<String, Vec<u8>>) -> ResultV1<()> {
    let expected = COMPILER_EVIDENCE_KINDS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let observed = objects.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if expected != observed {
        return fail(
            TutorialProductionTransactionErrorCodeV1::MissingEvidence,
            format!(
                "compiler evidence roster differs: missing={:?} extra={:?}",
                expected.difference(&observed).collect::<Vec<_>>(),
                observed.difference(&expected).collect::<Vec<_>>(),
            ),
        );
    }
    if objects
        .values()
        .any(|payload| payload.is_empty() || payload.len() > MAX_EVIDENCE_BYTES)
    {
        return fail(
            TutorialProductionTransactionErrorCodeV1::MissingEvidence,
            "compiler evidence contains an empty or oversized object",
        );
    }
    Ok(())
}

fn validate_exact_evidence_package_v1(
    expected: &BTreeMap<String, Vec<u8>>,
    observed: &BTreeMap<String, Vec<u8>>,
) -> ResultV1<()> {
    validate_compiler_evidence_roster_v1(expected)?;
    validate_compiler_evidence_roster_v1(observed)?;
    if expected != observed {
        return fail(
            TutorialProductionTransactionErrorCodeV1::EvidenceMismatch,
            "compiler evidence package contains substituted bytes",
        );
    }
    Ok(())
}

fn validate_evidence_references_v1(
    value: &Value,
    objects: &BTreeMap<String, Vec<u8>>,
    simulator: &Value,
) -> ResultV1<()> {
    let references = object(value, "pre-hardware evidenceFiles")?;
    let mut expected = COMPILER_EVIDENCE_KINDS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    expected.insert("simulator");
    if references
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>()
        != expected
    {
        return fail(
            TutorialProductionTransactionErrorCodeV1::MissingEvidence,
            "pre-hardware evidence references are omitted or unexpected",
        );
    }
    for (kind, payload) in objects {
        if references.get(kind) != Some(&object_reference(payload)) {
            return fail(
                TutorialProductionTransactionErrorCodeV1::EvidenceMismatch,
                format!("pre-hardware evidence reference for {kind} is substituted"),
            );
        }
    }
    if references.get("simulator") != Some(simulator) {
        return fail(
            TutorialProductionTransactionErrorCodeV1::EvidenceMismatch,
            "pre-hardware record substituted the request-bound simulator evidence",
        );
    }
    Ok(())
}
