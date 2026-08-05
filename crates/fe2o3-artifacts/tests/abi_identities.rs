mod common;

use common::{digest, manifest};
use fe2o3_artifacts::{
    DeclaredRustLayoutIdentity, DeclaredRustTypeIdentity, ManifestV1, TypeIdentity,
};

#[test]
fn type_identity_preserves_typed_accessors() {
    let rust_type = DeclaredRustTypeIdentity::from_untrusted_bytes(digest(0x31));
    let layout = DeclaredRustLayoutIdentity::from_untrusted_bytes(digest(0x32));
    let identity = TypeIdentity::new(rust_type, layout);

    assert_eq!(identity.rust_type(), rust_type);
    assert_eq!(identity.layout(), layout);
    assert_eq!(identity.rust_type().bytes(), digest(0x31));
    assert_eq!(identity.layout().bytes(), digest(0x32));
}

#[test]
fn manifest_wire_round_trip_preserves_typed_identity_bytes() {
    let original_bytes = manifest().to_bytes();
    let decoded = ManifestV1::from_bytes(&original_bytes).unwrap();
    let identity = decoded.kernels()[0].abi().fields()[0].type_identity();

    assert_eq!(identity.rust_type().bytes(), digest(0xa0));
    assert_eq!(identity.layout().bytes(), digest(0xa1));
    assert_eq!(decoded.to_bytes(), original_bytes);
}
