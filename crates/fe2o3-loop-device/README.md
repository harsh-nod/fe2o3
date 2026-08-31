# fe2o3 loop-device boundary

This crate isolates the Linux loop-control `ioctl` ABI needed to mount one
already admitted sealed base image. It configures a free loop device atomically
with `LOOP_CONFIGURE`, requires read-only and autoclear flags, rechecks the
kernel's complete `loop_info64`, and retains the device until its consumer has
mounted and later unmounted the image.

The public move-only value exposes only its canonical `/dev/loopN` identity. It
does not expose a descriptor, writable configuration, offsets, size limits,
partition scanning, direct I/O, detachment authority, or arbitrary loop-device
selection.
