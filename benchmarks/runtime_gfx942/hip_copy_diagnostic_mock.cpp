// CPU-only adapter fixture. Never link this file into hardware benchmarks.
#include <hip/hip_runtime_api.h>

#include <algorithm>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <map>

struct ihipStream_t {
  void *destination = nullptr;
  const void *source = nullptr;
  size_t bytes = 0;
  hipMemcpyKind kind = hipMemcpyHostToDevice;
};

namespace {
bool mode(const char *name) {
  const char *value = std::getenv("FE2O3_HIP_COPY_TEST_CASE");
  return value != nullptr && std::strcmp(value, name) == 0;
}

void require(bool condition) {
  if (!condition) {
    std::fputs("MOCK_PROTOCOL_ERROR\n", stderr);
    std::abort();
  }
}

struct Allocation {
  size_t bytes;
  unsigned role; // 0: device, 1: upload, 2: download, 3: allocator exercise.
};
std::map<const void *, Allocation> allocations;
unsigned host_allocations = 0;
size_t copies = 0;
ihipStream_t *live_stream = nullptr;
void *download = nullptr;

hipError_t allocate(void **pointer, size_t bytes, unsigned role) {
  std::fprintf(stderr, "MOCK allocate role=%u\n", role);
  if (mode("allocation_error") ||
      (role == 2 && mode("download_allocation_error")) ||
      (role == 0 && mode("device_allocation_error")))
    return hipErrorOutOfMemory;
  *pointer = std::malloc(bytes);
  require(*pointer != nullptr);
  std::memset(*pointer, 0xa5, bytes);
  require(allocations.emplace(*pointer, Allocation{bytes, role}).second);
  if (role == 2)
    download = *pointer;
  return hipSuccess;
}

hipError_t release(void *pointer, unsigned role, const char *failure) {
  require(live_stream != nullptr && live_stream->destination == nullptr);
  require(allocations.count(pointer) == 1 &&
          allocations.at(pointer).role == role);
  if (mode(failure))
    return hipErrorInvalidValue;
  allocations.erase(pointer);
  std::free(pointer);
  return hipSuccess;
}
} // namespace

extern "C" {
const char *hipGetErrorString(hipError_t) { return "mock HIP error"; }

hipError_t hipSetDevice(int device) {
  std::fputs("MOCK select\n", stderr);
  require(device == 1);
  return mode("select_error") ? hipErrorInvalidDevice : hipSuccess;
}

hipError_t hipDeviceGetUuid(hipUUID *uuid, hipDevice_t device) {
  require(device == 1);
  if (mode("uuid_error"))
    return hipErrorInvalidValue;
  std::memcpy(uuid->bytes,
              mode("uuid") ? "0000000000000001" : "ab83d2ffef0d3cdf", 16);
  return hipSuccess;
}

hipError_t hipGetDeviceProperties(hipDeviceProp_t *properties, int device) {
  require(device == 1);
  if (mode("properties_error"))
    return hipErrorInvalidValue;
  const char *target = mode("target")           ? "gfx950:xnack-"
                       : mode("target_prefix")  ? "gfx942x:xnack-"
                       : mode("xnack_token")    ? "gfx942:xnack-other"
                       : mode("xnack_conflict") ? "gfx942:xnack-:xnack+"
                       : mode("xnack")          ? "gfx942:xnack+"
                                                : "gfx942:sramecc+:xnack-";
  std::strcpy(properties->gcnArchName, target);
  return hipSuccess;
}

hipError_t hipStreamCreateWithFlags(hipStream_t *stream, unsigned flags) {
  require(flags == hipStreamNonBlocking && live_stream == nullptr);
  std::fputs("MOCK stream\n", stderr);
  if (mode("stream_error"))
    return hipErrorInvalidValue;
  *stream = live_stream = new ihipStream_t;
  return hipSuccess;
}

hipError_t hipHostMalloc(void **pointer, size_t bytes, unsigned flags) {
  require(flags == hipHostMallocDefault && host_allocations < 2);
  return allocate(pointer, bytes, ++host_allocations);
}

hipError_t hipMalloc(void **pointer, size_t bytes) {
  require(host_allocations == 2);
  return allocate(pointer, bytes, 0);
}

hipError_t hipMemcpyAsync(void *destination, const void *source, size_t bytes,
                          hipMemcpyKind kind, hipStream_t stream) {
  require(stream == live_stream && stream->destination == nullptr);
  require(allocations.count(source) == 1 &&
          allocations.count(destination) == 1);
  require(allocations.at(source).bytes == bytes &&
          allocations.at(destination).bytes == bytes);
  const bool upload = kind == hipMemcpyHostToDevice;
  require(upload || kind == hipMemcpyDeviceToHost);
  require(upload == (copies % 2 == 0));
  require(allocations.at(source).role == (upload ? 1u : 0u));
  require(allocations.at(destination).role == (upload ? 0u : 2u));
  std::fprintf(stderr, "MOCK copy %s\n", upload ? "h2d" : "d2h");
  if ((upload && mode("submit_error")) || (!upload && mode("d2h_submit_error")))
    return hipErrorInvalidValue;
  if (upload) {
    const auto expected =
        static_cast<unsigned char>(((copies / 2) * 67 + 1) % 251 + 1);
    const auto *input = static_cast<const unsigned char *>(source);
    const auto *poisoned = static_cast<const unsigned char *>(download);
    require(std::all_of(input, input + bytes,
                        [expected](auto b) { return b == expected; }));
    require(std::all_of(poisoned, poisoned + bytes,
                        [expected](auto b) { return b == (expected ^ 0xff); }));
  }
  *stream = {destination, source, bytes, kind};
  ++copies;
  return hipSuccess;
}

hipError_t hipStreamSynchronize(hipStream_t stream) {
  require(stream == live_stream);
  std::fputs("MOCK wait\n", stderr);
  if (stream->destination == nullptr)
    return hipSuccess;
  const bool upload = stream->kind == hipMemcpyHostToDevice;
  if ((upload && mode("wait_error")) || (!upload && mode("d2h_wait_error")))
    return hipErrorInvalidValue;
  std::memcpy(stream->destination, stream->source, stream->bytes);
  if (!upload) {
    auto *output = static_cast<unsigned char *>(stream->destination);
    if (mode("corrupt_first"))
      output[0] ^= 1;
    if (mode("corrupt_middle"))
      output[stream->bytes / 2] ^= 1;
    if (mode("corrupt_last"))
      output[stream->bytes - 1] ^= 1;
  }
  *stream = {};
  return hipSuccess;
}

hipError_t hipFree(void *pointer) {
  std::fputs("MOCK device_free\n", stderr);
  return release(pointer, 0, "device_free_error");
}

hipError_t hipHostFree(void *pointer) {
  std::fputs("MOCK host_free\n", stderr);
  require(allocations.count(pointer) == 1);
  const auto role = allocations.at(pointer).role;
  require(role == 1 || role == 2);
  if (role == 2 && mode("download_free_error"))
    return hipErrorInvalidValue;
  return release(pointer, role, "host_free_error");
}

hipError_t hipStreamDestroy(hipStream_t stream) {
  std::fputs("MOCK destroy\n", stderr);
  require(stream == live_stream && allocations.empty() &&
          stream->destination == nullptr);
  if (mode("destroy_error"))
    return hipErrorInvalidValue;
  delete stream;
  live_stream = nullptr;
  if (mode("late_output_error"))
    require(std::freopen("/dev/full", "w", stdout) != nullptr);
  return hipSuccess;
}

hipError_t hipMallocAsync(void **pointer, size_t bytes, hipStream_t stream) {
  require(stream == live_stream && stream->destination == nullptr);
  std::fputs("MOCK pool_allocate\n", stderr);
  return allocate(pointer, bytes, 3);
}

hipError_t hipFreeAsync(void *pointer, hipStream_t stream) {
  require(stream == live_stream);
  std::fputs("MOCK pool_free\n", stderr);
  return release(pointer, 3, "pool_free_error");
}
}
