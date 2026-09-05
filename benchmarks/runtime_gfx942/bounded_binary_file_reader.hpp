#ifndef FE2O3_RUNTIME_GFX942_BOUNDED_BINARY_FILE_READER_HPP
#define FE2O3_RUNTIME_GFX942_BOUNDED_BINARY_FILE_READER_HPP

#include <cstddef>
#include <fstream>
#include <ios>
#include <utility>
#include <vector>

namespace fe2o3::r26 {

enum class BoundedBinaryFileReadStatus {
  Success,
  OpenFailed,
  InvalidSize,
  SeekFailed,
  ReadFailed,
  ChangedOrReadFailed,
};

inline BoundedBinaryFileReadStatus
read_bounded_binary_file(const char *path, std::streamoff maximum_byte_count,
                         std::vector<char> *output) {
  std::ifstream input(path, std::ios::binary | std::ios::ate);
  if (!input)
    return BoundedBinaryFileReadStatus::OpenFailed;

  const std::streamoff byte_count = input.tellg();
  if (byte_count <= 0 || byte_count > maximum_byte_count)
    return BoundedBinaryFileReadStatus::InvalidSize;

  input.seekg(0, std::ios::beg);
  if (!input)
    return BoundedBinaryFileReadStatus::SeekFailed;

  std::vector<char> bytes(static_cast<std::size_t>(byte_count));
  const auto expected_byte_count = static_cast<std::streamsize>(byte_count);
  input.read(bytes.data(), expected_byte_count);
  if (!input || input.gcount() != expected_byte_count)
    return BoundedBinaryFileReadStatus::ReadFailed;

  char trailing_byte = 0;
  input.read(&trailing_byte, 1);
  if (input.gcount() != 0 || !input.eof() || input.bad())
    return BoundedBinaryFileReadStatus::ChangedOrReadFailed;

  *output = std::move(bytes);
  return BoundedBinaryFileReadStatus::Success;
}

} // namespace fe2o3::r26

#endif
