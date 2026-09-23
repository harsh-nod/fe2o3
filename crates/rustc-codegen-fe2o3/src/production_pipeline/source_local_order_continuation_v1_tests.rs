//! Actual-source qualification of fixed Policy6 plus an owned local-order tail.
//! This observer is test-only; it does not admit a persistent recipe or publish.
use super::*;
use fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1;
use fe2o3_lower_mir_kernel::SourceU32LocalOrderRequestV1;

fn result_order(owner: &Owner, region: Region) -> Result<[ValueId; 3], String> {
    let operations = owner
        .module()
        .functions
        .get(region.block.function.0 as usize)
        .and_then(|function| function.body.as_ref())
        .and_then(|body| body.blocks.get(region.block.block as usize))
        .and_then(|block| block.operations.get(region.first_operation as usize..))
        .and_then(|operations| operations.get(..3))
        .ok_or("local-order continuation selected region missing")?;
    let mut results = [ValueId(0); 3];
    for (index, operation) in operations.iter().enumerate() {
        let [result] = operation.results.as_slice() else {
            return Err("local-order continuation result arity changed".into());
        };
        results[index] = result.id;
    }
    Ok(results)
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    pub(crate) fn observe_source_local_order_continuation_v1(
        self,
        mut input: RetainedInput,
        preference: Preference,
    ) -> Result<Value, String> {
        let tcx = self.stage.tcx;
        let mut captured = capture_local_order(tcx, &self.stage.closure, &input)?;
        let ranked = self
            .verify_general_kernel_checks()
            .map_err(|error| format!("local-order continuation source checks: {error}"))?;
        let mut work = Work::new(
            usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT)
                .map_err(|_| "local-order continuation work limit")?,
        );
        let mut budget = Budget::new(
            &mut work,
            crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
        );
        let prefix = ranked
            .prepare_source_local_order_prefix_v1(&mut budget)
            .map_err(|error| format!("local-order continuation fixed prefix: {error}"))?;
        captured.recheck(tcx, &mut input)?;
        let joined = join::exact_join(&mut captured, prefix.admitted.source_semantic_kir())?;
        let original = prefix
            .admitted
            .source_semantic_kir()
            .pre_ranked_executable()
            .ok_or("local-order continuation actual original owner absent")?;
        // This selects original N, not a guessed coordinate in optimized I.
        // Layer B independently transports these anchors through replayed prefix rows.
        let selected = joined.bound_region(original, &mut captured.meter)?;
        let request = SourceU32LocalOrderRequestV1 {
            expected_source: *original.canonical().identity(),
            operations: [0u32, 1, 2].map(|offset| CanonicalKirOperationCoordinateV1 {
                block: selected.block,
                operation: selected.first_operation + offset,
            }),
            preference,
        };
        let original_identity = *original.canonical().identity().digest();
        let input_identity = *prefix.admitted.output().canonical().identity().digest();
        let continued = crate::production_pipeline::source_local_order_v1::
            continue_admitted_source_local_order_v1(prefix, request, &mut budget)
            .map_err(|error| format!("local-order continuation actual L: {error}"))?;
        let floor = budget.storage();
        continued
            .verify_equivalence(&mut budget)
            .map_err(|error| format!("local-order continuation independent replay: {error}"))?;
        if budget.storage() != floor {
            return Err("local-order continuation replay changed storage floor".into());
        }
        let output = continued.output();
        let region = continued.region();
        captured
            .meter
            .scan(output.canonical().canonical_bytes().len())?;
        let current_input = continued.admitted().prefix().output();
        captured
            .meter
            .scan(current_input.canonical().canonical_bytes().len())?;
        let input_results = result_order(current_input, region)?;
        let expected_results = match preference {
            Preference::SourceOrder => input_results,
            Preference::ReverseReady => [input_results[1], input_results[0], input_results[2]],
        };
        let results = result_order(output, region)?;
        let retained_original = continued.original().map_err(|error| error.to_string())?;
        if results != expected_results
            || retained_original.canonical().identity().digest() != &original_identity
        {
            return Err(
                "local-order continuation actual order or original identity mismatch".into(),
            );
        }
        let llvm = continued.llvm_ir().map_err(|error| error.to_string())?;
        if llvm.is_empty() || llvm.len() > 48 * 1024 {
            return Err("local-order continuation LLVM observation bound".into());
        }
        captured.meter.storage(128 * 1024)?;
        captured.meter.scan(128 * 1024)?;
        let descriptor = continued.descriptor_source();
        let producer = descriptor.table().producer();
        if producer.version().as_str() != "source-local-order-policy6-v1/gfx942" {
            return Err("local-order continuation descriptor producer mismatch".into());
        }
        let mut oracle = simulation::OracleLedger::default();
        let simulation = simulation::observe(output, &mut oracle)?;
        captured.recheck(tcx, &mut input)?;
        let mut report = json!({
            "stage":"actual_source_local_order_owned_continuation",
            "composition":"source-local-order-policy6-v1",
            "preference":match preference { Preference::SourceOrder=>"source-order", Preference::ReverseReady=>"reverse-ready" },
            "source_sha256":captured.original_sha256,
            "source_initializer":captured.initializer,
            "parameter_ordinals":captured.parameters.each_ref().map(|parameter| parameter.ordinal),
            "original_identity":original_identity,
            "input_identity":input_identity,
            "output_identity":output.canonical().identity().digest(),
            "output_order":results.map(|value| value.0),
            "region":{"function":region.block.function.0,"block":region.block.block,
                "first":region.first_operation,"count":region.operation_count},
        });
        let execution = json!({
            "llvm_text":llvm,
            "llvm_sha256":<[u8;32]>::from(Sha256::digest(llvm.as_bytes())),
            "llvm_bytes":llvm.len(),
            "descriptor_producer":producer.version().as_str(),
            "simulation":simulation,"oracle_accounting":oracle,
            "canonical_work":budget.work(),"canonical_peak_storage":budget.peak_storage(),
            "retained_storage_floor":floor,
            "source_scan_accounting":captured.meter,"source_io_accounting":input.io,
            "actual_output_used_for_llvm":true,
            "final_source_output_admitted":true,
            "persistent_recipe_admitted":false,
            "existing_policy6_modified":false,
            "native_object_emitted":false,"hardware_observed":false,
            "grants_artifact_or_launch_authority":false
        });
        let Value::Object(execution) = execution else {
            return Err("local-order execution report object".into());
        };
        report
            .as_object_mut()
            .ok_or("local-order report object")?
            .extend(execution);
        Ok(report)
    }
}
