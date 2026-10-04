#include <stddef.h>
#include <stdint.h>
#include <stdio.h>

#if defined(FE2O3_LIBDRM_ORACLE)
#include <amdgpu_drm.h>
#include <drm.h>
#else
#include <drm/amdgpu_drm.h>
#include <drm/drm.h>
#endif

int main(void) {
    printf("drm_version:size=%zu align=%zu major=%zu minor=%zu patch=%zu "
           "name_len=%zu name=%zu date_len=%zu date=%zu desc_len=%zu desc=%zu "
           "request=%#lx\n",
           sizeof(struct drm_version), _Alignof(struct drm_version),
           offsetof(struct drm_version, version_major),
           offsetof(struct drm_version, version_minor),
           offsetof(struct drm_version, version_patchlevel),
           offsetof(struct drm_version, name_len),
           offsetof(struct drm_version, name),
           offsetof(struct drm_version, date_len),
           offsetof(struct drm_version, date),
           offsetof(struct drm_version, desc_len),
           offsetof(struct drm_version, desc),
           (unsigned long)DRM_IOCTL_VERSION);

    printf("amdgpu_info:size=%zu align=%zu return_pointer=%zu return_size=%zu "
           "query=%zu union=%zu request=%#lx accel=%#x dev_info=%#x\n",
           sizeof(struct drm_amdgpu_info), _Alignof(struct drm_amdgpu_info),
           offsetof(struct drm_amdgpu_info, return_pointer),
           offsetof(struct drm_amdgpu_info, return_size),
           offsetof(struct drm_amdgpu_info, query),
           offsetof(struct drm_amdgpu_info, read_mmr_reg),
           (unsigned long)DRM_IOCTL_AMDGPU_INFO,
           AMDGPU_INFO_ACCEL_WORKING, AMDGPU_INFO_DEV_INFO);
    printf("currentness:vram_lost_counter=%#x result_size=%zu\n",
           AMDGPU_INFO_VRAM_LOST_COUNTER, sizeof(__u32));

    printf("device:size=%zu align=%zu device_id=%zu chip_rev=%zu external_rev=%zu "
           "pci_rev=%zu family=%zu family_ai=%u\n",
           sizeof(struct drm_amdgpu_info_device),
           _Alignof(struct drm_amdgpu_info_device),
           offsetof(struct drm_amdgpu_info_device, device_id),
           offsetof(struct drm_amdgpu_info_device, chip_rev),
           offsetof(struct drm_amdgpu_info_device, external_rev),
           offsetof(struct drm_amdgpu_info_device, pci_rev),
           offsetof(struct drm_amdgpu_info_device, family), AMDGPU_FAMILY_AI);
    return 0;
}
