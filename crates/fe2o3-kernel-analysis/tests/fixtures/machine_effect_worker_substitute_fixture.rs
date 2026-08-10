mod common {
    include!("machine_effect_worker_fixture_common.rs");
}

fn main() {
    common::run(0xc3, 0xd4);
}
