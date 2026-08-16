use fe2o3_host::MoeExpertExecutionDeniedV1;

fn escape<B>(denied: MoeExpertExecutionDeniedV1<B>) {
    denied.load();
}

fn main() {}
