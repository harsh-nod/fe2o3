# Native Compiler Device Confinement

Checkpointed compiler launches into a fresh owned cgroup install a fixed
`BPF_PROG_TYPE_CGROUP_DEVICE` program before `CLONE_INTO_CGROUP`. Its two
instructions return zero for every device/access input. It has no maps, helpers,
branches, configurable allowlist or caller-provided program. A successful
`BPF_PROG_ATTACH` binds that program to the original directory held by the
existing domain cleanup owner; closing the temporary program FD does not detach
it. Flags zero do not allow descendant replacement. The complete original domain
remains owned until aggregate cleanup removes it.

The gate controller must call `RootTaskObservationV2::require_device_open_confinement`
through its original live trace. A stage flag, unit directive, PID, reopened path,
or failed device probe is not accepted as enforcement. The observation checks
actual retained domain installation and identity under the original account.
Existing custody contracts still require exclusion of privileged foreign policy,
mount and migration writers and prevent children accessing cgroup controls.

This denies device opens before their driver callbacks. It does not revoke
inherited or subsequently imported descriptors; those remain subject to the
independent descriptor policy. Deny-all includes `/dev/null` and `/dev/urandom`.
Any required new device exception needs a separately reviewed fixed policy and
actual compiler qualification, not a path-name exemption. Regular-file access
and output-write confinement remain separate.

Loading this program requires `CAP_BPF` plus `CAP_NET_ADMIN` (or the broader
legacy `CAP_SYS_ADMIN`) in the deployment's actual privileged context. Missing
capabilities, unsupported kernels, or incompatible ancestor attachment modes
refuse before cloning. The canonical service unit grants only the two narrower
capabilities to its trusted creator; it does not grant `CAP_SYS_ADMIN`. Existing
child setup drops effective, permitted, inheritable, ambient and bounding
capabilities before READY, including these two. Updating the repository unit
does not install it or qualify actual privileged/native production execution.

Kernel contracts: [cgroup v2 device controller](https://docs.kernel.org/admin-guide/cgroup-v2.html#device-controller),
[Linux 6.8 BPF syscall permission checks](https://github.com/torvalds/linux/blob/v6.8/kernel/bpf/syscall.c),
and [cgroup attachment and inheritance](https://github.com/torvalds/linux/blob/v6.8/kernel/bpf/cgroup.c).
