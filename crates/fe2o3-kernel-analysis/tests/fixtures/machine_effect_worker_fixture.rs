mod common {
    include!("machine_effect_worker_fixture_common.rs");
}

fn main() {
    common::run(0xa1, 0xb2);
}
