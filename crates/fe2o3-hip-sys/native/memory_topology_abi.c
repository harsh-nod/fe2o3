#include <hip/hip_runtime_api.h>
#include <hip/hip_version.h>
#include <stdint.h>
#include <string.h>

#if !defined(HIP_VERSION_MAJOR) || HIP_VERSION_MAJOR < 5
#error "fe2o3 memory topology bindings require HIP 5 or newer headers"
#endif

#define FE2O3_ASSERT_FUNCTION_TYPE(function, type)                            \
  _Static_assert(_Generic(&(function), type: 1, default: 0),                 \
                 "unexpected " #function " signature")

typedef struct Fe2o3HipPhysicalDeviceIdentity {
  uint8_t uuid[16];
  char pci_bus_id[32];
} Fe2o3HipPhysicalDeviceIdentity;

typedef struct Fe2o3HipMemoryCapabilities {
  int managed_memory;
  int concurrent_managed_access;
  int pageable_memory_access;
  int virtual_memory_management;
} Fe2o3HipMemoryCapabilities;

typedef hipError_t (*Fe2o3DeviceGetUuid)(hipUUID *, hipDevice_t);
typedef hipError_t (*Fe2o3DeviceGetPCIBusId)(char *, int, int);
typedef hipError_t (*Fe2o3MallocManaged)(void **, size_t, unsigned int);
typedef hipError_t (*Fe2o3MemPrefetchAsync)(const void *, size_t, int,
                                             hipStream_t);
typedef hipError_t (*Fe2o3MemAdvise)(const void *, size_t, hipMemoryAdvise,
                                      int);
typedef hipError_t (*Fe2o3MemRangeGetAttribute)(void *, size_t,
                                                 hipMemRangeAttribute,
                                                 const void *, size_t);
typedef hipError_t (*Fe2o3MemAddressReserve)(void **, size_t, size_t, void *,
                                              unsigned long long);
typedef hipError_t (*Fe2o3MemAddressFree)(void *, size_t);
typedef hipError_t (*Fe2o3MemCreate)(hipMemGenericAllocationHandle_t *, size_t,
                                      const hipMemAllocationProp *,
                                      unsigned long long);
typedef hipError_t (*Fe2o3MemGetAllocationGranularity)(
    size_t *, const hipMemAllocationProp *, hipMemAllocationGranularity_flags);
typedef hipError_t (*Fe2o3MemMap)(void *, size_t, size_t,
                                   hipMemGenericAllocationHandle_t,
                                   unsigned long long);
typedef hipError_t (*Fe2o3MemSetAccess)(void *, size_t,
                                         const hipMemAccessDesc *, size_t);
typedef hipError_t (*Fe2o3MemGetAccess)(unsigned long long *,
                                         const hipMemLocation *, void *);
typedef hipError_t (*Fe2o3MemUnmap)(void *, size_t);
typedef hipError_t (*Fe2o3MemRelease)(hipMemGenericAllocationHandle_t);

_Static_assert(sizeof(hipUUID) == 16, "unexpected HIP UUID size");
_Static_assert(sizeof(hipMemGenericAllocationHandle_t) == sizeof(uintptr_t),
               "unexpected VMM handle size");
_Static_assert(hipMemAdviseSetReadMostly == 1,
               "unexpected read-mostly advice value");
_Static_assert(hipMemAdviseUnsetReadMostly == 2,
               "unexpected unset read-mostly advice value");
_Static_assert(hipMemAdviseSetPreferredLocation == 3,
               "unexpected preferred-location advice value");
_Static_assert(hipMemAdviseUnsetPreferredLocation == 4,
               "unexpected unset preferred-location advice value");
_Static_assert(hipMemAdviseSetAccessedBy == 5,
               "unexpected accessed-by advice value");
_Static_assert(hipMemAdviseUnsetAccessedBy == 6,
               "unexpected unset accessed-by advice value");
_Static_assert(hipMemAdviseSetCoarseGrain == 100,
               "unexpected coarse-grain advice value");
_Static_assert(hipMemAdviseUnsetCoarseGrain == 101,
               "unexpected unset coarse-grain advice value");

FE2O3_ASSERT_FUNCTION_TYPE(hipDeviceGetUuid, Fe2o3DeviceGetUuid);
FE2O3_ASSERT_FUNCTION_TYPE(hipDeviceGetPCIBusId, Fe2o3DeviceGetPCIBusId);
FE2O3_ASSERT_FUNCTION_TYPE(hipMallocManaged, Fe2o3MallocManaged);
FE2O3_ASSERT_FUNCTION_TYPE(hipMemPrefetchAsync, Fe2o3MemPrefetchAsync);
FE2O3_ASSERT_FUNCTION_TYPE(hipMemAdvise, Fe2o3MemAdvise);
FE2O3_ASSERT_FUNCTION_TYPE(hipMemRangeGetAttribute,
                           Fe2o3MemRangeGetAttribute);
FE2O3_ASSERT_FUNCTION_TYPE(hipMemAddressReserve, Fe2o3MemAddressReserve);
FE2O3_ASSERT_FUNCTION_TYPE(hipMemAddressFree, Fe2o3MemAddressFree);
FE2O3_ASSERT_FUNCTION_TYPE(hipMemCreate, Fe2o3MemCreate);
FE2O3_ASSERT_FUNCTION_TYPE(hipMemGetAllocationGranularity,
                           Fe2o3MemGetAllocationGranularity);
FE2O3_ASSERT_FUNCTION_TYPE(hipMemMap, Fe2o3MemMap);
FE2O3_ASSERT_FUNCTION_TYPE(hipMemSetAccess, Fe2o3MemSetAccess);
FE2O3_ASSERT_FUNCTION_TYPE(hipMemGetAccess, Fe2o3MemGetAccess);
FE2O3_ASSERT_FUNCTION_TYPE(hipMemUnmap, Fe2o3MemUnmap);
FE2O3_ASSERT_FUNCTION_TYPE(hipMemRelease, Fe2o3MemRelease);

static hipMemAllocationProp fe2o3_device_allocation_properties(int device_id) {
  hipMemAllocationProp properties;
  memset(&properties, 0, sizeof(properties));
  properties.type = hipMemAllocationTypePinned;
  properties.location.type = hipMemLocationTypeDevice;
  properties.location.id = device_id;
  properties.requestedHandleTypes = hipMemHandleTypeNone;
  return properties;
}

hipError_t fe2o3HipGetPhysicalDeviceIdentity(
    int device_id, Fe2o3HipPhysicalDeviceIdentity *identity) {
  hipUUID uuid;
  hipError_t status;
  if (identity == NULL) {
    return hipErrorInvalidValue;
  }
  memset(identity, 0, sizeof(*identity));
  status = hipDeviceGetUuid(&uuid, device_id);
  if (status != hipSuccess) {
    return status;
  }
  memcpy(identity->uuid, uuid.bytes, sizeof(identity->uuid));
  return hipDeviceGetPCIBusId(identity->pci_bus_id,
                              (int)sizeof(identity->pci_bus_id), device_id);
}

hipError_t fe2o3HipGetMemoryCapabilities(
    int device_id, Fe2o3HipMemoryCapabilities *capabilities) {
  hipError_t status;
  if (capabilities == NULL) {
    return hipErrorInvalidValue;
  }
  memset(capabilities, 0, sizeof(*capabilities));
#define FE2O3_QUERY(field, attribute)                                         \
  do {                                                                       \
    status = hipDeviceGetAttribute(&capabilities->field, attribute,          \
                                   device_id);                               \
    if (status != hipSuccess) {                                              \
      memset(capabilities, 0, sizeof(*capabilities));                        \
      return status;                                                         \
    }                                                                        \
  } while (0)
  FE2O3_QUERY(managed_memory, hipDeviceAttributeManagedMemory);
  FE2O3_QUERY(concurrent_managed_access,
              hipDeviceAttributeConcurrentManagedAccess);
  FE2O3_QUERY(pageable_memory_access, hipDeviceAttributePageableMemoryAccess);
  FE2O3_QUERY(virtual_memory_management,
              hipDeviceAttributeVirtualMemoryManagementSupported);
#undef FE2O3_QUERY
  return hipSuccess;
}

hipError_t fe2o3HipMallocManaged(void **pointer, size_t size) {
  return hipMallocManaged(pointer, size, hipMemAttachGlobal);
}

hipError_t fe2o3HipMemPrefetchAsync(const void *pointer, size_t size,
                                     int device_id, hipStream_t stream) {
  return hipMemPrefetchAsync(pointer, size, device_id, stream);
}

hipError_t fe2o3HipMemAdvise(const void *pointer, size_t size,
                              unsigned int advice, int device_id) {
  hipMemoryAdvise native_advice;
  switch (advice) {
  case 1:
    native_advice = hipMemAdviseSetReadMostly;
    break;
  case 2:
    native_advice = hipMemAdviseUnsetReadMostly;
    break;
  case 3:
    native_advice = hipMemAdviseSetPreferredLocation;
    break;
  case 4:
    native_advice = hipMemAdviseUnsetPreferredLocation;
    break;
  case 5:
    native_advice = hipMemAdviseSetAccessedBy;
    break;
  case 6:
    native_advice = hipMemAdviseUnsetAccessedBy;
    break;
  case 7:
    native_advice = hipMemAdviseSetCoarseGrain;
    break;
  case 8:
    native_advice = hipMemAdviseUnsetCoarseGrain;
    break;
  default:
    return hipErrorInvalidValue;
  }
  return hipMemAdvise(pointer, size, native_advice, device_id);
}

hipError_t fe2o3HipMemRangeGetLastPrefetchLocation(const void *pointer,
                                                    size_t size,
                                                    int *device_id) {
  if (device_id == NULL) {
    return hipErrorInvalidValue;
  }
  *device_id = 0;
  return hipMemRangeGetAttribute(device_id, sizeof(*device_id),
                                 hipMemRangeAttributeLastPrefetchLocation,
                                 pointer, size);
}

hipError_t fe2o3HipMemAddressReserve(void **pointer, size_t size,
                                      size_t alignment) {
  return hipMemAddressReserve(pointer, size, alignment, NULL, 0);
}

hipError_t fe2o3HipMemAddressFree(void *pointer, size_t size) {
  return hipMemAddressFree(pointer, size);
}

hipError_t fe2o3HipMemGetAllocationGranularity(size_t *granularity,
                                                int device_id) {
  hipMemAllocationProp properties =
      fe2o3_device_allocation_properties(device_id);
  return hipMemGetAllocationGranularity(
      granularity, &properties, hipMemAllocationGranularityMinimum);
}

hipError_t fe2o3HipMemCreate(uintptr_t *handle, size_t size, int device_id) {
  hipMemGenericAllocationHandle_t native_handle = NULL;
  hipMemAllocationProp properties;
  hipError_t status;
  if (handle == NULL) {
    return hipErrorInvalidValue;
  }
  *handle = 0;
  properties = fe2o3_device_allocation_properties(device_id);
  status = hipMemCreate(&native_handle, size, &properties, 0);
  if (status == hipSuccess) {
    *handle = (uintptr_t)native_handle;
  }
  return status;
}

hipError_t fe2o3HipMemMap(void *pointer, size_t size, uintptr_t handle) {
  return hipMemMap(pointer, size, 0,
                   (hipMemGenericAllocationHandle_t)handle, 0);
}

hipError_t fe2o3HipMemSetAccess(void *pointer, size_t size, int device_id,
                                 unsigned int access) {
  hipMemAccessDesc descriptor;
  memset(&descriptor, 0, sizeof(descriptor));
  descriptor.location.type = hipMemLocationTypeDevice;
  descriptor.location.id = device_id;
  switch (access) {
  case 1:
    descriptor.flags = hipMemAccessFlagsProtRead;
    break;
  case 2:
    descriptor.flags = hipMemAccessFlagsProtReadWrite;
    break;
  default:
    return hipErrorInvalidValue;
  }
  return hipMemSetAccess(pointer, size, &descriptor, 1);
}

hipError_t fe2o3HipMemGetAccess(unsigned int *access, void *pointer,
                                 int device_id) {
  hipMemLocation location;
  unsigned long long native_access = 0;
  hipError_t status;
  if (access == NULL) {
    return hipErrorInvalidValue;
  }
  *access = 0;
  memset(&location, 0, sizeof(location));
  location.type = hipMemLocationTypeDevice;
  location.id = device_id;
  status = hipMemGetAccess(&native_access, &location, pointer);
  if (status != hipSuccess) {
    return status;
  }
  if (native_access == hipMemAccessFlagsProtRead) {
    *access = 1;
  } else if (native_access == hipMemAccessFlagsProtReadWrite) {
    *access = 2;
  } else if (native_access != hipMemAccessFlagsProtNone) {
    return hipErrorInvalidValue;
  }
  return hipSuccess;
}

hipError_t fe2o3HipMemUnmap(void *pointer, size_t size) {
  return hipMemUnmap(pointer, size);
}

hipError_t fe2o3HipMemRelease(uintptr_t handle) {
  return hipMemRelease((hipMemGenericAllocationHandle_t)handle);
}
