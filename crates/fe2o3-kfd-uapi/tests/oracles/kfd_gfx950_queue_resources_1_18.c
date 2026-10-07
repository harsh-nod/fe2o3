/* CPU-only independent geometry oracle; no KFD/DRM handles or native operations. */
#include "kfd_ioctl.h"
#include "rocr-hsakmttypes.h"
#include "rocr-amd_hsa_queue.h"
#include <errno.h>
#include <inttypes.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>

#if !defined(__linux__) || !defined(__x86_64__)
#error "the reviewed geometry oracle requires Linux x86_64"
#endif

_Static_assert(sizeof(void *) == 8, "reviewed x86_64 profile");
_Static_assert(sizeof(hsa_kernel_dispatch_packet_t) == 64, "AQL packet width");
_Static_assert(sizeof(HsaUserContextSaveAreaHeader) == 40, "CWSR header size");
_Static_assert(offsetof(HsaUserContextSaveAreaHeader, DebugOffset) == 16, "debug offset field");
_Static_assert(offsetof(HsaUserContextSaveAreaHeader, DebugSize) == 20, "debug size field");
_Static_assert(offsetof(HsaUserContextSaveAreaHeader, ErrorReason) == 24, "payload field");
_Static_assert(offsetof(HsaUserContextSaveAreaHeader, ErrorEventId) == 32, "event field");
_Static_assert(offsetof(amd_queue_t, write_dispatch_id) == 0x38, "write counter field");
_Static_assert(offsetof(amd_queue_t, read_dispatch_id) == 0x80, "read counter field");
_Static_assert(offsetof(amd_queue_t, read_dispatch_id_field_base_byte_offset) == 0x88, "read base field");
_Static_assert(offsetof(amd_queue_v2_t, write_dispatch_id) == 0x38, "v2 write counter field");
_Static_assert(offsetof(amd_queue_v2_t, read_dispatch_id) == 0x80, "v2 read counter field");
_Static_assert(offsetof(amd_queue_v2_t, read_dispatch_id_field_base_byte_offset) == 0x88, "v2 read base field");
_Static_assert(sizeof(((struct kfd_ioctl_create_queue_args *)0)->ctx_save_restore_size) == 4, "CWSR wire width");

static void reject(void) { fputs("invalid gfx950 oracle geometry\n", stderr); exit(2); }
static uint64_t add(uint64_t a, uint64_t b) { if (UINT64_MAX - a < b) reject(); return a + b; }
static uint64_t mul(uint64_t a, uint64_t b) { if (b && a > UINT64_MAX / b) reject(); return a * b; }
static uint64_t align(uint64_t value, uint64_t boundary) { return add(value, boundary - 1) & ~(boundary - 1); }
static uint64_t number(const char *text) {
    char *end;
    errno = 0;
    uint64_t value = strtoull(text, &end, 0);
    if (errno || !*text || *text == '-' || *end) reject();
    return value;
}

int main(int argc, char **argv) {
    if (argc != 1 && argc != 3) reject();
    const uint64_t kernel_context = argc == 3 ? number(argv[1]) : 0;
    const uint64_t kernel_control = argc == 3 ? number(argv[2]) : 0;
    const uint64_t page = 4096, simd = 1024, simd_per_cu = 4, xcc = 8;
    const uint64_t arrays = 32, arrays_per_engine = 1, lds_kib = 160;
    const uint64_t cu = simd / simd_per_cu / xcc;
    const uint64_t cu_waves = mul(cu, 40), engine_waves = mul(arrays / arrays_per_engine, 512);
    const uint64_t waves = cu_waves < engine_waves ? cu_waves : engine_waves;
    const uint64_t derived_control = align(add(sizeof(HsaUserContextSaveAreaHeader), add(mul(waves, 8), 8)), page);
    const uint64_t per_cu = add(add(0x80000, 0x4000), add(mul(lds_kib, 1024), 0x1000));
    const uint64_t workgroup = align(mul(cu, per_cu), page);
    const uint64_t context = kernel_context ? kernel_context : add(derived_control, workgroup);
    const uint64_t control = kernel_control ? kernel_control : derived_control;
    const uint64_t debug = align(mul(waves, 32), 64);
    if (control < page || control % page || context % page || context < add(control, workgroup)) reject();
    const uint64_t mapping = align(mul(add(context, debug), xcc), page);
    if (control > UINT32_MAX || context > UINT32_MAX || mapping > UINT32_MAX) reject();
    printf("geometry cu=%" PRIu64 " waves=%" PRIu64 " control=%" PRIu64 " workgroup=%" PRIu64
           " context=%" PRIu64 " debug=%" PRIu64 " mapping=%" PRIu64 "\n",
           cu, waves, control, workgroup, context, debug, mapping);
    for (uint64_t i = 0; i < xcc; ++i) {
        HsaUserContextSaveAreaHeader header = {0};
        header.DebugOffset = (uint32_t)((xcc - i) * context);
        header.DebugSize = (uint32_t)(debug * xcc);
        header.ErrorReason = (volatile HSAint64 *)(uintptr_t)UINT64_C(0x1122334455667788);
        header.ErrorEventId = 23;
        printf("header %" PRIu64 " offset=%" PRIu64 " debug_offset=%u debug_size=%u bytes=",
               i, i * context, header.DebugOffset, header.DebugSize);
        const unsigned char *bytes = (const unsigned char *)&header;
        for (size_t byte = 0; byte < sizeof(header); ++byte) printf("%02x", bytes[byte]);
        putchar('\n');
    }
    const uint64_t per_xcc = control / page;
    for (uint64_t i = 0; i < per_xcc * xcc; ++i)
        printf("shadow %" PRIu64 " offset=%" PRIu64 "\n", i, i / per_xcc * context + i % per_xcc * page);
    return 0;
}
