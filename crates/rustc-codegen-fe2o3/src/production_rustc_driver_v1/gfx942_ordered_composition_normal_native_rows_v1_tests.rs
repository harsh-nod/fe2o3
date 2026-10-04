//! Inert native-observation rows from the same checked canonical owner.
//! This is test-only serialization, not a source/worker authority constructor.
use super::*;
pub(super) fn rows(
    checked: &fe2o3_lower_mir_kernel::ProductionOrderedCompositionCheckedKirOwnerV1,
) -> Value {
    let owner = checked.composition();
    let module = checked.executable().module();
    assert!(owner.helpers().len() <= 2 && owner.definitions().len() <= 8);
    assert!(owner.calls().len() <= 8 && owner.occurrences().len() <= 8);
    let symbol = |function: u32| {
        if function == owner.root_function_ordinal() {
            module.kernels[0].id.as_str()
        } else {
            module.functions[function as usize].id.as_str()
        }
    };
    let site = |coordinate: fe2o3_kernel_ir::OrderedProgramSiteV1| {
        json!({"function_ordinal":coordinate.function_ordinal(),
            "symbol":symbol(coordinate.function_ordinal()),
            "block_ordinal":coordinate.block_ordinal(),"block":coordinate.block().0,
            "operation_ordinal":coordinate.operation_ordinal()})
    };
    let helpers: Vec<_> = owner
        .helpers()
        .iter()
        .map(|helper| {
            json!({
                "key":helper.key().ordinal(),"function_ordinal":helper.function_ordinal(),
                "symbol":symbol(helper.function_ordinal())
            })
        })
        .collect();
    let definitions: Vec<_> = owner.definitions().iter().map(|definition| {
        let coordinate=definition.site();
        let block=&module.functions[coordinate.function_ordinal() as usize]
            .body.as_ref().unwrap().blocks[coordinate.block_ordinal() as usize];
        assert_eq!(block.id, coordinate.block());
        let operation=&block.operations[coordinate.operation_ordinal() as usize];
        let fe2o3_kernel_ir::OperationKind::Gfx942OrderedProgram(program)=&operation.kind else {
            panic!("actual composition definition is not ordered assembly")
        };
        let registers=program.registers();
        json!({"key":definition.key().ordinal(),"site":site(coordinate),
            "scratch":registers.scratch(),"output":registers.output(),
            "inputs":registers.inputs(),"input_values":program.inputs().map(|value|value.0),
            "result":operation.results[0].id.0,
            "descriptors":program.program().instructions().map(|i|i.descriptor()).collect::<Vec<_>>()})
    }).collect();
    let calls: Vec<_> = owner
        .calls()
        .iter()
        .map(|call| {
            json!({
                "key":call.key().ordinal(),"site":site(call.site()),"callee":call.callee().ordinal()
            })
        })
        .collect();
    let occurrences: Vec<_> = owner
        .occurrences()
        .iter()
        .map(|occurrence| {
            json!({
                "key":occurrence.key().ordinal(),"root":occurrence.root_function_ordinal(),
                "incoming_call":occurrence.incoming_call().map(|key|key.ordinal()),
                "definition":occurrence.definition().ordinal()
            })
        })
        .collect();
    let result = json!({"schema":"fe2o3-test-composition-native-rows-v1",
        "canonical_identity":super::super::super::super::lower_hex_v1(checked.executable().identity().digest()),
        "root_function_ordinal":owner.root_function_ordinal(),
        "helpers":helpers,"definitions":definitions,"calls":calls,"occurrences":occurrences,
        "source_custody_exported":false,"native_correspondence_proved":false});
    assert!(serde_json::to_vec(&result).unwrap().len() <= 32768);
    result
}
