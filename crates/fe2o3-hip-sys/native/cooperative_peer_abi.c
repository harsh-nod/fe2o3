#include <hip/hip_runtime_api.h>
#include <hip/hip_version.h>

#if !defined(HIP_VERSION_MAJOR) || HIP_VERSION_MAJOR < 5
#error "fe2o3 cooperative/peer bindings require HIP 5 or newer headers"
#endif

#define FE2O3_ASSERT_FUNCTION_TYPE(function, type)                             \
  _Static_assert(_Generic(&(function), type: 1, default: 0),                  \
                 "unexpected " #function " signature")

typedef hipError_t (*Fe2o3DeviceGetAttribute)(int *, hipDeviceAttribute_t,
                                               int);
typedef hipError_t (*Fe2o3DeviceCanAccessPeer)(int *, int, int);
typedef hipError_t (*Fe2o3DeviceEnablePeerAccess)(int, unsigned int);
typedef hipError_t (*Fe2o3DeviceDisablePeerAccess)(int);
typedef hipError_t (*Fe2o3ModuleLaunchCooperativeKernel)(
    hipFunction_t, unsigned int, unsigned int, unsigned int, unsigned int,
    unsigned int, unsigned int, unsigned int, hipStream_t, void **);
typedef hipError_t (*Fe2o3LaunchCooperativeKernel)(
    const void *, dim3, dim3, void **, unsigned int, hipStream_t);

_Static_assert(hipDeviceAttributeCooperativeLaunch == 10,
               "unexpected cooperative-launch attribute value");
_Static_assert(hipDeviceAttributeCooperativeMultiDeviceLaunch == 11,
               "unexpected multi-device cooperative attribute value");
_Static_assert(hipErrorPeerAccessAlreadyEnabled == 704,
               "unexpected peer-enabled error value");
_Static_assert(hipErrorPeerAccessNotEnabled == 705,
               "unexpected peer-disabled error value");
_Static_assert(hipErrorCooperativeLaunchTooLarge == 720,
               "unexpected cooperative-launch error value");
_Static_assert(sizeof(hipDeviceAttribute_t) == sizeof(int),
               "unexpected device-attribute representation");
_Static_assert(sizeof(dim3) == 12, "unexpected dim3 size");
_Static_assert(_Alignof(dim3) == 4, "unexpected dim3 alignment");

FE2O3_ASSERT_FUNCTION_TYPE(hipDeviceGetAttribute, Fe2o3DeviceGetAttribute);
FE2O3_ASSERT_FUNCTION_TYPE(hipDeviceCanAccessPeer, Fe2o3DeviceCanAccessPeer);
FE2O3_ASSERT_FUNCTION_TYPE(hipDeviceEnablePeerAccess,
                           Fe2o3DeviceEnablePeerAccess);
FE2O3_ASSERT_FUNCTION_TYPE(hipDeviceDisablePeerAccess,
                           Fe2o3DeviceDisablePeerAccess);
FE2O3_ASSERT_FUNCTION_TYPE(hipModuleLaunchCooperativeKernel,
                           Fe2o3ModuleLaunchCooperativeKernel);
FE2O3_ASSERT_FUNCTION_TYPE(hipLaunchCooperativeKernel,
                           Fe2o3LaunchCooperativeKernel);
