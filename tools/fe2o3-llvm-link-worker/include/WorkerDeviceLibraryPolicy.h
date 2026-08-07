#ifndef FE2O3_LLVM_LINK_WORKER_DEVICE_LIBRARY_POLICY_H
#define FE2O3_LLVM_LINK_WORKER_DEVICE_LIBRARY_POLICY_H

#include "WorkerProtocol.h"

#include "llvm/ADT/ArrayRef.h"
#include "llvm/ADT/StringRef.h"
#include "llvm/Support/Error.h"

#include <array>
#include <cstdint>
#include <string>
#include <vector>

namespace fe2o3::worker {

inline constexpr uint64_t MaxDeviceLibraryFileBytes = 16 * 1024 * 1024;

struct PinnedDeviceLibraryFile {
  std::string Basename;
  std::array<uint8_t, 32> Digest{};
  uint64_t MaxBytes = MaxDeviceLibraryFileBytes;
};

struct Gfx942DeviceLibraryPolicy {
  std::string Directory;
  std::vector<PinnedDeviceLibraryFile> Files;
};

bool isSupportedGfx942OcmlImport(llvm::StringRef Symbol);
bool isOcmlImportNamespace(llvm::StringRef Symbol);

llvm::Expected<Gfx942DeviceLibraryPolicy> measuredGfx942DeviceLibraryPolicy();

llvm::Expected<std::vector<Input>>
loadGfx942DeviceLibraries(llvm::ArrayRef<std::string> Imports,
                          const Gfx942DeviceLibraryPolicy &Policy);

} // namespace fe2o3::worker

#endif
