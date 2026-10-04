//! The only input is the actual fresh checked target; exported files remain inert.
use super::*;
pub(super) fn observe(
    target: crate::production_pipeline::ordered_composition_target_v1::AuthenticatedOrderedCompositionTargetModuleV1<'_>,
    variant: &str,
    output: &Path,
    started: std::time::Instant,
) -> Result<Value, String> {
    let checked = target.checked();
    let owner = checked.composition();
    let original = variant == "original";
    let edited = variant == "edit";
    let roster = if original { (1, 0, 0, 1) } else { (1, 1, 1, 1) };
    assert_eq!(
        (
            owner.definitions().len(),
            owner.helpers().len(),
            owner.calls().len(),
            owner.occurrences().len()
        ),
        roster
    );
    let definition = owner.definitions()[0];
    let site = definition.site();
    assert_eq!(
        site.function_ordinal() == owner.root_function_ordinal(),
        original
    );
    let module = checked.executable().module();
    let function = &module.functions[site.function_ordinal() as usize];
    let block = &function.body.as_ref().unwrap().blocks[site.block_ordinal() as usize];
    assert_eq!(block.id, site.block());
    let operation = &block.operations[site.operation_ordinal() as usize];
    let fe2o3_kernel_ir::OperationKind::Gfx942OrderedProgram(program) = &operation.kind else {
        panic!("actual published definition");
    };
    assert_eq!(program.registers().scratch(), 8);
    assert_eq!(program.registers().output(), 9);
    assert_eq!(program.registers().inputs(), [10, 11, 12]);
    let descriptors = program
        .program()
        .instructions()
        .map(|i| i.descriptor())
        .collect::<Vec<_>>();
    assert_eq!(descriptors, if edited { vec![40] } else { vec![133, 315] });
    let occurrence = owner.occurrences()[0];
    assert_eq!(occurrence.definition(), definition.key());
    if original {
        assert!(occurrence.incoming_call().is_none());
    } else {
        let helper = owner.helpers()[0];
        let call = owner.calls()[0];
        assert_eq!(helper.function_ordinal(), site.function_ordinal());
        assert_eq!(call.callee(), helper.key());
        assert_eq!(
            call.site().function_ordinal(),
            owner.root_function_ordinal()
        );
        assert_eq!(occurrence.incoming_call(), Some(call.key()));
    }
    let semantic = driver::lower_hex_v1(
        checked
            .semantic_ssa()
            .source_semantic()
            .semantic_sha256()
            .as_bytes(),
    );
    // Reuse the exact existing CPU engine/corpus, borrowing the genuine fresh owner.
    // Its independent bounded execution domain does not reset the retained compiler ledger.
    let cpu = publisher::publication::cpu(checked.executable(), edited, started);
    assert_eq!(cpu["cases"], 32);
    let mut observed =
        qualification::normal::observation::observe_with_roster(target, roster, output)?;
    observed["semantic_identity"] = json!(semantic);
    observed["source_profile"] = json!(variant);
    observed["cpu"] = cpu;
    observed["native_rows"]["schema"] = json!("fe2o3-test-composition-promoted-native-rows-v1");
    observed["native_rows"]["source_profile"] = json!(variant);
    observed["native_rows"]["register_profile"] =
        json!({"scratch":8,"output":9,"inputs":[10,11,12]});
    observed["native_rows"]["exact_descriptors"] = json!(descriptors);
    observed["fresh_checked_owner"] = json!(true);
    Ok(observed)
}
