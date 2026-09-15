use super::*;
use rustc_middle::mir::{UnwindAction, UnwindTerminateReason};

pub(super) fn check(tcx: TyCtxt<'_>) {
    let instance = helper(tcx, "owned");
    let c = contract(tcx, instance).unwrap();
    let model = local_body(tcx, "cleanup_model");
    assert!(!reviewed_body(tcx, instance, model, &c));
    // The installed core was built without output-drop cleanup. Exercise the
    // compiler's unwind shape independently; this is not core provenance.
    let mut source = model.clone();
    source.source.instance = instance.def;
    assert!(reviewed_body(tcx, instance, &source, &c), "{source:#?}");

    let (cleanup, target) = source
        .basic_blocks
        .iter_enumerated()
        .find_map(|(id, b)| match b.terminator().kind {
            TerminatorKind::Drop {
                target,
                unwind: UnwindAction::Terminate(UnwindTerminateReason::InCleanup),
                ..
            } if b.is_cleanup => Some((id, target)),
            _ => None,
        })
        .expect("rustc output-drop cleanup with double-panic edge");
    for kind in [
        TerminatorKind::Goto { target },
        TerminatorKind::Return,
        TerminatorKind::UnwindResume,
        TerminatorKind::Unreachable,
    ] {
        let mut b = source.clone();
        b.basic_blocks_mut()[cleanup].terminator_mut().kind = kind;
        assert!(
            !reviewed_body(tcx, instance, &b, &c),
            "cleanup cannot lose owned output"
        );
    }
    for unwind in [
        UnwindAction::Continue,
        UnwindAction::Unreachable,
        UnwindAction::Cleanup(cleanup),
        UnwindAction::Terminate(UnwindTerminateReason::Abi),
    ] {
        let mut b = source.clone();
        let TerminatorKind::Drop { unwind: edge, .. } =
            &mut b.basic_blocks_mut()[cleanup].terminator_mut().kind
        else {
            unreachable!()
        };
        *edge = unwind;
        assert!(
            !reviewed_body(tcx, instance, &b, &c),
            "invalid cleanup unwind: {unwind:?}"
        );
    }
    for (block, data) in source.basic_blocks.iter_enumerated() {
        if data.is_cleanup {
            continue;
        }
        let mut b = source.clone();
        let unwind = match &mut b.basic_blocks_mut()[block].terminator_mut().kind {
            TerminatorKind::Drop { unwind, .. } | TerminatorKind::Call { unwind, .. } => unwind,
            _ => continue,
        };
        *unwind = UnwindAction::Terminate(UnwindTerminateReason::InCleanup);
        assert!(
            !reviewed_body(tcx, instance, &b, &c),
            "double-panic edge outside cleanup"
        );
    }
}
