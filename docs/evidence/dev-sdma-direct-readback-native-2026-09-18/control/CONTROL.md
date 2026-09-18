# Reviewed Direct Readback Campaign

Root reviewed the frozen payload
`395806274d56b787af9bdd43771ef6a0cf422e2afbf7be190ae7a68930f059b9`
and authorizes exactly one campaign in newly created mode-0700 directory
`/tmp/fe2o3-sdma-readback-fd1cf3dd-20260918.2gdfkcmf`.
The source is signed `fd1cf3dd05e691797533b1beab1f015c4b2d8fad`; its
fresh CPU packet seal is
`d35435e8d78df345f32697c8468a16b1aa28a7c008b845b9383a3d9c9bd2f686`.

Compared with the previous late-selection controller, only the commit,
payload, bundle and exact remote path identities changed. The remote helper's
cleanup and independent absence logic is unchanged. Four protocol and four
controller wiring calibration tests passed; complete frozen payload verification
passed. The separate create helper only created and marked the fresh directory.

The user permits using free GPUs, not interruption of other work. Candidate
GPU 4 must pass the unchanged strict guard before each test. No reservation is
claimed. Device and host cold-allocation probes each exercise a changed readback
route; prior HIP/native handoffs are closed. There is no matched performance,
native-fault, formal-refinement or R126 acceptance claim.

Execute the reviewed controller with `--root-ready-approved`, the exact remote
path and a new local output directory. Retain rejection if any, collect and
verify all files before exact-owned cleanup, and separately confirm path and
recorded PID/group absence. Do not retry or relax the protocol.
