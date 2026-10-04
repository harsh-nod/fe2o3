#include <stddef.h>
#include <stdint.h>
#include "amd_hsa_queue.h"
#include "amd_hsa_signal.h"

_Static_assert(sizeof(void *) == 8, "64-bit ROCr model required");
_Static_assert(offsetof(amd_queue_v2_t, write_dispatch_id) == 0x38,
               "write dispatch offset drift");
_Static_assert(offsetof(amd_queue_v2_t, read_dispatch_id) == 0x80,
               "read dispatch offset drift");
_Static_assert(offsetof(amd_queue_v2_t, queue_properties) == 0xb4,
               "queue properties offset drift");
_Static_assert(AMD_QUEUE_PROPERTIES_ENABLE_PROFILING == 8,
               "queue profiling mask drift");
_Static_assert(sizeof(amd_signal_t) == 64, "signal size drift");
_Static_assert(_Alignof(amd_signal_t) == 64, "signal alignment drift");
_Static_assert(offsetof(amd_signal_t, kind) == 0, "signal kind offset drift");
_Static_assert(offsetof(amd_signal_t, value) == 8, "signal value offset drift");
_Static_assert(offsetof(amd_signal_t, start_ts) == 32, "start tick offset drift");
_Static_assert(offsetof(amd_signal_t, end_ts) == 40, "end tick offset drift");

int main(void) { return 0; }
