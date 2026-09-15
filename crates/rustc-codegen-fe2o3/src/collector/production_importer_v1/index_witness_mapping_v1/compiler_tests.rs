//! Real cached AMD source types; these tests do not grant root or graph authority.

use super::*;
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::DefKind;
use rustc_interface::interface::{Compiler, Config};
use rustc_middle::{
    mir::{Operand, TerminatorKind},
    ty::{EarlyBinder, FnSig, TypingEnv},
};
use rustc_session::config::Input;
use rustc_span::FileName;

mod full_import;
mod harness;

const SOURCE: &str = include_str!("fixture.rs");
const MAX_DEFINITIONS: usize = 128;
const MAX_BLOCKS: usize = 128;
const MAX_LOCALS: usize = 256;
const MAX_STATEMENTS: usize = 4096;
const MAX_ARGUMENTS: usize = 16;

#[derive(Clone, Copy)]
enum Case {
    WorkgroupConversion,
    Substitutions,
    SliceMapping,
    FullImport,
    FullRankedLowering,
}

fn local_function<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> Instance<'tcx> {
    let definitions = tcx
        .iter_local_def_id()
        .take(MAX_DEFINITIONS + 1)
        .collect::<Vec<_>>();
    assert!(definitions.len() <= MAX_DEFINITIONS);
    let matching = definitions
        .into_iter()
        .filter(|id| {
            tcx.def_kind(*id) == DefKind::Fn && tcx.item_name(id.to_def_id()).as_str() == name
        })
        .collect::<Vec<_>>();
    assert_eq!(matching.len(), 1, "local fixture function {name}");
    Instance::mono(tcx, matching[0].to_def_id())
}

fn signature<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> FnSig<'tcx> {
    tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    )
}

fn trusted_call<'tcx>(
    tcx: TyCtxt<'tcx>,
    caller: Instance<'tcx>,
    item: TrustedDeviceItem,
) -> Instance<'tcx> {
    let body = tcx.instance_mir(caller.def);
    assert!(body.basic_blocks.len() <= MAX_BLOCKS);
    assert!(body.local_decls.len() <= MAX_LOCALS);
    let mut statements = MAX_STATEMENTS;
    let mut found = None;
    for block in body.basic_blocks.iter() {
        statements = statements
            .checked_sub(block.statements.len())
            .expect("statement budget");
        let TerminatorKind::Call {
            func: Operand::Constant(callee),
            args,
            ..
        } = &block.terminator().kind
        else {
            continue;
        };
        assert!(args.len() <= MAX_ARGUMENTS);
        let TyKind::FnDef(definition, arguments) = callee.const_.ty().kind() else {
            continue;
        };
        if trusted_device_items::classify(tcx, *definition) != Some(item) {
            continue;
        }
        assert!(
            !definition.is_local(),
            "positive must use the cached device definition"
        );
        assert!(arguments.len() <= MAX_ARGUMENTS);
        let arguments = caller
            .try_instantiate_mir_and_normalize_erasing_regions(
                tcx,
                TypingEnv::fully_monomorphized(),
                EarlyBinder::bind(*arguments),
            )
            .expect("actual call type arguments");
        let instance = Instance::try_resolve(
            tcx,
            TypingEnv::fully_monomorphized(),
            *definition,
            arguments,
        )
        .unwrap()
        .expect("resolved actual device call");
        if item == TrustedDeviceItem::ThreadIndexIntoDisjoint {
            assert_eq!(args.len(), 1);
            let sig = signature(tcx, instance);
            assert_eq!(sig.inputs().len(), 1);
            let argument = caller
                .try_instantiate_mir_and_normalize_erasing_regions(
                    tcx,
                    TypingEnv::fully_monomorphized(),
                    EarlyBinder::bind(args[0].node.ty(&body.local_decls, tcx)),
                )
                .expect("actual conversion argument type");
            assert_eq!(argument, sig.inputs()[0]);
            // Optimized MIR may spell a by-value non-Copy argument as Copy.
            // Source ownership is carried by the exact signature, not this tag.
            assert!(
                matches!(args[0].node, Operand::Copy(_) | Operand::Move(_)),
                "conversion receives an existing witness place"
            );
            for witness in [argument, sig.output()] {
                assert!(!tcx.type_is_copy_modulo_regions(
                    TypingEnv::fully_monomorphized(),
                    witness,
                ));
            }
        }
        assert!(found.replace(instance).is_none(), "unique {item:?} call");
    }
    found.unwrap_or_else(|| panic!("missing actual {item:?} call"))
}

fn rowsoft_conversion<'tcx>(tcx: TyCtxt<'tcx>) -> (Ty<'tcx>, Ty<'tcx>) {
    let root = local_function(tcx, "rowsoft_conversion");
    let conversion = trusted_call(tcx, root, TrustedDeviceItem::ThreadIndexIntoDisjoint);
    let sig = signature(tcx, conversion);
    assert_eq!(sig.inputs().len(), 1);
    let input = sig.inputs()[0];
    let output = sig.output();
    let original = tcx.instance_mir(conversion.def);
    assert_eq!(original.source.instance, conversion.def);
    assert!(original.source.promoted.is_none());

    let producer = signature(
        tcx,
        trusted_call(tcx, root, TrustedDeviceItem::WorkgroupMemoryIndex1D),
    );
    assert_eq!(rust_option_payload_v1(tcx, producer.output()), Some(input));
    let store = signature(
        tcx,
        trusted_call(tcx, root, TrustedDeviceItem::WorkgroupMemoryDisjointStore),
    );
    assert_eq!(store.inputs()[2], output);
    let _publish = trusted_call(tcx, root, TrustedDeviceItem::WorkgroupMemoryPublish);
    (input, output)
}

fn check_workgroup_conversion(tcx: TyCtxt<'_>) {
    let (input, output) = rowsoft_conversion(tcx);
    let contract = thread_into_disjoint_contract_v1(tcx, input, output)
        .expect("actual workgroup conversion has matching space and brand");
    let RustIndexMappingV1::WorkgroupMemory(brand) = contract.mapping else {
        panic!("workgroup index must never be represented as a global invocation mapping");
    };
    let (space, input_brand) =
        rust_workgroup_memory_index_v1(tcx, input, TrustedDeviceItem::ThreadIndex).unwrap();
    let (output_space, output_brand) =
        rust_workgroup_memory_index_v1(tcx, output, TrustedDeviceItem::DisjointIndex).unwrap();
    assert_eq!(contract.space, space);
    assert_eq!(space, output_space);
    assert_eq!(contract.brand, brand.ty);
    assert_eq!(brand.ty, input_brand.ty);
    assert_eq!(brand.ty, output_brand.ty);
    assert_eq!(brand.kernel_brand.ty, output_brand.kernel_brand.ty);
    assert_eq!(brand.epoch, output_brand.epoch);
    assert!(
        rust_disjoint_index_space_v1(tcx, space).is_none(),
        "legacy mapping stays global"
    );

    for (name, expected) in [
        ("global_conversion", SemanticDisjointIndexSpaceV1::Index1d),
        (
            "shifted_conversion",
            SemanticDisjointIndexSpaceV1::ShiftedIndex1d { offset: 3 },
        ),
    ] {
        let callee = trusted_call(
            tcx,
            local_function(tcx, name),
            TrustedDeviceItem::ThreadIndexIntoDisjoint,
        );
        let sig = signature(tcx, callee);
        let contract =
            thread_into_disjoint_contract_v1(tcx, sig.inputs()[0], sig.output()).unwrap();
        let RustIndexMappingV1::Invocation(actual) = contract.mapping else {
            panic!("existing invocation mapping changed family");
        };
        assert_eq!(actual, expected);
    }
}

fn check_substitutions(tcx: TyCtxt<'_>) {
    let (input, output) = rowsoft_conversion(tcx);
    assert!(thread_into_disjoint_contract_v1(tcx, input, output).is_some());
    let alternatives = signature(tcx, local_function(tcx, "alternate_outputs"));
    assert_eq!(alternatives.inputs().len(), 6);
    for (replacement, reason) in alternatives.inputs().iter().zip([
        "global space",
        "other kernel brand",
        "other epoch",
        "unbranded workgroup",
        "ThreadIndex in output position",
        "untrusted same-layout wrapper",
    ]) {
        assert_ne!(*replacement, output);
        assert!(
            thread_into_disjoint_contract_v1(tcx, input, *replacement).is_none(),
            "reject {reason}"
        );
    }
    assert!(thread_into_disjoint_contract_v1(tcx, output, output).is_none());
    assert!(thread_into_disjoint_contract_v1(tcx, input, input).is_none());
    assert!(index_witness_contract_v1(tcx, input, TrustedDeviceItem::DisjointSlice).is_none());
}

fn check_slice_mapping(tcx: TyCtxt<'_>) {
    let mut signatures = Vec::new();
    for (name, expected) in [
        (
            "slice_identity",
            Some(SemanticDisjointIndexSpaceV1::Index1d),
        ),
        (
            "slice_shifted",
            Some(SemanticDisjointIndexSpaceV1::ShiftedIndex1d { offset: 3 }),
        ),
        ("slice_unsupported", None),
    ] {
        let call = trusted_call(
            tcx,
            local_function(tcx, name),
            TrustedDeviceItem::DisjointSliceGetMut,
        );
        let sig = signature(tcx, call);
        assert_eq!(sig.inputs().len(), 2);
        let (slice, index) = (sig.inputs()[0], sig.inputs()[1]);
        assert_eq!(
            disjoint_slice_get_mut_contract_v1(tcx, slice, index).map(|(_, space)| space),
            expected
        );
        signatures.push((slice, index));
    }
    let (slice, index) = signatures[2];
    assert!(
        rust_reference_pointee_v1(slice)
            .and_then(|ty| rust_disjoint_slice_v1(tcx, ty))
            .is_none()
    );
    assert!(rust_index_witness_space_v1(tcx, index, TrustedDeviceItem::ThreadIndex).is_none());
    for (slice_index, witness_index) in [(0, 1), (1, 0), (0, 2), (2, 0)] {
        assert!(
            disjoint_slice_get_mut_contract_v1(
                tcx,
                signatures[slice_index].0,
                signatures[witness_index].1
            )
            .is_none()
        );
    }
    let fake = signature(tcx, local_function(tcx, "slice_fake"));
    assert!(disjoint_slice_get_mut_contract_v1(tcx, fake.inputs()[0], fake.inputs()[1]).is_none());
}

struct Probe {
    case: Case,
    completed: bool,
}

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("workgroup_index_mapping_fixture.rs".into()),
            input: match self.case {
                Case::FullImport | Case::FullRankedLowering => {
                    include_str!("compiler_tests/registered_source.rs").into()
                }
                _ => SOURCE.into(),
            },
        };
    }

    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        match self.case {
            Case::WorkgroupConversion => check_workgroup_conversion(tcx),
            Case::Substitutions => check_substitutions(tcx),
            Case::SliceMapping => check_slice_mapping(tcx),
            Case::FullImport => full_import::check(tcx, false),
            Case::FullRankedLowering => full_import::check(tcx, true),
        }
        self.completed = true;
        Compilation::Stop
    }
}

#[test]
#[ignore = "requires complete cached FE2O3_CORE_TRY_{DEVICE_RMETA,HOST_DEPS,AMDGPU_CORE,AMDGPU_BUILTINS}; no Cargo"]
fn actual_rowsoft_workgroup_conversion_preserves_mapping_amdgpu() {
    harness::run(
        Case::WorkgroupConversion,
        "actual_rowsoft_workgroup_conversion_preserves_mapping_amdgpu",
    );
}

#[test]
#[ignore = "requires complete cached FE2O3_CORE_TRY_{DEVICE_RMETA,HOST_DEPS,AMDGPU_CORE,AMDGPU_BUILTINS}; no Cargo"]
fn actual_workgroup_conversion_rejects_space_brand_epoch_substitution_amdgpu() {
    harness::run(
        Case::Substitutions,
        "actual_workgroup_conversion_rejects_space_brand_epoch_substitution_amdgpu",
    );
}

#[test]
#[ignore = "requires complete cached FE2O3_CORE_TRY_{DEVICE_RMETA,HOST_DEPS,AMDGPU_CORE,AMDGPU_BUILTINS}; no Cargo"]
fn actual_disjoint_slice_get_mut_requires_two_classified_mappings_amdgpu() {
    harness::run(
        Case::SliceMapping,
        "actual_disjoint_slice_get_mut_requires_two_classified_mappings_amdgpu",
    );
}

#[test]
#[ignore = "requires complete cached AMD metadata; full registered source import, no Cargo"]
fn actual_registered_workgroup_index_import_v22_amdgpu() {
    harness::run(
        Case::FullImport,
        "actual_registered_workgroup_index_import_v22_amdgpu",
    );
}

#[test]
#[ignore = "requires complete cached AMD metadata and real ranked proof environment; no substituted receipts or Cargo"]
fn actual_registered_workgroup_index_ranked_lowering_amdgpu() {
    harness::run(
        Case::FullRankedLowering,
        "actual_registered_workgroup_index_ranked_lowering_amdgpu",
    );
}
