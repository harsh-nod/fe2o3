#include <linux/kfd_ioctl.h>
#include <stddef.h>
#include <stdio.h>

int main(void) {
    printf("get_version:size=%zu align=%zu major=%zu minor=%zu request=%#lx\n",
           sizeof(struct kfd_ioctl_get_version_args),
           _Alignof(struct kfd_ioctl_get_version_args),
           offsetof(struct kfd_ioctl_get_version_args, major_version),
           offsetof(struct kfd_ioctl_get_version_args, minor_version),
           (unsigned long)AMDKFD_IOC_GET_VERSION);

    printf("process_apertures:size=%zu align=%zu lds_base=%zu lds_limit=%zu "
           "scratch_base=%zu scratch_limit=%zu gpuvm_base=%zu gpuvm_limit=%zu "
           "gpu_id=%zu pad=%zu\n",
           sizeof(struct kfd_process_device_apertures),
           _Alignof(struct kfd_process_device_apertures),
           offsetof(struct kfd_process_device_apertures, lds_base),
           offsetof(struct kfd_process_device_apertures, lds_limit),
           offsetof(struct kfd_process_device_apertures, scratch_base),
           offsetof(struct kfd_process_device_apertures, scratch_limit),
           offsetof(struct kfd_process_device_apertures, gpuvm_base),
           offsetof(struct kfd_process_device_apertures, gpuvm_limit),
           offsetof(struct kfd_process_device_apertures, gpu_id),
           offsetof(struct kfd_process_device_apertures, pad));

    printf("get_process_apertures_new:size=%zu align=%zu pointer=%zu nodes=%zu "
           "pad=%zu request=%#lx\n",
           sizeof(struct kfd_ioctl_get_process_apertures_new_args),
           _Alignof(struct kfd_ioctl_get_process_apertures_new_args),
           offsetof(struct kfd_ioctl_get_process_apertures_new_args,
                    kfd_process_device_apertures_ptr),
           offsetof(struct kfd_ioctl_get_process_apertures_new_args,
                    num_of_nodes),
           offsetof(struct kfd_ioctl_get_process_apertures_new_args, pad),
           (unsigned long)AMDKFD_IOC_GET_PROCESS_APERTURES_NEW);

    printf("acquire_vm:size=%zu align=%zu drm_fd=%zu gpu_id=%zu request=%#lx\n",
           sizeof(struct kfd_ioctl_acquire_vm_args),
           _Alignof(struct kfd_ioctl_acquire_vm_args),
           offsetof(struct kfd_ioctl_acquire_vm_args, drm_fd),
           offsetof(struct kfd_ioctl_acquire_vm_args, gpu_id),
           (unsigned long)AMDKFD_IOC_ACQUIRE_VM);

    printf("xnack:size=%zu align=%zu field=%zu request=%#lx query=-1 disabled=0 "
           "enabled=1\n",
           sizeof(struct kfd_ioctl_set_xnack_mode_args),
           _Alignof(struct kfd_ioctl_set_xnack_mode_args),
           offsetof(struct kfd_ioctl_set_xnack_mode_args, xnack_enabled),
           (unsigned long)AMDKFD_IOC_SET_XNACK_MODE);
    return 0;
}
