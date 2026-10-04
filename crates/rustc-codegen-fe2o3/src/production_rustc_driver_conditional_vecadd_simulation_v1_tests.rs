//! Finite CPU observations of actual pre-ranked source, never final-O evidence.
use super::*;
use fe2o3_kir_sim::{SimulationErrorV1, SimulationExecutionErrorKindV1};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[path = "../tests/fixtures/production-extraction-device/src/conditional_vecadd_reference.rs"]
mod cpu;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) enum Mutation {
    None,
    InputGuard,
    CpuRead,
    SourceArgument,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Observation {
    pub canonical_digest: [u8; 32],
    pub matching_scenarios: usize,
    pub short_input_refusals: usize,
    pub semantic_counterexamples: usize,
}

pub(crate) fn observe(
    source: &VerifiedCanonicalKernelIrV12,
    mutation: Mutation,
) -> Result<Observation, SourceFailure> {
    assert_eq!(
        std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "tests/fixtures/production-extraction-device/src/conditional_vecadd_reference.rs",
        ))
        .unwrap(),
        include_bytes!(
            "../tests/fixtures/production-extraction-device/src/conditional_vecadd_reference.rs"
        ),
        "the host oracle and authenticated CPU MIR must use the same source bytes",
    );
    let limits = limits();
    if source.canonical_bytes().len() > limits.max_canonical_bytes {
        return Err(failure("Vecadd source observation byte cap"));
    }
    let decoded =
        VerifiedCanonicalKernelIrV12::from_canonical_bytes(source.canonical_bytes().to_vec())
            .map_err(failure)?;
    assert_eq!(decoded.identity(), source.identity());
    assert_eq!(decoded.canonical_bytes(), source.canonical_bytes());
    let module = AdmittedSimulationModuleV1::admit_v12(decoded, limits).map_err(failure)?;
    assert!(!module.grants_execution_authority());
    let identity = *module.identity();
    let kernel = require_abi(&module, Case::Vecadd)?;
    let mut result = Observation {
        canonical_digest: *source.identity().digest(),
        matching_scenarios: 0,
        short_input_refusals: 0,
        semantic_counterexamples: 0,
    };
    for (length, extra) in [
        (0, 0),
        (1, 0),
        (63, 0),
        (64, 0),
        (65, 0),
        (255, 0),
        (256, 0),
        (257, 0),
        (0, 3),
        (65, 7),
    ] {
        let mut scenario = elementwise(Case::Vecadd, length, extra)?;
        let a = input_values(length + extra, 1);
        let b = input_values(length + extra, 13);
        let mut values = vec![SENTINEL; length];
        for (point, value) in values.iter_mut().enumerate() {
            cpu::vecadd_reference(point, &a, &b, value);
        }
        let cpu_output = guarded_buffer(2, &values, AccessMode::ReadWrite)?.1;
        assert_eq!(scenario.expected[2], cpu_output, "shared CPU MIR oracle");
        scenario.expected[2] = cpu_output;
        let (grid, workgroup) = launch(kernel, scenario.active)?;
        let request =
            SimulationRequestV1::new(kernel.id.clone(), grid, workgroup, scenario.arguments)
                .with_shared_buffers(scenario.backings);
        let before = request.clone();
        let execution = module.simulate(&request, TARGET, limits).map_err(failure)?;
        let check = check_execution(
            &execution,
            &request,
            &scenario.expected,
            identity,
            Case::Vecadd,
        );
        if matches!(mutation, Mutation::SourceArgument | Mutation::InputGuard) && length > 0 {
            let error = check.expect_err("changed source must disagree with the CPU oracle");
            assert!(error.detail.contains("differs from CPU reference"));
            if mutation == Mutation::SourceArgument {
                for (point, value) in values.iter_mut().enumerate() {
                    cpu::vecadd_reference(point, &a, &a, value);
                }
            } else {
                // Both inputs are sufficient, but the half-length guard omits
                // a nonempty output suffix. Confirm that precise counterexample.
                assert!(a.len() / 2 < length);
                values[a.len() / 2..].fill(SENTINEL);
            }
            scenario.expected[2] = guarded_buffer(2, &values, AccessMode::ReadWrite)?.1;
            check_execution(
                &execution,
                &request,
                &scenario.expected,
                identity,
                Case::Vecadd,
            )?;
            result.semantic_counterexamples += 1;
        } else {
            check?;
            result.matching_scenarios += 1;
        }
        let replay = module.simulate(&request, TARGET, limits).map_err(failure)?;
        assert_eq!(execution, replay);
        assert_eq!(request, before);
    }
    if mutation == Mutation::CpuRead {
        let a = [1.25, -3.5];
        let b = [2.5, 1.25];
        let (mut expected, mut wrong) = (SENTINEL, SENTINEL);
        cpu::vecadd_reference(1, &a, &b, &mut expected);
        cpu::wrong_index_reference(1, &a, &b, &mut wrong);
        assert_ne!(wrong.to_bits(), expected.to_bits());
        result.semantic_counterexamples += 1;
    }
    if matches!(mutation, Mutation::None | Mutation::CpuRead) {
        for short in [0usize, 1] {
            let mut scenario = elementwise(Case::Vecadd, 2, 0)?;
            let a = input_values(if short == 0 { 1 } else { 2 }, 1);
            let b = input_values(if short == 1 { 1 } else { 2 }, 13);
            let mut value = SENTINEL;
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    cpu::vecadd_reference(1, &a, &b, &mut value);
                }))
                .is_err()
            );
            assert_eq!(value.to_bits(), SENTINEL.to_bits());
            let values = if short == 0 { &a } else { &b };
            let (argument, backing) = guarded_buffer(short as u32, values, AccessMode::ReadOnly)?;
            scenario.arguments[short] = argument;
            scenario.backings[short] = backing.clone();
            scenario.expected[short] = backing;
            let (grid, workgroup) = launch(kernel, 2)?;
            let request =
                SimulationRequestV1::new(kernel.id.clone(), grid, workgroup, scenario.arguments)
                    .with_shared_buffers(scenario.backings);
            let Err(SimulationErrorV1::Execution(error)) =
                module.simulate(&request, TARGET, limits)
            else {
                return Err(failure("short input must fail during source execution"));
            };
            assert!(
                matches!(
                    error.kind,
                    SimulationExecutionErrorKindV1::ReachedUnreachable
                        | SimulationExecutionErrorKindV1::OutOfBounds { .. }
                ),
                "short input is a bounds/trap failure, not a resource failure: {error:?}"
            );
            assert_eq!(
                error.invocation.map(|invocation| invocation.global),
                Some([1, 0, 0])
            );
            assert!(error.site.is_some());
            assert!(error.observation_failure.is_none());
            result.short_input_refusals += 1;
        }
    }
    let expected = match mutation {
        Mutation::None => (10, 2, 0),
        Mutation::InputGuard => (2, 0, 8),
        Mutation::CpuRead => (10, 2, 1),
        Mutation::SourceArgument => (2, 0, 8),
    };
    assert_eq!(
        (
            result.matching_scenarios,
            result.short_input_refusals,
            result.semantic_counterexamples
        ),
        expected
    );
    Ok(result)
}
