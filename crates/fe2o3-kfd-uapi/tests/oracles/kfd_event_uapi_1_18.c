#include <linux/kfd_ioctl.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <sys/ioctl.h>

#define PRINT_LAYOUT(type) \
  printf(#type " size=%zu align=%zu\n", sizeof(struct type), _Alignof(struct type))
#define PRINT_OFFSET(type, field) \
  printf(#type "." #field "=%zu\n", offsetof(struct type, field))

int main(void) {
  _Static_assert(KFD_IOCTL_MAJOR_VERSION == 1, "KFD major drift");
  _Static_assert(KFD_IOCTL_MINOR_VERSION == 18, "KFD minor drift");
  _Static_assert(KFD_SIGNAL_EVENT_LIMIT == 4096, "signal limit drift");
  _Static_assert(KFD_EC_MASK_QUEUE == UINT64_C(0x607f803f), "queue mask drift");

  printf("version=%u.%u\n", KFD_IOCTL_MAJOR_VERSION, KFD_IOCTL_MINOR_VERSION);
  printf("create_event=0x%08lx\n", (unsigned long)AMDKFD_IOC_CREATE_EVENT);
  printf("destroy_event=0x%08lx\n", (unsigned long)AMDKFD_IOC_DESTROY_EVENT);
  printf("set_event=0x%08lx\n", (unsigned long)AMDKFD_IOC_SET_EVENT);
  printf("reset_event=0x%08lx\n", (unsigned long)AMDKFD_IOC_RESET_EVENT);
  printf("wait_events=0x%08lx\n", (unsigned long)AMDKFD_IOC_WAIT_EVENTS);
  printf("event_types=%u,%u,%u,%u,%u,%u,%u,%u,%u\n",
         KFD_IOC_EVENT_SIGNAL, KFD_IOC_EVENT_NODECHANGE,
         KFD_IOC_EVENT_DEVICESTATECHANGE, KFD_IOC_EVENT_HW_EXCEPTION,
         KFD_IOC_EVENT_SYSTEM_EVENT, KFD_IOC_EVENT_DEBUG_EVENT,
         KFD_IOC_EVENT_PROFILE_EVENT, KFD_IOC_EVENT_QUEUE_EVENT,
         KFD_IOC_EVENT_MEMORY);
  printf("wait_results=%u,%u,%u\n", KFD_IOC_WAIT_RESULT_COMPLETE,
         KFD_IOC_WAIT_RESULT_TIMEOUT, KFD_IOC_WAIT_RESULT_FAIL);
  printf("signal_limit=%u\n", KFD_SIGNAL_EVENT_LIMIT);
  printf("queue_exception_mask=0x%016llx\n",
         (unsigned long long)KFD_EC_MASK_QUEUE);

  PRINT_LAYOUT(kfd_ioctl_create_event_args);
  PRINT_OFFSET(kfd_ioctl_create_event_args, event_page_offset);
  PRINT_OFFSET(kfd_ioctl_create_event_args, event_trigger_data);
  PRINT_OFFSET(kfd_ioctl_create_event_args, event_type);
  PRINT_OFFSET(kfd_ioctl_create_event_args, auto_reset);
  PRINT_OFFSET(kfd_ioctl_create_event_args, node_id);
  PRINT_OFFSET(kfd_ioctl_create_event_args, event_id);
  PRINT_OFFSET(kfd_ioctl_create_event_args, event_slot_index);
  PRINT_LAYOUT(kfd_ioctl_destroy_event_args);
  PRINT_LAYOUT(kfd_ioctl_set_event_args);
  PRINT_LAYOUT(kfd_ioctl_reset_event_args);
  PRINT_LAYOUT(kfd_memory_exception_failure);
  PRINT_LAYOUT(kfd_hsa_memory_exception_data);
  PRINT_OFFSET(kfd_hsa_memory_exception_data, failure);
  PRINT_OFFSET(kfd_hsa_memory_exception_data, va);
  PRINT_OFFSET(kfd_hsa_memory_exception_data, gpu_id);
  PRINT_OFFSET(kfd_hsa_memory_exception_data, ErrorType);
  PRINT_LAYOUT(kfd_hsa_hw_exception_data);
  PRINT_LAYOUT(kfd_hsa_signal_event_data);
  PRINT_LAYOUT(kfd_event_data);
  PRINT_OFFSET(kfd_event_data, kfd_event_data_ext);
  PRINT_OFFSET(kfd_event_data, event_id);
  PRINT_OFFSET(kfd_event_data, pad);
  PRINT_LAYOUT(kfd_ioctl_wait_events_args);
  PRINT_OFFSET(kfd_ioctl_wait_events_args, events_ptr);
  PRINT_OFFSET(kfd_ioctl_wait_events_args, num_events);
  PRINT_OFFSET(kfd_ioctl_wait_events_args, wait_for_all);
  PRINT_OFFSET(kfd_ioctl_wait_events_args, timeout);
  PRINT_OFFSET(kfd_ioctl_wait_events_args, wait_result);
  PRINT_LAYOUT(kfd_context_save_area_header);
  PRINT_OFFSET(kfd_context_save_area_header, debug_offset);
  PRINT_OFFSET(kfd_context_save_area_header, debug_size);
  PRINT_OFFSET(kfd_context_save_area_header, err_payload_addr);
  PRINT_OFFSET(kfd_context_save_area_header, err_event_id);
  PRINT_OFFSET(kfd_context_save_area_header, reserved1);
  return 0;
}
