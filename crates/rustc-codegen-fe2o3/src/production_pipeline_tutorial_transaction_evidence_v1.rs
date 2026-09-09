fn validate_local_evidence_roster(objects: &BTreeMap<String, Vec<u8>>) -> ResultV1<()> {
    let expected = LOCAL_EVIDENCE_KINDS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let observed = objects.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if expected != observed {
        let missing = expected.difference(&observed).copied().collect::<Vec<_>>();
        let extra = observed.difference(&expected).copied().collect::<Vec<_>>();
        return fail(
            TutorialProductionTransactionErrorCodeV1::MissingEvidence,
            format!("production evidence roster differs: missing={missing:?} extra={extra:?}"),
        );
    }
    for (kind, payload) in objects {
        if payload.is_empty() || payload.len() > MAX_EVIDENCE_BYTES {
            return fail(
                TutorialProductionTransactionErrorCodeV1::MissingEvidence,
                format!("evidence object {kind:?} is empty or oversized"),
            );
        }
    }
    Ok(())
}

fn validate_source_evidence(
    context: &RequestContext,
    objects: &BTreeMap<String, Vec<u8>>,
) -> ResultV1<()> {
    require_exact_evidence(objects, "source-closure", &context.source_closure_preimage)?;
    let input = object(
        required(
            object(
                required(object(&context.document, "request")?, "fixture", "request")?,
                "request.fixture",
            )?,
            "compilerInput",
            "request.fixture",
        )?,
        "request.fixture.compilerInput",
    )?;
    let source_sha256 = hex_sha256(&context.source_closure_preimage);
    if hex_sha256(evidence(objects, "compiler-input")?)
        != string_field(
            required(
                object(
                    required(object(&context.document, "request")?, "fixture", "request")?,
                    "request.fixture",
                )?,
                "compilerInput",
                "request.fixture",
            )?,
            "contractSha256",
            "request.fixture.compilerInput",
        )?
        || input.get("sourceClosureSha256").and_then(Value::as_str) != Some(source_sha256.as_str())
    {
        return fail(
            TutorialProductionTransactionErrorCodeV1::EvidenceMismatch,
            "compiler-input or source-closure evidence differs from the admitted fixture",
        );
    }
    Ok(())
}

fn validate_typed_evidence(
    context: &RequestContext,
    result: &InertProductionCapabilityResultV5,
    objects: &BTreeMap<String, Vec<u8>>,
) -> ResultV1<usize> {
    let handoff = result.handoff();
    let bundle_bytes = evidence(objects, "simulation-bundle-v8")?;
    require_exact_evidence(
        objects,
        "sealed-production-receipt",
        result.canonical_bytes(),
    )?;
    require_exact_evidence(
        objects,
        "optimized-kir-v13",
        handoff.executable_kir().canonical_preimage(),
    )?;
    require_exact_evidence(
        objects,
        "simulation-bundle-v8",
        result.simulation_bundle().canonical_bytes(),
    )?;
    require_exact_evidence(
        objects,
        "capability-analysis",
        handoff.final_graph_report().canonical_results(),
    )?;
    require_exact_evidence(
        objects,
        "source-mir-to-kir-refinement",
        handoff.source_refinement().canonical_preimage(),
    )?;
    require_exact_evidence(
        objects,
        "machine-refinement",
        result.machine_refinement().canonical_preimage(),
    )?;
    let receipts = handoff.legacy_handoff().capsule().receipts();
    require_exact_evidence(
        objects,
        "lowering",
        receipts.amdgpu_lowering().canonical_preimage(),
    )?;
    require_exact_evidence(
        objects,
        "llvm-module",
        handoff.legacy_handoff().module_handoff().module_bytes(),
    )?;
    require_exact_measurement(
        result.llvm_output().output_sha256(),
        result.llvm_output().output_bytes(),
        evidence(objects, "llvm-module")?,
        "LLVM module",
    )?;
    require_exact_measurement(
        result.object_output().output_sha256(),
        result.object_output().output_bytes(),
        evidence(objects, "artifact")?,
        "object/HSACO",
    )?;

    let bundle = VerifiedSimulationBundleV8::from_canonical_bytes(bundle_bytes.to_vec()).map_err(
        |error| {
            TutorialProductionTransactionErrorV1::new(
                TutorialProductionTransactionErrorCodeV1::InvalidProductionResult,
                format!("simulation Bundle V8 failed typed decoding: {error}"),
            )
        },
    )?;
    bundle.revalidate().map_err(|error| {
        TutorialProductionTransactionErrorV1::new(
            TutorialProductionTransactionErrorCodeV1::InvalidProductionResult,
            format!("simulation Bundle V8 failed revalidation: {error}"),
        )
    })?;
    if bundle.target() != context.target {
        return fail(
            TutorialProductionTransactionErrorCodeV1::WrongTarget,
            format!(
                "Bundle V8 target {:?} differs from requested target {:?}",
                bundle.target(),
                context.target
            ),
        );
    }
    let (kir, module) = VerifiedCanonicalKernelIrV13::from_canonical_bytes_with_module(
        evidence(objects, "optimized-kir-v13")?.to_vec(),
    )
    .map_err(|error| {
        TutorialProductionTransactionErrorV1::new(
            TutorialProductionTransactionErrorCodeV1::InvalidProductionResult,
            format!("optimized KIR V13 failed typed decoding: {error}"),
        )
    })?;
    kir.revalidate().map_err(|error| {
        TutorialProductionTransactionErrorV1::new(
            TutorialProductionTransactionErrorCodeV1::InvalidProductionResult,
            format!("optimized KIR V13 failed revalidation: {error}"),
        )
    })?;
    if bundle.canonical_kir_v13() != kir.canonical_bytes()
        || bundle.final_graph_epoch() != handoff.final_graph_report().final_epoch()
        || handoff.final_graph_report().final_graph() != *kir.identity().digest()
    {
        return fail(
            TutorialProductionTransactionErrorCodeV1::EvidenceMismatch,
            "Bundle V8, optimized KIR V13, and final graph report differ",
        );
    }
    require_exact_evidence(objects, "semantic-mir", bundle.semantic_mir())?;
    if bundle.semantic_mir() != receipts.semantic_mir().canonical_preimage()
        || sha256(bundle.semantic_mir()) != handoff.inputs().semantic_mir_identity()
    {
        return fail(
            TutorialProductionTransactionErrorCodeV1::EvidenceMismatch,
            "semantic MIR differs from Bundle V8 or compiler lineage custody",
        );
    }
    if handoff
        .legacy_handoff()
        .capsule()
        .target()
        .as_amd_target_id()
        .processor()
        != context.target
    {
        return fail(
            TutorialProductionTransactionErrorCodeV1::WrongTarget,
            "compiler capsule target differs from the requested tutorial target",
        );
    }
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
    if handoff.subjects().len() != module.kernels.len()
        || handoff.obligation_roster().len() != module.kernels.len()
        || result.capability_associations().entries().len() != module.kernels.len()
    {
        return fail(
            TutorialProductionTransactionErrorCodeV1::EvidenceMismatch,
            "KIR, subject, obligation, and capability-result rosters differ",
        );
    }
    require_exact_evidence(
        objects,
        "proof-obligation-set",
        handoff.obligation_roster()[ordinal].canonical_bytes(),
    )?;
    require_exact_evidence(
        objects,
        "proof-evidence",
        result.capability_associations().entries()[ordinal].result_set_bytes(),
    )?;
    let subject = handoff.subjects()[ordinal];
    require_hash_evidence(
        objects,
        "compiler-policy",
        handoff.inputs().compiler_policy(),
    )?;
    require_hash_evidence(
        objects,
        "capability-closure",
        handoff.target_closure().closure_identity(),
    )?;
    require_hash_evidence(
        objects,
        "target-identity",
        *subject.target_model().digest().as_bytes(),
    )?;
    require_hash_evidence(
        objects,
        "launch-contract",
        *subject.launch_contract().digest().as_bytes(),
    )?;
    Ok(ordinal)
}

fn validate_record(
    context: &RequestContext,
    result: &InertProductionCapabilityResultV5,
    kernel_ordinal: usize,
    record: &Value,
    objects: &BTreeMap<String, Vec<u8>>,
) -> ResultV1<Vec<u8>> {
    let record_object = object(record, "production record")?;
    exact_keys(record_object, RECORD_KEYS, "production record")?;
    let request = object(&context.document, "request")?;
    let request_kernel = object(
        required(request, "capabilityKernel", "request")?,
        "request.capabilityKernel",
    )?;
    let request_fixture = object(required(request, "fixture", "request")?, "request.fixture")?;
    if record_object.get("fixtureId") != Some(&Value::String(context.fixture_id.clone()))
        || record_object.get("target") != Some(&Value::String(context.target.clone()))
        || record_object.get("kernelSymbol") != Some(&Value::String(context.kernel_symbol.clone()))
        || record_object.get("lessonIds") != request_kernel.get("lessonIds")
    {
        return fail(
            TutorialProductionTransactionErrorCodeV1::EvidenceMismatch,
            "production record is stale or belongs to another fixture, target, or kernel",
        );
    }
    let compiler_input = object(
        required(record_object, "compilerInput", "production record")?,
        "production record.compilerInput",
    )?;
    exact_keys(
        compiler_input,
        &[
            "cargoLockSha256",
            "contractSha256",
            "packageManifestSha256",
            "sourceClosureSha256",
        ],
        "production record.compilerInput",
    )?;
    let request_input = object(
        required(request_fixture, "compilerInput", "request.fixture")?,
        "request.fixture.compilerInput",
    )?;
    for field in [
        "cargoLockSha256",
        "contractSha256",
        "packageManifestSha256",
        "sourceClosureSha256",
    ] {
        if compiler_input.get(field) != request_input.get(field) {
            return fail(
                TutorialProductionTransactionErrorCodeV1::SourceMismatch,
                format!("production record compiler input {field} is stale"),
            );
        }
    }
    let references = object(
        required(record_object, "evidenceFiles", "production record")?,
        "production record.evidenceFiles",
    )?;
    exact_keys(
        references,
        EVIDENCE_KINDS,
        "production record.evidenceFiles",
    )?;
    for kind in LOCAL_EVIDENCE_KINDS {
        let expected = object_reference(evidence(objects, kind)?);
        if references.get(*kind) != Some(&expected) {
            return fail(
                TutorialProductionTransactionErrorCodeV1::EvidenceMismatch,
                format!("production record reference for {kind} is substituted"),
            );
        }
    }
    if references.get("simulator") != request.get("simulatorEvidence") {
        return fail(
            TutorialProductionTransactionErrorCodeV1::EvidenceMismatch,
            "production record substituted simulator evidence from the request",
        );
    }
    let production = object(
        required(record_object, "productionEvidence", "production record")?,
        "production record.productionEvidence",
    )?;
    exact_keys(
        production,
        PRODUCTION_EVIDENCE_KEYS,
        "production record.productionEvidence",
    )?;
    if production.get("compilerCommit")
        != object(
            required(request, "candidate", "request")?,
            "request.candidate",
        )?
        .get("compilerCommit")
        || production.get("compilerTree")
            != object(
                required(request, "candidate", "request")?,
                "request.candidate",
            )?
            .get("compilerTree")
    {
        return fail(
            TutorialProductionTransactionErrorCodeV1::SourceMismatch,
            "production record compiler candidate is stale",
        );
    }
    for (claim, kind) in PRODUCTION_EVIDENCE_JOINS {
        let reference = object(
            required(references, kind, "production record.evidenceFiles")?,
            "production evidence reference",
        )?;
        if production.get(*claim) != reference.get("sha256") {
            return fail(
                TutorialProductionTransactionErrorCodeV1::EvidenceMismatch,
                format!("production evidence {claim} differs from {kind}"),
            );
        }
    }
    let bundle = VerifiedSimulationBundleV8::from_canonical_bytes(
        evidence(objects, "simulation-bundle-v8")?.to_vec(),
    )
    .map_err(|error| {
        TutorialProductionTransactionErrorV1::new(
            TutorialProductionTransactionErrorCodeV1::InvalidProductionResult,
            error.to_string(),
        )
    })?;
    if production.get("sourceMirIdentitySha256")
        != Some(&Value::String(hex32(bundle.semantic_mir_identity())))
        || production.get("finalOptimizedKirSha256")
            != Some(&Value::String(hex32(*bundle.canonical_kir_v13_digest())))
    {
        return fail(
            TutorialProductionTransactionErrorCodeV1::EvidenceMismatch,
            "production source MIR or final KIR identity differs from Bundle V8",
        );
    }
    let handoff = result.handoff();
    let association = &result.capability_associations().entries()[kernel_ordinal];
    let checker_identity =
        authenticated_compiler_capability_evidence_identity_v5(evidence(objects, "proof-checker")?)
            .map_err(|error| {
                TutorialProductionTransactionErrorV1::new(
                    TutorialProductionTransactionErrorCodeV1::EvidenceMismatch,
                    format!("checker evidence identity derivation failed: {error}"),
                )
            })?;
    let typed_identities = [
        (
            "loweringIdentitySha256",
            hex32(
                *handoff
                    .legacy_handoff()
                    .capsule()
                    .receipts()
                    .amdgpu_lowering()
                    .identity()
                    .sha256(),
            ),
        ),
        (
            "sourceMirToKirRefinementSha256",
            hex32(handoff.source_refinement().identity().sha256()),
        ),
        (
            "machineRefinementSha256",
            hex32(result.machine_refinement().identity().sha256()),
        ),
        (
            "proofObligationSetSha256",
            hex32(
                *handoff.obligation_roster()[kernel_ordinal]
                    .identity()
                    .digest()
                    .as_bytes(),
            ),
        ),
        (
            "proofEvidenceSha256",
            hex32(*association.result_set_identity().digest().as_bytes()),
        ),
        ("proofCheckerSha256", hex32(checker_identity.sha256())),
    ];
    for (claim, expected) in typed_identities {
        if production.get(claim) != Some(&Value::String(expected)) {
            return fail(
                TutorialProductionTransactionErrorCodeV1::EvidenceMismatch,
                format!("production typed identity {claim} differs from its canonical receipt"),
            );
        }
    }
    validate_record_summaries(context, result, record_object, request_kernel, references)?;
    let claimed_binding = sha_field(record, "recordBindingSha256", "production record")?;
    let mut subject = record_object.clone();
    subject.remove("recordBindingSha256");
    let expected_binding = domain_sha256(RECORD_DOMAIN, &Value::Object(subject))?;
    if claimed_binding != expected_binding {
        return fail(
            TutorialProductionTransactionErrorCodeV1::RequestBinding,
            "production record binding is stale",
        );
    }
    validate_production_negative_fixture_receipt_v1(
        required(record_object, "negativeFixtures", "production record")?,
        evidence(objects, "negative-fixture-set")?,
    )?;
    canonical_document(record)
}

const PRODUCTION_NEGATIVE_FIXTURE_RECEIPT_INCOMPLETE_V1: &str = "production negative-fixture qualification remains pending: no compiler-produced typed production negative-fixture receipt is available";

fn validate_production_negative_fixture_receipt_v1(
    _claimed_summary: &Value,
    _archived_evidence: &[u8],
) -> ResultV1<()> {
    // JSON claims and a content-addressed payload cannot replace a compiler-produced receipt.
    fail(
        TutorialProductionTransactionErrorCodeV1::ProtectedCompletionUnavailable,
        PRODUCTION_NEGATIVE_FIXTURE_RECEIPT_INCOMPLETE_V1,
    )
}

fn validate_record_summaries(
    context: &RequestContext,
    result: &InertProductionCapabilityResultV5,
    record: &Map<String, Value>,
    request_kernel: &Map<String, Value>,
    references: &Map<String, Value>,
) -> ResultV1<()> {
    let bundle = VerifiedSimulationBundleV8::from_canonical_bytes(
        result.simulation_bundle().canonical_bytes().to_vec(),
    )
    .map_err(|error| {
        TutorialProductionTransactionErrorV1::new(
            TutorialProductionTransactionErrorCodeV1::InvalidProductionResult,
            format!("simulation Bundle V8 failed typed decoding: {error}"),
        )
    })?;
    let graph = object(
        required(record, "graph", "production record")?,
        "production record.graph",
    )?;
    exact_keys(graph, GRAPH_KEYS, "production record.graph")?;
    let lineage = bundle.source_lineage();
    let expected_graph = serde_json::json!({
        "bundleContentIdentitySha256": hex32(*bundle.identity().as_bytes()),
        "bundleSubjectIdentitySha256": hex32(*bundle.subject_identity()),
        "canonicalKirBytes": bundle.canonical_kir_v13_length(),
        "canonicalKirVersion": 13,
        "finalGraphEpoch": bundle.final_graph_epoch(),
        "kernelAbiIdentitySha256": hex32(*bundle.kernel_abi_identity()),
        "kernelCount": bundle.kernel_count(),
        "productionKirIdentitySha256": hex32(*bundle.canonical_kir_v13_digest()),
        "semanticMirIdentitySha256": hex32(bundle.semantic_mir_identity()),
        "sourceInventoryReceiptSha256": hex32(
            lineage.rustc_identity_inventory_receipt_sha256()
        ),
        "sourcePreflightReceiptSha256": hex32(
            lineage.rustc_preflight_plan_receipt_sha256()
        ),
    });
    if Value::Object(graph.clone()) != expected_graph {
        return fail(
            TutorialProductionTransactionErrorCodeV1::EvidenceMismatch,
            "production graph coordinates differ from exact Bundle V8 custody",
        );
    }
    let transaction = object(
        required(record, "productionTransaction", "production record")?,
        "production record.productionTransaction",
    )?;
    let expected_transaction = serde_json::json!({
        "allowsFallback": false,
        "allowsPipelineSelection": false,
        "pipelineEntry": PIPELINE_ENTRY,
        "policyVersion": 4,
        "status": "sealed-production-complete",
        "transactionSha256": hex32(result.transaction().identity().sha256()),
    });
    if Value::Object(transaction.clone()) != expected_transaction {
        return fail(
            TutorialProductionTransactionErrorCodeV1::EvidenceMismatch,
            "production transaction summary does not name the exact completed V5 transaction",
        );
    }
    let closure = object(
        required(record, "capabilityClosure", "production record")?,
        "production record.capabilityClosure",
    )?;
    exact_keys(
        closure,
        &["requirements", "sha256", "status"],
        "production record.capabilityClosure",
    )?;
    if closure.get("status") != Some(&Value::String("complete".to_owned()))
        || closure.get("requirements")
            != object(
                required(
                    request_kernel,
                    "capabilityClosure",
                    "request.capabilityKernel",
                )?,
                "request.capabilityKernel.capabilityClosure",
            )?
            .get("requirements")
        || closure.get("sha256")
            != object(
                required(references, "capability-closure", "evidence references")?,
                "capability closure reference",
            )?
            .get("sha256")
    {
        return fail(
            TutorialProductionTransactionErrorCodeV1::EvidenceMismatch,
            "capability closure summary differs from typed closure custody",
        );
    }
    let proof = object(
        required(record, "proof", "production record")?,
        "production record.proof",
    )?;
    exact_keys(
        proof,
        &[
            "checkerSha256",
            "evidenceSha256",
            "obligationSetSha256",
            "properties",
            "status",
        ],
        "production record.proof",
    )?;
    let properties = sorted_unique_strings(
        required(proof, "properties", "production record.proof")?,
        "production record.proof.properties",
    )?;
    let required_properties = sorted_unique_strings(
        required(
            request_kernel,
            "requiredProperties",
            "request.capabilityKernel",
        )?,
        "request.capabilityKernel.requiredProperties",
    )?;
    if proof.get("status") != Some(&Value::String("complete".to_owned()))
        || !properties
            .iter()
            .all(|property| required_properties.contains(property))
        || !PRODUCTION_PROOF_PROPERTIES
            .iter()
            .all(|required| properties.iter().any(|property| property == required))
    {
        return fail(
            TutorialProductionTransactionErrorCodeV1::EvidenceMismatch,
            "proof summary is incomplete or exceeds the requested proof contract",
        );
    }
    for (field, claim) in [
        ("checkerSha256", "proofCheckerSha256"),
        ("evidenceSha256", "proofEvidenceSha256"),
        ("obligationSetSha256", "proofObligationSetSha256"),
    ] {
        if proof.get(field)
            != object(
                required(record, "productionEvidence", "production record")?,
                "production evidence",
            )?
            .get(claim)
        {
            return fail(
                TutorialProductionTransactionErrorCodeV1::EvidenceMismatch,
                format!("proof summary typed identity {field} is substituted"),
            );
        }
    }
    for summary in ["simulator", "hardware"] {
        let summary_record = object(
            required(record, summary, "production record")?,
            "production status summary",
        )?;
        if summary_record.get("status") != Some(&Value::String("passed".to_owned()))
            || summary_record.get("evidenceSha256")
                != object(
                    required(references, summary, "evidence references")?,
                    "status evidence reference",
                )?
                .get("sha256")
        {
            return fail(
                TutorialProductionTransactionErrorCodeV1::EvidenceMismatch,
                format!("{summary} status is absent or substituted"),
            );
        }
    }
    let simulator = object(
        required(record, "simulator", "production record")?,
        "production record.simulator",
    )?;
    exact_keys(
        simulator,
        &["commandSha256", "evidenceSha256", "status", "subjectSha256"],
        "production record.simulator",
    )?;
    if simulator.get("commandSha256")
        != Some(&Value::String(context.simulator_command_sha256.clone()))
        || simulator.get("subjectSha256")
            != Some(&Value::String(hex32(*bundle.canonical_kir_v13_digest())))
    {
        return fail(
            TutorialProductionTransactionErrorCodeV1::EvidenceMismatch,
            "simulator summary is stale against its manifest command or final KIR",
        );
    }
    let hardware = object(
        required(record, "hardware", "production record")?,
        "production record.hardware",
    )?;
    exact_keys(
        hardware,
        &[
            "artifactInspectionSha256",
            "artifactSha256",
            "canariesChecked",
            "commandSha256",
            "driverIdentitySha256",
            "evidenceSha256",
            "fullOutputChecked",
            "inputsUnchangedChecked",
            "lane",
            "launchContractSha256",
            "paddingChecked",
            "runtimeIdentitySha256",
            "status",
            "subjectSha256",
            "target",
            "targetIdentitySha256",
            "timeoutSeconds",
        ],
        "production record.hardware",
    )?;
    for check in [
        "canariesChecked",
        "fullOutputChecked",
        "inputsUnchangedChecked",
        "paddingChecked",
    ] {
        if hardware.get(check) != Some(&Value::Bool(true)) {
            return fail(
                TutorialProductionTransactionErrorCodeV1::EvidenceMismatch,
                format!("hardware summary did not complete {check}"),
            );
        }
    }
    for (field, kind) in [
        ("artifactInspectionSha256", "artifact-inspection"),
        ("artifactSha256", "artifact"),
        ("driverIdentitySha256", "driver-identity"),
        ("evidenceSha256", "hardware"),
        ("launchContractSha256", "launch-contract"),
        ("runtimeIdentitySha256", "runtime-identity"),
        ("subjectSha256", "artifact"),
        ("targetIdentitySha256", "target-identity"),
    ] {
        if hardware.get(field)
            != object(
                required(references, kind, "evidence references")?,
                "hardware evidence reference",
            )?
            .get("sha256")
        {
            return fail(
                TutorialProductionTransactionErrorCodeV1::EvidenceMismatch,
                format!("hardware summary {field} is substituted"),
            );
        }
    }
    if hardware.get("commandSha256")
        != Some(&Value::String(context.hardware_command_sha256.clone()))
        || hardware.get("lane") != Some(&Value::String(context.hardware_lane.clone()))
        || hardware.get("target") != Some(&Value::String(context.target.clone()))
        || hardware.get("timeoutSeconds")
            != Some(&Value::Number(context.hardware_timeout_seconds.into()))
    {
        return fail(
            TutorialProductionTransactionErrorCodeV1::WrongTarget,
            "hardware summary is stale against the target-matched manifest command",
        );
    }
    let target = object(
        required(record, "targetDecision", "production record")?,
        "production record.targetDecision",
    )?;
    exact_keys(
        target,
        &["capabilityDecisionSha256", "status", "targetIdentitySha256"],
        "production record.targetDecision",
    )?;
    if target.get("status") != Some(&Value::String("capability-complete".to_owned()))
        || target.get("capabilityDecisionSha256")
            != object(
                required(
                    references,
                    "target-capability-decision",
                    "evidence references",
                )?,
                "target decision reference",
            )?
            .get("sha256")
        || target.get("targetIdentitySha256")
            != object(
                required(references, "target-identity", "evidence references")?,
                "target identity reference",
            )?
            .get("sha256")
    {
        return fail(
            TutorialProductionTransactionErrorCodeV1::EvidenceMismatch,
            "target capability decision summary is substituted",
        );
    }
    Ok(())
}
