#ifndef FE2O3_STATIC_PREEXEC_MANIFEST_H
#define FE2O3_STATIC_PREEXEC_MANIFEST_H

#include <stdint.h>

#define FE2O3_PREEXEC_MANIFEST_FD 198
#define FE2O3_PREEXEC_EXECUTABLE_FD 199
#define FE2O3_PREEXEC_SOURCE_FD_BASE 200
#define FE2O3_PREEXEC_MAX_DESCRIPTORS 16
#define FE2O3_PREEXEC_MAX_DESTINATION_FD 127
#define FE2O3_PREEXEC_MANIFEST_VERSION 1U
#define FE2O3_PREEXEC_MANIFEST_MAGIC "FE2PXM1\0"
#define FE2O3_PREEXEC_OBJECT_CLASS_FSTAT 0U
#define FE2O3_PREEXEC_OBJECT_CLASS_PROCESS_PIDFD 1U

struct fe2o3_preexec_object_identity_v1 {
  uint64_t device;
  uint64_t inode;
  uint64_t size;
  uint32_t mode;
  uint32_t object_class;
};

struct fe2o3_preexec_descriptor_v1 {
  int32_t source_fd;
  int32_t destination_fd;
  struct fe2o3_preexec_object_identity_v1 object;
};

struct fe2o3_preexec_manifest_v1 {
  uint8_t magic[8];
  uint32_t version;
  uint32_t descriptor_count;
  int32_t parent_pid;
  uint32_t reserved;
  uint64_t parent_start_time;
  struct fe2o3_preexec_object_identity_v1 executable;
  struct fe2o3_preexec_descriptor_v1
      descriptors[FE2O3_PREEXEC_MAX_DESCRIPTORS];
};

#endif
