//! Host source/MIR checks only. A local structural match never authenticates the provider.

use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAggregateLayoutV1, SemanticAggregateTypeV1, SemanticLayoutIdentityV1,
    SemanticTypeLayoutV1,
};
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::DefKind;
use rustc_interface::interface::{Compiler, Config};
use rustc_session::config::Input;
use rustc_span::FileName;

mod real_source;

const SOURCE: &str = r#"
#![no_std]
pub struct Epoch<B, E>(core::marker::PhantomData<(B, E)>);
pub struct Workgroup<B, E> {
    size: u64,
    rank: u64,
    epoch: Epoch<B, E>,
    other: Epoch<B, E>,
}
pub struct DifferentEpochWorkgroup {
    size: u64,
    rank: u64,
    epoch: Epoch<(), ()>,
    other: Epoch<(), u32>,
}
pub fn exact(w: &Workgroup<(), ()>) -> &Epoch<(), ()> { &w.epoch }
pub fn wrong_field(w: &Workgroup<(), ()>) -> &Epoch<(), ()> { &w.other }
pub fn mutable_receiver(w: &mut Workgroup<(), ()>) -> &Epoch<(), ()> { &w.epoch }
pub fn raw_result(w: &Workgroup<(), ()>) -> *const Epoch<(), ()> { &raw const w.epoch }
pub fn different_epoch(w: &DifferentEpochWorkgroup) -> &Epoch<(), u32> { &w.other }
pub fn different_owner<'a>(_: &'a Workgroup<(), ()>, w: &'a Workgroup<(), ()>) -> &'a Epoch<(), ()> { &w.epoch }
pub fn extra_effect(w: &Workgroup<(), ()>) -> &Epoch<(), ()> {
    core::hint::black_box(w);
    &w.epoch
}
"#;

fn instance<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> Instance<'tcx> {
    let definition = tcx
        .iter_local_def_id()
        .find(|definition| {
            tcx.def_kind(*definition) == DefKind::Fn
                && tcx.item_name(definition.to_def_id()).as_str() == name
        })
        .expect("fixture function");
    Instance::mono(tcx, definition.to_def_id())
}

fn signature<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> FnSig<'tcx> {
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    tcx.try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), signature)
        .unwrap()
}

struct Probe {
    substitutions: bool,
    ran: bool,
}

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("workgroup_epoch_source.rs".into()),
            input: SOURCE.into(),
        };
    }

    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        if self.substitutions {
            for name in [
                "wrong_field",
                "mutable_receiver",
                "raw_result",
                "different_epoch",
                "different_owner",
                "extra_effect",
            ] {
                let instance = instance(tcx, name);
                assert!(
                    !reviewed_epoch_body_v1(
                        tcx,
                        instance,
                        tcx.instance_mir(instance.def),
                        signature(tcx, instance)
                    ),
                    "accepted {name}"
                );
                assert!(
                    !epoch_provider_v1(tcx, instance),
                    "local {name} cannot acquire reviewed authority"
                );
            }
        } else {
            let exact = instance(tcx, "exact");
            let body = tcx.instance_mir(exact.def);
            let signature = signature(tcx, exact);
            assert!(
                reviewed_epoch_body_v1(tcx, exact, body, signature),
                "expected exactly _0 = &((*_1).2)"
            );
            assert!(
                !epoch_provider_v1(tcx, exact),
                "matching MIR and local type names are not provider identity"
            );
            let StatementKind::Assign(assignment) =
                &body.basic_blocks[START_BLOCK].statements[0].kind
            else {
                panic!("exact shared projection");
            };
            let Rvalue::Ref(_, BorrowKind::Shared, origin) = &assignment.1 else {
                panic!("shared borrow must remain in source MIR");
            };
            assert_eq!(origin.local, Local::from_usize(1));
            assert!(
                matches!(origin.projection.as_ref(), [ProjectionElem::Deref, ProjectionElem::Field(field, _)] if field.as_usize() == EPOCH_FIELD)
            );
            let mut changed_owner = body.clone();
            let StatementKind::Assign(assignment) =
                &mut changed_owner.basic_blocks.as_mut()[START_BLOCK].statements[0].kind
            else {
                unreachable!();
            };
            let Rvalue::Ref(_, _, origin) = &mut assignment.1 else {
                unreachable!();
            };
            origin.local = RETURN_PLACE;
            assert!(!reviewed_epoch_body_v1(
                tcx,
                exact,
                &changed_owner,
                signature
            ));
        }
        self.ran = true;
        Compilation::Stop
    }
}

fn run(substitutions: bool) {
    let sysroot = crate::process_execution::capture_output(
        std::process::Command::new("rustc").args(["--print", "sysroot"]),
    )
    .unwrap();
    assert!(sysroot.status.success());
    let args = vec![
        "rustc".into(),
        "--crate-name=workgroup_epoch_source".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "-Zno-codegen".into(),
        "-Zinline-mir=no".into(),
        "-Copt-level=0".into(),
        "-Cpanic=abort".into(),
        "-".into(),
    ];
    let mut probe = Probe {
        substitutions,
        ran: false,
    };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(probe.ran);
}

#[test]
fn exact_epoch_projection_retains_owner_but_cannot_authenticate_a_local_provider() {
    run(false);
}

#[test]
fn epoch_projection_rejects_field_epoch_owner_mutability_raw_pointer_and_effect_substitutions() {
    run(true);
}

fn subgroup_layout_fixture() -> Vec<SemanticTypeDeclV1> {
    let aggregate = |tag: u8, bytes, alignment, fields: Vec<u32>, offsets| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(bytes),
                alignment,
                SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(
                SemanticAggregateTypeV1::new(
                    fields
                        .into_iter()
                        .map(SemanticTypeIdV1::from_index)
                        .collect(),
                )
                .unwrap(),
            ),
        )
    };
    vec![
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([1; 32]),
            SemanticLayoutIdentityV1::from_sha256([1; 32]),
            SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }),
        ),
        aggregate(2, 0, 1, vec![], vec![]),
        aggregate(3, 4, 4, vec![0, 1, 1, 1], vec![0, 4, 4, 4]),
        aggregate(4, 4, 4, vec![2, 1, 1], vec![0, 4, 4]),
    ]
}

#[test]
fn physical_subgroup_layout_retains_u32_lane_and_is_not_a_zst() {
    let types = subgroup_layout_fixture();
    let subgroup = SemanticTypeIdV1::from_index(3);
    assert!(exact_subgroup_source_layout_v1(&types, subgroup));
    assert!(!semantic_exact_inhabited_aggregate_zst_v1(&types, subgroup));
    assert!(!exact_subgroup_source_layout_v1(
        &types,
        SemanticTypeIdV1::from_index(1)
    ));
}

#[test]
fn physical_subgroup_layout_rejects_missing_or_substituted_lane_storage() {
    let subgroup = SemanticTypeIdV1::from_index(3);
    for replacement in [1, 2, 3] {
        let mut types = subgroup_layout_fixture();
        types[0] = types[replacement].clone();
        assert!(!exact_subgroup_source_layout_v1(&types, subgroup));
    }
    let mut types = subgroup_layout_fixture();
    types[2] = types[1].clone();
    assert!(!exact_subgroup_source_layout_v1(&types, subgroup));
}

#[test]
fn physical_subgroup_layout_rejects_shifted_lane_or_marker_offsets() {
    let subgroup = SemanticTypeIdV1::from_index(3);
    for (index, offsets) in [
        (2, vec![1, 4, 4, 4]),
        (2, vec![0, 0, 4, 4]),
        (3, vec![0, 0, 4]),
    ] {
        let mut types = subgroup_layout_fixture();
        let original = &types[index];
        types[index] = SemanticTypeDeclV1::new(
            original.identity(),
            SemanticLayoutIdentityV1::from_sha256([80; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(4),
                4,
                SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
            )
            .unwrap(),
            original.shape().clone(),
        );
        assert!(!exact_subgroup_source_layout_v1(&types, subgroup));
    }
}
