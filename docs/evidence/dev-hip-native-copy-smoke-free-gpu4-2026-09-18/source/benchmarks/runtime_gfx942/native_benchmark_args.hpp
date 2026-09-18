#ifndef FE2O3_RUNTIME_GFX942_NATIVE_BENCHMARK_ARGS_HPP
#define FE2O3_RUNTIME_GFX942_NATIVE_BENCHMARK_ARGS_HPP

#include <cstddef>
#include <cstdint>
#include <limits>

namespace fe2o3::runtime_gfx942 {

struct WorkloadShape {
  std::size_t bytes = 0;
  std::size_t depth = 0;
  std::size_t warmups = 0;
  std::size_t samples = 0;
  std::size_t total_iterations = 0;
  std::size_t total_depth = 0;
  std::size_t transfer_bytes = 0;
};

namespace detail {

inline int digit_value(char character) {
  if (character >= '0' && character <= '9')
    return character - '0';
  if (character >= 'a' && character <= 'f')
    return character - 'a' + 10;
  if (character >= 'A' && character <= 'F')
    return character - 'A' + 10;
  return -1;
}

inline bool parse_unsigned(const char *text, unsigned base,
                           std::uintmax_t maximum, std::uintmax_t *result) {
  if (text == nullptr || result == nullptr || text[0] == '\0')
    return false;

  const char *cursor = text;
  if (base == 0) {
    if (cursor[0] == '0' && (cursor[1] == 'x' || cursor[1] == 'X')) {
      base = 16;
      cursor += 2;
      if (*cursor == '\0')
        return false;
    } else {
      base = 10;
    }
  }

  std::uintmax_t value = 0;
  for (; *cursor != '\0'; ++cursor) {
    const int digit = digit_value(*cursor);
    if (digit < 0 || static_cast<unsigned>(digit) >= base)
      return false;
    const auto unsigned_digit = static_cast<std::uintmax_t>(digit);
    if (value > (maximum - unsigned_digit) / base)
      return false;
    value = value * base + unsigned_digit;
  }
  *result = value;
  return true;
}

} // namespace detail

inline bool parse_size(const char *text, std::size_t *result) {
  std::uintmax_t parsed = 0;
  if (result == nullptr ||
      !detail::parse_unsigned(text, 10, std::numeric_limits<std::size_t>::max(),
                              &parsed))
    return false;
  *result = static_cast<std::size_t>(parsed);
  return true;
}

inline bool parse_device_index(const char *text, int *result) {
  std::uintmax_t parsed = 0;
  if (result == nullptr ||
      !detail::parse_unsigned(
          text, 10,
          static_cast<std::uintmax_t>(std::numeric_limits<int>::max()),
          &parsed))
    return false;
  *result = static_cast<int>(parsed);
  return true;
}

inline bool parse_unique_id(const char *text, std::uint64_t *result) {
  std::uintmax_t parsed = 0;
  if (result == nullptr ||
      !detail::parse_unsigned(
          text, 0, std::numeric_limits<std::uint64_t>::max(), &parsed))
    return false;
  *result = static_cast<std::uint64_t>(parsed);
  return true;
}

inline bool checked_add(std::size_t left, std::size_t right,
                        std::size_t *result) {
  if (result == nullptr ||
      left > std::numeric_limits<std::size_t>::max() - right)
    return false;
  *result = left + right;
  return true;
}

inline bool checked_multiply(std::size_t left, std::size_t right,
                             std::size_t *result) {
  if (result == nullptr ||
      (left != 0 && right > std::numeric_limits<std::size_t>::max() / left))
    return false;
  *result = left * right;
  return true;
}

inline bool parse_workload_shape(const char *bytes_text, const char *depth_text,
                                 const char *warmups_text,
                                 const char *samples_text,
                                 std::size_t device_count,
                                 WorkloadShape *result) {
  WorkloadShape parsed;
  if (result == nullptr || device_count == 0 ||
      !parse_size(bytes_text, &parsed.bytes) ||
      !parse_size(depth_text, &parsed.depth) ||
      !parse_size(warmups_text, &parsed.warmups) ||
      !parse_size(samples_text, &parsed.samples) || parsed.bytes == 0 ||
      parsed.depth == 0 || parsed.samples == 0 ||
      !checked_add(parsed.warmups, parsed.samples, &parsed.total_iterations) ||
      !checked_multiply(parsed.depth, device_count, &parsed.total_depth) ||
      !checked_multiply(parsed.bytes, parsed.total_depth,
                        &parsed.transfer_bytes))
    return false;
  *result = parsed;
  return true;
}

} // namespace fe2o3::runtime_gfx942

#endif
