//! Compiler-owned negative replay, not a completed tutorial negative-fixture suite.
//!
//! The production constructor requires recovered V5 custody. Archived JSON is inert:
//! validation regenerates each mutation and reruns bounded MIR/KIR admission.

use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    AdmittedInertSemanticMirV1, InertSemanticMirRequestV1, SemanticFunctionIdV1,
    SemanticFunctionRoleV1, SemanticMirEntityV1, SemanticMirErrorV1, SemanticMirLimitsV1,
};

const SCHEMA: &str = "fe2o3-tutorial-source-negative-replay-v2";
const RUN_DOMAIN: &[u8] = b"fe2o3-tutorial-source-negative-replay-run-v2\0";
const MUTATION_DOMAIN: &[u8] = b"fe2o3-tutorial-source-negative-mutation-v1\0";
const CASE_ID: &str = "original-kernel-root-roster-omission";
const MAX_RECEIPT_BYTES: usize = 64 * 1024;
const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;

#[path = "production_pipeline_tutorial_transaction_negative_v1_recipes.rs"]
mod recipes;

// Closed recipes, never a caller-provided mutation program.
#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceNegativeReplayBindingV1 {
    request_binding: String,
    fixture_id: String,
    target: String,
    kernel_symbol: String,
    source_closure: [u8; 32],
    compiler_input: [u8; 32],
    sealed_result: [u8; 32],
    transaction: [u8; 32],
    compiler_policy: [u8; 32],
    semantic_mir: [u8; 32],
    executable_kir: [u8; 32],
    declaration_roster: recipes::RequiredNegativeRosterV1,
}

impl SourceNegativeReplayBindingV1 {
    fn from_result(
        context: &RequestContext,
        result: &InertProductionCapabilityResultV5,
    ) -> ResultV1<Self> {
        let compiler_input = sha256(&compiler_input_preimage_v1(context)?);
        let source_closure = sha256(&context.source_closure_preimage);
        Ok(Self {
            request_binding: context.request_binding_sha256.clone(),
            fixture_id: context.fixture_id.clone(),
            target: context.target.clone(),
            kernel_symbol: context.kernel_symbol.clone(),
            source_closure,
            compiler_input,
            sealed_result: result.identity().sha256(),
            transaction: result.transaction().identity().sha256(),
            compiler_policy: result.handoff().inputs().compiler_policy(),
            semantic_mir: result.handoff().inputs().semantic_mir_identity(),
            executable_kir: sha256(result.handoff().executable_kir().canonical_preimage()),
            declaration_roster: recipes::RequiredNegativeRosterV1::load(
                context,
                source_closure,
                compiler_input,
            )?,
        })
    }

    fn document(&self) -> Value {
        serde_json::json!({
            "compilerInputSha256": hex32(self.compiler_input),
            "compilerPolicySha256": hex32(self.compiler_policy),
            "executableKirSha256": hex32(self.executable_kir),
            "fixtureId": self.fixture_id,
            "kernelSymbol": self.kernel_symbol,
            "recipeSourceSha256": hex_sha256(include_bytes!("production_pipeline_tutorial_transaction_negative_v1.rs")),
            "recipeImplementationSha256": hex_sha256(include_bytes!("production_pipeline_tutorial_transaction_negative_v1_recipes.rs")),
            "negativeDeclarationRoster": self.declaration_roster,
            "requestBindingSha256": self.request_binding,
            "sealedResultSha256": hex32(self.sealed_result),
            "semanticMirSha256": hex32(self.semantic_mir),
            "sourceClosureSha256": hex32(self.source_closure),
            "target": self.target,
            "transactionSha256": hex32(self.transaction),
        })
    }
}

/// Only a live run of the closed recipe constructs this value. No JSON decoder or
/// constructor accepts an expected diagnostic, an arbitrary case, or a success flag.
#[derive(Debug)]
struct SourceNegativeReplayReceiptV1 {
    document: Value,
}

impl SourceNegativeReplayReceiptV1 {
    fn run(
        binding: &SourceNegativeReplayBindingV1,
        original: &[u8],
        executable_kir: &[u8],
    ) -> ResultV1<Self> {
        if [
            &binding.request_binding,
            &binding.fixture_id,
            &binding.target,
            &binding.kernel_symbol,
        ]
        .iter()
        .any(|value| value.len() > MAX_RECEIPT_BYTES / 8)
        {
            return mismatch("negative replay binding exceeds its byte bound");
        }
        if original.len() > MAX_INPUT_BYTES || executable_kir.len() > MAX_INPUT_BYTES {
            return mismatch("negative replay input exceeds its byte bound");
        }
        if sha256(original) != binding.semantic_mir {
            return mismatch("negative replay source differs from the sealed source MIR");
        }
        if sha256(executable_kir) != binding.executable_kir {
            return mismatch("negative replay KIR differs from the sealed executable KIR");
        }
        // Positive control: invalid source must not be credited as a rejected mutation.
        let source = AdmittedInertSemanticMirV1::decode_current_production_canonical(
            original,
            SemanticMirLimitsV1::default(),
        )
        .map_err(|error| {
            replay_error(format!("negative replay positive control failed: {error}"))
        })?;
        source.require_complete_kernel_entries().map_err(|error| {
            replay_error(format!(
                "negative replay source entries are incomplete: {error}"
            ))
        })?;
        let matching_roots = source
            .roots()
            .iter()
            .copied()
            .filter(|root| {
                source.functions()[root.index() as usize]
                    .kernel_entry()
                    .is_some_and(|entry| {
                        entry.export_symbol().as_bytes() == binding.kernel_symbol.as_bytes()
                    })
            })
            .collect::<Vec<_>>();
        let [root] = matching_roots.as_slice() else {
            return mismatch("negative replay requires one exact original kernel root");
        };
        let root = *root;
        let function = &source.functions()[root.index() as usize];
        if function.role() != SemanticFunctionRoleV1::KernelRoot {
            return mismatch("negative replay selected a non-kernel source root");
        }
        let mutation = serde_json::json!({
            "caseId": CASE_ID,
            "coordinateSpace": "original-source-mir",
            "functionIdentitySha256": hex32(*function.identity().as_bytes()),
            "operation": "omit-root-roster-entry",
            "root": root.index(),
            "semanticMirSha256": hex32(binding.semantic_mir),
        });
        let mutated = omit_root(&source, root).map_err(|error| {
            replay_error(format!("negative mutation construction failed: {error}"))
        })?;
        let outcome = mutated.admit_current_production(SemanticMirLimitsV1::default());
        let diagnostic = match outcome {
            Err(SemanticMirErrorV1::EmptyModel {
                entity: SemanticMirEntityV1::Root,
            }) if source.roots().len() == 1 => "EmptyModelRoot",
            Err(SemanticMirErrorV1::InvalidFunctionRole {
                function: observed,
                role: SemanticFunctionRoleV1::KernelRoot,
                rooted: false,
            }) if observed == root && source.roots().len() > 1 => "InvalidFunctionRole",
            _ => {
                return mismatch(
                    "negative replay did not observe the exact root-roster admission rejection",
                );
            }
        };
        let abi_case = recipes::reject_unwinding_kernel_abi(&source, root)?;
        let kir_case =
            recipes::reject_unresolved_kernel_call(executable_kir, &binding.kernel_symbol)?;
        let mut document = serde_json::json!({
            "binding": binding.document(),
            "cases": [{
                "caseId": CASE_ID,
                "diagnostic": diagnostic,
                "mutation": mutation,
                "mutationRecipeSha256": domain_sha256(MUTATION_DOMAIN, &mutation)?,
                "productionBoundary": "semantic-mir-admission",
                "status": "rejected-by-live-replay",
                "coverage": "supplemental-not-declared-mutation",
            }, abi_case, kir_case],
            "mode": "in-process-source-mir-and-kir-admission-replay",
            "positiveControl": "admitted-exact-source-and-executable-kir",
            "qualificationStatus": "incomplete",
            "requiredCasesSatisfied": 0,
            "requiredCases": binding.declaration_roster.pending_cases(),
            "schema": SCHEMA,
            "sourceRustRecompiled": false,
        });
        let run_identity = domain_sha256(RUN_DOMAIN, &document)?;
        document
            .as_object_mut()
            .expect("receipt object")
            .insert("replayRunSha256".to_owned(), Value::String(run_identity));
        if canonical_document(&document)?.len() > MAX_RECEIPT_BYTES {
            return mismatch("negative replay receipt exceeds its byte bound");
        }
        Ok(Self { document })
    }

    fn verify_document(&self, observed: &Value) -> ResultV1<()> {
        if observed != &self.document {
            return mismatch("negative replay receipt differs from independent source replay");
        }
        Ok(())
    }
}

fn omit_root(
    source: &AdmittedInertSemanticMirV1,
    root: SemanticFunctionIdV1,
) -> std::result::Result<InertSemanticMirRequestV1, SemanticMirErrorV1> {
    // Change only roster membership. Root-role changes could invalidate typed
    // capability provenance before admission reaches its root-roster checks.
    InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        source.types().to_vec(),
        source.allocations().to_vec(),
        source.statics().to_vec(),
        source.vtables().to_vec(),
        source.functions().to_vec(),
        source.callables().to_vec(),
        source
            .roots()
            .iter()
            .copied()
            .filter(|candidate| *candidate != root)
            .collect(),
    )
}

pub(super) fn produce_v1(
    context: &RequestContext,
    recovered: &RecoveredProtectedFixtureResultV1,
) -> ResultV1<Value> {
    recovered.revalidate_currentness()?;
    let receipt = replay_result(context, recovered.production_result())?;
    recovered.revalidate_currentness()?;
    Ok(receipt.document)
}

pub(super) fn validate_v1(
    context: &RequestContext,
    result: &InertProductionCapabilityResultV5,
    observed: &Value,
) -> ResultV1<()> {
    replay_result(context, result)?.verify_document(observed)
}

fn replay_result(
    context: &RequestContext,
    result: &InertProductionCapabilityResultV5,
) -> ResultV1<SourceNegativeReplayReceiptV1> {
    if result
        .handoff()
        .legacy_handoff()
        .capsule()
        .target()
        .as_amd_target_id()
        .processor()
        != context.target
    {
        return mismatch("negative replay target differs from the sealed compiler target");
    }
    let binding = SourceNegativeReplayBindingV1::from_result(context, result)?;
    let original = result
        .handoff()
        .legacy_handoff()
        .capsule()
        .receipts()
        .semantic_mir()
        .canonical_preimage();
    SourceNegativeReplayReceiptV1::run(
        &binding,
        original,
        result.handoff().executable_kir().canonical_preimage(),
    )
}

fn replay_error(message: impl Into<String>) -> TutorialProductionTransactionErrorV1 {
    TutorialProductionTransactionErrorV1::new(
        TutorialProductionTransactionErrorCodeV1::EvidenceMismatch,
        message,
    )
}

fn mismatch<T>(message: &str) -> ResultV1<T> {
    Err(replay_error(message))
}

#[cfg(test)]
#[path = "production_pipeline_tutorial_transaction_negative_v1_tests.rs"]
mod tests;
