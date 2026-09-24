// Exercise the actual producer callbacks without linking or calling a GPU
// runtime.
#include <algorithm>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <utility>
#include <vector>

static void require(bool condition) {
  if (!condition) {
    std::fputs("persistent-hot callback assertion failed\n", stderr);
    std::abort();
  }
}

struct PendingCopy {
  void *destination = nullptr;
  const void *source = nullptr;
  std::size_t bytes = 0;
};

static std::vector<PendingCopy> pending;
static std::vector<std::vector<std::uint8_t>> sources, destinations;
static std::vector<std::pair<char, std::size_t>> events;
static std::vector<std::pair<std::size_t, bool>> readbacks;
static bool timed = false;
static constexpr std::size_t copy_bytes = 7, allocation_bytes = copy_bytes + 64;

static void enqueue(void *destination, const void *source, std::size_t bytes,
                    std::size_t id) {
  require(id > 0 && id <= pending.size());
  const auto slot = id - 1;
  require(pending[slot].destination == nullptr);
  if (timed) {
    require(bytes == copy_bytes);
    require(source == sources[slot].data() + 32);
    require(destination == destinations[slot].data() + 32);
    events.emplace_back('e', slot);
  }
  pending[slot] = {destination, source, bytes};
}

static void inspect_readback(void *destination, const void *source,
                             std::size_t bytes) {
  for (std::size_t slot = 0; slot < sources.size(); ++slot) {
    for (bool is_source : {true, false}) {
      const auto &buffer = is_source ? sources[slot] : destinations[slot];
      if (source == buffer.data()) {
        require(bytes == allocation_bytes);
        readbacks.emplace_back(slot, is_source);
      }
    }
  }
  std::memcpy(destination, source, bytes);
}

static void complete(std::size_t id) {
  require(id > 0 && id <= pending.size());
  const auto slot = id - 1;
  require(pending[slot].destination != nullptr);
  if (timed) {
    // The first wait cannot occur until every slot has been enqueued.
    if (events.size() == pending.size())
      for (const auto &copy : pending)
        require(copy.destination != nullptr);
    events.emplace_back('w', slot);
  }
  const auto copy = pending[slot];
  if (timed)
    std::memcpy(copy.destination, copy.source, copy.bytes);
  else
    inspect_readback(copy.destination, copy.source, copy.bytes);
  pending[slot] = {};
}

#ifdef TEST_HIP
#include <hip/hip_runtime.h>

static hipError_t mock_set_device(int device) {
  require(device == 0 || device == 1);
  return hipSuccess;
}
static hipError_t mock_memcpy(void *destination, const void *source,
                              std::size_t bytes, hipMemcpyKind kind) {
  require(!timed && bytes == allocation_bytes);
  require(kind == hipMemcpyHostToDevice || kind == hipMemcpyDeviceToHost);
  if (kind == hipMemcpyDeviceToHost)
    inspect_readback(destination, source, bytes);
  else
    std::memcpy(destination, source, bytes);
  return hipSuccess;
}
static hipError_t mock_peer_copy(void *destination, int destination_device,
                                 const void *source, int source_device,
                                 std::size_t bytes, hipStream_t stream) {
  require(timed && source_device == 0 && destination_device == 1);
  enqueue(destination, source, bytes, reinterpret_cast<std::uintptr_t>(stream));
  return hipSuccess;
}
static hipError_t mock_synchronize(hipStream_t stream) {
  complete(reinterpret_cast<std::uintptr_t>(stream));
  return hipSuccess;
}
static const char *mock_error_string(hipError_t) { return "mock HIP failure"; }

#define hipSetDevice mock_set_device
#define hipMemcpy mock_memcpy
#define hipMemcpyPeerAsync mock_peer_copy
#define hipStreamSynchronize mock_synchronize
#define hipGetErrorString mock_error_string
#define main unused_producer_main
#include "xgmi_peer_hip.cpp"
#undef main

#else
#include <hsa/hsa.h>
#include <hsa/hsa_ext_amd.h>

static std::vector<hsa_signal_value_t> signal_values;
static hsa_status_t mock_async_copy(void *destination, hsa_agent_t,
                                    const void *source, hsa_agent_t,
                                    std::size_t bytes, uint32_t count,
                                    const hsa_signal_t *dependencies,
                                    hsa_signal_t signal) {
  require(count == 0 && dependencies == nullptr);
  require(signal.handle > 0 && signal.handle <= signal_values.size());
  require(signal_values[signal.handle - 1] == 1);
  enqueue(destination, source, bytes, signal.handle);
  return HSA_STATUS_SUCCESS;
}
static hsa_signal_value_t mock_wait(hsa_signal_t signal,
                                    hsa_signal_condition_t condition,
                                    hsa_signal_value_t compare, uint64_t,
                                    hsa_wait_state_t state) {
  require(condition == HSA_SIGNAL_CONDITION_LT && compare == 1 &&
          state == HSA_WAIT_STATE_BLOCKED);
  complete(signal.handle);
  signal_values[signal.handle - 1] = 0;
  return 0;
}
static void mock_reset(hsa_signal_t signal, hsa_signal_value_t value) {
  require(signal.handle > 0 && signal.handle <= signal_values.size());
  require(signal_values[signal.handle - 1] == 0 && value == 1);
  signal_values[signal.handle - 1] = value;
}
static hsa_status_t mock_status_string(hsa_status_t, const char **message) {
  *message = "mock HSA failure";
  return HSA_STATUS_SUCCESS;
}

#define hsa_amd_memory_async_copy mock_async_copy
#define hsa_signal_wait_scacquire mock_wait
#define hsa_signal_store_screlease mock_reset
#define hsa_status_string mock_status_string
#define main unused_producer_main
#include "xgmi_peer_hsa.cpp"
#undef main
#endif

int main() {
  for (std::size_t depth : {1, 16, 32}) {
    for (std::size_t direction = 0; direction < 2; ++direction) {
      for (int corrupt :
           {-1, 0, static_cast<int>(depth / 2), static_cast<int>(depth - 1)}) {
        for (bool corrupt_source : {true, false}) {
          sources.assign(depth, std::vector<std::uint8_t>(allocation_bytes, 0));
          destinations = sources;
          pending.assign(depth, {});
          events.clear();
          readbacks.clear();
          peer::WorkloadShape shape;
          const auto depth_text = std::to_string(depth);
          require(peer::parse_workload_shape("7", depth_text.c_str(), "2", "3",
                                             1, &shape));
          peer::PeerBenchmarkControls controls;
          require(
              peer::parse_peer_controls("--persistent-hot", shape, &controls));
          DirectionBuffers buffers;
#ifdef TEST_HIP
          buffers.host.resize(allocation_bytes);
#else
          std::vector<std::vector<std::uint8_t>> uploads(
              depth, std::vector<std::uint8_t>(allocation_bytes));
          auto downloads = uploads;
          signal_values.assign(depth, 1);
#endif
          for (std::size_t slot = 0; slot < depth; ++slot) {
            buffers.source.push_back(sources[slot].data());
            buffers.destination.push_back(destinations[slot].data());
#ifdef TEST_HIP
            buffers.streams.push_back(reinterpret_cast<hipStream_t>(slot + 1));
#else
            buffers.upload.push_back(uploads[slot].data());
            buffers.download.push_back(downloads[slot].data());
            buffers.signals.push_back(hsa_signal_t{slot + 1});
#endif
          }
#ifdef TEST_HIP
          require(prepare_persistent_hot(buffers, 0, 1, controls, copy_bytes,
                                         direction));
#else
          require(prepare_persistent_direction(
              buffers, {1}, {2}, {3}, copy_bytes, controls, direction));
#endif
          require(readbacks.empty());
          for (std::size_t slot = 0; slot < depth; ++slot) {
            const auto pattern =
                peer::peer_pattern(controls.hot_pattern_round, slot, direction);
            require(peer::validate_peer_guarded(
                sources[slot].data(), allocation_bytes, copy_bytes,
                peer::peer_source_canary(direction), pattern));
            require(peer::validate_peer_guarded(
                destinations[slot].data(), allocation_bytes, copy_bytes,
                peer::peer_destination_canary(direction), pattern ^ 0xff));
          }
          // Two batches catch forgotten HSA signal reset and stale per-slot
          // reuse.
          for (int batch = 0; batch < 2; ++batch) {
            events.clear();
            timed = true;
#ifdef TEST_HIP
            copy_persistent_hot(buffers, 0, 1, controls, copy_bytes);
#else
            copy_persistent_direction(buffers, {1}, {2}, copy_bytes, controls);
#endif
            timed = false;
            std::vector<std::pair<char, std::size_t>> expected;
            for (char kind : {'e', 'w'})
              for (std::size_t slot = 0; slot < depth; ++slot)
                expected.emplace_back(kind, slot);
            require(events == expected && readbacks.empty());
          }
          if (corrupt >= 0)
            (corrupt_source ? sources : destinations)[corrupt][32] ^= 0xff;
#ifdef TEST_HIP
          const bool valid = validate_persistent_hot(buffers, 0, 1, controls,
                                                     copy_bytes, direction);
#else
          const bool valid = validate_persistent_direction(
              buffers, {1}, {2}, {3}, copy_bytes, controls, direction);
#endif
          require(valid == (corrupt == -1));
          std::vector<std::pair<std::size_t, bool>> expected;
          for (std::size_t slot = 0; slot < depth; ++slot)
            for (bool source : {true, false})
              expected.emplace_back(slot, source);
          require(readbacks == expected);
        }
      }
    }
  }
  std::puts("actual hot callbacks: pass (mock APIs, no GPU runtime linked)");
}
