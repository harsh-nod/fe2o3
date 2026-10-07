/* CPU-only oracle. Encoding macros are extracted verbatim from pinned kfd_priv.h. */
#include "kfd_ioctl.h"
#include <inttypes.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include "reviewed-output-macros.h"

#if !defined(__linux__) || !defined(__x86_64__)
#error "the reviewed output oracle requires Linux x86_64"
#endif

_Static_assert(sizeof(void *) == 8, "reviewed x86_64 profile");
_Static_assert(KFD_IOC_QUEUE_TYPE_COMPUTE_AQL == 2, "compute/AQL only");
_Static_assert(sizeof(((struct kfd_ioctl_create_queue_args *)0)->queue_id) == 4, "queue ID width");
_Static_assert(sizeof(((struct kfd_ioctl_create_queue_args *)0)->doorbell_offset) == 8, "offset width");
_Static_assert(offsetof(struct kfd_ioctl_create_queue_args, doorbell_offset) == 24, "offset field");
_Static_assert(KFD_MAX_NUM_OF_QUEUES_PER_PROCESS == 1024, "PQM slots");
_Static_assert(KFD_GPU_ID_HASH_WIDTH == 16, "wire hash width");
_Static_assert(KFD_MMAP_TYPE_SHIFT == 62 && KFD_MMAP_GPU_ID_SHIFT == 46, "wire field shifts");

int main(void) {
    /* kfd_device.c selects 8 for SOC15; kfd_doorbell.c rounds slots*width to page. */
    const uint64_t width = 8, page = 4096;
    const uint64_t slice = (KFD_MAX_NUM_OF_QUEUES_PER_PROCESS * width + page - 1) & ~(page - 1);
    const struct { uint32_t gpu, queue, doorbell; } cases[] = {
        {11429, 0, 1023}, {11429, 1023, 0}, {73, 1, 512}, {0xffff0049, 7, 3},
    };
    printf("slots=%u width=%" PRIu64 " slice=%" PRIu64 "\n",
           (unsigned)KFD_MAX_NUM_OF_QUEUES_PER_PROCESS, width, slice);
    for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); ++i) {
        /* PQM converts the source BAR dword index difference back to bytes. */
        const uint64_t dword_index = cases[i].doorbell * ((width + 3) / 4);
        const uint64_t offset = dword_index * sizeof(uint32_t);
        const uint64_t raw = KFD_MMAP_TYPE_DOORBELL | KFD_MMAP_GPU_ID(cases[i].gpu) | offset;
        if (KFD_MMAP_GET_GPU_ID(raw) != (cases[i].gpu & 0xffff)) return 2;
        if ((raw & KFD_MMAP_TYPE_MASK) != KFD_MMAP_TYPE_DOORBELL) return 2;
        printf("gpu=%08" PRIx32 " queue=%" PRIu32 " raw=%016" PRIx64
               " mmap=%016" PRIx64 " offset=%" PRIu64 "\n",
               cases[i].gpu, cases[i].queue, raw, raw & ~(slice - 1), raw & (slice - 1));
    }
    return 0;
}
