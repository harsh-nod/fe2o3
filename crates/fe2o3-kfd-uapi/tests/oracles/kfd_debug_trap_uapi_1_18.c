#include <linux/kfd_ioctl.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <sys/ioctl.h>

#define L(type) printf(#type " size=%zu align=%zu\n", sizeof(struct type), _Alignof(struct type))
#define O(type, field) printf(#type "." #field "=%zu\n", offsetof(struct type, field))

int main(void) {
  _Static_assert(KFD_IOCTL_MAJOR_VERSION == 1, "KFD major drift");
  _Static_assert(KFD_IOCTL_MINOR_VERSION == 18, "KFD minor drift");
  printf("dbg_trap=0x%08lx\n", (unsigned long)AMDKFD_IOC_DBG_TRAP);
  printf("runtime_enable=0x%08lx modes=%u,%u\n",
         (unsigned long)AMDKFD_IOC_RUNTIME_ENABLE,
         KFD_RUNTIME_ENABLE_MODE_ENABLE_MASK,
         KFD_RUNTIME_ENABLE_MODE_TTMP_SAVE_MASK);
  printf("ops=%u,%u,%u,%u,%u,%u,%u,%u,%u,%u,%u,%u,%u,%u,%u\n",
         KFD_IOC_DBG_TRAP_ENABLE, KFD_IOC_DBG_TRAP_DISABLE,
         KFD_IOC_DBG_TRAP_SEND_RUNTIME_EVENT, KFD_IOC_DBG_TRAP_SET_EXCEPTIONS_ENABLED,
         KFD_IOC_DBG_TRAP_SET_WAVE_LAUNCH_OVERRIDE, KFD_IOC_DBG_TRAP_SET_WAVE_LAUNCH_MODE,
         KFD_IOC_DBG_TRAP_SUSPEND_QUEUES, KFD_IOC_DBG_TRAP_RESUME_QUEUES,
         KFD_IOC_DBG_TRAP_SET_NODE_ADDRESS_WATCH, KFD_IOC_DBG_TRAP_CLEAR_NODE_ADDRESS_WATCH,
         KFD_IOC_DBG_TRAP_SET_FLAGS, KFD_IOC_DBG_TRAP_QUERY_DEBUG_EVENT,
         KFD_IOC_DBG_TRAP_QUERY_EXCEPTION_INFO, KFD_IOC_DBG_TRAP_GET_QUEUE_SNAPSHOT,
         KFD_IOC_DBG_TRAP_GET_DEVICE_SNAPSHOT);
  L(kfd_runtime_info);
  L(kfd_ioctl_runtime_enable_args); O(kfd_ioctl_runtime_enable_args, r_debug); O(kfd_ioctl_runtime_enable_args, mode_mask); O(kfd_ioctl_runtime_enable_args, capabilities_mask);
  L(kfd_queue_snapshot_entry); O(kfd_queue_snapshot_entry, queue_id); O(kfd_queue_snapshot_entry, reserved);
  L(kfd_dbg_device_info_entry); O(kfd_dbg_device_info_entry, gpu_id); O(kfd_dbg_device_info_entry, gfx_target_version); O(kfd_dbg_device_info_entry, debug_prop);
  L(kfd_context_save_area_header); O(kfd_context_save_area_header, err_payload_addr);
  L(kfd_ioctl_dbg_trap_enable_args); L(kfd_ioctl_dbg_trap_send_runtime_event_args);
  L(kfd_ioctl_dbg_trap_set_exceptions_enabled_args); L(kfd_ioctl_dbg_trap_set_wave_launch_override_args);
  L(kfd_ioctl_dbg_trap_set_wave_launch_mode_args); L(kfd_ioctl_dbg_trap_suspend_queues_args);
  L(kfd_ioctl_dbg_trap_resume_queues_args); L(kfd_ioctl_dbg_trap_set_node_address_watch_args);
  L(kfd_ioctl_dbg_trap_clear_node_address_watch_args); L(kfd_ioctl_dbg_trap_set_flags_args);
  L(kfd_ioctl_dbg_trap_query_debug_event_args); L(kfd_ioctl_dbg_trap_query_exception_info_args);
  L(kfd_ioctl_dbg_trap_queue_snapshot_args); L(kfd_ioctl_dbg_trap_device_snapshot_args);
  L(kfd_ioctl_dbg_trap_args); O(kfd_ioctl_dbg_trap_args, pid); O(kfd_ioctl_dbg_trap_args, op); O(kfd_ioctl_dbg_trap_args, enable);
  return 0;
}
