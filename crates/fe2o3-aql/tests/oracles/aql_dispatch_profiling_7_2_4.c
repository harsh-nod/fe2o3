#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "hsa.h"
#include "amd_hsa_queue.h"
#include "amd_hsa_signal.h"

_Static_assert(sizeof(void *) == 8, "64-bit ABI");
_Static_assert(sizeof(hsa_queue_t) == 40, "HSA queue prefix");
_Static_assert(sizeof(amd_queue_v2_t) == 2304, "AMD queue v2 size");
_Static_assert(_Alignof(amd_queue_v2_t) == 64, "AMD queue alignment");
_Static_assert(offsetof(amd_queue_v2_t, write_dispatch_id) == 0x38, "write index");
_Static_assert(offsetof(amd_queue_v2_t, read_dispatch_id) == 0x80, "read index");
_Static_assert(offsetof(amd_queue_v2_t, read_dispatch_id_field_base_byte_offset) == 0x88,
               "read index base offset");
_Static_assert(offsetof(amd_queue_v2_t, queue_properties) == 0xb4, "properties offset");
_Static_assert(sizeof(((amd_queue_v2_t *)0)->queue_properties) == 4, "properties width");
_Static_assert(AMD_QUEUE_PROPERTIES_ENABLE_PROFILING_SHIFT == 3, "profiling bit");
_Static_assert(AMD_QUEUE_PROPERTIES_ENABLE_PROFILING_WIDTH == 1, "profiling width");
_Static_assert(AMD_QUEUE_PROPERTIES_ENABLE_PROFILING == 8, "profiling mask");
_Static_assert(AMD_QUEUE_PROPERTIES_IS_PTR64 == 2, "pointer-width mask");
_Static_assert(sizeof(amd_signal_t) == 64, "signal size");
_Static_assert(_Alignof(amd_signal_t) == 64, "signal alignment");
_Static_assert(offsetof(amd_signal_t, kind) == 0, "signal kind");
_Static_assert(offsetof(amd_signal_t, value) == 8, "signal value");
_Static_assert(offsetof(amd_signal_t, start_ts) == 32, "start timestamp");
_Static_assert(offsetof(amd_signal_t, end_ts) == 40, "end timestamp");

int main(void) {
    const uint32_t words[] = {0, 2, 8, 0xa5a55a5a, UINT32_MAX};
    for (size_t i = 0; i < sizeof(words) / sizeof(words[0]); ++i) {
        uint32_t enabled = words[i];
        AMD_HSA_BITS_SET(enabled, AMD_QUEUE_PROPERTIES_ENABLE_PROFILING, 1);
        if (enabled != (words[i] | UINT32_C(8))) return 1;
        printf("properties=%08x enabled=%08x\n", words[i], enabled);
    }
    amd_signal_t signal;
    memset(&signal, 0, sizeof(signal));
    signal.kind = AMD_SIGNAL_KIND_USER;
    signal.value = 0;
    signal.start_ts = UINT64_C(0x0102030405060708);
    signal.end_ts = UINT64_C(0x1122334455667788);
    const unsigned char *bytes = (const unsigned char *)&signal;
    printf("signal=");
    for (size_t i = 0; i < sizeof(signal); ++i) printf("%02x", bytes[i]);
    putchar('\n');
    puts("queue=2304/64 properties=180/4 profiling=3/8 signal=64/64 timestamps=32/40");
    return 0;
}
