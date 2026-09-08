fn preflight_request(
    repository: &Path,
    bytes: Vec<u8>,
    verify_candidate: bool,
) -> ResultV1<RequestContext> {
    let document = parse_canonical_document(&bytes, "transaction request")?;
    let request = object(&document, "transaction request")?;
    exact_keys(request, REQUEST_KEYS, "transaction request")?;
    if string_field(&document, "schema", "transaction request")? != REQUEST_SCHEMA
        || string_field(&document, "roadmapIssue", "transaction request")? != ROADMAP_ISSUE
    {
        return fail(
            TutorialProductionTransactionErrorCodeV1::RequestSchema,
            "transaction request schema or roadmap identity differs",
        );
    }
    let claimed_binding = sha_field(&document, "requestBindingSha256", "request")?;
    let mut subject = request.clone();
    subject.remove("requestBindingSha256");
    let actual_binding = domain_sha256(REQUEST_DOMAIN, &Value::Object(subject))?;
    if claimed_binding != actual_binding {
        return fail(
            TutorialProductionTransactionErrorCodeV1::RequestBinding,
            "transaction request binding is stale",
        );
    }

    let transaction = object(
        required(request, "productionTransaction", "request")?,
        "request.productionTransaction",
    )?;
    exact_keys(
        transaction,
        &[
            "allowsFallback",
            "allowsPipelineSelection",
            "pipelineEntry",
            "policyVersion",
        ],
        "request.productionTransaction",
    )?;
    if transaction.get("allowsFallback") != Some(&Value::Bool(false))
        || transaction.get("allowsPipelineSelection") != Some(&Value::Bool(false))
        || string_field(
            required(request, "productionTransaction", "request")?,
            "pipelineEntry",
            "request.productionTransaction",
        )? != PIPELINE_ENTRY
        || u64_field(
            required(request, "productionTransaction", "request")?,
            "policyVersion",
            "request.productionTransaction",
        )? != 4
    {
        return fail(
            TutorialProductionTransactionErrorCodeV1::RequestSchema,
            "request selects a fallback, alternate pipeline, or policy version",
        );
    }

    let fixture = object(required(request, "fixture", "request")?, "request.fixture")?;
    exact_keys(
        fixture,
        &[
            "compilerInput",
            "fixtureId",
            "hardwareCommand",
            "hardwareLane",
            "hardwareReservation",
            "target",
        ],
        "request.fixture",
    )?;
    let fixture_id = identity_field(
        required(request, "fixture", "request")?,
        "fixtureId",
        "request.fixture",
    )?
    .to_owned();
    let target = string_field(
        required(request, "fixture", "request")?,
        "target",
        "request.fixture",
    )?
    .to_owned();
    let expected_lane = match target.as_str() {
        "gfx942" => "mi300x",
        "gfx950" => "mi350",
        _ => {
            return fail(
                TutorialProductionTransactionErrorCodeV1::WrongTarget,
                format!("tutorial target {target:?} has no authenticated production lane"),
            );
        }
    };
    if string_field(
        required(request, "fixture", "request")?,
        "hardwareLane",
        "request.fixture",
    )? != expected_lane
    {
        return fail(
            TutorialProductionTransactionErrorCodeV1::WrongTarget,
            "hardware lane does not match the requested target",
        );
    }

    let kernel = object(
        required(request, "capabilityKernel", "request")?,
        "request.capabilityKernel",
    )?;
    exact_keys(
        kernel,
        &[
            "capabilityClosure",
            "kernelSymbol",
            "lessonIds",
            "requiredProperties",
        ],
        "request.capabilityKernel",
    )?;
    let kernel_symbol = identity_field(
        required(request, "capabilityKernel", "request")?,
        "kernelSymbol",
        "request.capabilityKernel",
    )?
    .to_owned();
    sorted_unique_strings(
        required(kernel, "lessonIds", "request.capabilityKernel")?,
        "request.capabilityKernel.lessonIds",
    )?;
    sorted_unique_strings(
        required(kernel, "requiredProperties", "request.capabilityKernel")?,
        "request.capabilityKernel.requiredProperties",
    )?;
    validate_reference(
        required(request, "simulatorEvidence", "request")?,
        "request.simulatorEvidence",
    )?;

    let manifest_claim = object(
        required(request, "manifest", "request")?,
        "request.manifest",
    )?;
    exact_keys(
        manifest_claim,
        &["corpusContractSha256", "path", "rawSha256"],
        "request.manifest",
    )?;
    if string_field(
        required(request, "manifest", "request")?,
        "path",
        "request.manifest",
    )? != MANIFEST_PATH
    {
        return fail(
            TutorialProductionTransactionErrorCodeV1::SourceMismatch,
            "request names a noncanonical tutorial manifest",
        );
    }
    sha_field(
        required(request, "manifest", "request")?,
        "corpusContractSha256",
        "request.manifest",
    )?;
    let manifest_bytes = read_repository_file(repository, MANIFEST_PATH, MAX_JSON_BYTES)?;
    if hex_sha256(&manifest_bytes)
        != sha_field(
            required(request, "manifest", "request")?,
            "rawSha256",
            "request.manifest",
        )?
    {
        return fail(
            TutorialProductionTransactionErrorCodeV1::SourceMismatch,
            "tutorial manifest changed after the transaction request was created",
        );
    }
    let manifest = parse_repository_document(&manifest_bytes, "tutorial manifest")?;
    let mut corpus = object(&manifest, "tutorial manifest")?.clone();
    corpus.remove("baseline");
    if domain_sha256(CORPUS_DOMAIN, &Value::Object(corpus))?
        != string_field(
            required(request, "manifest", "request")?,
            "corpusContractSha256",
            "request.manifest",
        )?
    {
        return fail(
            TutorialProductionTransactionErrorCodeV1::SourceMismatch,
            "tutorial manifest corpus contract differs from the transaction request",
        );
    }
    validate_request_against_manifest(&document, &manifest, &fixture_id)?;
    let simulator_command_sha256 = expected_simulator_command_sha256(
        &manifest,
        &fixture_id,
        required(kernel, "lessonIds", "request.capabilityKernel")?,
    )?;
    let hardware_command = required(fixture, "hardwareCommand", "request.fixture")?;
    let hardware_command_sha256 = domain_sha256(COMMAND_DOMAIN, hardware_command)?;
    let hardware_timeout_seconds = u64_field(
        hardware_command,
        "timeoutSeconds",
        "request.fixture.hardwareCommand",
    )?;
    let source_closure_preimage = validate_compiler_inputs(repository, &document)?;
    if verify_candidate {
        validate_candidate(repository, &document)?;
    }
    Ok(RequestContext {
        document,
        canonical_bytes: bytes,
        fixture_id,
        target,
        kernel_symbol,
        request_binding_sha256: actual_binding,
        source_closure_preimage,
        simulator_command_sha256,
        hardware_command_sha256,
        hardware_lane: expected_lane.to_owned(),
        hardware_timeout_seconds,
        repository: repository.to_path_buf(),
    })
}

fn expected_simulator_command_sha256(
    manifest: &Value,
    fixture_id: &str,
    lesson_ids: &Value,
) -> ResultV1<String> {
    let lessons = sorted_unique_strings(lesson_ids, "request.capabilityKernel.lessonIds")?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let qualification = object(
        required(
            object(manifest, "tutorial manifest")?,
            "qualification",
            "tutorial manifest",
        )?,
        "tutorial manifest.qualification",
    )?;
    let suites = required(qualification, "suites", "tutorial manifest.qualification")?
        .as_array()
        .ok_or_else(|| {
            TutorialProductionTransactionErrorV1::new(
                TutorialProductionTransactionErrorCodeV1::RequestSchema,
                "tutorial manifest qualification.suites must be an array",
            )
        })?;
    let mut commands = Vec::<Value>::new();
    for suite in suites {
        let suite = object(suite, "tutorial qualification suite")?;
        if suite.get("gate").and_then(Value::as_str) != Some("semantic-simulation")
            || suite.get("availability").and_then(Value::as_str) != Some("available")
        {
            continue;
        }
        let coverage = required(suite, "coverage", "tutorial qualification suite")?
            .as_array()
            .ok_or_else(|| {
                TutorialProductionTransactionErrorV1::new(
                    TutorialProductionTransactionErrorCodeV1::RequestSchema,
                    "tutorial qualification coverage must be an array",
                )
            })?;
        let covered = coverage.iter().any(|entry| {
            let Some(entry) = entry.as_object() else {
                return false;
            };
            let lesson = entry.get("lessonId").and_then(Value::as_str);
            let fixtures = entry.get("fixtureIds").and_then(Value::as_array);
            lesson.is_some_and(|lesson| lessons.contains(lesson))
                && fixtures.is_some_and(|fixtures| {
                    fixtures
                        .iter()
                        .any(|fixture| fixture.as_str() == Some(fixture_id))
                })
        });
        if covered {
            let command = required(suite, "command", "tutorial qualification suite")?.clone();
            if !commands.contains(&command) {
                commands.push(command);
            }
        }
    }
    if commands.len() != 1 {
        return fail(
            TutorialProductionTransactionErrorCodeV1::MissingEvidence,
            format!(
                "fixture {fixture_id} requires exactly one manifest semantic-simulation command; found {}",
                commands.len()
            ),
        );
    }
    domain_sha256(COMMAND_DOMAIN, &commands[0])
}

fn validate_request_against_manifest(
    request: &Value,
    manifest: &Value,
    fixture_id: &str,
) -> ResultV1<()> {
    let manifest = object(manifest, "tutorial manifest")?;
    let fixture = find_fixture(
        required(manifest, "compilerFixtures", "tutorial manifest")?,
        fixture_id,
        "tutorial manifest.compilerFixtures",
    )?;
    let kernel = find_fixture(
        required(manifest, "capabilityKernels", "tutorial manifest")?,
        fixture_id,
        "tutorial manifest.capabilityKernels",
    )?;
    let compiler_input = object(
        required(fixture, "compilerInput", "tutorial fixture")?,
        "tutorial fixture.compilerInput",
    )?;
    let mut contract_input = compiler_input.clone();
    contract_input.remove("contractSha256");
    let contract_subject = serde_json::json!({
        "compilerInput": contract_input,
        "fixtureId": required(fixture, "fixtureId", "tutorial fixture")?,
        "matrix": required(fixture, "matrix", "tutorial fixture")?,
        "target": required(fixture, "target", "tutorial fixture")?,
    });
    if domain_sha256(FIXTURE_INPUT_DOMAIN, &contract_subject)?
        != string_field(
            required(fixture, "compilerInput", "tutorial fixture")?,
            "contractSha256",
            "tutorial fixture.compilerInput",
        )?
    {
        return fail(
            TutorialProductionTransactionErrorCodeV1::SourceMismatch,
            "tutorial fixture compiler-input contract is stale",
        );
    }
    let request_fixture = required(object(request, "request")?, "fixture", "request")?;
    let request_kernel = required(object(request, "request")?, "capabilityKernel", "request")?;
    for field in ["compilerInput", "fixtureId", "target"] {
        if object(request_fixture, "request.fixture")?.get(field) != fixture.get(field) {
            return fail(
                TutorialProductionTransactionErrorCodeV1::SourceMismatch,
                format!("request.fixture.{field} differs from the tutorial manifest"),
            );
        }
    }
    for field in [
        "capabilityClosure",
        "kernelSymbol",
        "lessonIds",
        "requiredProperties",
    ] {
        if object(request_kernel, "request.capabilityKernel")?.get(field) != kernel.get(field) {
            return fail(
                TutorialProductionTransactionErrorCodeV1::SourceMismatch,
                format!("request.capabilityKernel.{field} differs from the tutorial manifest"),
            );
        }
    }
    let matrix = object(
        required(fixture, "matrix", "tutorial fixture")?,
        "tutorial fixture.matrix",
    )?;
    let expected_command = serde_json::json!({
        "arguments": required(matrix, "runnerArguments", "tutorial fixture.matrix")?,
        "environment": required(matrix, "environment", "tutorial fixture.matrix")?,
        "executable": required(matrix, "runnerPath", "tutorial fixture.matrix")?,
        "timeoutSeconds": 1200,
        "workingDirectory": ".",
    });
    if object(request_fixture, "request.fixture")?.get("hardwareCommand") != Some(&expected_command)
    {
        return fail(
            TutorialProductionTransactionErrorCodeV1::WrongTarget,
            "request hardware command differs from the target-matched manifest command",
        );
    }
    Ok(())
}

fn find_fixture<'a>(
    value: &'a Value,
    fixture_id: &str,
    label: &str,
) -> ResultV1<&'a Map<String, Value>> {
    let array = value.as_array().ok_or_else(|| {
        TutorialProductionTransactionErrorV1::new(
            TutorialProductionTransactionErrorCodeV1::RequestSchema,
            format!("{label} must be an array"),
        )
    })?;
    let mut found = None;
    for entry in array {
        let entry = object(entry, label)?;
        if entry.get("fixtureId").and_then(Value::as_str) == Some(fixture_id) {
            if found.is_some() {
                return fail(
                    TutorialProductionTransactionErrorCodeV1::RequestSchema,
                    format!("{label} duplicates fixture {fixture_id}"),
                );
            }
            found = Some(entry);
        }
    }
    found.ok_or_else(|| {
        TutorialProductionTransactionErrorV1::new(
            TutorialProductionTransactionErrorCodeV1::SourceMismatch,
            format!("{label} omits fixture {fixture_id}"),
        )
    })
}

fn validate_candidate(repository: &Path, request: &Value) -> ResultV1<()> {
    let candidate = object(
        required(object(request, "request")?, "candidate", "request")?,
        "request.candidate",
    )?;
    exact_keys(
        candidate,
        &["compilerCommit", "compilerTree", "worktreeClean"],
        "request.candidate",
    )?;
    if candidate.get("worktreeClean") != Some(&Value::Bool(true)) {
        return fail(
            TutorialProductionTransactionErrorCodeV1::SourceMismatch,
            "transaction candidate is not a clean compiler worktree",
        );
    }
    let status = git(
        repository,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )?;
    let commit = git(repository, &["rev-parse", "--verify", "HEAD"])?;
    let tree = git(repository, &["show", "-s", "--format=%T", "HEAD"])?;
    if !status.is_empty()
        || candidate.get("compilerCommit").and_then(Value::as_str) != Some(commit.as_str())
        || candidate.get("compilerTree").and_then(Value::as_str) != Some(tree.as_str())
    {
        return fail(
            TutorialProductionTransactionErrorCodeV1::SourceMismatch,
            "compiler candidate is dirty, stale, or from another tree",
        );
    }
    Ok(())
}

fn validate_compiler_inputs(repository: &Path, request: &Value) -> ResultV1<Vec<u8>> {
    let fixture = object(
        required(object(request, "request")?, "fixture", "request")?,
        "request.fixture",
    )?;
    for field in [
        "cargoLockSha256",
        "contractSha256",
        "packageManifestSha256",
        "sourceClosureSha256",
    ] {
        sha_field(
            required(fixture, "compilerInput", "request.fixture")?,
            field,
            "request.fixture.compilerInput",
        )?;
    }
    let package_relative = string_field(
        required(fixture, "compilerInput", "request.fixture")?,
        "packageManifest",
        "request.fixture.compilerInput",
    )?;
    let lock_relative = string_field(
        required(fixture, "compilerInput", "request.fixture")?,
        "cargoLockPath",
        "request.fixture.compilerInput",
    )?;
    let package = read_repository_file(repository, package_relative, 1024 * 1024)?;
    let lock = read_repository_file(repository, lock_relative, 8 * 1024 * 1024)?;
    if hex_sha256(&package)
        != string_field(
            required(fixture, "compilerInput", "request.fixture")?,
            "packageManifestSha256",
            "request.fixture.compilerInput",
        )?
        || hex_sha256(&lock)
            != string_field(
                required(fixture, "compilerInput", "request.fixture")?,
                "cargoLockSha256",
                "request.fixture.compilerInput",
            )?
    {
        return fail(
            TutorialProductionTransactionErrorCodeV1::SourceMismatch,
            "package manifest or Cargo.lock changed after fixture admission",
        );
    }
    let package_manifest_path =
        protected_relative(repository, package_relative, "package manifest")?;
    let package_root = package_manifest_path.parent().ok_or_else(|| {
        TutorialProductionTransactionErrorV1::new(
            TutorialProductionTransactionErrorCodeV1::SourceMismatch,
            "package manifest has no package root",
        )
    })?;
    let preimage = source_closure_preimage(repository, package_root)?;
    if hex_sha256(&preimage)
        != string_field(
            required(fixture, "compilerInput", "request.fixture")?,
            "sourceClosureSha256",
            "request.fixture.compilerInput",
        )?
    {
        return fail(
            TutorialProductionTransactionErrorCodeV1::SourceMismatch,
            "Rust source closure changed after fixture admission",
        );
    }
    Ok(preimage)
}

fn source_closure_preimage(repository: &Path, package_root: &Path) -> ResultV1<Vec<u8>> {
    let mut files = Vec::new();
    collect_rust_sources(package_root, &mut files)?;
    files.sort();
    if files.is_empty() || files.len() > MAX_SOURCE_FILES {
        return fail(
            TutorialProductionTransactionErrorCodeV1::SourceMismatch,
            "package Rust source closure has an invalid file count",
        );
    }
    let mut preimage = SOURCE_CLOSURE_DOMAIN.to_vec();
    let mut total = 0_usize;
    for path in files {
        let payload = read_regular(&path, MAX_SOURCE_BYTES as u64, "package Rust source")?;
        std::str::from_utf8(&payload).map_err(|error| {
            TutorialProductionTransactionErrorV1::new(
                TutorialProductionTransactionErrorCodeV1::SourceMismatch,
                format!("package Rust source is not UTF-8: {error}"),
            )
        })?;
        total = total.checked_add(payload.len()).ok_or_else(|| {
            TutorialProductionTransactionErrorV1::new(
                TutorialProductionTransactionErrorCodeV1::SourceMismatch,
                "package Rust source closure length overflowed",
            )
        })?;
        if total > MAX_SOURCE_BYTES {
            return fail(
                TutorialProductionTransactionErrorCodeV1::SourceMismatch,
                "package Rust source closure exceeds its byte bound",
            );
        }
        let relative = path.strip_prefix(repository).map_err(|_| {
            TutorialProductionTransactionErrorV1::new(
                TutorialProductionTransactionErrorCodeV1::SourceMismatch,
                "package Rust source escapes the repository",
            )
        })?;
        let relative = portable_path(relative, "package Rust source")?;
        preimage.extend_from_slice(&(relative.len() as u32).to_le_bytes());
        preimage.extend_from_slice(relative.as_bytes());
        preimage.extend_from_slice(&(payload.len() as u64).to_le_bytes());
        preimage.extend_from_slice(&payload);
    }
    Ok(preimage)
}

fn collect_rust_sources(directory: &Path, files: &mut Vec<PathBuf>) -> ResultV1<()> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| io_error("cannot read package source directory", error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| io_error("cannot enumerate package source directory", error))?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| io_error("cannot inspect package source", error))?;
        if metadata.file_type().is_symlink() {
            return fail(
                TutorialProductionTransactionErrorCodeV1::SourceMismatch,
                "package source closure contains a symlink",
            );
        }
        if metadata.is_dir() {
            if entry.file_name() != "target" {
                collect_rust_sources(&path, files)?;
            }
        } else if metadata.is_file()
            && path.extension().and_then(|value| value.to_str()) == Some("rs")
        {
            files.push(path);
            if files.len() > MAX_SOURCE_FILES {
                return fail(
                    TutorialProductionTransactionErrorCodeV1::SourceMismatch,
                    "package Rust source closure exceeds its file bound",
                );
            }
        }
    }
    Ok(())
}
