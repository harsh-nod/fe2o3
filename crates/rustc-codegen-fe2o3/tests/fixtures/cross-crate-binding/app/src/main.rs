fn main() {
    let (binding_a, address_a) = kernel_a::binding_and_registration();
    let (binding_b, address_b) = kernel_b::binding_and_registration();

    assert_ne!(
        binding_a, binding_b,
        "compilation units shared a binding ID"
    );
    assert_ne!(address_a, address_b, "both rlibs resolved to one registration");
}
