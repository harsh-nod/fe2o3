#include "llvm/ADT/StringRef.h"
#include "llvm/Support/Error.h"
#include "llvm/Support/FileOutputBuffer.h"

#include <cstddef>
#include <memory>

using CreateResult = llvm::Expected<std::unique_ptr<llvm::FileOutputBuffer>>;

extern "C" CreateResult fe2o3RealFileOutputBufferCreate(
    llvm::StringRef path, std::size_t size,
    unsigned
        flags) asm("__real__ZN4llvm16FileOutputBuffer6createENS_9StringRefEmj");

extern "C" CreateResult fe2o3WrapFileOutputBufferCreate(
    llvm::StringRef path, std::size_t size,
    unsigned
        flags) asm("__wrap__ZN4llvm16FileOutputBuffer6createENS_9StringRefEmj");

extern "C" CreateResult fe2o3WrapFileOutputBufferCreate(llvm::StringRef path,
                                                        std::size_t size,
                                                        unsigned flags) {
  return fe2o3RealFileOutputBufferCreate(
      path, size, flags | llvm::FileOutputBuffer::F_mmap);
}
