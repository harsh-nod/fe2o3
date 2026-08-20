# dialect-tile

`dialect-tile` is the target-neutral Pliron shell that owns bounded distributed
tile and layout materialization. Its D0 surface checks rank, lane distribution,
elements per lane, and total tile extent without naming a target address space,
instruction, or hardware resource.

The shell does not lower operations, select a compiler or hardware target,
produce artifacts, or grant proof, publication, load, tuning, or launch
authority. Its Pliron values and printed syntax are not durable fe2o3
identities.

Its production registration adapter depends only on
`fe2o3-pliron-owner-core`; ownership of the full Pliron session remains in
`fe2o3-pliron`.
