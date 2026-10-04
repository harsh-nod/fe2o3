#include "fe2o3_static_preexec_manifest.h"

#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

_Static_assert(FE2O3_PREEXEC_MANIFEST_FD == 198,
               "V1 manifest FD changed");
_Static_assert(FE2O3_PREEXEC_EXECUTABLE_FD == 199,
               "V1 executable FD changed");
_Static_assert(FE2O3_PREEXEC_SOURCE_FD_BASE == 200,
               "V1 source FD base changed");
_Static_assert(FE2O3_PREEXEC_MAX_DESCRIPTORS == 16,
               "V1 descriptor bound changed");
_Static_assert(FE2O3_PREEXEC_MAX_DESTINATION_FD == 127,
               "V1 destination FD bound changed");
_Static_assert(FE2O3_PREEXEC_MANIFEST_VERSION == 1U,
               "V1 manifest version changed");

_Static_assert(sizeof(struct fe2o3_preexec_object_identity_v1) == 32,
               "V1 object identity size changed");
_Static_assert(offsetof(struct fe2o3_preexec_object_identity_v1, device) == 0,
               "V1 object device offset changed");
_Static_assert(offsetof(struct fe2o3_preexec_object_identity_v1, inode) == 8,
               "V1 object inode offset changed");
_Static_assert(offsetof(struct fe2o3_preexec_object_identity_v1, size) == 16,
               "V1 object size offset changed");
_Static_assert(offsetof(struct fe2o3_preexec_object_identity_v1, mode) == 24,
               "V1 object mode offset changed");
_Static_assert(offsetof(struct fe2o3_preexec_object_identity_v1, object_class) ==
                   28,
               "V1 object class offset changed");

_Static_assert(sizeof(struct fe2o3_preexec_descriptor_v1) == 40,
               "V1 descriptor size changed");
_Static_assert(offsetof(struct fe2o3_preexec_descriptor_v1, source_fd) == 0,
               "V1 source FD offset changed");
_Static_assert(offsetof(struct fe2o3_preexec_descriptor_v1, destination_fd) == 4,
               "V1 destination FD offset changed");
_Static_assert(offsetof(struct fe2o3_preexec_descriptor_v1, object) == 8,
               "V1 descriptor object offset changed");

_Static_assert(sizeof(struct fe2o3_preexec_manifest_v1) == 704,
               "V1 manifest size changed");
_Static_assert(offsetof(struct fe2o3_preexec_manifest_v1, magic) == 0,
               "V1 magic offset changed");
_Static_assert(offsetof(struct fe2o3_preexec_manifest_v1, version) == 8,
               "V1 version offset changed");
_Static_assert(offsetof(struct fe2o3_preexec_manifest_v1, descriptor_count) == 12,
               "V1 descriptor count offset changed");
_Static_assert(offsetof(struct fe2o3_preexec_manifest_v1, parent_pid) == 16,
               "V1 parent PID offset changed");
_Static_assert(offsetof(struct fe2o3_preexec_manifest_v1, reserved) == 20,
               "V1 manifest reserved offset changed");
_Static_assert(offsetof(struct fe2o3_preexec_manifest_v1, parent_start_time) == 24,
               "V1 parent start-time offset changed");
_Static_assert(offsetof(struct fe2o3_preexec_manifest_v1, executable) == 32,
               "V1 executable offset changed");
_Static_assert(offsetof(struct fe2o3_preexec_manifest_v1, descriptors) == 64,
               "V1 descriptors offset changed");

static void set_object(struct fe2o3_preexec_object_identity_v1 *object,
                       uint64_t device, uint64_t inode, uint64_t size,
                       uint32_t mode) {
  object->device = device;
  object->inode = inode;
  object->size = size;
  object->mode = mode;
  object->object_class = FE2O3_PREEXEC_OBJECT_CLASS_FSTAT;
}

int main(void) {
  struct fe2o3_preexec_manifest_v1 manifest = {0};
  (void)memcpy(manifest.magic, FE2O3_PREEXEC_MANIFEST_MAGIC,
               sizeof(manifest.magic));
  manifest.version = FE2O3_PREEXEC_MANIFEST_VERSION;
  manifest.descriptor_count = 3U;
  manifest.parent_pid = INT32_C(0x11223344);
  manifest.parent_start_time = UINT64_C(0x0102030405060708);
  set_object(&manifest.executable, UINT64_C(0x1112131415161718),
             UINT64_C(0x2122232425262728), UINT64_C(0x3132333435363738),
             UINT32_C(0x41424344));

  const uint64_t devices[] = {UINT64_C(0x5152535455565758),
                              UINT64_C(0x9192939495969798),
                              UINT64_C(0xd1d2d3d4d5d6d7d8)};
  const uint64_t inodes[] = {UINT64_C(0x6162636465666768),
                             UINT64_C(0xa1a2a3a4a5a6a7a8),
                             UINT64_C(0xe1e2e3e4e5e6e7e8)};
  const uint64_t sizes[] = {UINT64_C(0x7172737475767778),
                            UINT64_C(0xb1b2b3b4b5b6b7b8),
                            UINT64_C(0xf1f2f3f4f5f6f7f8)};
  const uint32_t modes[] = {UINT32_C(0x81828384), UINT32_C(0xc1c2c3c4),
                            UINT32_C(0x01020304)};
  for (int32_t index = 0; index < 3; ++index) {
    manifest.descriptors[index].source_fd =
        FE2O3_PREEXEC_SOURCE_FD_BASE + index;
    manifest.descriptors[index].destination_fd = index;
    set_object(&manifest.descriptors[index].object, devices[index],
               inodes[index], sizes[index], modes[index]);
  }

  return fwrite(&manifest, 1U, sizeof(manifest), stdout) == sizeof(manifest)
             ? 0
             : 1;
}
