//! Typed Rust reference for supplied actual output graphs; no graph synthesis.
pub(in super::super) use super::constant_shift::Integer;
use super::*;
use fe2o3_kernel_ir::{FunctionRole, LaunchDomain, Module, WorkgroupSize};

pub(super) const NUMERICAL_POLICY: &str = "rust-fixed-integer-identities-exact-bytes-v1";
pub(in super::super) const ROOTS: [&str; 5] = [
    "identity_xor_zero",
    "identity_or_zero",
    "identity_and_ones",
    "identity_control",
    "identity_noop",
];
const LENGTHS: [usize; 5] = [0, 1, 63, 64, 65];

pub(super) fn expected_grid(active: usize) -> [u64; 3] {
    [(active.max(1) as u64).div_ceil(64) * 64, 1, 1]
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(in super::super) struct Batch {
    pub(in super::super) integer: Integer,
    pub(in super::super) retained: bool,
}

macro_rules! typed_oracle {
    ($(($variant:ident, $ty:ident, $plain:literal, $retained:literal)),* $(,)?) => {
        impl Batch {
            pub(in super::super) fn name(self) -> &'static str {
                match (self.integer, self.retained) {
                    $((Integer::$variant, false) => $plain,
                      (Integer::$variant, true) => $retained),*
                }
            }
            pub(super) fn parse(name: &str) -> Option<Self> {
                match name {
                    $($plain => Some(Self { integer: Integer::$variant, retained: false }),
                      $retained => Some(Self { integer: Integer::$variant, retained: true })),*,
                    _ => None,
                }
            }
        }
        #[allow(clippy::identity_op)]
        fn rust_result(integer: Integer, root: usize, input: u128) -> u128 {
            let result = match integer {
                $(Integer::$variant => {
                    let value = input as $ty;
                    let output: $ty = match root {
                        0 => value ^ 0,
                        1 => value | 0,
                        2 => value & !0,
                        3 => value ^ 3,
                        4 => value,
                        _ => unreachable!("closed integer-identity root"),
                    };
                    output as u128
                }),*
            };
            result & mask(integer)
        }
    }
}

typed_oracle! {
    (I8, i8, "integer-identity-i8", "integer-identity-i8-retained"),
    (U8, u8, "integer-identity-u8", "integer-identity-u8-retained"),
    (I16, i16, "integer-identity-i16", "integer-identity-i16-retained"),
    (U16, u16, "integer-identity-u16", "integer-identity-u16-retained"),
    (I32, i32, "integer-identity-i32", "integer-identity-i32-retained"),
    (U32, u32, "integer-identity-u32", "integer-identity-u32-retained"),
    (I64, i64, "integer-identity-i64", "integer-identity-i64-retained"),
    (U64, u64, "integer-identity-u64", "integer-identity-u64-retained"),
}

fn mask(integer: Integer) -> u128 {
    (1_u128 << integer.width()) - 1
}

fn vectors(integer: Integer) -> [u128; 10] {
    let high = 1_u128 << (integer.width() - 1);
    let alternating = 0xaaaa_aaaa_aaaa_aaaa_u128 & mask(integer);
    [
        0,
        1,
        2,
        high - 1,
        high,
        high + 1,
        mask(integer) - 1,
        mask(integer),
        alternating,
        alternating ^ mask(integer),
    ]
}

fn backing(
    integer: Integer,
    value: u128,
    len: usize,
    initialized_output: bool,
) -> Result<(SimulationArgumentV1, SharedBufferV1), SourceFailure> {
    let width = integer.width() as usize / 8;
    let start = GUARD_ELEMENTS * width;
    let end = (GUARD_ELEMENTS + len) * width;
    let mut bytes = vec![0xa5; (len + 2 * GUARD_ELEMENTS) * width];
    bytes[end..].fill(0x5a);
    for element in bytes[start..end].chunks_exact_mut(width) {
        element.copy_from_slice(&value.to_le_bytes()[..width]);
    }
    let mut initialized = vec![true; bytes.len()];
    initialized[start..end].fill(initialized_output);
    let buffer = BufferArgumentV1::new(
        integer.scalar(),
        AccessMode::ReadWrite,
        width as u32,
        bytes,
        initialized,
        TARGET,
    )
    .map_err(failure)?;
    let view = BufferViewArgumentV1::new(
        BufferBackingIdV1(0),
        integer.scalar(),
        AccessMode::ReadWrite,
        width as u32,
        start,
        len,
        TARGET,
    )
    .map_err(failure)?;
    Ok((
        SimulationArgumentV1::BufferView(view),
        SharedBufferV1 {
            id: BufferBackingIdV1(0),
            buffer,
        },
    ))
}

fn root_scenarios(batch: Batch, root: usize) -> Result<Vec<Scenario>, SourceFailure> {
    let mut result = Vec::new();
    for input in vectors(batch.integer) {
        for len in LENGTHS {
            let (output, initial) = backing(batch.integer, 0x37, len, false)?;
            let expected = backing(
                batch.integer,
                rust_result(batch.integer, root, input),
                len,
                true,
            )?
            .1;
            result.push(Scenario {
                label: format!("{}-bits-{input:016x}-len-{len}", ROOTS[root]),
                active: len,
                arguments: vec![
                    output,
                    SimulationArgumentV1::Scalar(
                        ScalarBitsV1::new(batch.integer.scalar(), input, TARGET)
                            .map_err(failure)?,
                    ),
                ],
                backings: vec![initial],
                expected: vec![expected],
                output_elements: len,
                written_elements: len,
            });
        }
    }
    Ok(result)
}

pub(super) fn scenarios(batch: Batch) -> Result<Vec<Scenario>, SourceFailure> {
    let mut result = Vec::new();
    for root in 0..ROOTS.len() {
        result.extend(root_scenarios(batch, root)?);
    }
    Ok(result)
}

pub(in super::super) fn kernel_for_root<'a>(
    module: &'a Module,
    root: &str,
) -> Result<&'a Kernel, SourceFailure> {
    let mut matches = module
        .kernels
        .iter()
        .filter(|kernel| kernel.id.as_str() == root);
    let kernel = matches
        .next()
        .ok_or_else(|| failure("integer identity root missing"))?;
    if matches.next().is_some() {
        return Err(failure("integer identity duplicate root identity"));
    }
    Ok(kernel)
}

fn ordered_kernels(module: &Module, batch: Batch) -> Result<Vec<&Kernel>, SourceFailure> {
    if module.kernels.len() != ROOTS.len() {
        return Err(failure("integer identity exact five-root roster"));
    }
    ROOTS
        .into_iter()
        .map(|root| {
            let kernel = kernel_for_root(module, root)?;
            let entry = module
                .function(&kernel.entry)
                .ok_or_else(|| failure("integer identity actual entry missing"))?;
            let [Type::Slice(output), scalar] = entry.signature.parameters.as_slice() else {
                return Err(failure("integer identity output/scalar ABI"));
            };
            if entry.role != FunctionRole::KernelEntry
                || !entry.signature.results.is_empty()
                || entry.body.is_none()
                || output.address_space != AddressSpace::Global
                || output.access != AccessMode::ReadWrite
                || output.element.as_ref() != &Type::Scalar(batch.integer.scalar())
                || scalar != &Type::Scalar(batch.integer.scalar())
                || !matches!(
                    kernel.domain,
                    LaunchDomain::D1 {
                        x: LaunchExtent::Dynamic
                    }
                )
                || kernel.workgroup_size != Some(WorkgroupSize::new(64, 1, 1))
            {
                return Err(failure(
                    "integer identity exact signed-width ABI and declared launch",
                ));
            }
            Ok(kernel)
        })
        .collect()
}

pub(super) fn runs(
    module: &AdmittedSimulationModuleV1,
    batch: Batch,
) -> Result<Vec<(&Kernel, Scenario)>, SourceFailure> {
    let mut result = Vec::new();
    // This joins root identities for reporting; it never reorders the module.
    for (root, kernel) in ordered_kernels(module.module(), batch)?
        .into_iter()
        .enumerate()
    {
        for scenario in root_scenarios(batch, root)? {
            result.push((kernel, scenario));
        }
    }
    Ok(result)
}

#[test]
fn integer_identity_typed_reference_covers_all_widths_roots_and_boundary_lengths() {
    for integer in Integer::ALL {
        let width = integer.width() as usize / 8;
        for retained in [false, true] {
            let batch = Batch { integer, retained };
            assert_eq!(Batch::parse(batch.name()), Some(batch));
            assert_eq!(scenarios(batch).unwrap().len(), 250);
            for root in 0..ROOTS.len() {
                let scenarios = root_scenarios(batch, root).unwrap();
                for (input, group) in vectors(integer).into_iter().zip(scenarios.chunks_exact(5)) {
                    let expected_bits = if root == 3 { input ^ 3 } else { input };
                    assert_eq!(rust_result(integer, root, input), expected_bits);
                    for (len, scenario) in LENGTHS.into_iter().zip(group) {
                        assert_eq!(
                            (
                                scenario.active,
                                scenario.output_elements,
                                scenario.written_elements
                            ),
                            (len, len, len)
                        );
                        let initial = &scenario.backings[0].buffer;
                        let expected = &scenario.expected[0].buffer;
                        let start = GUARD_ELEMENTS * width;
                        let end = (GUARD_ELEMENTS + len) * width;
                        assert_eq!(initial.bytes().len(), (len + 8) * width);
                        assert_eq!(&initial.bytes()[..start], &expected.bytes()[..start]);
                        assert_eq!(&initial.bytes()[end..], &expected.bytes()[end..]);
                        assert!(initial.initialized()[..start].iter().all(|x| *x));
                        assert!(initial.initialized()[end..].iter().all(|x| *x));
                        assert!(initial.initialized()[start..end].iter().all(|x| !x));
                        assert!(expected.initialized().iter().all(|x| *x));
                        assert!(
                            expected.bytes()[start..end]
                                .chunks_exact(width)
                                .all(|bytes| bytes == &expected_bits.to_le_bytes()[..width])
                        );
                    }
                }
            }
        }
    }
    assert_eq!(Batch::parse("integer-identity-i128"), None);
    assert_eq!(Batch::parse("integer-identity-u32-extra"), None);
}

fn abi_only_module(integer: Integer) -> Module {
    use fe2o3_kernel_ir::{
        BasicBlock, BlockId, Function, LaunchDomain, Signature, Terminator, ValueId,
    };
    let mut module = Module::new("integer-identity-negative-only");
    for root in ROOTS {
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: vec![] });
        let entry = format!("entry_{root}");
        module.functions.push(Function::kernel_entry(
            entry.clone(),
            Signature::new(
                vec![
                    Type::slice(
                        Type::Scalar(integer.scalar()),
                        AddressSpace::Global,
                        AccessMode::ReadWrite,
                    ),
                    Type::Scalar(integer.scalar()),
                ],
                vec![],
            ),
            vec![ValueId(0), ValueId(1)],
            vec![block],
        ));
        let mut kernel = Kernel::new(
            root,
            entry,
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        module.kernels.push(kernel);
    }
    module
}

#[test]
fn integer_identity_root_join_accepts_shuffling_not_duplicates_missing_or_foreign_roots() {
    let batch = Batch {
        integer: Integer::I32,
        retained: false,
    };
    let mut module = abi_only_module(batch.integer);
    module.kernels.rotate_left(2);
    let before = module.clone();
    let roots: Vec<_> = ordered_kernels(&module, batch)
        .unwrap()
        .iter()
        .map(|kernel| kernel.id.as_str())
        .collect();
    assert_eq!(roots, ROOTS);
    assert_eq!(module, before);
    for mutation in 0..9 {
        let mut changed = module.clone();
        match mutation {
            0 => {
                changed.kernels.pop();
            }
            1 => changed.kernels[1] = changed.kernels[0].clone(),
            2 => changed.kernels[0].id = "foreign-root".into(),
            3 => changed.functions[0].signature.parameters[1] = Type::Scalar(ScalarType::U32),
            4 => changed.kernels[0].workgroup_size = None,
            5 => changed.kernels[0].workgroup_size = Some(WorkgroupSize::new(32, 1, 1)),
            6 => changed.functions[0].signature.parameters.swap(0, 1),
            7 => {
                changed.kernels[0].domain = LaunchDomain::D1 {
                    x: LaunchExtent::Static(128),
                }
            }
            8 => {
                changed.kernels[0].domain = LaunchDomain::D2 {
                    x: LaunchExtent::Dynamic,
                    y: LaunchExtent::Dynamic,
                }
            }
            _ => unreachable!(),
        }
        assert!(
            ordered_kernels(&changed, batch).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn integer_identity_oracle_rejects_no_stores_even_with_the_exact_five_root_abi() {
    let batch = Batch {
        integer: Integer::U32,
        retained: false,
    };
    let canonical =
        VerifiedCanonicalKernelIrV12::from_module(abi_only_module(batch.integer)).unwrap();
    let error = observe(&canonical, Case::IntegerIdentity(batch)).unwrap_err();
    assert_eq!(error.stage, SourceStage::Simulation);
    assert!(
        error.detail.contains("differs from CPU reference"),
        "{error:?}"
    );
}

#[test]
fn integer_identity_full_backing_oracle_rejects_canaries_values_and_initialization_changes() {
    for integer in Integer::ALL {
        let width = integer.width() as usize / 8;
        let expected = backing(integer, mask(integer), 1, true).unwrap().1;
        check_backings(
            std::slice::from_ref(&expected),
            std::slice::from_ref(&expected),
        )
        .unwrap();
        for byte in [0, GUARD_ELEMENTS * width, (GUARD_ELEMENTS + 1) * width] {
            let mut changed = expected.clone();
            let mut bytes = changed.buffer.bytes().to_vec();
            bytes[byte] ^= 1;
            changed.buffer = BufferArgumentV1::new(
                integer.scalar(),
                AccessMode::ReadWrite,
                width as u32,
                bytes,
                expected.buffer.initialized().to_vec(),
                TARGET,
            )
            .unwrap();
            assert!(check_backings(&[changed], std::slice::from_ref(&expected)).is_err());
        }
        let changed = backing(integer, mask(integer), 1, false).unwrap().1;
        assert!(check_backings(&[changed], &[expected]).is_err());
    }
}

fn inert_report(batch: Batch) -> SimulationObservation {
    // Framing-only rows: they do not claim execution or construct an owner.
    let scenarios = scenarios(batch)
        .unwrap()
        .into_iter()
        .map(|scenario| {
            let grid = expected_grid(scenario.active);
            ScenarioObservation {
                label: scenario.label,
                grid,
                workgroup: [64, 1, 1],
                output_elements: scenario.output_elements,
                written_elements: scenario.written_elements,
                checked_backing_bytes: scenario
                    .expected
                    .iter()
                    .map(|b| b.buffer.bytes().len())
                    .sum(),
                steps: 1,
                invocations: grid[0],
                deterministic_replays: 2,
                conflicts: Assessment::NoObserved,
                races: Assessment::NoObserved,
            }
        })
        .collect();
    SimulationObservation {
        case: Case::IntegerIdentity(batch),
        native_output_digest: [1; 32],
        simulator_digest: [1; 32],
        simulator_wire_version: 12,
        canonical_bytes: 1,
        numerical_policy: NUMERICAL_POLICY.to_owned(),
        scenarios,
    }
}

#[test]
fn integer_identity_report_binds_exact_final_digest_case_roster_launch_and_replays() {
    let batch = Batch {
        integer: Integer::U64,
        retained: true,
    };
    let case = Case::IntegerIdentity(batch);
    check_report(&inert_report(batch), [1; 32], case).unwrap();
    assert!(check_report(&inert_report(batch), [2; 32], case).is_err());
    assert!(
        check_report(
            &inert_report(batch),
            [1; 32],
            Case::IntegerIdentity(Batch {
                integer: Integer::I64,
                retained: true,
            })
        )
        .is_err()
    );
    assert!(
        check_report(
            &inert_report(batch),
            [1; 32],
            Case::IntegerIdentity(Batch {
                integer: Integer::U64,
                retained: false,
            })
        )
        .is_err()
    );
    for mutation in 0..11 {
        let mut report = inert_report(batch);
        match mutation {
            0 => {
                report.scenarios.pop();
            }
            1 => report.scenarios.swap(0, 50),
            2 => report.scenarios[1].label = report.scenarios[0].label.clone(),
            3 => report.scenarios[0].workgroup = [32, 1, 1],
            4 => report.scenarios[0].deterministic_replays = 1,
            5 => report.scenarios[0].checked_backing_bytes -= 1,
            6 => report.simulator_digest[0] ^= 1,
            7 => report.simulator_wire_version = 11,
            8 => {
                report.scenarios[0].grid[0] = 1;
                report.scenarios[0].invocations = 1;
            }
            9 => {
                report.scenarios[0].grid[0] += 64;
                report.scenarios[0].invocations = report.scenarios[0].grid[0];
            }
            10 => {
                let row = report
                    .scenarios
                    .iter_mut()
                    .find(|row| row.output_elements == 65)
                    .unwrap();
                row.grid[0] = 65;
                row.invocations = 65;
            }
            _ => unreachable!(),
        }
        assert!(
            check_report(&report, [1; 32], case).is_err(),
            "mutation {mutation}"
        );
    }
    for (active, extent) in [(0, 64), (1, 64), (63, 64), (64, 64), (65, 128)] {
        assert_eq!(expected_grid(active), [extent, 1, 1]);
    }
}
