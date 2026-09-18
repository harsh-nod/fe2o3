//! Test-only CPU oracle for exact admitted O; never a compiler admission route.
use super::{Observation, SourceFailure, SourceStage};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, Kernel, LaunchExtent, ScalarType, Type, VerifiedCanonicalKernelIrV12,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, BufferBackingIdV1, BufferViewArgumentV1,
    ScalarBitsV1, SharedBufferV1, SimulationArgumentV1, SimulationConflictAssessmentV1,
    SimulationExecutionV1, SimulationKernelIrIdentityV1, SimulationLimitsV1,
    SimulationRaceAssessmentV1, SimulationRequestV1, SimulationTargetV1,
};
use serde::{Deserialize, Serialize};

const CHILD_CASE: &str = "FE2O3_TEST_CHECKED_OUTPUT_SIMULATION_V1";
const GUARD_ELEMENTS: usize = 4;
const SENTINEL: f32 = -1234.5;
const TARGET: SimulationTargetV1 = SimulationTargetV1::amdgpu_64();
#[path = "production_rustc_driver_checked_output_shift_simulation_v1_tests.rs"]
pub(super) mod constant_shift;
#[path = "production_rustc_driver_checked_output_f32_simulation_v1_tests.rs"]
mod f32_arithmetic;
#[path = "production_rustc_driver_checked_output_numeric_cast_simulation_v1_tests.rs"]
pub(super) mod numeric_cast;
#[path = "production_rustc_driver_checked_output_saturating_simulation_v1_tests.rs"]
pub(super) mod saturating_integer;
#[path = "production_rustc_driver_checked_output_scalar_borrow_simulation_v1_tests.rs"]
mod scalar_borrow;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Case {
    ConstantShift(constant_shift::Batch),
    ScalarBorrow,
    NumericCast(numeric_cast::OperationCase),
    SaturatingInteger(saturating_integer::OperationCase),
    Fill,
    Vecadd,
    ScalarGemm,
    F32Negate,
    F32Divide,
}

impl Case {
    fn name(self) -> &'static str {
        match self {
            Self::ScalarBorrow => "scalar-borrow-policy5",
            Self::ConstantShift(case) => case.name(),
            Self::NumericCast(case) => case.name(),
            Self::SaturatingInteger(case) => case.name(),
            Self::Fill => "fill",
            Self::Vecadd => "vecadd",
            Self::ScalarGemm => "scalar-gemm",
            Self::F32Negate => "f32-negate",
            Self::F32Divide => "f32-divide",
        }
    }

    fn numerical_policy(self) -> &'static str {
        match self {
            Self::ScalarBorrow => scalar_borrow::NUMERICAL_POLICY,
            Self::ConstantShift(_) => constant_shift::NUMERICAL_POLICY,
            Self::NumericCast(_) => numeric_cast::NUMERICAL_POLICY,
            Self::SaturatingInteger(_) => saturating_integer::NUMERICAL_POLICY,
            Self::Fill | Self::Vecadd | Self::ScalarGemm => {
                "finite-dyadic-f32-separate-multiply-add-bit-exact-v1"
            }
            Self::F32Negate | Self::F32Divide => f32_arithmetic::NUMERICAL_POLICY,
        }
    }
}

pub(super) fn check_native_arithmetic(case: Case, llvm: &str) -> Result<(), SourceFailure> {
    if matches!(case, Case::ConstantShift(_)) {
        return Err(failure(
            "shifts require exact per-root native owner observations",
        ));
    }
    if case == Case::ScalarBorrow {
        // The fixed Policy5 child independently compares native private-load
        // count with its exact S/O observation; no arithmetic-specific opcode.
        return Ok(());
    }
    if let Case::NumericCast(case) = case {
        return numeric_cast::check_native(case, llvm);
    }
    if let Case::SaturatingInteger(case) = case {
        return saturating_integer::check_native(case, llvm);
    }
    f32_arithmetic::check_native(case, llvm)
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct SimulationObservation {
    case: Case,
    native_output_digest: [u8; 32],
    simulator_digest: [u8; 32],
    simulator_wire_version: u16,
    canonical_bytes: u64,
    numerical_policy: String,
    scenarios: Vec<ScenarioObservation>,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
struct ScenarioObservation {
    label: String,
    grid: [u64; 3],
    workgroup: [u32; 3],
    output_elements: usize,
    written_elements: usize,
    checked_backing_bytes: usize,
    steps: u64,
    invocations: u64,
    deterministic_replays: u8,
    conflicts: Assessment,
    races: Assessment,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Assessment {
    NoObserved,
    Incomplete,
}

fn failure(detail: impl std::fmt::Display) -> SourceFailure {
    SourceFailure::new(SourceStage::Simulation, detail)
}

pub(super) fn configure_child(command: &mut std::process::Command, case: Option<Case>) {
    command.env_remove(CHILD_CASE);
    if let Some(case) = case {
        command.env(CHILD_CASE, case.name());
    }
}

pub(super) fn requested() -> Result<Option<Case>, SourceFailure> {
    let Some(raw) = std::env::var_os(CHILD_CASE) else {
        return Ok(None);
    };
    match raw.to_str() {
        Some("scalar-borrow-policy5") => Ok(Some(Case::ScalarBorrow)),
        Some("fill") => Ok(Some(Case::Fill)),
        Some("vecadd") => Ok(Some(Case::Vecadd)),
        Some("scalar-gemm") => Ok(Some(Case::ScalarGemm)),
        Some("f32-negate") => Ok(Some(Case::F32Negate)),
        Some("f32-divide") => Ok(Some(Case::F32Divide)),
        Some(name) => saturating_integer::OperationCase::parse(name)
            .map(Case::SaturatingInteger)
            .or_else(|| numeric_cast::OperationCase::parse(name).map(Case::NumericCast))
            .or_else(|| constant_shift::Batch::parse(name).map(Case::ConstantShift))
            .map(Some)
            .ok_or_else(|| failure("unknown explicit test simulation request")),
        _ => Err(failure("unknown explicit test simulation request")),
    }
}

fn limits() -> SimulationLimitsV1 {
    SimulationLimitsV1 {
        max_canonical_bytes: 4 * 1024 * 1024,
        max_reachable_functions: 64,
        max_reachable_operations: 32_768,
        max_invocations: 1_024,
        max_workgroups: 16,
        max_scheduled_slots: 4_096,
        max_steps: 4_000_000,
        max_call_depth: 16,
        max_ssa_values: 16_384,
        max_allocations: 16_384,
        max_allocation_bytes: 1024 * 1024,
        max_total_bytes: 16 * 1024 * 1024,
        max_resident_bytes: 128 * 1024 * 1024,
        max_events: 1,
        max_memory_access_records: 16_384,
    }
}

struct Scenario {
    label: String,
    active: usize,
    arguments: Vec<SimulationArgumentV1>,
    backings: Vec<SharedBufferV1>,
    expected: Vec<SharedBufferV1>,
    output_elements: usize,
    written_elements: usize,
}

fn guarded_buffer(
    id: u32,
    values: &[f32],
    access: AccessMode,
) -> Result<(SimulationArgumentV1, SharedBufferV1), SourceFailure> {
    let mut bytes = Vec::with_capacity((values.len() + 2 * GUARD_ELEMENTS) * 4);
    // Finite distinct guards make both prefix/suffix and accidental in-view
    // copies visible without relying on NaN payload behavior.
    for value in [901.25_f32, -902.5, 903.75, -904.0]
        .iter()
        .chain(values.iter())
        .chain([905.25_f32, -906.5, 907.75, -908.0].iter())
    {
        bytes.extend_from_slice(&value.to_bits().to_le_bytes());
    }
    let buffer = BufferArgumentV1::new(
        ScalarType::F32,
        access,
        4,
        bytes.clone(),
        vec![true; bytes.len()],
        TARGET,
    )
    .map_err(failure)?;
    let view = BufferViewArgumentV1::new(
        BufferBackingIdV1(id),
        ScalarType::F32,
        access,
        4,
        GUARD_ELEMENTS * 4,
        values.len(),
        TARGET,
    )
    .map_err(failure)?;
    Ok((
        SimulationArgumentV1::BufferView(view),
        SharedBufferV1 {
            id: BufferBackingIdV1(id),
            buffer,
        },
    ))
}

fn input_values(len: usize, salt: usize) -> Vec<f32> {
    (0..len)
        .map(|i| (((i * 7 + salt) % 23) as i32 - 11) as f32 * 0.125)
        .collect()
}

fn rounded_add(lhs: f32, rhs: f32) -> f32 {
    (f64::from(lhs) + f64::from(rhs)) as f32
}

fn rounded_multiply(lhs: f32, rhs: f32) -> f32 {
    (f64::from(lhs) * f64::from(rhs)) as f32
}

fn elementwise(case: Case, out_len: usize, extra_inputs: usize) -> Result<Scenario, SourceFailure> {
    let mut arguments = Vec::new();
    let mut backings = Vec::new();
    let expected_values = match case {
        Case::Fill => vec![42.5_f32; out_len],
        Case::Vecadd => {
            let a = input_values(out_len + extra_inputs, 1);
            let b = input_values(out_len + extra_inputs, 13);
            for (id, values) in [(0, &a), (1, &b)] {
                let (argument, backing) = guarded_buffer(id, values, AccessMode::ReadOnly)?;
                arguments.push(argument);
                backings.push(backing);
            }
            (0..out_len).map(|i| rounded_add(a[i], b[i])).collect()
        }
        Case::ScalarGemm => return Err(failure("GEMM requires its recurrence fixture")),
        Case::ScalarBorrow
        | Case::SaturatingInteger(_)
        | Case::NumericCast(_)
        | Case::ConstantShift(_) => {
            return Err(failure("integer saturation requires its typed fixture"));
        }
        Case::F32Negate | Case::F32Divide => {
            return Err(failure("F32 arithmetic requires scalar bit-vector inputs"));
        }
    };
    let output_id = backings.len() as u32;
    let (argument, backing) =
        guarded_buffer(output_id, &vec![SENTINEL; out_len], AccessMode::ReadWrite)?;
    arguments.push(argument);
    let mut expected = backings.clone();
    backings.push(backing);
    expected.push(guarded_buffer(output_id, &expected_values, AccessMode::ReadWrite)?.1);
    Ok(Scenario {
        label: format!("len-{out_len}-extra-inputs-{extra_inputs}"),
        active: out_len,
        arguments,
        backings,
        expected,
        output_elements: out_len,
        written_elements: out_len,
    })
}

fn gemm(m: usize, n: usize, k: usize, out_len: usize) -> Result<Scenario, SourceFailure> {
    let a = input_values(m * k, 3);
    let b = input_values(k * n, 17);
    gemm_inputs(m, n, k, out_len, a, b)
}

fn gemm_inputs(
    m: usize,
    n: usize,
    k: usize,
    out_len: usize,
    a: Vec<f32>,
    b: Vec<f32>,
) -> Result<Scenario, SourceFailure> {
    if a.len() != m * k || b.len() != k * n {
        return Err(failure(
            "CPU GEMM fixture does not have complete source input extents",
        ));
    }
    let mut expected_values = vec![SENTINEL; out_len];
    for (p, output) in expected_values.iter_mut().enumerate().take(m * n) {
        let row = p / n;
        let column = p % n;
        let mut accumulator = 0.0_f32;
        for t in 0..k {
            let product = rounded_multiply(a[row * k + t], b[t * n + column]);
            accumulator = rounded_add(accumulator, product);
        }
        *output = accumulator;
    }
    let mut arguments = Vec::new();
    let mut backings = Vec::new();
    for (id, values, access) in [
        (0, a, AccessMode::ReadOnly),
        (1, b, AccessMode::ReadOnly),
        (2, vec![SENTINEL; out_len], AccessMode::ReadWrite),
    ] {
        let (argument, backing) = guarded_buffer(id, &values, access)?;
        arguments.push(argument);
        backings.push(backing);
    }
    arguments.extend(
        [m, n, k].map(|value| SimulationArgumentV1::Scalar(ScalarBitsV1::u32(value as u32))),
    );
    let mut expected = backings.clone();
    expected[2] = guarded_buffer(2, &expected_values, AccessMode::ReadWrite)?.1;
    Ok(Scenario {
        label: format!("m-{m}-n-{n}-k-{k}-out-{out_len}"),
        active: m * n,
        arguments,
        backings,
        expected,
        output_elements: out_len,
        written_elements: (m * n).min(out_len),
    })
}

fn scenarios(case: Case) -> Result<Vec<Scenario>, SourceFailure> {
    match case {
        Case::ScalarBorrow => scalar_borrow::scenarios(),
        Case::ConstantShift(case) => constant_shift::scenarios(case),
        Case::NumericCast(case) => numeric_cast::scenarios(case),
        Case::SaturatingInteger(case) => saturating_integer::scenarios(case),
        Case::F32Negate | Case::F32Divide => f32_arithmetic::scenarios(case),
        Case::Fill | Case::Vecadd => {
            let mut cases = [0, 1, 63, 64, 65, 255, 256, 257]
                .into_iter()
                .map(|len| elementwise(case, len, 0))
                .collect::<Result<Vec<_>, _>>()?;
            if case == Case::Vecadd {
                // Source only guards C before reading A/B; never label short
                // input slices a valid positive execution.
                cases.push(elementwise(case, 0, 3)?);
                cases.push(elementwise(case, 65, 7)?);
            }
            Ok(cases)
        }
        Case::ScalarGemm => {
            let mut cases = [
                (0, 3, 2, 0),
                (3, 0, 2, 0),
                (1, 1, 0, 1),
                (1, 1, 1, 1),
                (3, 5, 7, 15),
                (1, 255, 3, 255),
                (1, 256, 3, 256),
                (1, 257, 3, 257),
                (2, 3, 4, 4),
                (2, 3, 4, 9),
            ]
            .into_iter()
            .map(|(m, n, k, out)| gemm(m, n, k, out))
            .collect::<Result<Vec<_>, _>>()?;
            let mut sensitive = gemm_inputs(
                1,
                1,
                2,
                1,
                vec![-1.0, f32::from_bits(0x3f80_0001)],
                vec![1.0, f32::from_bits(0x3f7f_fffe)],
            )?;
            sensitive.label = "m-1-n-1-k-2-separate-multiply-add".to_owned();
            cases.push(sensitive);
            Ok(cases)
        }
    }
}

fn require_abi(module: &AdmittedSimulationModuleV1, case: Case) -> Result<&Kernel, SourceFailure> {
    if matches!(case, Case::ConstantShift(_)) {
        return Err(failure("shifts require exact multi-root execution plans"));
    }
    if case == Case::ScalarBorrow {
        return scalar_borrow::require_abi(module);
    }
    if let Case::NumericCast(case) = case {
        return numeric_cast::require_abi(module, case);
    }
    if let Case::SaturatingInteger(case) = case {
        return saturating_integer::require_abi(module, case);
    }
    if matches!(case, Case::F32Negate | Case::F32Divide) {
        return f32_arithmetic::require_abi(module, case);
    }
    let [kernel] = module.module().kernels.as_slice() else {
        return Err(failure("oracle requires exactly one actual output root"));
    };
    let function = module
        .module()
        .functions
        .iter()
        .find(|function| function.id == kernel.entry)
        .ok_or_else(|| failure("actual kernel entry missing"))?;
    let buffers = if case == Case::Fill { 1 } else { 3 };
    let scalars = if case == Case::ScalarGemm { 3 } else { 0 };
    if function.signature.parameters.len() != buffers + scalars
        || !function.signature.results.is_empty()
        || kernel.domain.rank() != 1
    {
        return Err(failure("actual O does not have the requested test ABI"));
    }
    for (ordinal, ty) in function.signature.parameters.iter().enumerate() {
        if ordinal < buffers {
            let Type::Slice(slice) = ty else {
                return Err(failure("test buffers require actual slice ABI parameters"));
            };
            let readonly = ordinal + 1 < buffers;
            if slice.address_space != AddressSpace::Global
                || slice.element.as_ref() != &Type::Scalar(ScalarType::F32)
                || (readonly && slice.access != AccessMode::ReadOnly)
                || (!readonly && slice.access == AccessMode::ReadOnly)
            {
                return Err(failure(
                    "actual O slice type/access differs from requested test ABI",
                ));
            }
        } else if ty != &Type::Scalar(ScalarType::U32) {
            return Err(failure(
                "GEMM dimensions require exact actual U32 parameters",
            ));
        }
    }
    Ok(kernel)
}

fn launch(kernel: &Kernel, active: usize) -> Result<([u64; 3], [u32; 3]), SourceFailure> {
    let workgroup = kernel
        .workgroup_size
        .map(|size| [size.x, size.y, size.z])
        .unwrap_or([64, 1, 1]);
    if workgroup[0] == 0 || workgroup[1..] != [1, 1] {
        return Err(failure(
            "test oracle requires a nonzero one-dimensional workgroup",
        ));
    }
    let active = active.max(1) as u64;
    let width = u64::from(workgroup[0]);
    let padded = active.div_ceil(width) * width;
    let extent = match kernel.domain.extents().next() {
        Some(LaunchExtent::Static(extent)) => u64::from(extent),
        Some(LaunchExtent::Dynamic) => padded,
        None => return Err(failure("actual output has no launch extent")),
    };
    if extent < active {
        return Err(failure(
            "actual static launch is too small for the full oracle case",
        ));
    }
    Ok(([extent, 1, 1], workgroup))
}

fn check_backings(
    actual: &[SharedBufferV1],
    expected: &[SharedBufferV1],
) -> Result<(), SourceFailure> {
    if actual.len() != expected.len() {
        return Err(failure("simulator changed backing allocation count"));
    }
    for expected in expected {
        let actual = actual
            .iter()
            .find(|actual| actual.id == expected.id)
            .ok_or_else(|| failure("simulator omitted expected backing"))?;
        if actual != expected {
            return Err(failure(format!(
                "backing {} differs from CPU reference, readonly bytes, initialization, or canaries",
                expected.id.0
            )));
        }
    }
    Ok(())
}

fn check_execution(
    execution: &SimulationExecutionV1,
    request: &SimulationRequestV1,
    expected: &[SharedBufferV1],
    identity: SimulationKernelIrIdentityV1,
    case: Case,
) -> Result<(), SourceFailure> {
    if execution.identity() != &identity || execution.arguments() != request.arguments {
        return Err(failure(
            "simulation changed exact O identity or scalar/view ABI arguments",
        ));
    }
    if execution.invocations_executed() != request.grid.0[0] {
        return Err(failure(
            "simulation did not execute every requested invocation",
        ));
    }
    match case {
        Case::F32Negate | Case::F32Divide => {
            f32_arithmetic::check_backings(case, execution.shared_buffers(), expected)
        }
        Case::Fill
        | Case::Vecadd
        | Case::ScalarGemm
        | Case::ScalarBorrow
        | Case::SaturatingInteger(_)
        | Case::NumericCast(_)
        | Case::ConstantShift(_) => check_backings(execution.shared_buffers(), expected),
    }
}

fn assessments(
    execution: &SimulationExecutionV1,
) -> Result<(Assessment, Assessment), SourceFailure> {
    let conflicts = match execution.conflict_assessment() {
        SimulationConflictAssessmentV1::NoConflictsObserved => Assessment::NoObserved,
        SimulationConflictAssessmentV1::Incomplete {
            conflicting_bytes: 0,
            first: None,
            ..
        } => Assessment::Incomplete,
        other => {
            return Err(failure(format!(
                "simulation observed conflicting access: {other:?}"
            )));
        }
    };
    let races = match execution.race_assessment() {
        SimulationRaceAssessmentV1::NoRacesObserved { .. } => Assessment::NoObserved,
        SimulationRaceAssessmentV1::Incomplete {
            racing_bytes: 0,
            first: None,
            ..
        } => Assessment::Incomplete,
        other => {
            return Err(failure(format!(
                "simulation observed racing access: {other:?}"
            )));
        }
    };
    // These finite execution observations never prove another schedule safe.
    // An incomplete assessment remains explicit even when numeric checks pass.
    Ok((conflicts, races))
}

pub(super) fn observe(
    output: &VerifiedCanonicalKernelIrV12,
    case: Case,
) -> Result<SimulationObservation, SourceFailure> {
    let limits = limits();
    if output.canonical_bytes().len() > limits.max_canonical_bytes {
        return Err(failure(
            "actual O exceeds the test simulator canonical-byte cap",
        ));
    }
    // Copy exact admitted O bytes, never reconstruct an analogous graph or
    // convert through another wire version. This test allocation is not a
    // compiler retained-storage receipt or source proof.
    let reconstructed =
        VerifiedCanonicalKernelIrV12::from_canonical_bytes(output.canonical_bytes().to_vec())
            .map_err(failure)?;
    if reconstructed.identity() != output.identity()
        || reconstructed.canonical_bytes() != output.canonical_bytes()
    {
        return Err(failure(
            "test decode did not preserve exact admitted O bytes/identity",
        ));
    }
    let simulator_identity = SimulationKernelIrIdentityV1::from(*reconstructed.identity());
    let module = AdmittedSimulationModuleV1::admit_v12(reconstructed, limits).map_err(failure)?;
    if module.identity() != &simulator_identity || module.grants_execution_authority() {
        return Err(failure(
            "simulator admission changed custody or granted authority",
        ));
    }
    let runs = if let Case::ConstantShift(batch) = case {
        constant_shift::runs(&module, batch)?
    } else {
        let kernel = require_abi(&module, case)?;
        scenarios(case)?
            .into_iter()
            .map(|scenario| (kernel, scenario))
            .collect()
    };
    let mut observations = Vec::new();
    for (kernel, scenario) in runs {
        let (grid, workgroup) = launch(kernel, scenario.active)?;
        let request =
            SimulationRequestV1::new(kernel.id.clone(), grid, workgroup, scenario.arguments)
                .with_shared_buffers(scenario.backings);
        let before = request.clone();
        let execution = module
            .simulate(&request, TARGET, limits)
            .map_err(|e| failure(format!("{}: {e:?}", scenario.label)))?;
        check_execution(
            &execution,
            &request,
            &scenario.expected,
            simulator_identity,
            case,
        )?;
        let (conflicts, races) = assessments(&execution)?;
        let repeated = module
            .simulate(&request, TARGET, limits)
            .map_err(|e| failure(format!("{} replay: {e:?}", scenario.label)))?;
        if request != before || repeated != execution {
            return Err(failure(format!(
                "{}: mutated inputs or nondeterministic replay",
                scenario.label
            )));
        }
        observations.push(ScenarioObservation {
            label: scenario.label,
            grid,
            workgroup,
            output_elements: scenario.output_elements,
            written_elements: scenario.written_elements,
            checked_backing_bytes: scenario
                .expected
                .iter()
                .map(|row| row.buffer.bytes().len())
                .sum(),
            steps: execution.steps_executed(),
            invocations: execution.invocations_executed(),
            deterministic_replays: 2,
            conflicts,
            races,
        });
    }
    Ok(SimulationObservation {
        case,
        native_output_digest: *output.identity().digest(),
        simulator_digest: *module.identity().digest(),
        simulator_wire_version: module.identity().wire_version(),
        canonical_bytes: module.identity().canonical_length(),
        numerical_policy: case.numerical_policy().to_owned(),
        scenarios: observations,
    })
}

pub(super) fn check_observation(observed: &Observation, case: Case) -> Result<(), SourceFailure> {
    let report = observed
        .simulation
        .as_ref()
        .ok_or_else(|| failure("actual-O simulation was not observed"))?;
    let scenarios = scenarios(case)?;
    if report.case != case
        || report.native_output_digest != observed.output_digest
        || report.simulator_digest != report.native_output_digest
        || report.simulator_wire_version != 12
        || report.canonical_bytes == 0
        || report.numerical_policy != case.numerical_policy()
        || report.scenarios.len() != scenarios.len()
    {
        return Err(failure(
            "simulation observation does not bind exact output and complete case roster",
        ));
    }
    for (actual, expected) in report.scenarios.iter().zip(scenarios) {
        let backing_bytes = expected
            .expected
            .iter()
            .map(|row| row.buffer.bytes().len())
            .sum::<usize>();
        if actual.label != expected.label
            || actual.output_elements != expected.output_elements
            || actual.written_elements != expected.written_elements
            || actual.checked_backing_bytes != backing_bytes
            || actual.grid[0] < expected.active.max(1) as u64
            || actual.grid[1..] != [1, 1]
            || actual.workgroup.contains(&0)
            || actual.workgroup[1..] != [1, 1]
            || actual.invocations != actual.grid[0]
            || actual.steps == 0
            || actual.deterministic_replays != 2
        {
            return Err(failure(
                "simulation scenario coverage/reference checks are incomplete",
            ));
        }
    }
    Ok(())
}

#[test]
fn actual_o_oracle_scenarios_preserve_source_zero_and_short_output_rules() {
    assert_eq!(scenarios(Case::Fill).unwrap().len(), 8);
    assert_eq!(scenarios(Case::Vecadd).unwrap().len(), 10);
    let cases = scenarios(Case::ScalarGemm).unwrap();
    assert_eq!(cases.len(), 11);
    assert_eq!(cases[0].written_elements, 0);
    assert_eq!(cases[1].written_elements, 0);
    assert_eq!(cases[2].written_elements, 1);
    let zero_k = &cases[2].expected[2].buffer.bytes()[GUARD_ELEMENTS * 4..][..4];
    assert_eq!(zero_k, 0.0_f32.to_bits().to_le_bytes());
    assert_eq!((cases[8].active, cases[8].written_elements), (6, 4));
    assert_eq!((cases[9].active, cases[9].written_elements), (6, 6));
    let extra = &cases[9].expected[2].buffer.bytes()[(GUARD_ELEMENTS + 6) * 4..][..12];
    assert!(
        extra
            .chunks_exact(4)
            .all(|value| value == SENTINEL.to_bits().to_le_bytes())
    );
    let left = f32::from_bits(0x3f80_0001);
    let right = f32::from_bits(0x3f7f_fffe);
    let separated = rounded_add(-1.0, rounded_multiply(left, right));
    let fused = left.mul_add(right, -1.0);
    assert_eq!(separated.to_bits(), 0.0_f32.to_bits());
    assert_ne!(separated.to_bits(), fused.to_bits());
    let sensitive = &cases[10].expected[2].buffer.bytes()[GUARD_ELEMENTS * 4..][..4];
    assert_eq!(sensitive, separated.to_bits().to_le_bytes());
}

#[test]
fn actual_o_oracle_rejects_changed_canaries_readonly_inputs_and_initialization() {
    let expected = scenarios(Case::Vecadd).unwrap().remove(1).expected;
    check_backings(&expected, &expected).unwrap();
    for (backing, byte) in [(0, 0), (0, GUARD_ELEMENTS * 4), (2, 0), (2, 16), (2, 32)] {
        let mut changed = expected.clone();
        let original = &changed[backing].buffer;
        let mut bytes = original.bytes().to_vec();
        bytes[byte] ^= 1;
        changed[backing].buffer = BufferArgumentV1::new(
            original.element(),
            original.access(),
            original.alignment(),
            bytes,
            original.initialized().to_vec(),
            TARGET,
        )
        .unwrap();
        assert!(check_backings(&changed, &expected).is_err());
    }
    let mut changed = expected.clone();
    let original = &changed[0].buffer;
    let mut initialized = original.initialized().to_vec();
    initialized[0] = false;
    changed[0].buffer = BufferArgumentV1::new(
        original.element(),
        original.access(),
        original.alignment(),
        original.bytes().to_vec(),
        initialized,
        TARGET,
    )
    .unwrap();
    assert!(check_backings(&changed, &expected).is_err());
}

fn component_canonical(read: bool) -> VerifiedCanonicalKernelIrV12 {
    use fe2o3_kernel_ir::{
        BasicBlock, BlockId, Function, LaunchDomain, Module, Signature, Terminator, ValueId,
    };
    use fe2o3_kernel_ir::{MemoryAccess, Operation, OperationKind, ValueDef};
    let mut block = BasicBlock::new(BlockId(0));
    let access = if read {
        AccessMode::ReadOnly
    } else {
        AccessMode::ReadWrite
    };
    if read {
        block.operations = vec![
            Operation::effect_free(
                ValueDef::new(
                    ValueId(1),
                    Type::pointer(Type::Scalar(ScalarType::F32), AddressSpace::Global, access),
                ),
                OperationKind::SliceData { slice: ValueId(0) },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(2), Type::Scalar(ScalarType::F32)),
                OperationKind::Load {
                    pointer: ValueId(1),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
        ];
    }
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("oracle::negative-only");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(
            vec![Type::slice(
                Type::Scalar(ScalarType::F32),
                AddressSpace::Global,
                access,
            )],
            vec![],
        ),
        vec![ValueId(0)],
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        "entry",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    VerifiedCanonicalKernelIrV12::from_module(module).unwrap()
}

#[test]
fn actual_o_oracle_rejects_verified_same_abi_noop_instead_of_accepting_simulation_alone() {
    let canonical = component_canonical(false);
    let error = observe(&canonical, Case::Fill).unwrap_err();
    assert_eq!(error.stage, SourceStage::Simulation);
    assert!(
        error.detail.contains("differs from CPU reference"),
        "{error:?}"
    );
    assert!(
        observe(&canonical, Case::Vecadd)
            .unwrap_err()
            .detail
            .contains("requested test ABI")
    );
}

#[test]
fn actual_o_oracle_simulator_uninitialized_read_and_step_limit_are_not_successes() {
    use fe2o3_kir_sim::{SimulationErrorV1, SimulationExecutionErrorKindV1};
    let canonical = component_canonical(true);
    let expected_identity = SimulationKernelIrIdentityV1::from(*canonical.identity());
    let module = AdmittedSimulationModuleV1::admit_v12(canonical, limits()).unwrap();
    assert_eq!(module.identity(), &expected_identity);
    let (argument, mut backing) = guarded_buffer(0, &[3.0], AccessMode::ReadOnly).unwrap();
    let buffer = &backing.buffer;
    let mut initialized = buffer.initialized().to_vec();
    initialized[GUARD_ELEMENTS * 4] = false;
    backing.buffer = BufferArgumentV1::new(
        buffer.element(),
        buffer.access(),
        buffer.alignment(),
        buffer.bytes().to_vec(),
        initialized,
        TARGET,
    )
    .unwrap();
    let request = SimulationRequestV1::new("entry", [1, 1, 1], [1, 1, 1], vec![argument])
        .with_shared_buffers(vec![backing]);
    let before = request.clone();
    let uninitialized = module.simulate(&request, TARGET, limits());
    assert!(
        matches!(uninitialized,
            Err(SimulationErrorV1::Execution(ref error))
            if matches!(error.kind, SimulationExecutionErrorKindV1::UninitializedRead { .. })
        ),
        "{uninitialized:?}"
    );
    assert_eq!(request, before);

    let canonical = component_canonical(false);
    let module = AdmittedSimulationModuleV1::admit_v12(canonical, limits()).unwrap();
    let (argument, backing) = guarded_buffer(0, &[], AccessMode::ReadWrite).unwrap();
    let request = SimulationRequestV1::new("entry", [64, 1, 1], [64, 1, 1], vec![argument])
        .with_shared_buffers(vec![backing]);
    let before = request.clone();
    let mut low = limits();
    low.max_steps = 1;
    let exhausted = module.simulate(&request, TARGET, low);
    assert!(
        matches!(exhausted,
            Err(SimulationErrorV1::Execution(ref error))
            if matches!(error.kind, SimulationExecutionErrorKindV1::StepLimit { limit: 1 })
        ),
        "{exhausted:?}"
    );
    assert_eq!(request, before);
}

#[test]
fn actual_o_oracle_archived_observation_has_no_invented_simulation_pass() {
    let observed: Observation = serde_json::from_value(serde_json::json!({
        "roots": ["old"], "internal_helpers": 0, "helper_calls": 0,
        "reads": 0, "writes": 1, "global_reads": 0, "global_writes": 1,
        "private_reads": 0, "private_writes": 0, "other_reads": 0, "other_writes": 0,
        "formal_accesses": 1, "policy": 4, "output_digest": vec![1; 32],
        "llvm_bytes": 1, "descriptor_roots": 1, "missing_proof_refused": false,
    }))
    .unwrap();
    assert!(observed.simulation.is_none());
    assert!(check_observation(&observed, Case::Fill).is_err());
    assert!(
        serde_json::to_value(observed)
            .unwrap()
            .get("simulation")
            .is_none()
    );
}

#[test]
fn actual_o_oracle_inert_report_framing_rejects_identity_and_roster_substitutions() {
    // Inert JSON framing only: these rows are not asserted to come from an
    // execution and cannot create a compiler owner or simulator result.
    let rows = scenarios(Case::Fill)
        .unwrap()
        .into_iter()
        .map(|scenario| {
            let extent = (scenario.active.max(1) as u64).div_ceil(64) * 64;
            ScenarioObservation {
                label: scenario.label,
                grid: [extent, 1, 1],
                workgroup: [64, 1, 1],
                output_elements: scenario.output_elements,
                written_elements: scenario.written_elements,
                checked_backing_bytes: scenario
                    .expected
                    .iter()
                    .map(|row| row.buffer.bytes().len())
                    .sum(),
                steps: 1,
                invocations: extent,
                deterministic_replays: 2,
                conflicts: Assessment::NoObserved,
                races: Assessment::NoObserved,
            }
        })
        .collect();
    let report = SimulationObservation {
        case: Case::Fill,
        native_output_digest: [1; 32],
        simulator_digest: [1; 32],
        simulator_wire_version: 12,
        canonical_bytes: 123,
        numerical_policy: "finite-dyadic-f32-separate-multiply-add-bit-exact-v1".to_owned(),
        scenarios: rows,
    };
    let json = serde_json::json!({
        "roots": ["framing-only"], "internal_helpers": 0, "helper_calls": 0,
        "reads": 0, "writes": 1, "global_reads": 0, "global_writes": 1,
        "private_reads": 0, "private_writes": 0, "other_reads": 0, "other_writes": 0,
        "formal_accesses": 1, "policy": 4, "output_digest": vec![1; 32],
        "llvm_bytes": 1, "descriptor_roots": 1, "missing_proof_refused": false,
        "simulation": report,
    });
    let valid: Observation = serde_json::from_value(json.clone()).unwrap();
    check_observation(&valid, Case::Fill).unwrap();
    for (path, replacement) in [
        ("/simulation/native_output_digest/0", serde_json::json!(2)),
        ("/simulation/simulator_digest/0", serde_json::json!(2)),
        ("/simulation/simulator_wire_version", serde_json::json!(11)),
        ("/simulation/numerical_policy", serde_json::json!("fused")),
        ("/simulation/scenarios/0/invocations", serde_json::json!(0)),
        (
            "/simulation/scenarios/0/deterministic_replays",
            serde_json::json!(1),
        ),
    ] {
        let mut changed = json.clone();
        *changed.pointer_mut(path).unwrap() = replacement;
        let observed: Observation = serde_json::from_value(changed).unwrap();
        assert!(check_observation(&observed, Case::Fill).is_err(), "{path}");
    }
    for mode in 0..3 {
        let mut changed = json.clone();
        let rows = changed["simulation"]["scenarios"].as_array_mut().unwrap();
        match mode {
            0 => {
                rows.pop();
            }
            1 => rows.swap(0, 1),
            2 => rows[1] = rows[0].clone(),
            _ => unreachable!(),
        }
        let observed: Observation = serde_json::from_value(changed).unwrap();
        assert!(check_observation(&observed, Case::Fill).is_err());
    }
}
