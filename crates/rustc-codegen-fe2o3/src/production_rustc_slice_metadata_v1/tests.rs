use super::*;
use crate::test_temp_dir::TestTempDir;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::Compiler;
use rustc_middle::mir::{BasicBlockData, Statement, Terminator, TerminatorKind};

mod construction;

const SOURCE: &str = r#"
pub struct Slices<'a>(&'a [u32], &'a [u32]);
pub fn indexed(input: Slices<'_>, i: usize) -> u32 {
    if i < input.0.len() && i < input.1.len() {
        input.0[i] ^ input.1[i] ^ input.0[i]
    } else { 0 }
}
pub fn metadata(a: &[u32]) -> usize { a.len() }
pub fn plain(a: &[u32], i: usize) -> u32 { a[i] }
pub fn constructor(input: Slices<'_>, i: usize) -> u32 { input.0[i] }
"#;

fn derive<'a, 'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &'a Body<'tcx>,
) -> SliceMetadataPlanV1<'a, 'tcx> {
    SliceMetadataPlanV1::derive(tcx, instance, body, |_| Ok::<_, ()>(()))
        .unwrap_or_else(|error| panic!("metadata normalization: {error:?}"))
}

fn assignment_mut<'a, 'tcx>(
    body: &'a mut Body<'tcx>,
    location: Location,
) -> &'a mut (Place<'tcx>, Rvalue<'tcx>) {
    let StatementKind::Assign(assignment) =
        &mut body.basic_blocks.as_mut()[location.block].statements[location.statement_index].kind
    else {
        panic!("assignment required")
    };
    assignment
}

#[derive(Default)]
struct MetadataCallbacks {
    completed: bool,
}

impl Callbacks for MetadataCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let find = |name: &str| {
            tcx.hir_body_owners()
                .find(|id| tcx.item_name(id.to_def_id()).as_str() == name)
                .expect("fixture function")
                .to_def_id()
        };
        let instance = Instance::mono(tcx, find("indexed"));
        let body = tcx.instance_mir(instance.def);
        let original = format!("{body:?}");
        let plan = derive(tcx, instance, body);
        assert_eq!(
            plan.pairs.len(),
            3,
            "actual optimized MIR retains all indexed occurrences"
        );
        let mut slices = std::collections::BTreeSet::new();
        for pair in &plan.pairs {
            slices.insert(pair.slice);
            assert_eq!(
                plan.at(pair.producer),
                Some(SliceMetadataRewriteV1::ElideTemporary)
            );
            assert_eq!(
                plan.at(Location {
                    statement_index: pair.producer.statement_index + 1,
                    ..pair.producer
                }),
                Some(SliceMetadataRewriteV1::ReadLength(pair.slice))
            );
            assert!(matches!(
                body.local_decls[pair.temporary].ty.kind(),
                TyKind::RawPtr(..)
            ));
        }
        assert!(slices.len() >= 2, "distinct source carriers are retained");
        assert_eq!(
            format!("{body:?}"),
            original,
            "recognizer must not rewrite rustc MIR or debug declarations"
        );
        let first = plan.pairs[0];
        let large = Rvalue::Aggregate(
            Box::new(rustc_middle::mir::AggregateKind::Tuple),
            std::iter::repeat_n(Operand::Copy(Place::from(first.temporary)), 1024).collect(),
        );
        let mut visits = Vec::new();
        let mut counters = vec![0; body.local_decls.len()];
        let mut charge = |amount| {
            visits.push(amount);
            Err(amount)
        };
        {
            let mut visitor = TemporaryUsesV1 {
                uses: &mut counters,
                charge: &mut charge,
                error: None,
            };
            visitor.visit_rvalue(&large, first.producer);
            assert!(matches!(
                visitor.error,
                Some(SliceMetadataErrorV1::Resource(1025))
            ));
        }
        assert_eq!(
            visits,
            [1025],
            "collection traversal is denied before visiting any child"
        );
        assert!(counters.iter().all(|count| *count == 0));
        let consumer = Location {
            statement_index: first.producer.statement_index + 1,
            ..first.producer
        };
        let other_slice = *slices.iter().find(|local| **local != first.slice).unwrap();

        let mut changed = body.clone();
        changed.basic_blocks.as_mut()[consumer.block].statements[consumer.statement_index]
            .debuginfos
            .push(StmtDebugInfo::InvalidAssign(first.temporary));
        changed.basic_blocks.as_mut()[consumer.block].statements[consumer.statement_index]
            .debuginfos
            .push(StmtDebugInfo::AssignRef(
                first.temporary,
                Place::from(first.temporary),
            ));
        assert_eq!(
            derive(tcx, instance, &changed).pairs,
            plan.pairs,
            "debug observations are not executable uses"
        );

        let reject = |label: &str, changed: &Body<'tcx>| {
            assert!(
                matches!(
                    SliceMetadataPlanV1::derive(tcx, instance, changed, |_| Ok::<_, ()>(())),
                    Err(SliceMetadataErrorV1::Unsupported(_))
                ),
                "{label} must reject"
            );
        };
        for duplicate_producer in [false, true] {
            let mut changed = body.clone();
            let index = if duplicate_producer {
                first.producer.statement_index
            } else {
                consumer.statement_index
            };
            let extra = changed.basic_blocks[first.producer.block].statements[index].clone();
            changed.basic_blocks.as_mut()[first.producer.block]
                .statements
                .push(extra);
            reject("extra definition or value use", &changed);
        }
        let mut changed = body.clone();
        let extra =
            changed.basic_blocks[consumer.block].statements[consumer.statement_index].clone();
        let unreachable_block = changed.basic_blocks.as_mut().push(BasicBlockData::new(
            Some(Terminator {
                source_info: extra.source_info,
                kind: TerminatorKind::Unreachable,
            }),
            false,
        ));
        changed.basic_blocks.as_mut()[unreachable_block]
            .statements
            .push(extra);
        reject("use in unreachable block", &changed);

        let mut changed = body.clone();
        assignment_mut(&mut changed, consumer).1 =
            Rvalue::Use(Operand::Copy(Place::from(first.temporary)));
        reject("non-metadata consumer", &changed);

        let mut changed = body.clone();
        assignment_mut(&mut changed, consumer).1 =
            Rvalue::UnaryOp(UnOp::PtrMetadata, Operand::Copy(Place::from(other_slice)));
        reject("wrong consumer local", &changed);

        let mut changed = body.clone();
        let source_info =
            changed.basic_blocks[consumer.block].statements[consumer.statement_index].source_info;
        changed.basic_blocks.as_mut()[consumer.block]
            .statements
            .insert(
                consumer.statement_index,
                Statement::new(source_info, StatementKind::Nop),
            );
        reject("non-adjacent pair", &changed);

        let mut changed = body.clone();
        let moved = changed.basic_blocks.as_mut()[consumer.block]
            .statements
            .remove(consumer.statement_index);
        let block = changed.basic_blocks.as_mut().push(BasicBlockData::new(
            Some(Terminator {
                source_info,
                kind: TerminatorKind::Unreachable,
            }),
            false,
        ));
        changed.basic_blocks.as_mut()[block].statements.push(moved);
        reject("cross-block pair", &changed);

        let mut changed = body.clone();
        assignment_mut(&mut changed, first.producer).0 = Place::from(Local::from_usize(1));
        reject("argument temporary", &changed);

        let mut changed = body.clone();
        assignment_mut(&mut changed, first.producer).1 =
            Rvalue::RawPtr(RawPtrKind::FakeForPtrMetadata, Place::from(first.slice));
        reject("missing dereference", &changed);

        let mut changed = body.clone();
        let pointee = match *body.local_decls[first.temporary].ty.kind() {
            TyKind::RawPtr(ty, _) => ty,
            _ => unreachable!(),
        };
        changed.local_decls[first.slice].ty = Ty::new_ptr(tcx, pointee, Mutability::Not);
        reject("raw-pointer origin", &changed);
        changed.local_decls[first.slice].ty =
            Ty::new_ref(tcx, tcx.lifetimes.re_erased, pointee, Mutability::Mut);
        reject("mutable origin", &changed);

        let mut changed = body.clone();
        changed.local_decls[first.temporary].ty = Ty::new_ptr(tcx, pointee, Mutability::Mut);
        reject("mutable pointer result", &changed);
        changed.local_decls[first.temporary].ty = tcx.types.usize;
        reject("nonpointer temporary", &changed);

        let mut changed = body.clone();
        let other_pointee = Ty::new_slice(tcx, tcx.types.u64);
        changed.local_decls[first.temporary].ty = Ty::new_ptr(tcx, other_pointee, Mutability::Not);
        reject("mismatched pointer pointee", &changed);
        changed.local_decls[first.slice].ty =
            Ty::new_ref(tcx, tcx.lifetimes.re_erased, other_pointee, Mutability::Not);
        reject("matching but unsupported element type", &changed);

        let mut changed = body.clone();
        let length = assignment_mut(&mut changed, consumer).0.as_local().unwrap();
        changed.local_decls[length].ty = tcx.types.u32;
        reject("non-usize metadata", &changed);

        let mut changed = body.clone();
        assignment_mut(&mut changed, consumer).1 = Rvalue::UnaryOp(
            UnOp::PtrMetadata,
            Operand::Move(Place::from(first.temporary)),
        );
        assert_eq!(derive(tcx, instance, &changed).pairs, plan.pairs);

        let mut changed = body.clone();
        let Rvalue::RawPtr(_, ref mut place) = assignment_mut(&mut changed, first.producer).1
        else {
            unreachable!()
        };
        place.local = other_slice;
        let changed_plan = derive(tcx, instance, &changed);
        assert_ne!(
            changed_plan.pairs, plan.pairs,
            "same-typed source substitution cannot reuse a stale relation"
        );
        assert_eq!(changed_plan.pairs[0].slice, other_slice);

        let mut changed = body.clone();
        assignment_mut(&mut changed, first.producer).1 = Rvalue::RawPtr(
            RawPtrKind::Const,
            Place {
                local: first.slice,
                projection: tcx.mk_place_elems(&[ProjectionElem::Deref]),
            },
        );
        let ordinary = derive(tcx, instance, &changed);
        assert_eq!(
            ordinary.at(first.producer),
            None,
            "real raw pointer must not receive the exemption"
        );

        let mut work = 0;
        SliceMetadataPlanV1::derive(tcx, instance, body, |amount| {
            work += amount;
            Ok::<_, ()>(())
        })
        .unwrap();
        for limit in [0, work - 1, work] {
            let mut spent = 0;
            let result = SliceMetadataPlanV1::derive(tcx, instance, body, |amount| {
                spent += amount;
                if spent > limit { Err(()) } else { Ok(()) }
            });
            assert_eq!(
                result.is_ok(),
                limit == work,
                "exact cumulative work boundary"
            );
            if limit < work {
                assert!(matches!(result, Err(SliceMetadataErrorV1::Resource(()))));
            }
        }
        assert_eq!(
            derive(tcx, instance, body).pairs,
            plan.pairs,
            "failed requests leave no reusable state"
        );
        let metadata = Instance::mono(tcx, find("metadata"));
        assert!(
            derive(tcx, metadata, tcx.instance_mir(metadata.def))
                .pairs
                .is_empty()
        );
        let plain = Instance::mono(tcx, find("plain"));
        assert!(
            derive(tcx, plain, tcx.instance_mir(plain.def))
                .pairs
                .is_empty()
        );
        construction::check(tcx, Instance::mono(tcx, find("constructor")));
        self.completed = true;
        Compilation::Stop
    }
}

#[test]
fn slice_metadata_normalization_checks_actual_mir_uses_types_and_work_limits() {
    let directory = TestTempDir::create("fe2o3-slice-metadata");
    let source = directory.path().join("fixture.rs");
    std::fs::write(&source, SOURCE).unwrap();
    let output = std::process::Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let sysroot = String::from_utf8(output.stdout).unwrap();
    let args = vec![
        "rustc".into(),
        "--crate-name".into(),
        "fe2o3_metadata_fixture".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--emit=metadata".into(),
        "-Zmir-opt-level=0".into(),
        "-Cpanic=abort".into(),
        "--sysroot".into(),
        sysroot.trim().into(),
        "-o".into(),
        directory.path().join("fixture.rmeta").display().to_string(),
        source.display().to_string(),
    ];
    let mut callbacks = MetadataCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    assert!(callbacks.completed);
}
