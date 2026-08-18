# dialect-schedule

`dialect-schedule` is the target-neutral Pliron shell that owns bounded,
non-executable scheduling plans. Its D0 surface records rank, tile extent, and
pipeline-stage choices without materializing tiles or executable behavior.

The shell does not lower operations, select a compiler or hardware target,
produce artifacts, or grant proof, publication, load, tuning, or launch
authority. Its Pliron values and printed syntax are not durable fe2o3
identities.
