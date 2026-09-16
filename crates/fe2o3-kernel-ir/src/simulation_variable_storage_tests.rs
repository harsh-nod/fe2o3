use super::*;
use crate::*;

#[derive(Clone, Copy, Debug)]
enum Change {
    None,
    Slot,
    Value,
    Generation,
    Representation,
    Function,
    Alias,
    Missing,
    External,
    WrapperRoot,
    UnavailableWrapperRoot,
}

#[test]
fn helper_variable_join_checks_function_slot_value_and_representation() {
    for change in [
        Change::None,
        Change::Slot,
        Change::Value,
        Change::Generation,
        Change::Representation,
        Change::Function,
        Change::Alias,
        Change::Missing,
        Change::External,
        Change::WrapperRoot,
        Change::UnavailableWrapperRoot,
    ] {
        let mut module = Module::new("variables");
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: vec![] });
        module.functions.push(Function::kernel_entry(
            "root",
            Signature::new(vec![], vec![]),
            vec![],
            vec![block.clone()],
        ));
        let slice = Type::slice(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        );
        for name in ["helper", "other"] {
            module.functions.push(Function::internal_helper(
                name,
                Signature::new(vec![slice.clone(); 2], vec![]),
                vec![ValueId(7), ValueId(9)],
                vec![block.clone()],
            ));
        }
        if matches!(change, Change::External) {
            module.functions[1].role = FunctionRole::ExternalImport;
        }
        let span = DebugSourceMapSpanV1::new([1; 32], 0, 10, 1, 1).unwrap();
        let scopes = [1, 2]
            .map(|ordinal| {
                DebugSourceScopeV2::new([ordinal as u8 + 1; 32], ordinal, None, 0, span).unwrap()
            })
            .to_vec();
        let mut source_variables = Vec::new();
        let mut variables = Vec::new();
        for index in 0..2 {
            let id = [index + 4; 32];
            let ordinal = if index == 1 && matches!(change, Change::Function) {
                2
            } else {
                1
            };
            let value = if index == 0 { 7 } else { 9 };
            let source_variable = DebugSourceVariableV2::new(
                id,
                format!("arg{index}"),
                ordinal,
                [ordinal as u8 + 1; 32],
                DebugSourceVariableFallbackV2::NotInScope,
                vec![],
            )
            .unwrap();
            source_variables.push(if matches!(change, Change::UnavailableWrapperRoot) {
                source_variable
            } else {
                source_variable
                    .with_function_binding(
                        DebugSourceVariableFunctionBindingV2::new(
                            if index == 1 && matches!(change, Change::Generation) {
                                2
                            } else {
                                1
                            },
                            value,
                        )
                        .unwrap(),
                    )
                    .unwrap()
            });
            variables.push(SemanticVariableStorageV1::new(
                id,
                if index == 1 && matches!(change, Change::Alias) {
                    0
                } else if matches!(change, Change::WrapperRoot | Change::UnavailableWrapperRoot) {
                    10
                } else {
                    5
                },
                Some(index.into()),
                Some(0),
                if matches!(change, Change::UnavailableWrapperRoot) {
                    Storage::Unavailable {
                        reason: SemanticStorageUnavailableReasonV1::NoRetainedKirStorage,
                    }
                } else {
                    Storage::ExactKirParameter {
                        kir_parameter_ordinal: if index == 1 && matches!(change, Change::Slot) {
                            0
                        } else {
                            index.into()
                        },
                        kir_value_ordinal: if index == 1 && matches!(change, Change::Value) {
                            7
                        } else {
                            value as u32
                        },
                        representation: if index == 1 && matches!(change, Change::Representation) {
                            Representation::Scalar
                        } else {
                            Representation::RegionSlice
                        },
                    }
                },
            ));
        }
        let source = DebugSourceMapDocumentV2::new(
            DebugSourceMapBindingV1::new([7; 32], [8; 32], 123).unwrap(),
            vec![DebugSourceMapFileV1::new([1; 32], 16, "/src/variables.rs".into()).unwrap()],
            vec![],
            vec![span],
            scopes,
            source_variables,
        )
        .unwrap();
        if matches!(change, Change::Missing) {
            variables.pop();
        }
        let result = validate_variable_storage(
            &[SemanticKernelStorageV1::new(10, 0, 0, vec![])],
            &variables,
            &source,
            &module,
        );
        assert_eq!(result.is_ok(), matches!(change, Change::None), "{change:?}");
    }
}
