/* SPDX-License-Identifier: GPL-3.0-or-later
   Exact frozen 776-byte target checkpoint codec. Parsing yields inert facts,
   not a queue handle, permission or source provenance. No dynamic allocation. */
#ifndef GDB_AMD_DBGAPI_ONE_STOP_CHECKPOINT_V1_H
#define GDB_AMD_DBGAPI_ONE_STOP_CHECKPOINT_V1_H
#include <array>
#include <cstddef>
#include <cstdint>
#include <limits>

namespace amd_owned_one_stop_v1 {
using checkpoint_bytes = std::array<std::uint8_t, 776>;
using digest_bytes = std::array<std::uint8_t, 32>;
inline constexpr digest_bytes fixed_object_sha {{
  0xcd,0x3d,0xaa,0xb7,0x6d,0xb3,0x47,0x10,0x41,0x66,0xc8,0x98,0xa5,0x8d,0xcc,0x29,
  0xc1,0x97,0xee,0x1e,0xac,0x8b,0x25,0x8b,0x10,0xa2,0xc7,0x93,0xe8,0x77,0x9c,0x87
}};
inline constexpr digest_bytes fixed_source_sha {{
  0xe5,0x67,0xe9,0xb4,0x14,0xf0,0x08,0x22,0xfb,0xf6,0x43,0x69,0x84,0xa4,0x52,0xd9,
  0x94,0x06,0x30,0xec,0xce,0xc4,0x0a,0x0e,0x86,0xb2,0xc5,0xc1,0x91,0x59,0x80,0x17
}};
/* Actual fixed object's zero-filled 12288-byte image, using the three
   independently checked target-loader segments. NOT the original ELF digest. */
inline constexpr digest_bytes fixed_image_sha {{
  0x2b,0x3a,0xe7,0xaa,0xfb,0x41,0x9b,0xf6,0x58,0x2b,0x72,0xaa,0x8a,0xf2,0x2f,0x46,
  0x85,0xa8,0xa4,0xf6,0x81,0x1c,0x30,0x5c,0xdd,0xfe,0x5a,0x1f,0x84,0x21,0xfe,0xc6
}};
inline constexpr digest_bytes fixed_trap_sha {{
  0x4f,0xfe,0xa8,0x93,0xee,0x53,0xe0,0x18,0xa6,0x29,0xc1,0x92,0x54,0x85,0x57,0x21,
  0x88,0x24,0x44,0x51,0x71,0x55,0x25,0x1f,0x1f,0x45,0xdd,0x35,0x19,0xbc,0x81,0xfe
}};
inline bool checked_sum (std::uint64_t a, std::uint64_t b, std::uint64_t &out) noexcept {
  if (b > std::numeric_limits<std::uint64_t>::max () - a) return false;
  out = a + b; return true;
}
/* CODE_OBJECT_INFO_LOAD_ADDRESS is ptrdiff_t signed relocation bias.
   Never cast a negative bias to an unsigned base before checked arithmetic. */
inline bool apply_bias (std::uint64_t value, std::int64_t bias, std::uint64_t &out) noexcept {
  if (bias >= 0) return checked_sum (value, static_cast<std::uint64_t> (bias), out);
  const auto magnitude = static_cast<std::uint64_t> (-(bias + 1)) + 1;
  if (value < magnitude) return false;
  out = value - magnitude; return true;
}
class checkpoint_view {
public:
  explicit checkpoint_view (const checkpoint_bytes &bytes) noexcept : b (bytes) {}
  std::uint64_t u64 (std::size_t offset) const noexcept {
    if (offset > b.size () - 8) return 0;
    std::uint64_t v = 0;
    for (unsigned n = 0; n != 8; ++n) v |= std::uint64_t (b[offset+n]) << (8*n);
    return v;
  }
  std::uint32_t u32 (std::size_t offset) const noexcept {
    if (offset > b.size () - 4) return 0;
    std::uint32_t v = 0;
    for (unsigned n = 0; n != 4; ++n) v |= std::uint32_t (b[offset+n]) << (8*n);
    return v;
  }
  std::uint16_t u16 (std::size_t offset) const noexcept {
    if (offset > b.size () - 2) return 0;
    return std::uint16_t (b[offset]) | (std::uint16_t (b[offset+1]) << 8);
  }
  bool digest_at (std::size_t at, const digest_bytes &expected) const noexcept {
    if (at > b.size () - expected.size ()) return false;
    for (std::size_t n = 0; n != expected.size (); ++n)
      if (b[at+n] != expected[n]) return false;
    return true;
  }
  bool valid (std::uint64_t actual_pid) const noexcept {
    constexpr std::array<std::uint8_t, 8> magic {{'F','3','G','S','T','P','0','1'}};
    for (std::size_t n = 0; n != magic.size (); ++n) if (b[n] != magic[n]) return false;
    if (u32 (8) != 1 || u32 (12) != 776 || actual_pid == 0
        || actual_pid > std::numeric_limits<std::uint32_t>::max ()
        || u32 (16) != actual_pid || u32 (24) != 39903
        || u64 (32) != UINT64_C (16366993098680759275)
        || u64 (40) != 0 || u64 (48) != 0 || u64 (64) != 12288
        || u64 (96) != 8388608 || u64 (152) != 264
        || u64 (168) != 272 || u64 (176) != 4096 || u64 (192) != 256
        || u64 (208) != 4096 || u64 (216) == 0 || (u64 (216) & 7) != 0
        || u64 (232) != 181829632 || u64 (248) != 4096)
      return false;
    std::uint64_t descriptor = 0, entry = 0, write = 0, read = 0, value = 0, output = 0;
    if (!checked_sum (u64 (56), 1984, descriptor) || descriptor != u64 (72)
        || !checked_sum (u64 (56), 6144, entry) || entry != u64 (80)
        || !checked_sum (u64 (104), 0x38, write) || write != u64 (112)
        || !checked_sum (u64 (104), 0x80, read) || read != u64 (120)
        || !checked_sum (u64 (128), 8, value) || value != u64 (136)
        || !checked_sum (u64 (160), 8, output) || output != u64 (184)) return false;
    const std::array<std::array<std::uint64_t, 2>, 9> ranges {{
      {{u64 (56), 12288}}, {{u64 (88), 8388608}}, {{u64 (104), 4096}},
      {{u64 (128), 4096}}, {{u64 (144), 65536}}, {{u64 (160), 4096}},
      {{u64 (200), 4096}}, {{u64 (224), 181829632}}, {{u64 (240), 4096}}
    }};
    for (std::size_t n = 0; n != ranges.size (); ++n) {
      std::uint64_t end = 0;
      if (ranges[n][0] == 0 || (ranges[n][0] & 4095) != 0
          || !checked_sum (ranges[n][0], ranges[n][1], end)) return false;
      for (std::size_t p = 0; p < n; ++p) {
        std::uint64_t previous_end = 0;
        if (!checked_sum (ranges[p][0], ranges[p][1], previous_end)
            || !(end <= ranges[p][0] || previous_end <= ranges[n][0])) return false;
      }
    }
    if (!digest_at (256, fixed_object_sha) || !digest_at (288, fixed_source_sha)
        || !digest_at (320, fixed_image_sha) || !digest_at (352, fixed_trap_sha))
      return false;
    // Retain/requery the actual target's closure digest. This nonzero check
    // does NOT independently recompute the Rust loader's identity algorithm.
    bool closure_nonzero = false;
    for (std::size_t n = 384; n != 416; ++n) closure_nonzero |= b[n] != 0;
    if (!closure_nonzero) return false;
    for (std::size_t n = 744; n != 776; ++n) if (b[n] != 0) return false;
    return packet_valid () && kernarg_valid ();
  }
  bool packet_valid () const noexcept {
    constexpr std::size_t p = 416;
    return u32 (p) == 0x00010001 && u16 (p+4) == 64 && u16 (p+6) == 1
      && u16 (p+8) == 1 && u16 (p+10) == 0 && u32 (p+12) == 64
      && u32 (p+16) == 1 && u32 (p+20) == 1 && u32 (p+24) == 0
      && u32 (p+28) == 0 && u64 (p+32) == u64 (72)
      && u64 (p+40) == u64 (144) && u64 (p+48) == 0
      && u64 (p+56) == u64 (128); // packet signal BASE, never value+8.
  }
  bool kernarg_valid () const noexcept {
    for (std::size_t n = 0; n != 264; ++n) {
      std::uint8_t expected = 0;
      if (n < 8) expected = static_cast<std::uint8_t> (u64 (184) >> (8*n));
      else if (n == 8 || n == 12 || n == 16 || n == 22 || n == 24 || n == 72) expected = 1;
      else if (n == 20) expected = 64;
      if (b[480+n] != expected) return false;
    }
    return true;
  }
private:
  const checkpoint_bytes &b;
};
} // namespace amd_owned_one_stop_v1
#endif
