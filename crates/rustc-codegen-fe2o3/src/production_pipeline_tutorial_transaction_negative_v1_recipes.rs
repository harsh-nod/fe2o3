use super::*;
use fe2o3_kernel_ir::{DiagnosticCode, FunctionId, Operation, OperationKind, verify_module};
use fe2o3_mir_model::semantic_mir_v1::{SemanticFunctionAbiV1, SemanticFunctionDeclV1};

const DECLARATIONS_PATH: &str = "config/tutorial-negative-fixtures-v1.json";
const MAX_DECLARATION_BYTES: u64 = 8 * 1024 * 1024;
const MAX_REQUIRED_CASES: usize = 64;
const FIXTURE_BINDING_DOMAIN: &[u8] = b"fe2o3-tutorial-negative-fixture-binding-v2\0";

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RequiredNegativeRosterV1 {
    manifest_sha256: String,
    fixture_binding_sha256: String,
    cases: Vec<RequiredNegativeCaseV1>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RequiredNegativeCaseV1 {
    case_id: String,
    category: String,
    mutation_class: String,
    declaration_sha256: String,
}

impl RequiredNegativeRosterV1 {
    pub(super) fn load(
        context: &RequestContext,
        source_closure: [u8; 32],
        compiler_input: [u8; 32],
    ) -> ResultV1<Self> {
        let bytes = read_repository_file(
            &context.repository,
            DECLARATIONS_PATH,
            MAX_DECLARATION_BYTES,
        )?;
        let document = parse_repository_document(&bytes, "negative declarations")?;
        if document.get("schema").and_then(Value::as_str)
            != Some("fe2o3-tutorial-negative-fixture-declarations-v2")
        {
            return mismatch("negative declarations have an unsupported schema");
        }
        let fixtures = document
            .get("fixtures")
            .ok_or_else(|| replay_error("missing negative fixtures"))?;
        if fixtures
            .as_array()
            .is_none_or(|rows| rows.is_empty() || rows.len() > 256)
        {
            return mismatch("negative fixture roster exceeds its bound");
        }
        let fixture = find_fixture(fixtures, &context.fixture_id, "negative fixtures")?;
        let properties = sorted_unique_strings(
            context
                .document
                .pointer("/capabilityKernel/requiredProperties")
                .ok_or_else(|| replay_error("negative replay lacks required properties"))?,
            "negative replay required properties",
        )?;
        let binding = serde_json::json!({
            "compilerInputContractSha256": hex32(compiler_input),
            "fixtureId": context.fixture_id,
            "kernelSymbol": context.kernel_symbol,
            "requiredProperties": properties,
            "sourceClosureSha256": hex32(source_closure),
            "target": context.target,
        });
        for (field, expected) in binding.as_object().expect("binding object") {
            if field != "requiredProperties" && fixture.get(field) != Some(expected) {
                return mismatch("negative declarations differ from the exact source fixture");
            }
        }
        let expected_binding = domain_sha256(FIXTURE_BINDING_DOMAIN, &binding)?;
        if fixture.get("bindingSha256").and_then(Value::as_str) != Some(expected_binding.as_str()) {
            return mismatch("negative fixture declaration binding is stale");
        }
        Self::from_fixture(&bytes, &Value::Object(fixture.clone()), expected_binding)
    }

    pub(super) fn from_fixture(
        bytes: &[u8],
        fixture: &Value,
        fixture_binding: String,
    ) -> ResultV1<Self> {
        let cases = fixture
            .get("cases")
            .and_then(Value::as_array)
            .filter(|cases| !cases.is_empty() && cases.len() <= MAX_REQUIRED_CASES)
            .ok_or_else(|| replay_error("negative case roster exceeds its bound"))?;
        let mut seen = BTreeSet::new();
        let mut required = Vec::with_capacity(cases.len());
        for case in cases {
            for field in ["fixtureId", "target"] {
                if case.get(field) != fixture.get(field) {
                    return mismatch("negative case belongs to another fixture");
                }
            }
            if case.get("fixtureBindingSha256").and_then(Value::as_str)
                != Some(fixture_binding.as_str())
            {
                return mismatch("negative case has a stale fixture binding");
            }
            let case_id = identity_field(case, "caseId", "negative case")?;
            let category = identity_field(case, "category", "negative case")?;
            let mutation = case
                .get("mutation")
                .ok_or_else(|| replay_error("negative case lacks mutation"))?;
            let mutation_class = identity_field(mutation, "class", "negative mutation")?;
            if [case_id, category, mutation_class]
                .iter()
                .any(|text| text.len() > 256)
                || !seen.insert(case_id)
            {
                return mismatch("negative cases have oversized or duplicate identities");
            }
            required.push(RequiredNegativeCaseV1 {
                case_id: case_id.to_owned(),
                category: category.to_owned(),
                mutation_class: mutation_class.to_owned(),
                declaration_sha256: hex_sha256(&canonical_document(case)?),
            });
        }
        Ok(Self {
            manifest_sha256: hex_sha256(bytes),
            fixture_binding_sha256: fixture_binding,
            cases: required,
        })
    }

    pub(super) fn pending_cases(&self) -> Vec<Value> {
        self.cases.iter().map(|case| serde_json::json!({
            "caseId": case.case_id,
            "declarationSha256": case.declaration_sha256,
            "status": "not-executed",
            "reason": "configured mutation requires its own exact production lifecycle; supplemental admission checks do not satisfy it",
        })).collect()
    }
}

pub(super) fn reject_unwinding_kernel_abi(
    source: &AdmittedInertSemanticMirV1,
    root: SemanticFunctionIdV1,
) -> ResultV1<Value> {
    let function = &source.functions()[root.index() as usize];
    let old = function.abi();
    if old.can_unwind() || function.role() != SemanticFunctionRoleV1::KernelRoot {
        return mismatch("ABI negative requires a non-unwinding kernel positive control");
    }
    let changed_abi = SemanticFunctionAbiV1::from_rustc_with_source_signature(
        old.identity(),
        old.layout_identity(),
        old.canon_abi(),
        old.extern_abi(),
        true,
        old.c_variadic(),
        old.fixed_count(),
        old.source_input_types().to_vec(),
        old.source_output_type(),
        old.arguments().to_vec(),
        old.return_value().clone(),
    )
    .and_then(|abi| abi.with_source_argument_ownership(old.source_argument_ownership().to_vec()))
    .map_err(|error| replay_error(format!("ABI mutation construction failed: {error}")))?;
    let changed = SemanticFunctionDeclV1::new(
        function.identity(),
        function.role(),
        function.item_definition_identity(),
        function.monomorphization_identity(),
        function.generic_type_arguments_identity(),
        function.const_generic_arguments_identity(),
        function.source(),
        changed_abi,
        function.locals().to_vec(),
        function.entry(),
        function.blocks().to_vec(),
    )
    .map_err(|error| replay_error(format!("ABI mutation function failed: {error}")))?
    .with_kernel_entry(
        function
            .kernel_entry()
            .ok_or_else(|| replay_error("ABI root lacks entry"))?
            .clone(),
    );
    let mut functions = source.functions().to_vec();
    functions[root.index() as usize] = changed;
    let mutated = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        source.types().to_vec(),
        source.allocations().to_vec(),
        source.statics().to_vec(),
        source.vtables().to_vec(),
        functions,
        source.callables().to_vec(),
        source.roots().to_vec(),
    )
    .map_err(|error| replay_error(format!("ABI mutation request failed: {error}")))?;
    match mutated.admit_current_production(SemanticMirLimitsV1::default()) {
        Err(SemanticMirErrorV1::InvalidFunctionAbi) => {}
        _ => return mismatch("ABI negative did not observe the exact admission rejection"),
    }
    case_document(
        "original-kernel-unwind-abi",
        "abi",
        "semantic-mir-admission",
        "InvalidFunctionAbi",
        serde_json::json!({
            "coordinateSpace": "original-source-mir",
            "root": root.index(),
            "functionIdentitySha256": hex32(*function.identity().as_bytes()),
            "semanticMirSha256": hex32(*source.semantic_sha256().as_bytes()),
            "operation": "set-kernel-abi-can-unwind",
            "before": false,
            "after": true,
        }),
    )
}

pub(super) fn reject_unresolved_kernel_call(original: &[u8], symbol: &str) -> ResultV1<Value> {
    let (_, mut module) = VerifiedCanonicalKernelIrV13::from_canonical_bytes_with_module(
        original.to_vec(),
    )
    .map_err(|error| replay_error(format!("negative KIR positive control failed: {error}")))?;
    let mut entries = module
        .kernels
        .iter()
        .filter(|kernel| kernel.id.as_str() == symbol || kernel.entry.as_str() == symbol);
    let entry = entries
        .next()
        .ok_or_else(|| replay_error("negative KIR lacks exact kernel"))?
        .entry
        .clone();
    if entries.next().is_some() {
        return mismatch("negative KIR root selection is ambiguous");
    }
    let callee = FunctionId::new("__fe2o3_negative_unresolved_callee_v1");
    if module.function(&callee).is_some() {
        return mismatch("negative unresolved callee already exists in positive control");
    }
    let function = module
        .functions
        .iter_mut()
        .find(|function| function.id == entry)
        .ok_or_else(|| replay_error("negative KIR kernel body is absent"))?;
    let block = function
        .body
        .as_mut()
        .and_then(|body| body.blocks.first_mut())
        .ok_or_else(|| replay_error("negative KIR kernel entry block is absent"))?;
    let block_id = block.id;
    block.operations.insert(
        0,
        Operation::new(
            vec![],
            OperationKind::Call {
                callee: callee.clone(),
                arguments: vec![],
            },
        ),
    );
    let errors = verify_module(&module)
        .err()
        .ok_or_else(|| replay_error("unresolved KIR call was accepted"))?;
    let [error] = errors.diagnostics() else {
        return mismatch("KIR mutation has additional or missing diagnostics");
    };
    if error.code != DiagnosticCode::UnknownCallee
        || error.location.module != module.id
        || error.location.function.as_ref() != Some(&entry)
        || error.location.block != Some(block_id)
        || error.location.operation != Some(0)
    {
        return mismatch("KIR mutation rejection is not at its exact inserted call");
    }
    case_document(
        "executable-kernel-unresolved-call",
        "unsupported-operation",
        "kir-verification",
        "UnknownCallee",
        serde_json::json!({
            "coordinateSpace": "executable-kir-v13",
            "executableKirSha256": hex_sha256(original),
            "function": entry.as_str(),
            "block": block_id.0,
            "operation": 0,
            "action": "insert-unresolved-call",
            "callee": callee.as_str(),
        }),
    )
}

fn case_document(
    id: &str,
    category: &str,
    boundary: &str,
    diagnostic: &str,
    mutation: Value,
) -> ResultV1<Value> {
    Ok(serde_json::json!({
        "caseId": id,
        "relatedCategory": category,
        "diagnostic": diagnostic,
        "productionBoundary": boundary,
        "status": "rejected-by-live-replay",
        "coverage": "supplemental-not-declared-mutation",
        "mutationRecipeSha256": domain_sha256(MUTATION_DOMAIN, &mutation)?,
        "mutation": mutation,
    }))
}
