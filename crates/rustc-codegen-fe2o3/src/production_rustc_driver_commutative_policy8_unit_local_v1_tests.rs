//! Ordinary-source Erased8 qualification, separate from the Direct8 roster.
use super::*;
use fe2o3_kernel_ir::{FunctionRole, OperationKind};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticCallableDeclV1, SemanticFunctionRoleV1, SemanticTerminatorKindV1,
};

#[path = "production_rustc_driver_commutative_policy8_unit_local_protocol_v1_tests.rs"]
mod protocol;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SourceHelperCall {
    root: String,
    source_body: [u8; 32],
    block: u32,
    callee: [u8; 32],
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct NativeHelperCall {
    root: String,
    site: [u32; 3],
    callee: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ErasedHelperCalls {
    original_n: [u8; 32],
    erased_e: [u8; 32],
    source_helper: [u8; 32],
    native_helper: String,
    source_calls: Vec<SourceHelperCall>,
    native_calls: Vec<NativeHelperCall>,
    erased_call_count: usize,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct UnitObservation {
    common: Observation8,
    helper_erasure: ErasedHelperCalls,
}

fn is_internal_helper_call(
    module: &fe2o3_kernel_ir::Module,
    operation: &fe2o3_kernel_ir::Operation,
) -> bool {
    matches!(&operation.kind, OperationKind::Call { callee, .. }
        if module.function(callee).is_some_and(|f| f.role == FunctionRole::InternalHelper))
}

fn observe_erased_view(
    view: live::View<'_>,
    case: Case,
    budget: &mut Budget<'_>,
) -> Result<UnitObservation, String> {
    let live::Owner::Erased(owner) = view.owner else {
        return Err("UnitLocal qualifier requires the actual Erased8 owner".into());
    };
    if case.integer != Integer::U32 {
        return Err("UnitLocal source fixture is concrete u32".into());
    }
    let prefix6 = owner.prefix().prefix();
    let semantic = prefix6.original_source().semantic_ssa().source_semantic();
    let n = prefix6.original_source().executable();
    let e = prefix6.erased();
    if !std::ptr::eq(n, view.owner.original())
        || prefix6.erased_source().deleted_call_count() != ROOTS.len()
        || n.canonical().identity() == e.canonical().identity()
    {
        return Err("actual UnitLocal original N/E deletion custody".into());
    }
    let helpers = n
        .module()
        .functions
        .iter()
        .filter(|f| f.role == FunctionRole::InternalHelper)
        .collect::<Vec<_>>();
    let [helper] = helpers.as_slice() else {
        return Err("actual original N must retain exactly one private unit helper".into());
    };
    if !helper.signature.parameters.is_empty()
        || !helper.signature.results.is_empty()
        || helper.body.is_none()
    {
        return Err("actual original N helper must have a unit ABI and a body".into());
    }
    for module in [
        e.module(),
        prefix6.output().module(),
        owner.prefix().output().module(),
        owner.output().module(),
    ] {
        if module.functions.iter().any(|f| f.role == FunctionRole::InternalHelper)
            || module.functions.iter().filter_map(|f| f.body.as_ref())
                .flat_map(|b| &b.blocks).flat_map(|b| &b.operations)
                .any(|op| matches!(&op.kind, OperationKind::Call { callee, .. } if callee == &helper.id))
        {
            return Err("erased helper/call survived in E, I, J or K".into());
        }
    }
    let mut source_calls = Vec::new();
    let mut native_calls = Vec::new();
    for root in ROOTS {
        let roots = semantic
            .roots()
            .iter()
            .filter(|id| {
                semantic.functions()[id.index() as usize]
                    .kernel_entry()
                    .is_some_and(|entry| entry.export_symbol().as_bytes() == root.as_bytes())
            })
            .copied()
            .collect::<Vec<_>>();
        let [id] = roots.as_slice() else {
            return Err("exact source root selection".into());
        };
        let selected = semantic
            .select_kernel_body_for_root_v1(*id)
            .filter(|s| s.root() == *id)
            .ok_or("exact selected source body")?;
        let body = &semantic.functions()[selected.body().index() as usize];
        let before = source_calls.len();
        for (bi, block) in body.blocks().iter().enumerate() {
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                continue;
            };
            let SemanticCallableDeclV1::Defined { function } =
                &semantic.callables()[call.callee().index() as usize]
            else {
                continue;
            };
            let callee = &semantic.functions()[function.index() as usize];
            if callee.role() != SemanticFunctionRoleV1::InternalHelper
                || !call.arguments().is_empty()
            {
                continue;
            }
            if callee.blocks().is_empty() {
                return Err("actual source helper body missing".into());
            }
            source_calls.push(SourceHelperCall {
                root: root.into(),
                source_body: *body.identity().as_bytes(),
                block: u32::try_from(bi).map_err(|e| e.to_string())?,
                callee: *callee.identity().as_bytes(),
            });
        }
        if source_calls.len() != before + 1 {
            return Err("one actual private unit call per source root".into());
        }
        let (_, function, fi) = graph::kernel(n.module(), case, root)?;
        let before = native_calls.len();
        for (bi, block) in function
            .body
            .as_ref()
            .ok_or("actual N root body")?
            .blocks
            .iter()
            .enumerate()
        {
            for (oi, operation) in block.operations.iter().enumerate() {
                if let OperationKind::Call { callee, arguments } = &operation.kind
                    && is_internal_helper_call(n.module(), operation)
                {
                    if callee != &helper.id
                        || !arguments.is_empty()
                        || !operation.results.is_empty()
                    {
                        return Err("actual N root has a foreign/nonunit helper call".into());
                    }
                    native_calls.push(NativeHelperCall {
                        root: root.into(),
                        site: [fi, bi, oi].map(|x| u32::try_from(x).unwrap()),
                        callee: callee.as_str().into(),
                    });
                }
            }
        }
        if native_calls.len() != before + 1 {
            return Err("one actual N unit call per root".into());
        }
    }
    let total_calls = n
        .module()
        .functions
        .iter()
        .filter_map(|f| f.body.as_ref())
        .flat_map(|b| &b.blocks)
        .flat_map(|b| &b.operations)
        .filter(|op| matches!(&op.kind, OperationKind::Call { callee, .. } if callee == &helper.id))
        .count();
    if total_calls != ROOTS.len() {
        return Err("complete original N helper call roster".into());
    }
    let helper_erasure = ErasedHelperCalls {
        original_n: *n.canonical().identity().digest(),
        erased_e: *e.canonical().identity().digest(),
        source_helper: source_calls[0].callee,
        native_helper: helper.id.as_str().into(),
        source_calls,
        native_calls,
        erased_call_count: prefix6.erased_source().deleted_call_count(),
    };
    // Shared replay binds the retained source/N/E and actual J/K owners. The
    // copied call census is diagnostic, not a replacement source certificate.
    let common = observe_bound_view(view, case, budget)?;
    let report = UnitObservation {
        common,
        helper_erasure,
    };
    validate_unit(&report, case)?;
    Ok(report)
}

fn validate_erasure(report: &ErasedHelperCalls, original: [u8; 32]) -> Result<(), String> {
    if report.original_n != original
        || original == [0; 32]
        || report.erased_e == [0; 32]
        || report.erased_e == original
        || report.source_helper == [0; 32]
        || report.native_helper.is_empty()
        || report.erased_call_count != 3
        || report.source_calls.len() != 3
        || report.native_calls.len() != 3
    {
        return Err("UnitLocal N/E or exact three erased call sites".into());
    }
    let mut source_sites = std::collections::BTreeSet::new();
    let mut native_sites = std::collections::BTreeSet::new();
    for ((source, native), root) in report
        .source_calls
        .iter()
        .zip(&report.native_calls)
        .zip(ROOTS)
    {
        if source.root != root
            || native.root != root
            || source.source_body == [0; 32]
            || source.callee != report.source_helper
            || native.callee != report.native_helper
            || !source_sites.insert((source.source_body, source.block))
            || !native_sites.insert(native.site)
        {
            return Err("exact source/native helper root/callee/occurrence join".into());
        }
    }
    Ok(())
}
fn validate_unit(report: &UnitObservation, case: Case) -> Result<(), String> {
    validate8(&report.common, case)?;
    validate_erasure(&report.helper_erasure, report.common.original)?;
    if case.integer != Integer::U32 {
        return Err("UnitLocal requires concrete u32 case".into());
    }
    for call in &report.helper_erasure.source_calls {
        let root = report
            .common
            .roots
            .iter()
            .find(|r| r.name == call.root)
            .ok_or("source root missing")?;
        if root.source_body != call.source_body {
            return Err("helper call is not in actual observed root body".into());
        }
    }
    Ok(())
}
fn erasure_fixture() -> ErasedHelperCalls {
    ErasedHelperCalls {
        original_n: [1; 32],
        erased_e: [2; 32],
        source_helper: [3; 32],
        native_helper: "helper".into(),
        source_calls: ROOTS
            .iter()
            .enumerate()
            .map(|(i, root)| SourceHelperCall {
                root: (*root).into(),
                source_body: [10 + i as u8; 32],
                block: 0,
                callee: [3; 32],
            })
            .collect(),
        native_calls: ROOTS
            .iter()
            .enumerate()
            .map(|(i, root)| NativeHelperCall {
                root: (*root).into(),
                site: [i as u32, 0, 0],
                callee: "helper".into(),
            })
            .collect(),
        erased_call_count: 3,
    }
}
#[test]
fn erased_call_roster_rejects_wrong_axes_counts_and_coordinates() {
    let good = erasure_fixture();
    validate_erasure(&good, [1; 32]).unwrap();
    for mutate in [
        |r: &mut ErasedHelperCalls| r.original_n = [4; 32],
        |r: &mut ErasedHelperCalls| r.erased_e = r.original_n,
        |r: &mut ErasedHelperCalls| r.erased_call_count = 2,
        |r: &mut ErasedHelperCalls| {
            r.source_calls.pop();
        },
        |r: &mut ErasedHelperCalls| r.native_calls.push(r.native_calls[0].clone()),
        |r: &mut ErasedHelperCalls| r.source_calls.swap(0, 1),
        |r: &mut ErasedHelperCalls| r.native_calls.swap(0, 1),
        |r: &mut ErasedHelperCalls| r.source_calls[1].source_body = r.source_calls[0].source_body,
        |r: &mut ErasedHelperCalls| r.native_calls[1].site = r.native_calls[0].site,
        |r: &mut ErasedHelperCalls| r.source_calls[0].callee = [5; 32],
        |r: &mut ErasedHelperCalls| r.native_calls[0].callee = "foreign".into(),
    ] {
        let mut bad = good.clone();
        mutate(&mut bad);
        assert!(validate_erasure(&bad, [1; 32]).is_err());
    }
}

#[test]
fn actual_callee_role_distinguishes_helper_calls_from_retained_diagnostics() {
    use fe2o3_kernel_ir::{
        AmdGpuDiagnosticOperation as Diagnostic, BasicBlock, BlockId, Function, Module, Operation,
        Signature, Terminator,
    };
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("test-helper-census-only");
    module.functions.push(Function::internal_helper(
        "private-unit",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    module.functions.push(Diagnostic::Trap.declaration());
    let diagnostic = Diagnostic::Trap.operation(None);
    assert!(matches!(diagnostic.kind, OperationKind::Call { .. }));
    assert!(!is_internal_helper_call(&module, &diagnostic));
    let helper = Operation::new(
        vec![],
        OperationKind::Call {
            callee: "private-unit".into(),
            arguments: vec![],
        },
    );
    assert!(is_internal_helper_call(&module, &helper));
    let missing = Operation::new(
        vec![],
        OperationKind::Call {
            callee: "missing".into(),
            arguments: vec![],
        },
    );
    assert!(!is_internal_helper_call(&module, &missing));
    assert_eq!(
        [&diagnostic, &helper, &diagnostic]
            .into_iter()
            .filter(|operation| is_internal_helper_call(&module, operation))
            .count(),
        1
    );
    // Classification is not graph admission. Shared Stage8 replay and the full
    // existing graph/trap oracle remain mandatory for every observed source.
}
