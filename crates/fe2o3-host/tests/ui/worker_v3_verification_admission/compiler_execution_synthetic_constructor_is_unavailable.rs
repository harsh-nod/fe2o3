use fe2o3_host::WorkerV3CompilerExecutionVerificationV1;

fn main() {
    let _ = WorkerV3CompilerExecutionVerificationV1::synthetic_for_test_only(
        [1; 32], [2; 32], [3; 32], [4; 32], [5; 32], [6; 32], [7; 32], [8; 32], [9; 32], 1,
        [10; 32], [11; 32], [12; 32], [13; 32], [14; 32], [15; 32], [16; 32],
    );
}
