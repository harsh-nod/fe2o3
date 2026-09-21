use super::*;
use fe2o3_kernel_ir::scalar_ops_v2::{IntWidth as Width, ScalarType as Ty};
use rustc_middle::mir::{BinOp, UnOp};

#[path = "tests/driver_tests.rs"]
mod driver;
mod layout;

fn integer(width: Width, signed: bool) -> Ty {
    Ty::Int { width, signed }
}
fn mask(width: u16) -> u128 {
    if width == 128 {
        u128::MAX
    } else {
        (1u128 << width) - 1
    }
}

#[test]
fn primitive_from_scalar_lossless_endpoints_are_fixed_and_strict() {
    let widths = [Width::W8, Width::W16, Width::W32, Width::W64, Width::W128];
    let mut admitted = 0;
    for a in widths {
        for b in widths {
            for sa in [false, true] {
                for sb in [false, true] {
                    let accepted = scalar::lossless(integer(a, sa), integer(b, sb));
                    assert_eq!(accepted, b.bits() > a.bits() && (!sa || sb));
                    admitted += usize::from(accepted);
                }
            }
        }
    }
    assert_eq!(admitted, 30);
    for unsupported in [
        Ty::Bool,
        Ty::Char,
        Ty::Float(fe2o3_kernel_ir::scalar_ops_v2::FloatWidth::F32),
        Ty::Pointer {
            address_space: 0,
            width: Width::W64,
        },
    ] {
        assert!(!scalar::lossless(unsupported, integer(Width::W128, true)));
        assert!(!scalar::lossless(integer(Width::W8, false), unsupported));
        assert!(scalar::Scalar::new(unsupported, 0).is_none() || unsupported == Ty::Bool);
    }
}

#[test]
fn primitive_from_scalar_casts_preserve_all_widths_signed_extension_and_narrowing() {
    let widths = [Width::W8, Width::W16, Width::W32, Width::W64, Width::W128];
    for a in widths {
        for b in widths {
            for signed in [false, true] {
                let source = integer(a, signed);
                for bits in [
                    0,
                    1,
                    (1u128 << (a.bits() - 1)) - 1,
                    1u128 << (a.bits() - 1),
                    mask(a.bits()),
                ] {
                    let value = scalar::Scalar::new(source, bits).unwrap();
                    for target_signed in [false, true] {
                        let result = value.cast(integer(b, target_signed)).unwrap();
                        let extended =
                            if signed && a.bits() < 128 && bits & (1u128 << (a.bits() - 1)) != 0 {
                                bits | !mask(a.bits())
                            } else {
                                bits
                            };
                        assert_eq!(result.bits, extended & mask(b.bits()));
                        assert_eq!(result.ty, integer(b, target_signed));
                    }
                }
            }
        }
    }
    assert!(scalar::Scalar::new(Ty::Bool, 2).is_none());
    assert!(scalar::Scalar::new(integer(Width::W8, false), 256).is_none());
    assert!(
        scalar::Scalar::new(Ty::Bool, 1)
            .unwrap()
            .cast(integer(Width::W8, false))
            .is_none()
    );
}

#[test]
fn primitive_from_scalar_checked_operations_never_prove_poison() {
    for width in [Width::W8, Width::W16, Width::W32, Width::W64, Width::W128] {
        let ty = integer(width, false);
        let value = |n| scalar::Scalar::new(ty, n).unwrap();
        assert!(
            value(mask(width.bits()))
                .binary(BinOp::Add, value(1))
                .is_none()
        );
        assert!(value(0).binary(BinOp::Sub, value(1)).is_none());
        assert!(value(1).binary(BinOp::Div, value(0)).is_none());
        assert!(value(1).binary(BinOp::Rem, value(0)).is_none());
        assert!(
            value(1)
                .binary(BinOp::Shl, value(u128::from(width.bits())))
                .is_none()
        );
        assert!(
            value(1)
                .binary(BinOp::Shr, value(u128::from(width.bits())))
                .is_none()
        );
        assert_eq!(
            value(1).unary(UnOp::Not).unwrap().bits,
            mask(width.bits()) ^ 1
        );
        assert!(value(1).unary(UnOp::Neg).is_none());
        let signed = integer(width, true);
        let minimum = scalar::Scalar::new(signed, 1u128 << (width.bits() - 1)).unwrap();
        assert!(minimum.unary(UnOp::Neg).is_none());
        assert_eq!(
            minimum
                .binary(BinOp::Lt, scalar::Scalar::new(signed, 0).unwrap())
                .unwrap(),
            scalar::Scalar::new(Ty::Bool, 1).unwrap()
        );
        assert!(minimum.binary(BinOp::Le, value(0)).is_none());
    }
    for bits in [0, 1] {
        assert_eq!(
            scalar::Scalar::new(Ty::Bool, bits)
                .unwrap()
                .unary(UnOp::Not)
                .unwrap()
                .bits,
            bits ^ 1
        );
    }
}
