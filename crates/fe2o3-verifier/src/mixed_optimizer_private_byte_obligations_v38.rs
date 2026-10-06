//! Physical facts select no optimization or unchecked memory operation. Every
//! unknown is either covered by the exact emitted operation's validity guard or
//! refused here. Dynamic coverage is not a universal initialized-read proof.
use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirPrivateByteObligationV38 as Obligation,
    CanonicalKirPrivateByteOperationKindV38 as Kind, CanonicalKirPrivateByteOperationV38 as Fact,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Guard {
    None,
    Allocate,
    Form,
    Read,
    Write,
    GuardedWrite,
    Copy,
    Trap,
    View(views::ViewGuard),
}

fn guard(plan: &ByteOperationV30<'_, '_>, actual: &OperationKind) -> Result<Guard> {
    let guard = match plan {
        ByteOperationV30::Trap(_) => Guard::Trap,
        ByteOperationV30::Alloca(_) => Guard::Allocate,
        ByteOperationV30::View(view) => Guard::View(view.guard()),
        ByteOperationV30::Storage(storage) => match storage.effect() {
            PointerByteEffectV30::Read { .. } => Guard::Read,
            PointerByteEffectV30::Write { .. } => Guard::Write,
            PointerByteEffectV30::Copy { .. } => Guard::Copy,
            PointerByteEffectV30::None => return Err(mismatch()),
            PointerByteEffectV30::GuardedWrite { .. } => return Err(mismatch()),
        },
        ByteOperationV30::Pointer(pointer) => match pointer.effect() {
            PointerByteEffectV30::Read { .. } => Guard::Read,
            PointerByteEffectV30::Write { .. } => Guard::Write,
            PointerByteEffectV30::GuardedWrite { .. } => match actual {
                OperationKind::GuardedStore { .. } => Guard::GuardedWrite,
                _ => return Err(mismatch()),
            },
            PointerByteEffectV30::Copy { .. } => return Err(mismatch()),
            PointerByteEffectV30::None => match actual {
                OperationKind::GetElementPointer { .. } => Guard::Form,
                OperationKind::Cast { .. }
                | OperationKind::SliceData { .. }
                | OperationKind::SliceLength { .. } => Guard::None,
                _ => return Err(mismatch()),
            },
        },
        ByteOperationV30::Scalar(_)
        | ByteOperationV30::Execution(_)
        | ByteOperationV30::Index(_)
        | ByteOperationV30::Checked(_)
        | ByteOperationV30::Float(_)
        | ByteOperationV30::TaggedSelect(_)
        | ByteOperationV30::IntegralCast(_) => Guard::None,
    };
    Ok(guard)
}

pub(super) fn check(
    physical: &Physical<'_, '_>,
    operation: usize,
    plan: &ByteOperationV30<'_, '_>,
    actual: &OperationKind,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    // Fixed dispatch plus the complete generated enum census; never charge only
    // the number of flags that happened to be set by one analysis outcome.
    out.budget.charge_work(8 + Obligation::COUNT)?;
    let fact = physical.operation(operation).ok_or_else(mismatch)?;
    let guard = guard(plan, actual)?;
    let expected = match guard {
        Guard::None => Kind::None,
        Guard::Allocate => Kind::Allocate,
        Guard::Form => Kind::Project,
        Guard::Read => Kind::Read,
        Guard::Write => Kind::Write,
        // V38 intentionally retains UnknownWrite rather than claiming that a
        // conditional store definitely initializes memory. Only this exact
        // derived opcode discharges its dynamic Operation obligation.
        Guard::GuardedWrite => Kind::Unmodeled,
        Guard::Copy => Kind::Copy,
        Guard::Trap => Kind::Unmodeled,
        Guard::View(
            views::ViewGuard::Form | views::ViewGuard::Construction | views::ViewGuard::Variant,
        ) => Kind::Project,
        Guard::View(
            views::ViewGuard::Discriminant
            | views::ViewGuard::TagWrite
            | views::ViewGuard::UntaggedNoop,
        ) => Kind::Unmodeled,
    };
    if fact.kind() != expected {
        return Err(Error::Statement(
            "physical byte operation class is not modeled",
        ));
    }
    fact.try_visit_obligations(|obligation| {
        let covered = match obligation {
            Obligation::Address | Obligation::Currentness | Obligation::ExternalMemory => {
                matches!(
                    guard,
                    Guard::Form
                        | Guard::Read
                        | Guard::Write
                        | Guard::Copy
                        | Guard::View(
                            views::ViewGuard::Form
                                | views::ViewGuard::Construction
                                | views::ViewGuard::Variant
                                | views::ViewGuard::Discriminant
                                | views::ViewGuard::TagWrite
                        )
                )
            }
            Obligation::Bounds | Obligation::Alignment => {
                matches!(
                    guard,
                    Guard::Allocate | Guard::Read | Guard::Write | Guard::Copy
                ) || guard == Guard::Form
                    || matches!(guard, Guard::View(view) if view != views::ViewGuard::UntaggedNoop)
            }
            Obligation::Initialization => {
                guard == Guard::Read
                    || matches!(
                        guard,
                        Guard::View(views::ViewGuard::Variant | views::ViewGuard::Discriminant)
                    )
            }
            // Unknown static reachability prevents a fixed-point claim. The
            // generated step still checks its actual PC and complete dynamic
            // memory preconditions; a paired run must establish the real path.
            Obligation::Reachability => guard != Guard::None,
            // Successful opcode derivation already checked exact types, access
            // modes, non-volatility and explicit formal INDEX width. Copy also
            // checks its non-overlap contract in the emitted validity guard.
            Obligation::Operation => {
                matches!(
                    guard,
                    Guard::Allocate
                        | Guard::Form
                        | Guard::Read
                        | Guard::Write
                        | Guard::GuardedWrite
                        | Guard::Copy
                        | Guard::View(_)
                        | Guard::Trap
                )
            }
            // Exact tag plans borrow checked row contracts. Read/formation
            // steps preserve/check enclosing guards; real writes use byte_store.
            // The untagged niche plan is proven physically inert by derivation,
            // so it neither dereferences an address nor certifies an active tag.
            Obligation::ActiveView | Obligation::TagContract => matches!(
                guard,
                Guard::View(
                    views::ViewGuard::Construction
                        | views::ViewGuard::Variant
                        | views::ViewGuard::Discriminant
                        | views::ViewGuard::TagWrite
                        | views::ViewGuard::UntaggedNoop
                )
            ),
            Obligation::CopyBoundaryClosure => false,
        };
        if covered {
            Ok(())
        } else {
            Err(Error::Statement("unresolved physical byte obligation"))
        }
    })
}

pub(super) fn headers() -> usize {
    size_of::<(
        &Physical<'_, '_>,
        &Fact,
        &OperationKind,
        &ByteOperationV30<'_, '_>,
    )>() + 2 * size_of::<Guard>()
        + size_of::<Result<Guard>>()
        + size_of::<(Kind, Obligation, [usize; 2], [bool; 2], [Result<()>; 2])>()
}
