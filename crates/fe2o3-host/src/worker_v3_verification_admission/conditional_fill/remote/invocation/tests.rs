//! Real generated packing and captured compiler/Worker bytes, never remote or GPU authority.
use super::*;
use crate::{GeneratedRuntimeChargedResultV1, GeneratedRuntimeWriteSlice};
use fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3;
use fe2o3_device::{WriteOnlyDisjointSlice, thread};
use fe2o3_kernel_analysis::PhysicalMachineEffectRequestV1;
use fe2o3_verifier::{
    ValidatedConditionalCompilerProofInputsV1, validate_conditional_compiler_proof_inputs_v1,
};

#[fe2o3_device::kernel(
    typed,
    namespace = "ef64c9e65aa7777848ff96f7538edee8e63eaada0869bd9f4b662aec03e0590e",
    reference = reference,
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn fill_write_only(mut output: WriteOnlyDisjointSlice<u32>) {
    let index = thread::index_1d();
    let value = index.get() as u32;
    let _ = output.write(index, value);
}

fn reference(point: usize, output: &mut u32) {
    *output = point as u32;
}

fn inputs() -> (
    InertSemanticCompilerModuleHandoffV3,
    ValidatedConditionalCompilerProofInputsV1,
) {
    let handoff = InertSemanticCompilerModuleHandoffV3::decode(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fe2o3-verifier/src/conditional_fill_program_v1/fill.handoff"
    )))
    .unwrap();
    let r = handoff.capsule().receipts();
    let inputs = validate_conditional_compiler_proof_inputs_v1(
        r.proof_binding(),
        r.semantic_mir(),
        r.middle_end(),
        r.kernel_ir(),
        r.mir_to_kir_correspondence(),
        r.formal_memory(),
    )
    .unwrap();
    (handoff, inputs)
}

fn payload() -> PhysicalMachineEffectRequestV1 {
    PhysicalMachineEffectRequestV1::decode_canonical(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fe2o3-kernel-analysis/src/gfx942_fill_analysis_v1/fill.request"
    )))
    .unwrap()
}

fn geometry(grid: u32) -> AqlDispatchGeometryV1 {
    AqlDispatchGeometryV1::new([grid, 1, 1], [64, 1, 1]).unwrap()
}

fn pack_output(
    count: usize,
    grid: u32,
    budget: &GeneratedRuntimeResultBudgetV1,
) -> (
    Result<crate::GeneratedRuntimeChargedArgumentsV1, WorkerV3ConditionalFillInvocationErrorV1>,
    GeneratedRuntimeChargedResultV1<u32>,
) {
    let (handoff, inputs) = inputs();
    let (output, observer) =
        GeneratedRuntimeWriteSlice::new_charged(vec![0u32; count].into_boxed_slice());
    (
        pack::<fill_write_only_gpu::Marker, _>(
            &inputs,
            &handoff,
            fill_write_only_gpu::RuntimeArguments::new(output),
            geometry(grid),
            GeneratedRuntimeArgumentLimitsV1::new(16384, 16384, 1),
            budget,
        ),
        observer,
    )
}

#[test]
fn actual_generated_fill_packing_reaches_conditional_projection_without_results() {
    let payload = payload();
    let hsaco = payload.exact_payload_bytes();
    let kernel = validate(hsaco, AdmittedProfile::Gfx942XnackOffCov6)
        .unwrap()
        .bind_kernel("fill_write_only")
        .unwrap();
    for (n, grid) in [(1, 64), (64, 64), (65, 128), (1024, 1024)] {
        let budget = GeneratedRuntimeResultBudgetV1::new(n as u64 * 8, 1).unwrap();
        let (packed, mut observer) = pack_output(n, grid, &budget);
        let packed = packed.unwrap();
        assert_eq!(packed.footprint().output_bytes, n * 4);
        let output = packed.packed_view_v1().buffers[0].bytes().as_ptr();
        let parts = packed.into_runtime_inputs(geometry(grid), 0, 1000);
        let storage = parts.storage.prepare(hsaco, "fill_write_only").unwrap();
        check_prepared_binding(
            storage.prepared(),
            hsaco,
            "fill_write_only",
            kernel.selected_binding(),
            kernel.identity_inputs().object_sha256(),
            hsaco.len() as u64,
        )
        .unwrap();
        let contract = storage.prepared().dispatch_contract_sha256();
        let storage = storage.project_conditional_fill(hsaco).unwrap();
        assert_eq!(storage.prepared().dispatch_contract_sha256(), contract);
        assert_eq!(storage.prepared().buffers().len(), 1);
        assert_eq!(storage.prepared().buffers()[0].bytes().len(), n * 4);
        assert_eq!(storage.prepared().buffers()[0].bytes().as_ptr(), output);
        assert_eq!(storage.prepared().timeout_milliseconds(), 1000);
        assert_eq!(
            storage.prepared().buffer_access(0),
            Some(fe2o3_runtime::Gfx942RuntimeBufferAccessV1::WriteOnly)
        );
        assert_eq!(budget.usage().reserved_peak_bytes, n as u64 * 8);
        assert!(observer.try_take().unwrap().is_none());
        drop(storage);
        assert_eq!(budget.usage().reserved_peak_bytes, 0);
        assert!(matches!(
            observer.try_take(),
            Err(GeneratedRuntimeArgumentErrorV1::OutputUnavailable)
        ));
    }
}

#[test]
fn unsupported_full64_shape_and_coverage_leave_no_output_or_credit() {
    for (n, grid) in [(65, 64), (65, 65), (0, 64)] {
        let budget = GeneratedRuntimeResultBudgetV1::new(520, 1).unwrap();
        let (result, mut observer) = pack_output(n, grid, &budget);
        match (n, grid) {
            (65, 64) => assert!(matches!(
                result,
                Err(WorkerV3ConditionalFillInvocationErrorV1::Coverage(
                    crate::ConditionalPackedCoverageErrorV1::Underlaunch {
                        elements: 65,
                        grid_x: 64
                    }
                ))
            )),
            _ => assert!(result.is_err(), "accepted N={n}, G={grid}"),
        }
        assert_eq!(budget.usage().reserved_peak_bytes, 0);
        assert!(matches!(
            observer.try_take(),
            Err(GeneratedRuntimeArgumentErrorV1::OutputUnavailable)
        ));
    }
    for (grid, group, lds) in [
        ([128, 1, 1], [64, 1, 1], 4),
        ([128, 1, 1], [32, 1, 1], 0),
        ([128, 2, 1], [64, 1, 1], 0),
        ([128, 1, 2], [64, 1, 1], 0),
    ] {
        assert!(check_geometry(AqlDispatchGeometryV1::new(grid, group).unwrap(), lds).is_err());
    }
}

#[test]
fn conditional_fill_uses_original_budget_and_rejects_exhaustion() {
    for (bytes, members) in [(519, 1), (520, 0)] {
        // A zero member capacity is rejected by the account itself.
        let Ok(budget) = GeneratedRuntimeResultBudgetV1::new(bytes, members) else {
            assert_eq!(members, 0);
            continue;
        };
        let (packed, mut observer) = pack_output(65, 128, &budget);
        assert!(matches!(
            packed,
            Err(WorkerV3ConditionalFillInvocationErrorV1::Arguments(
                GeneratedRuntimeArgumentErrorV1::ResultCredit(_)
            ))
        ));
        assert_eq!(budget.usage().reserved_peak_bytes, 0);
        assert!(matches!(
            observer.try_take(),
            Err(GeneratedRuntimeArgumentErrorV1::OutputUnavailable)
        ));
    }
}

#[test]
fn prepared_fill_rejects_changed_finalizer_entry_and_original_image() {
    let budget = GeneratedRuntimeResultBudgetV1::new(520, 1).unwrap();
    let (packed, _) = pack_output(65, 128, &budget);
    let payload = payload();
    let hsaco = payload.exact_payload_bytes();
    let kernel = validate(hsaco, AdmittedProfile::Gfx942XnackOffCov6)
        .unwrap()
        .bind_kernel("fill_write_only")
        .unwrap();
    let storage = packed
        .unwrap()
        .into_runtime_inputs(geometry(128), 0, 1000)
        .storage
        .prepare(hsaco, "fill_write_only")
        .unwrap();
    let sha256 = kernel.identity_inputs().object_sha256();
    let binding = kernel.selected_binding();
    let check = |bytes, name, hash, len| {
        check_prepared_binding(storage.prepared(), bytes, name, binding, hash, len)
    };
    let mut changed_hash = sha256;
    changed_hash[0] ^= 1;
    assert!(check(hsaco, "fill_write_only", changed_hash, hsaco.len() as u64).is_err());
    assert!(check(hsaco, "fill_write_only", sha256, hsaco.len() as u64 + 1).is_err());
    assert!(check(hsaco, "missing", sha256, hsaco.len() as u64).is_err());
    let mut changed_image = hsaco.to_vec();
    changed_image[0] ^= 1;
    assert!(
        check(
            &changed_image,
            "fill_write_only",
            sha256,
            hsaco.len() as u64
        )
        .is_err()
    );
    let foreign_bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fe2o3-runtime/fixtures/trusted-gfx942-vecadd-v1/vecadd.hsaco"
    ));
    let foreign = validate(foreign_bytes, AdmittedProfile::Gfx942XnackOffCov6)
        .unwrap()
        .bind_kernel("vecadd")
        .unwrap();
    assert_ne!(binding, foreign.selected_binding());
    assert!(
        check_prepared_binding(
            storage.prepared(),
            hsaco,
            "fill_write_only",
            foreign.selected_binding(),
            sha256,
            hsaco.len() as u64,
        )
        .is_err()
    );
    drop(storage);
    assert_eq!(budget.usage().reserved_peak_bytes, 0);
}
