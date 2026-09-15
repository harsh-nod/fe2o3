use crate::*;

include!("../../execution_capability_v1/subgroup_partition/fixture.rs");

fn occurrence(instance: u32, block: u32) -> ExecutionCapabilitySourceOccurrenceV1 {
    ExecutionCapabilitySourceOccurrenceV1::from_untrusted_parts(
        [71; 32], [72; 32], [73; 32], instance, block,
    )
    .unwrap()
}

fn expanded_module() -> Module {
    let mut module = module();
    for (block, operation) in operations_mut(&mut module).iter_mut().enumerate() {
        if let OperationKind::ExecutionCapability(contract) = &mut operation.kind {
            contract.source.occurrence = Some(occurrence(0, block as u32));
        }
    }
    module
}

fn repeat_reduce(module: &mut Module) -> usize {
    let mut repeated = operations(module)[6].clone();
    repeated.results[0].id = ValueId(90);
    let next = operations(module).len();
    operations_mut(module).push(repeated);
    next
}

fn rejects_source(module: &Module, message: &str) {
    let errors = verify_module(module).unwrap_err();
    assert!(
        errors.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == DiagnosticCode::InvalidExecutionCapability
                && diagnostic.message == message
        }),
        "{errors:?}",
    );
}

const REPLAY: &str = "execution source operation identity/location was duplicated or replayed";
const CUSTODY: &str = "execution source occurrences disagree on their root, expansion, or mode";

#[test]
fn expanded_source_instances_preserve_repeated_original_terminal_sites() {
    let mut module = expanded_module();
    contract_mut(&mut module, 6).source.occurrence = Some(occurrence(7, 11));
    let repeated = repeat_reduce(&mut module);
    contract_mut(&mut module, repeated).source.occurrence = Some(occurrence(8, 12));
    assert_eq!(
        operation_contract(&operations(&module)[6]).source.operation,
        operation_contract(&operations(&module)[repeated])
            .source
            .operation,
    );
    verify_module(&module).unwrap();
    assert_eq!(
        decode_module_v13(&encode_module_v13(&module).unwrap()).unwrap(),
        module,
    );
}

#[test]
fn exact_expanded_source_replays_still_reject() {
    let mut module = expanded_module();
    repeat_reduce(&mut module);
    rejects_source(&module, REPLAY);
}

#[test]
fn legacy_source_replay_identity_remains_unchanged() {
    let mut module = module();
    verify_module(&module).unwrap();
    let repeated = repeat_reduce(&mut module);
    // Changing a claimed function cannot waive the historical location check.
    contract_mut(&mut module, repeated).source.function = [99; 32];
    rejects_source(&module, REPLAY);
}

#[test]
fn expanded_source_custody_is_consistent_across_the_complete_function() {
    for changed in 0..3 {
        let mut module = expanded_module();
        let mut identities = [[71; 32], [72; 32], [73; 32]];
        identities[changed] = [99; 32];
        contract_mut(&mut module, 6).source.occurrence =
            ExecutionCapabilitySourceOccurrenceV1::from_untrusted_parts(
                identities[0],
                identities[1],
                identities[2],
                0,
                6,
            );
        rejects_source(&module, CUSTODY);
    }
}

#[test]
fn expanded_and_original_source_modes_cannot_be_mixed_in_either_order() {
    for first in [1, 6] {
        let mut module = expanded_module();
        contract_mut(&mut module, first).source.occurrence = None;
        rejects_source(&module, CUSTODY);
    }
}

#[test]
fn source_occurrence_records_do_not_waive_other_execution_contract_checks() {
    let mut module = expanded_module();
    contract_mut(&mut module, 6).provenance.kernel_binding = [99; 32];
    assert!(verify_module(&module).is_err());
}
