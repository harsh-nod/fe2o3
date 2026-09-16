# Owned MI300X Scratch Cleanup

Private directory: `/tmp/fe2o3-r126-primary.tOhOZ1`, created by `mktemp -d`.
Only two files were uploaded: `runtime-native` and `runtime-native-qualified`.

The preliminary single-stream test exited 0; the preliminary two-stream process
ended with the recorded Busy rejection/panic (SSH exit 255). Its bounded process
query returned no match, and listing the private directory showed only the first
binary, not a core file. Core size was limited to zero for every invocation.

All three final native tests exited 0. After consuming their SSH handles:

1. `pgrep -af '^/tmp/fe2o3-r126-primary[.]tOhOZ1/runtime-native(-qualified)?( |$)'`
   returned exit 1 with no output.
2. `rm -- /tmp/fe2o3-r126-primary.tOhOZ1/runtime-native /tmp/fe2o3-r126-primary.tOhOZ1/runtime-native-qualified`
   returned exit 0.
3. `rmdir -- /tmp/fe2o3-r126-primary.tOhOZ1` returned exit 0.
4. `test ! -e /tmp/fe2o3-r126-primary.tOhOZ1` returned exit 0.
5. The retained post-check returned exit 0 and shows GPU 1 idle, zero allocated
   VRAM percent, with GPU 0's pre-existing 44% unchanged.

These commands ran through `ssh -o BatchMode=yes -o ConnectTimeout=10 mi300x`.
No other directories, processes, GPU services or devices were modified.
