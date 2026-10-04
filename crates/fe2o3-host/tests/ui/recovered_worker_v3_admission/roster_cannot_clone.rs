use fe2o3_host::RecoveredWorkerV3PinnedRosterV1;

struct Roster;

fn duplicate(value: RecoveredWorkerV3PinnedRosterV1<Roster>) {
    let _duplicate = value.clone();
}

fn main() {}
