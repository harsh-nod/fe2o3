//! Independent typed Rust oracle for all roots in one actual-O shift module.
use super::*;
use fe2o3_kernel_ir::{BinaryOp, Constant, Function, FunctionRole, Module, OperationKind, ValueId};

pub(super) const NUMERICAL_POLICY: &str = "rust-fixed-integer-shifts-exact-bytes-v1";
pub(in super::super) const ROOTS: [&str; 6] = [
    "shift_left_last",
    "shift_left_mid",
    "shift_left_zero",
    "shift_right_last",
    "shift_right_mid",
    "shift_right_zero",
];

macro_rules! integers {
    ($(($variant:ident, $ty:ident, $bits:literal, $signed:literal, $plain:literal, $retained:literal)),* $(,)?) => {
        #[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
        #[serde(rename_all = "kebab-case")]
        pub(in super::super) enum Integer { $($variant),* }
        impl Integer {
            pub(in super::super) const ALL: [Self; 8] = [$(Self::$variant),*];
            pub(in super::super) fn name(self) -> &'static str { match self { $(Self::$variant => stringify!($ty)),* } }
            pub(in super::super) fn scalar(self) -> ScalarType { match self { $(Self::$variant => ScalarType::$variant),* } }
            pub(in super::super) fn width(self) -> u32 { match self { $(Self::$variant => $bits),* } }
            pub(in super::super) fn signed(self) -> bool { match self { $(Self::$variant => $signed),* } }
            fn rust_result(self, value: u128, right: bool, count: u32) -> u128 {
                assert!(count < self.width());
                let bits = match self { $(Self::$variant => {
                    let value = value as $ty;
                    if right { (value >> count) as u128 } else { (value << count) as u128 }
                }),* };
                bits & self.mask()
            }
        }
        impl Batch {
            pub(in super::super) fn name(self) -> &'static str { match (self.integer, self.retained) {
                $((Integer::$variant, false) => $plain, (Integer::$variant, true) => $retained),*
            } }
            pub(super) fn parse(name: &str) -> Option<Self> { match name {
                $($plain => Some(Self { integer: Integer::$variant, retained: false }),
                  $retained => Some(Self { integer: Integer::$variant, retained: true })),*,
                _ => None,
            } }
        }
    }
}

integers! {
    (I8, i8, 8, true, "shift-i8", "shift-i8-retained"),
    (U8, u8, 8, false, "shift-u8", "shift-u8-retained"),
    (I16, i16, 16, true, "shift-i16", "shift-i16-retained"),
    (U16, u16, 16, false, "shift-u16", "shift-u16-retained"),
    (I32, i32, 32, true, "shift-i32", "shift-i32-retained"),
    (U32, u32, 32, false, "shift-u32", "shift-u32-retained"),
    (I64, i64, 64, true, "shift-i64", "shift-i64-retained"),
    (U64, u64, 64, false, "shift-u64", "shift-u64-retained"),
}

impl Integer {
    fn mask(self) -> u128 {
        (1_u128 << self.width()) - 1
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(in super::super) struct Batch {
    pub(in super::super) integer: Integer,
    pub(in super::super) retained: bool,
}

pub(in super::super) fn root_case(integer: Integer, ordinal: usize) -> (bool, u32) {
    assert!(ordinal < ROOTS.len());
    (
        ordinal >= 3,
        [integer.width() - 1, integer.width() / 2, 0][ordinal % 3],
    )
}

fn vectors(integer: Integer) -> Vec<u128> {
    let high = 1_u128 << (integer.width() - 1);
    let alternating = 0xaaaa_aaaa_aaaa_aaaa_u128 & integer.mask();
    vec![
        0,
        1,
        2,
        high - 1,
        high,
        high + 1,
        integer.mask() - 1,
        integer.mask(),
        alternating,
        alternating ^ integer.mask(),
    ]
}

pub(super) fn backing(
    ty: ScalarType,
    value: u128,
    len: usize,
) -> Result<(SimulationArgumentV1, SharedBufferV1), SourceFailure> {
    let width = usize::from(ty.bit_width().unwrap() / 8);
    let mut bytes = vec![0xa5; (len + 2 * GUARD_ELEMENTS) * width];
    bytes[(len + GUARD_ELEMENTS) * width..].fill(0x5a);
    for cell in
        bytes[GUARD_ELEMENTS * width..(GUARD_ELEMENTS + len) * width].chunks_exact_mut(width)
    {
        cell.copy_from_slice(&value.to_le_bytes()[..width]);
    }
    let buffer = BufferArgumentV1::new(
        ty,
        AccessMode::ReadWrite,
        width as u32,
        bytes.clone(),
        vec![true; bytes.len()],
        TARGET,
    )
    .map_err(failure)?;
    let view = BufferViewArgumentV1::new(
        BufferBackingIdV1(0),
        ty,
        AccessMode::ReadWrite,
        width as u32,
        GUARD_ELEMENTS * width,
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

fn root_scenarios(batch: Batch, ordinal: usize) -> Result<Vec<Scenario>, SourceFailure> {
    let integer = batch.integer;
    let (right, count) = root_case(integer, ordinal);
    std::iter::once(0)
        .chain(vectors(integer))
        .enumerate()
        .map(|(vector, input)| {
            let len = if vector == 0 {
                0
            } else {
                [1, 63, 64, 65][(vector - 1) % 4]
            };
            let (output, initial) = backing(integer.scalar(), 0x37, len)?;
            let expected = backing(
                integer.scalar(),
                integer.rust_result(input, right, count),
                len,
            )?
            .1;
            Ok(Scenario {
                label: format!("{}-bits-{input:016x}-{vector}-len-{len}", ROOTS[ordinal]),
                active: len,
                arguments: vec![
                    output,
                    SimulationArgumentV1::Scalar(
                        ScalarBitsV1::new(integer.scalar(), input, TARGET).map_err(failure)?,
                    ),
                ],
                backings: vec![initial],
                expected: vec![expected],
                output_elements: len,
                written_elements: len,
            })
        })
        .collect()
}

pub(super) fn scenarios(batch: Batch) -> Result<Vec<Scenario>, SourceFailure> {
    let mut result = Vec::new();
    for ordinal in 0..ROOTS.len() {
        result.extend(root_scenarios(batch, ordinal)?);
    }
    Ok(result)
}

fn constant(function: &Function, id: ValueId, integer: Integer) -> Option<u128> {
    let operation = function
        .body
        .as_ref()?
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|op| op.results.iter().any(|result| result.id == id))?;
    if !matches!(operation.results.as_slice(), [result] if result.ty == Type::Scalar(integer.scalar()))
    {
        return None;
    }
    match operation.kind {
        OperationKind::Constant(Constant::U8(v)) => Some(u128::from(v)),
        OperationKind::Constant(Constant::U16(v)) => Some(u128::from(v)),
        OperationKind::Constant(Constant::U32(v)) => Some(u128::from(v)),
        OperationKind::Constant(Constant::U64(v)) => Some(u128::from(v)),
        OperationKind::Constant(Constant::I8(v)) => u128::try_from(v).ok(),
        OperationKind::Constant(Constant::I16(v)) => u128::try_from(v).ok(),
        OperationKind::Constant(Constant::I32(v)) => u128::try_from(v).ok(),
        OperationKind::Constant(Constant::I64(v)) => u128::try_from(v).ok(),
        _ => None,
    }
}

fn fixed_count(function: &Function, id: ValueId, integer: Integer) -> Option<u128> {
    if let Some(value) = constant(function, id, integer) {
        return Some(value);
    }
    let operation = function
        .body
        .as_ref()?
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|op| op.results.iter().any(|result| result.id == id))?;
    let OperationKind::Binary {
        op: BinaryOp::BitAnd,
        lhs,
        rhs,
    } = operation.kind
    else {
        return None;
    };
    if !matches!(operation.results.as_slice(), [result] if result.ty == Type::Scalar(integer.scalar()))
    {
        return None;
    }
    let a = constant(function, lhs, integer)?;
    let b = constant(function, rhs, integer)?;
    let width = integer.width();
    let mask = u128::from(width - 1);
    if b == mask && a < u128::from(width) {
        Some(a & b)
    } else {
        None
    }
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
        .ok_or_else(|| failure("shift root missing"))?;
    if matches.next().is_some() {
        return Err(failure("shift duplicate root identity"));
    }
    Ok(kernel)
}

fn ordered_kernels(module: &Module) -> Result<Vec<&Kernel>, SourceFailure> {
    if module.kernels.len() != ROOTS.len() {
        return Err(failure("shift exact six-root module"));
    }
    ROOTS
        .into_iter()
        .map(|root| kernel_for_root(module, root))
        .collect()
}

pub(in super::super) fn check_preserved_root_order(
    original: &Module,
    output: &Module,
) -> Result<(), SourceFailure> {
    if !original
        .kernels
        .iter()
        .map(|kernel| &kernel.id)
        .eq(output.kernels.iter().map(|kernel| &kernel.id))
    {
        return Err(failure(
            "shift original-to-output root identity or order changed",
        ));
    }
    Ok(())
}

pub(in super::super) fn operation_function(
    module: &Module,
    batch: Batch,
    ordinal: usize,
) -> Result<&Function, SourceFailure> {
    let root = ROOTS
        .get(ordinal)
        .ok_or_else(|| failure("shift root ordinal"))?;
    let kernel = kernel_for_root(module, root)?;
    let entry = module
        .function(&kernel.entry)
        .ok_or_else(|| failure("shift entry missing"))?;
    let [Type::Slice(output), input] = entry.signature.parameters.as_slice() else {
        return Err(failure("shift output/scalar ABI"));
    };
    if output.address_space != AddressSpace::Global
        || output.element.as_ref() != &Type::Scalar(batch.integer.scalar())
        || output.access != AccessMode::ReadWrite
        || input != &Type::Scalar(batch.integer.scalar())
        || !entry.signature.results.is_empty()
        || kernel.domain.rank() != 1
    {
        return Err(failure("shift exact signed/width ABI"));
    }
    let calls = entry
        .body
        .iter()
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
        .filter_map(|operation| {
            if let OperationKind::Call { callee, arguments } = &operation.kind {
                module
                    .function(callee)
                    .filter(|f| f.role == FunctionRole::InternalHelper)
                    .map(|function| (function, arguments, &operation.results))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    if !batch.retained {
        if !calls.is_empty() {
            return Err(failure("direct shift unexpectedly retained a helper"));
        }
        return Ok(entry);
    }
    let [(helper, arguments, results)] = calls.as_slice() else {
        return Err(failure(
            "shift requires exactly one actual retained helper call per root",
        ));
    };
    if arguments.len() != 1
        || !matches!(results.as_slice(), [value] if value.ty == Type::Scalar(batch.integer.scalar()))
        || helper.signature.parameters != [Type::Scalar(batch.integer.scalar())]
        || helper.signature.results != [Type::Scalar(batch.integer.scalar())]
    {
        return Err(failure("shift exact retained-helper scalar ABI"));
    }
    Ok(helper)
}

pub(in super::super) fn check_graph(
    module: &Module,
    batch: Batch,
) -> Result<std::collections::BTreeMap<&'static str, String>, SourceFailure> {
    ordered_kernels(module)?;
    let helpers = module
        .functions
        .iter()
        .filter(|f| f.role == FunctionRole::InternalHelper)
        .count();
    if helpers != if batch.retained { ROOTS.len() } else { 0 } {
        return Err(failure("shift exact retained helper roster"));
    }
    let mut owners = std::collections::BTreeMap::new();
    for (ordinal, root) in ROOTS.into_iter().enumerate() {
        let function = operation_function(module, batch, ordinal)?;
        let (right, count) = root_case(batch.integer, ordinal);
        let expected = if right {
            BinaryOp::ShiftRight
        } else {
            BinaryOp::ShiftLeft
        };
        let shifts = function
            .body
            .iter()
            .flat_map(|body| &body.blocks)
            .flat_map(|block| &block.operations)
            .filter(|op| {
                matches!(
                    op.kind,
                    OperationKind::Binary {
                        op: BinaryOp::ShiftLeft | BinaryOp::ShiftRight,
                        ..
                    }
                )
            })
            .collect::<Vec<_>>();
        if shifts.len() > 1 || (count != 0 && shifts.len() != 1) {
            return Err(failure("shift exact dynamic actual graph operation"));
        }
        for shift in shifts {
            let OperationKind::Binary { op, rhs, .. } = shift.kind else {
                unreachable!()
            };
            if op != expected
                || fixed_count(function, rhs, batch.integer) != Some(u128::from(count))
                || !matches!(shift.results.as_slice(), [value] if value.ty == Type::Scalar(batch.integer.scalar()))
            {
                return Err(failure(
                    "shift graph operator, literal count or result type differs",
                ));
            }
        }
        owners.insert(root, function.id.as_str().to_owned());
    }
    Ok(owners)
}

pub(super) fn runs(
    module: &AdmittedSimulationModuleV1,
    batch: Batch,
) -> Result<Vec<(&Kernel, Scenario)>, SourceFailure> {
    check_graph(module.module(), batch)?;
    let mut result = Vec::new();
    for (ordinal, kernel) in ordered_kernels(module.module())?.into_iter().enumerate() {
        for scenario in root_scenarios(batch, ordinal)? {
            result.push((kernel, scenario));
        }
    }
    Ok(result)
}

#[test]
fn fixed_shift_graph_joins_shuffled_root_identities_without_changing_actual_order() {
    use fe2o3_kernel_ir::{
        BasicBlock, BlockId, LaunchDomain, Operation, Signature, Terminator, ValueDef,
    };
    let batch = Batch {
        integer: Integer::U32,
        retained: false,
    };
    let mut module = Module::new("shift-roster-test");
    for (ordinal, root) in ROOTS.into_iter().enumerate() {
        let entry = format!("entry_{root}");
        let (right, count) = root_case(batch.integer, ordinal);
        let mut block = BasicBlock::new(BlockId(0));
        block.operations.push(Operation::new(
            vec![ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U32))],
            OperationKind::Constant(Constant::U32(count)),
        ));
        block.operations.push(Operation::new(
            vec![ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U32))],
            OperationKind::Binary {
                op: if right {
                    BinaryOp::ShiftRight
                } else {
                    BinaryOp::ShiftLeft
                },
                lhs: ValueId(1),
                rhs: ValueId(2),
            },
        ));
        block.terminator = Some(Terminator::Return { values: vec![] });
        module.functions.push(Function::kernel_entry(
            entry.clone(),
            Signature::new(
                vec![
                    Type::slice(
                        Type::Scalar(ScalarType::U32),
                        AddressSpace::Global,
                        AccessMode::ReadWrite,
                    ),
                    Type::Scalar(ScalarType::U32),
                ],
                vec![],
            ),
            vec![ValueId(0), ValueId(1)],
            vec![block],
        ));
        module.kernels.push(Kernel::new(
            root,
            entry,
            LaunchDomain::D1 {
                x: LaunchExtent::Static(64),
            },
        ));
    }
    let lexical = module.clone();
    module.kernels.rotate_left(2);
    module.kernels.swap(1, 4);
    let actual_order = module
        .kernels
        .iter()
        .map(|kernel| kernel.id.clone())
        .collect::<Vec<_>>();
    let owners = check_graph(&module, batch).unwrap();
    for (ordinal, kernel) in ordered_kernels(&module).unwrap().into_iter().enumerate() {
        assert_eq!(kernel.id.as_str(), ROOTS[ordinal]);
        assert_eq!(owners[ROOTS[ordinal]], format!("entry_{}", ROOTS[ordinal]));
        assert_eq!(
            operation_function(&module, batch, ordinal).unwrap().id,
            kernel.entry
        );
    }
    assert_eq!(
        module
            .kernels
            .iter()
            .map(|kernel| kernel.id.clone())
            .collect::<Vec<_>>(),
        actual_order
    );
    check_preserved_root_order(&module, &module.clone()).unwrap();
    assert!(check_preserved_root_order(&module, &lexical).is_err());

    let mut missing = module.clone();
    missing.kernels.pop();
    assert!(check_graph(&missing, batch).is_err());
    let mut duplicate = module.clone();
    duplicate.kernels[1] = duplicate.kernels[0].clone();
    assert!(check_graph(&duplicate, batch).is_err());
    let mut foreign = module.clone();
    foreign.kernels[0].id = "foreign_root".into();
    assert!(check_graph(&foreign, batch).is_err());
    assert!(check_preserved_root_order(&module, &foreign).is_err());
    let mut extra = module.clone();
    extra.kernels.push(extra.kernels[0].clone());
    assert!(check_graph(&extra, batch).is_err());
}

#[test]
fn fixed_shift_reference_has_exact_zero_high_bit_and_wrap_vectors() {
    for integer in Integer::ALL {
        let mask = integer.mask();
        let high = 1_u128 << (integer.width() - 1);
        for value in vectors(integer) {
            assert_eq!(integer.rust_result(value, false, 0), value);
            assert_eq!(integer.rust_result(value, true, 0), value);
        }
        assert_eq!(integer.rust_result(high, false, 1), 0);
        assert_eq!(integer.rust_result(mask, false, integer.width() - 1), high);
        assert_eq!(
            integer.rust_result(high, true, integer.width() - 1),
            if integer.signed() { mask } else { 1 }
        );
        assert_eq!(
            integer.rust_result(high | 1, true, integer.width() / 2),
            if integer.signed() {
                mask ^ ((1_u128 << (integer.width() - 1 - integer.width() / 2)) - 1)
            } else {
                high >> (integer.width() / 2)
            }
        );
        for retained in [false, true] {
            let batch = Batch { integer, retained };
            assert_eq!(Batch::parse(batch.name()), Some(batch));
            let rows = scenarios(batch).unwrap();
            assert_eq!(rows.len(), 66);
            for root in ROOTS {
                assert_eq!(
                    rows.iter()
                        .filter(|row| row.label.starts_with(root))
                        .count(),
                    11
                );
            }
        }
    }
}

#[test]
fn fixed_shift_reference_rejects_changed_bytes_and_initialization() {
    for integer in Integer::ALL {
        let expected = root_scenarios(
            Batch {
                integer,
                retained: false,
            },
            0,
        )
        .unwrap()
        .remove(1)
        .expected;
        let width = usize::from(integer.scalar().bit_width().unwrap() / 8);
        for byte in [
            0,
            GUARD_ELEMENTS * width,
            expected[0].buffer.bytes().len() - 1,
        ] {
            let mut changed = expected.clone();
            let original = &changed[0].buffer;
            let mut bytes = original.bytes().to_vec();
            bytes[byte] ^= 1;
            changed[0].buffer = BufferArgumentV1::new(
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
        initialized[GUARD_ELEMENTS * width] = false;
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
}
