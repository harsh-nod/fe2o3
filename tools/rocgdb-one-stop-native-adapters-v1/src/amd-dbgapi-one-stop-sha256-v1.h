/* SPDX-License-Identifier: GPL-3.0-or-later
   Fixed-buffer SHA-256 for source-pinned file/inferior streaming. Hashes are
   content observations, never source/runtime/queue authority.
   The native caller prepays input bytes and 64 rounds per compression block. */
#ifndef GDB_AMD_DBGAPI_ONE_STOP_SHA256_V1_H
#define GDB_AMD_DBGAPI_ONE_STOP_SHA256_V1_H
#include <array>
#include <cstddef>
#include <cstdint>
#include <limits>

namespace amd_owned_one_stop_v1 {
class sha256 final {
public:
  using digest = std::array<std::uint8_t, 32>;
  static constexpr std::uint64_t max_bytes = (std::uint64_t (1) << 61) - 1;
  bool input_blocks (std::size_t bytes, std::uint64_t &blocks) const noexcept {
    if (m_finished || bytes > max_bytes - m_total) return false;
    blocks = (std::uint64_t (m_used) + bytes) / 64; return true;
  }
  std::uint64_t finish_blocks () const noexcept { return m_used <= 55 ? 1 : 2; }
  bool update (const std::uint8_t *bytes, std::size_t count) noexcept {
    if (m_finished || (count && bytes == nullptr) || count > max_bytes - m_total) return false;
    m_total += count;
    for (std::size_t i = 0; i != count; ++i) {
      m_block[m_used++] = bytes[i];
      if (m_used == 64) { compress (); m_used = 0; }
    }
    return true;
  }
  bool finish (digest &out) noexcept {
    if (m_finished) return false;
    const std::uint64_t bits = m_total * 8;
    m_block[m_used++] = 0x80;
    if (m_used > 56) {
      while (m_used < 64) m_block[m_used++] = 0;
      compress (); m_used = 0;
    }
    while (m_used < 56) m_block[m_used++] = 0;
    for (unsigned n = 0; n != 8; ++n)
      m_block[56 + n] = static_cast<std::uint8_t> (bits >> (56 - 8 * n));
    compress ();
    for (unsigned n = 0; n != 8; ++n)
      for (unsigned b = 0; b != 4; ++b)
        out[n * 4 + b] = static_cast<std::uint8_t> (m_state[n] >> (24 - 8 * b));
    m_finished = true; return true;
  }
private:
  static std::uint32_t ror (std::uint32_t x, unsigned n) noexcept
  { return (x >> n) | (x << (32 - n)); }
  void compress () noexcept {
    for (unsigned n = 0; n != 16; ++n)
      m_schedule[n] = (std::uint32_t (m_block[n * 4]) << 24)
        | (std::uint32_t (m_block[n * 4 + 1]) << 16)
        | (std::uint32_t (m_block[n * 4 + 2]) << 8) | m_block[n * 4 + 3];
    auto a=m_state[0], b=m_state[1], c=m_state[2], d=m_state[3];
    auto e=m_state[4], f=m_state[5], g=m_state[6], h=m_state[7];
    for (unsigned n = 0; n != 64; ++n) {
      if (n >= 16) {
        const auto x = m_schedule[(n - 15) & 15], y = m_schedule[(n - 2) & 15];
        const auto s0 = ror (x, 7) ^ ror (x, 18) ^ (x >> 3);
        const auto s1 = ror (y, 17) ^ ror (y, 19) ^ (y >> 10);
        m_schedule[n & 15] += s0 + m_schedule[(n - 7) & 15] + s1;
      }
      const auto s1 = ror (e, 6) ^ ror (e, 11) ^ ror (e, 25);
      const auto t1 = h + s1 + ((e & f) ^ (~e & g)) + constants[n] + m_schedule[n & 15];
      const auto s0 = ror (a, 2) ^ ror (a, 13) ^ ror (a, 22);
      const auto t2 = s0 + ((a & b) ^ (a & c) ^ (b & c));
      h=g; g=f; f=e; e=d+t1; d=c; c=b; b=a; a=t1+t2;
    }
    m_state[0]+=a; m_state[1]+=b; m_state[2]+=c; m_state[3]+=d;
    m_state[4]+=e; m_state[5]+=f; m_state[6]+=g; m_state[7]+=h;
  }
  inline static constexpr std::array<std::uint32_t, 64> constants {{
    0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
    0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
    0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
    0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
    0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
    0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
    0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
    0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2
  }};
  std::array<std::uint32_t, 8> m_state {{
    0x6a09e667,0xbb67ae85,0x3c6ef372,0xa54ff53a,
    0x510e527f,0x9b05688c,0x1f83d9ab,0x5be0cd19
  }};
  std::array<std::uint8_t, 64> m_block {};
  std::array<std::uint32_t, 16> m_schedule {};
  std::uint64_t m_total = 0;
  std::size_t m_used = 0;
  bool m_finished = false;
};
static_assert (sizeof (sha256) <= 256, "prepaid streaming digest context");
} // namespace amd_owned_one_stop_v1
#endif
