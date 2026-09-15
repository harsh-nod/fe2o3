use super::*;

macro_rules! emission {
    ($name:ident, $cpu:literal) => {
        #[test]
        #[ignore = "requires cached authenticated AMD metadata; never runs Cargo"]
        fn $name() {
            probe_with_emission(
                concat!(module_path!(), "::", stringify!($name))
                    .strip_prefix("rustc_codegen_fe2o3::")
                    .unwrap(),
                $cpu,
                true,
                None,
                true,
            );
        }
    };
}

emission!(phase67_two_phase_source_kir_gfx942, "gfx942");
emission!(phase67_two_phase_source_kir_gfx950, "gfx950");
