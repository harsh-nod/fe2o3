#include "WorkerPipeline.h"
#include "WorkerProtocol.h"

#include "llvm/Support/Error.h"

#include <array>
#include <cstdint>
#include <cstdio>
#include <vector>

#ifndef FE2O3_LLVM_BUILD_ID
#error "FE2O3_LLVM_BUILD_ID must be supplied by CMake"
#endif
#ifndef FE2O3_WORKER_BUILD_ID
#error "FE2O3_WORKER_BUILD_ID must be supplied by CMake"
#endif

using namespace fe2o3::worker;

namespace {

bool readBoundedStdin(std::vector<uint8_t> &Bytes) {
  std::array<uint8_t, 16 * 1024> Buffer{};
  while (true) {
    size_t Count = std::fread(Buffer.data(), 1, Buffer.size(), stdin);
    if (Count != 0) {
      if (Bytes.size() > MaxRequestBytes - Count)
        return false;
      Bytes.insert(Bytes.end(), Buffer.begin(), Buffer.begin() + Count);
    }
    if (Count != Buffer.size())
      return std::feof(stdin) != 0 && std::ferror(stdin) == 0;
  }
}

int writeResponse(Response ResponseValue) {
  auto Encoded = encodeResponse(std::move(ResponseValue));
  if (!Encoded) {
    llvm::consumeError(Encoded.takeError());
    return 70;
  }
  if (std::fwrite(Encoded->data(), 1, Encoded->size(), stdout) !=
          Encoded->size() ||
      std::fflush(stdout) != 0)
    return 74;
  return 0;
}

} // namespace

int main(int ArgumentCount, char **) {
  if (ArgumentCount != 1)
    return 64;
  std::vector<uint8_t> Bytes;
  Bytes.reserve(64 * 1024);
  if (!readBoundedStdin(Bytes)) {
    return writeResponse({{},
                          {},
                          FE2O3_WORKER_BUILD_ID,
                          Stage::Decode,
                          {"worker request exceeds byte bound"},
                          std::nullopt});
  }
  auto RequestValue = decodeRequest(Bytes);
  if (!RequestValue) {
    return writeResponse({{},
                          {},
                          FE2O3_WORKER_BUILD_ID,
                          Stage::Decode,
                          {errorToDiagnostic(RequestValue.takeError())},
                          std::nullopt});
  }
  return writeResponse(execute(*RequestValue));
}
