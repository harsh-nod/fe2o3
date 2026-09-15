use super::*;
use rustc_middle::mir::{
    BasicBlock, Local, Operand, RETURN_PLACE, START_BLOCK, TerminatorKind, UnwindAction,
};
use rustc_middle::ty::Ty;

pub fn check<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    inputs: [Ty<'tcx>; 6],
    output: Ty<'tcx>,
    checked: Instance<'tcx>,
) {
    let original = tcx.instance_mir(instance.def);
    assert!(body::constructor(
        tcx, instance, original, inputs, output, checked
    ));
    for mutation in 0..10 {
        let mut changed = original.clone();
        let entry = &mut changed.basic_blocks.as_mut()[START_BLOCK];
        let TerminatorKind::Call {
            args,
            target,
            destination,
            unwind,
            ..
        } = &mut entry.terminator_mut().kind
        else {
            unreachable!()
        };
        match mutation {
            0..=4 => {
                let Operand::Copy(place) = args[mutation].node else {
                    unreachable!()
                };
                args[mutation].node = Operand::Move(place);
            }
            5 => args.swap(1, 2),
            6 => args[0].node = Operand::Copy(Local::from_usize(1).into()),
            7 => *target = Some(START_BLOCK),
            8 => *destination = Local::from_usize(6).into(),
            9 => *unwind = UnwindAction::Continue,
            _ => unreachable!(),
        }
        assert!(
            !body::constructor(tcx, instance, &changed, inputs, output, checked),
            "constructor body mutation {mutation}"
        );
    }
    let mut changed = original.clone();
    changed.basic_blocks.as_mut()[BasicBlock::from_usize(1)]
        .terminator_mut()
        .kind = TerminatorKind::Unreachable;
    assert!(!body::constructor(
        tcx, instance, &changed, inputs, output, checked
    ));
    let mut changed = original.clone();
    changed.local_decls[RETURN_PLACE].ty = tcx.types.u64;
    assert!(!body::constructor(
        tcx, instance, &changed, inputs, output, checked
    ));
    assert!(!body::constructor(
        tcx, instance, original, inputs, output, instance
    ));
}
