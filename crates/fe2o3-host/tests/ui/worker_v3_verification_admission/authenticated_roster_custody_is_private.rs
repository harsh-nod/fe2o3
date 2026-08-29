use fe2o3_host::AuthenticatedWorkerV3RosterV1;

fn decompose<R>(value: AuthenticatedWorkerV3RosterV1<R>) {
    let AuthenticatedWorkerV3RosterV1 {
        admission,
        current,
        verification,
        _roster,
    } = value;
    let _ = (admission, current, verification, _roster);
}

fn main() {}
