fn main() {
    let (binding_a, address_a, length_a) = kernel_a::binding_and_artifact();
    let (binding_b, address_b, length_b) = kernel_b::binding_and_artifact();

    assert_ne!(
        binding_a, binding_b,
        "compilation units shared a binding ID"
    );
    assert_ne!(address_a, address_b, "both rlibs resolved to one artifact");
    assert!(length_a > 0);
    assert!(length_b > 0);
}
