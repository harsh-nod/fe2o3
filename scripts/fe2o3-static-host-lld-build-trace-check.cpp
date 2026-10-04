#include <sys/file.h>
#include <sys/stat.h>

#include <fcntl.h>
#include <unistd.h>

#include <algorithm>
#include <array>
#include <cctype>
#include <cerrno>
#include <charconv>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <map>
#include <optional>
#include <set>
#include <sstream>
#include <stdexcept>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

namespace fs = std::filesystem;

namespace {

constexpr uint64_t kPerTraceByteBound = 64ULL * 1024ULL * 1024ULL;
constexpr uint64_t kAggregateTraceByteBound = 256ULL * 1024ULL * 1024ULL;
constexpr uint64_t kGlobalRetentionFileBound = 65536;
constexpr uint64_t kGlobalRetentionByteBound = 256ULL * 1024ULL * 1024ULL;

[[noreturn]] void fail(const std::string &message) {
  throw std::runtime_error(message);
}

class Fd {
public:
  explicit Fd(int value = -1) : value_(value) {}
  ~Fd() {
    if (value_ >= 0)
      ::close(value_);
  }
  Fd(const Fd &) = delete;
  Fd &operator=(const Fd &) = delete;
  Fd(Fd &&other) noexcept : value_(std::exchange(other.value_, -1)) {}
  Fd &operator=(Fd &&other) noexcept {
    if (this != &other) {
      if (value_ >= 0)
        ::close(value_);
      value_ = std::exchange(other.value_, -1);
    }
    return *this;
  }
  int get() const { return value_; }
  explicit operator bool() const { return value_ >= 0; }

private:
  int value_;
};

uint32_t rotateRight(uint32_t value, unsigned count) {
  return (value >> count) | (value << (32U - count));
}

class Sha256 {
public:
  void update(const void *data, size_t length) {
    const auto *bytes = static_cast<const uint8_t *>(data);
    totalBytes_ += length;
    while (length != 0) {
      const size_t copied = std::min(length, block_.size() - blockLength_);
      std::memcpy(block_.data() + blockLength_, bytes, copied);
      blockLength_ += copied;
      bytes += copied;
      length -= copied;
      if (blockLength_ == block_.size()) {
        transform(block_.data());
        blockLength_ = 0;
      }
    }
  }

  void update(std::string_view text) { update(text.data(), text.size()); }

  std::string finish() {
    const uint64_t bitLength = static_cast<uint64_t>(totalBytes_) * 8U;
    const uint8_t marker = 0x80;
    update(&marker, 1);
    const uint8_t zero = 0;
    while (blockLength_ != 56)
      update(&zero, 1);
    std::array<uint8_t, 8> lengthBytes{};
    for (size_t index = 0; index < lengthBytes.size(); ++index)
      lengthBytes[7 - index] =
          static_cast<uint8_t>(bitLength >> static_cast<unsigned>(index * 8));
    update(lengthBytes.data(), lengthBytes.size());

    static constexpr char kHex[] = "0123456789abcdef";
    std::string result;
    result.reserve(64);
    for (uint32_t word : state_)
      for (int shift = 28; shift >= 0; shift -= 4)
        result.push_back(kHex[(word >> shift) & 0xfU]);
    return result;
  }

private:
  void transform(const uint8_t *input) {
    static constexpr std::array<uint32_t, 64> kConstants = {
        0x428a2f98U, 0x71374491U, 0xb5c0fbcfU, 0xe9b5dba5U, 0x3956c25bU,
        0x59f111f1U, 0x923f82a4U, 0xab1c5ed5U, 0xd807aa98U, 0x12835b01U,
        0x243185beU, 0x550c7dc3U, 0x72be5d74U, 0x80deb1feU, 0x9bdc06a7U,
        0xc19bf174U, 0xe49b69c1U, 0xefbe4786U, 0x0fc19dc6U, 0x240ca1ccU,
        0x2de92c6fU, 0x4a7484aaU, 0x5cb0a9dcU, 0x76f988daU, 0x983e5152U,
        0xa831c66dU, 0xb00327c8U, 0xbf597fc7U, 0xc6e00bf3U, 0xd5a79147U,
        0x06ca6351U, 0x14292967U, 0x27b70a85U, 0x2e1b2138U, 0x4d2c6dfcU,
        0x53380d13U, 0x650a7354U, 0x766a0abbU, 0x81c2c92eU, 0x92722c85U,
        0xa2bfe8a1U, 0xa81a664bU, 0xc24b8b70U, 0xc76c51a3U, 0xd192e819U,
        0xd6990624U, 0xf40e3585U, 0x106aa070U, 0x19a4c116U, 0x1e376c08U,
        0x2748774cU, 0x34b0bcb5U, 0x391c0cb3U, 0x4ed8aa4aU, 0x5b9cca4fU,
        0x682e6ff3U, 0x748f82eeU, 0x78a5636fU, 0x84c87814U, 0x8cc70208U,
        0x90befffaU, 0xa4506cebU, 0xbef9a3f7U, 0xc67178f2U};
    std::array<uint32_t, 64> words{};
    for (size_t index = 0; index < 16; ++index)
      words[index] = static_cast<uint32_t>(input[index * 4]) << 24U |
                     static_cast<uint32_t>(input[index * 4 + 1]) << 16U |
                     static_cast<uint32_t>(input[index * 4 + 2]) << 8U |
                     static_cast<uint32_t>(input[index * 4 + 3]);
    for (size_t index = 16; index < words.size(); ++index) {
      const uint32_t s0 = rotateRight(words[index - 15], 7) ^
                          rotateRight(words[index - 15], 18) ^
                          (words[index - 15] >> 3U);
      const uint32_t s1 = rotateRight(words[index - 2], 17) ^
                          rotateRight(words[index - 2], 19) ^
                          (words[index - 2] >> 10U);
      words[index] = words[index - 16] + s0 + words[index - 7] + s1;
    }
    uint32_t a = state_[0];
    uint32_t b = state_[1];
    uint32_t c = state_[2];
    uint32_t d = state_[3];
    uint32_t e = state_[4];
    uint32_t f = state_[5];
    uint32_t g = state_[6];
    uint32_t h = state_[7];
    for (size_t index = 0; index < words.size(); ++index) {
      const uint32_t s1 =
          rotateRight(e, 6) ^ rotateRight(e, 11) ^ rotateRight(e, 25);
      const uint32_t choice = (e & f) ^ (~e & g);
      const uint32_t temporary1 =
          h + s1 + choice + kConstants[index] + words[index];
      const uint32_t s0 =
          rotateRight(a, 2) ^ rotateRight(a, 13) ^ rotateRight(a, 22);
      const uint32_t majority = (a & b) ^ (a & c) ^ (b & c);
      const uint32_t temporary2 = s0 + majority;
      h = g;
      g = f;
      f = e;
      e = d + temporary1;
      d = c;
      c = b;
      b = a;
      a = temporary1 + temporary2;
    }
    state_[0] += a;
    state_[1] += b;
    state_[2] += c;
    state_[3] += d;
    state_[4] += e;
    state_[5] += f;
    state_[6] += g;
    state_[7] += h;
  }

  std::array<uint32_t, 8> state_ = {0x6a09e667U, 0xbb67ae85U, 0x3c6ef372U,
                                    0xa54ff53aU, 0x510e527fU, 0x9b05688cU,
                                    0x1f83d9abU, 0x5be0cd19U};
  std::array<uint8_t, 64> block_{};
  size_t blockLength_ = 0;
  size_t totalBytes_ = 0;
};

std::string sha256(std::string_view content) {
  Sha256 digest;
  digest.update(content);
  return digest.finish();
}

std::string trim(std::string value) {
  const auto first =
      std::find_if_not(value.begin(), value.end(), [](char item) {
        return std::isspace(static_cast<unsigned char>(item)) != 0;
      });
  const auto last =
      std::find_if_not(value.rbegin(), value.rend(), [](char item) {
        return std::isspace(static_cast<unsigned char>(item)) != 0;
      }).base();
  return first < last ? std::string(first, last) : std::string{};
}

bool safeText(std::string_view value) {
  return !value.empty() && value.find('\n') == std::string_view::npos &&
         value.find('\r') == std::string_view::npos &&
         value.find('\t') == std::string_view::npos &&
         value.find('\0') == std::string_view::npos;
}

bool canonicalTracePrefixName(std::string_view value) {
  return !value.empty() && value.size() <= 255 &&
         std::all_of(value.begin(), value.end(), [](unsigned char item) {
           return (item >= 'a' && item <= 'z') ||
                  (item >= 'A' && item <= 'Z') ||
                  (item >= '0' && item <= '9') || item == '.' || item == '-' ||
                  item == '_';
         });
}

uint64_t parseUnsigned(std::string_view value, const char *label) {
  uint64_t result = 0;
  const auto [end, error] =
      std::from_chars(value.data(), value.data() + value.size(), result);
  if (error != std::errc{} || end != value.data() + value.size())
    fail(std::string("invalid ") + label);
  return result;
}

std::string canonicalExisting(const std::string &path, const char *label) {
  char *resolved = ::realpath(path.c_str(), nullptr);
  if (resolved == nullptr)
    fail(std::string("cannot canonicalize ") + label + ": " + path);
  std::string result(resolved);
  std::free(resolved);
  return result;
}

std::string lexicalAbsolute(const std::string &path) {
  if (path.empty() || path.front() != '/' || !safeText(path))
    fail("trace path is not an absolute safe path: " + path);
  const std::string result = fs::path(path).lexically_normal().string();
  if (result.empty() || result.front() != '/')
    fail("trace path did not normalize absolutely: " + path);
  return result;
}

std::string decodeQuoted(std::string_view token) {
  const size_t first = token.find('"');
  if (first == std::string_view::npos)
    fail("expected a quoted path argument: " + std::string(token));
  std::string result;
  bool closed = false;
  for (size_t index = first + 1; index < token.size(); ++index) {
    const char item = token[index];
    if (item == '"') {
      closed = true;
      break;
    }
    if (item != '\\') {
      result.push_back(item);
      continue;
    }
    if (++index >= token.size())
      fail("truncated escape in quoted path");
    const char escaped = token[index];
    if (escaped == '\\' || escaped == '"') {
      result.push_back(escaped);
    } else if (escaped >= '0' && escaped <= '7') {
      unsigned value = static_cast<unsigned>(escaped - '0');
      for (int count = 0; count < 2 && index + 1 < token.size() &&
                          token[index + 1] >= '0' && token[index + 1] <= '7';
           ++count) {
        value = value * 8U + static_cast<unsigned>(token[++index] - '0');
      }
      if (value == 0 || value > 127)
        fail("non-ASCII or NUL escape in traced path");
      result.push_back(static_cast<char>(value));
    } else {
      fail("unsupported escape in traced path");
    }
  }
  if (!closed || (!result.empty() && !safeText(result)))
    fail("unterminated or unsafe quoted path");
  return result;
}

std::optional<std::string> annotation(std::string_view token) {
  const size_t begin = token.find('<');
  if (begin == std::string_view::npos)
    return std::nullopt;
  const size_t end = token.rfind('>');
  if (end == std::string_view::npos || end <= begin)
    fail("malformed descriptor annotation: " +
         std::string(token.substr(0, 256)));
  std::string value(token.substr(begin + 1, end - begin - 1));
  const size_t nested = value.find('<');
  if (nested != std::string::npos) {
    const std::string device = value.substr(nested);
    const std::string numbers =
        device.starts_with("<char ") && device.ends_with('>')
            ? device.substr(6, device.size() - 7)
            : std::string{};
    const size_t colon = numbers.find(':');
    const auto digits = [](std::string_view text) {
      return std::all_of(text.begin(), text.end(), [](unsigned char item) {
        return std::isdigit(static_cast<unsigned char>(item)) != 0;
      });
    };
    if (value.find('<', nested + 1) != std::string::npos || colon == 0 ||
        colon == std::string::npos || colon + 1 == numbers.size() ||
        !digits(std::string_view(numbers).substr(0, colon)) ||
        !digits(std::string_view(numbers).substr(colon + 1)))
      fail("unsupported nested descriptor annotation: " +
           std::string(token.substr(0, 256)));
    value.resize(nested);
  }
  if (!safeText(value))
    fail("unsafe descriptor annotation");
  return value;
}

std::vector<std::string> splitArguments(std::string_view input) {
  std::vector<std::string> arguments;
  size_t begin = 0;
  int round = 0;
  int square = 0;
  int brace = 0;
  bool quoted = false;
  bool escaped = false;
  for (size_t index = 0; index < input.size(); ++index) {
    const char item = input[index];
    if (quoted) {
      if (escaped)
        escaped = false;
      else if (item == '\\')
        escaped = true;
      else if (item == '"')
        quoted = false;
      continue;
    }
    if (item == '"') {
      quoted = true;
      continue;
    }
    if (item == '(')
      ++round;
    else if (item == ')')
      --round;
    else if (item == '[')
      ++square;
    else if (item == ']')
      --square;
    else if (item == '{')
      ++brace;
    else if (item == '}')
      --brace;
    else if (item == ',' && round == 0 && square == 0 && brace == 0) {
      arguments.push_back(
          trim(std::string(input.substr(begin, index - begin))));
      begin = index + 1;
    }
    if (round < 0 || square < 0 || brace < 0)
      fail("unbalanced strace argument structure");
  }
  if (quoted || escaped || round != 0 || square != 0 || brace != 0)
    fail("truncated strace argument structure");
  arguments.push_back(trim(std::string(input.substr(begin))));
  return arguments;
}

struct Record {
  std::string name;
  std::vector<std::string> arguments;
  std::string result;
  bool success = false;
  std::string error;
  size_t line = 0;
};

std::optional<Record> parseRecord(const std::string &line, size_t lineNumber) {
  if (line.empty())
    fail("empty strace record");
  if (line.find("<unfinished ...>") != std::string::npos ||
      line.find("<... ") != std::string::npos ||
      line.find("resumed>") != std::string::npos ||
      line.find("detached") != std::string::npos ||
      line.find("strace:") != std::string::npos)
    fail("unfinished, resumed, detached, or tracer diagnostic record");
  if (line.starts_with("--- ") || line.starts_with("+++ "))
    return std::nullopt;
  const size_t open = line.find('(');
  if (open == std::string::npos || open == 0)
    fail("unparsed successful strace record");
  const std::string name = line.substr(0, open);
  if (!std::all_of(name.begin(), name.end(), [](char item) {
        return std::islower(static_cast<unsigned char>(item)) != 0 ||
               std::isdigit(static_cast<unsigned char>(item)) != 0 ||
               item == '_';
      }))
    fail("noncanonical syscall name: " + name);
  bool quoted = false;
  bool escaped = false;
  int depth = 1;
  size_t close = std::string::npos;
  for (size_t index = open + 1; index < line.size(); ++index) {
    const char item = line[index];
    if (quoted) {
      if (escaped)
        escaped = false;
      else if (item == '\\')
        escaped = true;
      else if (item == '"')
        quoted = false;
      continue;
    }
    if (item == '"')
      quoted = true;
    else if (item == '(')
      ++depth;
    else if (item == ')' && --depth == 0) {
      close = index;
      break;
    }
  }
  if (close == std::string::npos || quoted)
    fail("truncated syscall record");
  const std::string suffix = trim(line.substr(close + 1));
  if (!suffix.starts_with("= "))
    fail("syscall record lacks a result at line " + std::to_string(lineNumber) +
         ": " + line);
  const std::string result = suffix.substr(2);
  if (result.empty())
    fail("unavailable syscall record");
  if (result.starts_with("?")) {
    if (name == "exit" || name == "exit_group")
      return Record{name,
                    splitArguments(std::string_view(line).substr(
                        open + 1, close - open - 1)),
                    result,
                    false,
                    "UNAVAILABLE",
                    lineNumber};
    fail("unavailable non-exit syscall record");
  }
  const bool success = !result.starts_with("-1 ");
  std::string error;
  if (!success) {
    const size_t begin = 3;
    const size_t end = result.find(' ', begin);
    error = result.substr(begin, end - begin);
    if (error.empty() ||
        !std::all_of(error.begin(), error.end(), [](char item) {
          return item == '_' ||
                 std::isupper(static_cast<unsigned char>(item)) != 0 ||
                 std::isdigit(static_cast<unsigned char>(item)) != 0;
        }))
      fail("failed syscall has a noncanonical errno");
  }
  return Record{
      name,
      splitArguments(std::string_view(line).substr(open + 1, close - open - 1)),
      result,
      success,
      error,
      lineNumber};
}

struct Allowlist {
  std::map<std::string, std::string> exact;
  std::map<std::string, std::string> kernel;
  std::vector<std::pair<std::string, std::string>> roots;
  std::vector<std::pair<std::string, std::string>> absenceRoots;
  std::vector<std::pair<std::string, std::string>> absencePrefixes;
  std::vector<std::pair<std::string, std::string>> outputs;
};

std::string mergeLabels(const std::string &first, const std::string &second) {
  std::set<std::string> labels;
  for (const std::string *value : {&first, &second}) {
    size_t begin = 0;
    while (begin <= value->size()) {
      const size_t end = value->find('+', begin);
      const std::string label =
          value->substr(begin, end == std::string::npos ? value->size() - begin
                                                        : end - begin);
      if (label.empty())
        fail("empty component in merged trace allowlist label");
      labels.insert(label);
      if (end == std::string::npos)
        break;
      begin = end + 1;
    }
  }
  std::string merged;
  for (const std::string &label : labels) {
    if (!merged.empty())
      merged += '+';
    merged += label;
  }
  return merged;
}

Allowlist readAllowlist(const std::string &path) {
  std::ifstream input(path);
  if (!input)
    fail("cannot open trace allowlist");
  Allowlist result;
  std::string line;
  bool header = false;
  while (std::getline(input, line)) {
    if (line == "FORMAT=fe2o3-static-host-lld-trace-allowlist-v1") {
      if (header)
        fail("duplicate trace allowlist header");
      header = true;
      continue;
    }
    const size_t first = line.find('\t');
    const size_t second = first == std::string::npos
                              ? std::string::npos
                              : line.find('\t', first + 1);
    if (first != 1 || second == std::string::npos ||
        line.find('\t', second + 1) != std::string::npos)
      fail("noncanonical trace allowlist row");
    const char kind = line.front();
    const std::string label = line.substr(first + 1, second - first - 1);
    const std::string item = line.substr(second + 1);
    if (!safeText(label) || item.empty() || item.front() != '/')
      fail("unsafe trace allowlist row");
    if (kind == 'F' || kind == 'K') {
      auto &map = kind == 'F' ? result.exact : result.kernel;
      const auto [entry, inserted] = map.emplace(item, label);
      if (!inserted) {
        if (kind == 'F')
          fail("duplicate exact-file trace allowlist path");
        entry->second = mergeLabels(entry->second, label);
      }
    } else if (kind == 'R' || kind == 'N' || kind == 'P' || kind == 'O') {
      auto &items = kind == 'R'   ? result.roots
                    : kind == 'N' ? result.absenceRoots
                    : kind == 'P' ? result.absencePrefixes
                                  : result.outputs;
      items.emplace_back(label, item);
    } else {
      fail("unknown trace allowlist row kind");
    }
  }
  if (!header || result.exact.size() < 80 || result.roots.size() < 5 ||
      result.outputs.size() != 2)
    fail("trace allowlist is unexpectedly incomplete");
  for (const auto &[pathValue, label] : result.exact) {
    (void)label;
    if (!fs::is_regular_file(pathValue) || fs::is_symlink(pathValue) ||
        canonicalExisting(pathValue, "exact allowlist file") != pathValue)
      fail("exact allowlist path is not a canonical regular file: " +
           pathValue);
  }
  for (const auto &[label, root] : result.roots) {
    (void)label;
    if (!fs::is_directory(root) || fs::is_symlink(root) ||
        canonicalExisting(root, "allowlist root") != root)
      fail("allowlist root is not a canonical directory: " + root);
  }
  for (const auto &[label, root] : result.absenceRoots) {
    (void)label;
    if (!fs::is_directory(root) || fs::is_symlink(root) ||
        canonicalExisting(root, "absence allowlist root") != root)
      fail("absence allowlist root is not a canonical directory: " + root);
  }
  for (const auto &[label, prefix] : result.absencePrefixes) {
    (void)label;
    if (lexicalAbsolute(prefix) != prefix)
      fail("absence prefix is not a canonical lexical path: " + prefix);
  }
  for (const auto &[label, output] : result.outputs) {
    (void)label;
    if (!fs::is_directory(output) || fs::is_symlink(output) ||
        canonicalExisting(output, "allowlist output") != output)
      fail("allowlist output is not a canonical directory: " + output);
  }
  return result;
}

enum class PathSemantics {
  Follow,
  NoFollow,
  OutputLexical,
  Descriptor,
  SymlinkCreation,
};

struct Admission {
  Allowlist allowlist;
  std::set<std::string> canonicalRows;
  std::set<std::string> inputRows;
  std::set<std::string> descriptorConfirmedOutputs;

  using Output = std::pair<std::string, std::string>;

  std::optional<Output> outputFor(const std::string &path) const {
    for (const auto &[label, root] : allowlist.outputs)
      if (path == root || path.starts_with(root + "/"))
        return Output{label, root};
    return std::nullopt;
  }

  std::string evidencePath(const std::string &path) const {
    if (const auto output = outputFor(path))
      return "$" + output->first + path.substr(output->second.size());
    return path;
  }

  void addCanonicalInput(const std::string &origin,
                         const std::string &canonical,
                         const std::string &rawPath) {
    auto exact = allowlist.exact.find(canonical);
    if (exact != allowlist.exact.end()) {
      const std::string retainedPath = evidencePath(canonical);
      canonicalRows.insert("F\t" + origin + "\t" + exact->second + "\t" +
                           retainedPath);
      inputRows.insert("F\t" + exact->second + "\t" + retainedPath);
      return;
    }
    auto kernel = allowlist.kernel.find(canonical);
    if (kernel != allowlist.kernel.end()) {
      canonicalRows.insert("K\t" + origin + "\t" + kernel->second + "\t" +
                           canonical);
      inputRows.insert("K\t" + kernel->second + "\t" + canonical);
      return;
    }
    for (const auto &[label, root] : allowlist.roots) {
      if (canonical == root || canonical.starts_with(root + "/")) {
        canonicalRows.insert("R\t" + origin + "\t" + label + "\t" + canonical);
        inputRows.insert("R\t" + label + "\t" + canonical);
        return;
      }
    }
    fail("trace used an input outside the admitted closure: " + canonical +
         " (" + origin + " from " + rawPath + ")");
  }

  void addOutput(const std::string &origin, const std::string &path,
                 const Output &output, PathSemantics semantics) {
    const auto &[label, root] = output;
    const std::string token = "$" + label + path.substr(root.size());
    if (semantics == PathSemantics::SymlinkCreation)
      fail("successful symlink creation contradicts the Landlock writable-"
           "root policy: " +
           path);
    if (semantics == PathSemantics::OutputLexical) {
      canonicalRows.insert("E\t" + origin + "\t" + label + "\t" + token +
                           "\tlexical-output-operation");
      return;
    }
    if (semantics == PathSemantics::Descriptor) {
      descriptorConfirmedOutputs.insert(path);
      canonicalRows.insert("E\t" + origin + "\t" + label + "\t" + token +
                           "\tfd-resolved");
      return;
    }
    if (semantics == PathSemantics::NoFollow) {
      const std::string parent =
          path == root ? root : fs::path(path).parent_path().string();
      std::string canonicalParent;
      char *resolved = ::realpath(parent.c_str(), nullptr);
      if (resolved != nullptr) {
        canonicalParent = resolved;
        std::free(resolved);
      } else if (descriptorConfirmedOutputs.contains(parent)) {
        canonicalParent = parent;
      } else if (errno == ENOENT || errno == ENOTDIR) {
        canonicalRows.insert("E\t" + origin + "\t" + label + "\t" + token +
                             "\tpost-phase-absent-parent-landlock-ephemeral");
        return;
      } else {
        fail("cannot resolve no-follow output parent safely: " + parent);
      }
      if (canonicalParent != root && !canonicalParent.starts_with(root + "/"))
        fail("no-follow output path traverses an external parent: " + path);
      canonicalRows.insert("E\t" + origin + "\t" + label + "\t" + token +
                           "\tno-follow-parent-resolved");
      return;
    }

    char *resolved = ::realpath(path.c_str(), nullptr);
    if (resolved != nullptr) {
      const std::string canonical(resolved);
      std::free(resolved);
      if (canonical == root || canonical.starts_with(root + "/")) {
        canonicalRows.insert("E\t" + origin + "\t" + label + "\t" + token +
                             "\tfollow-resolved-within-output");
        return;
      }
      addCanonicalInput(origin, canonical, path);
      return;
    }
    if ((errno == ENOENT || errno == ENOTDIR) &&
        descriptorConfirmedOutputs.contains(path)) {
      canonicalRows.insert("E\t" + origin + "\t" + label + "\t" + token +
                           "\tprior-fd-resolved-landlock-output-ephemeral");
      return;
    }
    fail("successful following metadata output cannot be resolved safely: " +
         path);
  }

  void addEmptyPath(const std::string &origin, bool success,
                    const std::string &error) {
    if (success || error != "ENOENT")
      fail("empty pathname did not fail with ENOENT: " + origin);
    canonicalRows.insert("N\t" + origin +
                         "\tkernel-invalid-empty-path\t<empty>\tENOENT");
  }

  void add(const std::string &origin, const std::string &rawPath, bool success,
           const std::string &error = {},
           PathSemantics semantics = PathSemantics::Follow) {
    std::string path = rawPath;
    if (path.ends_with(" (deleted)")) {
      if (semantics != PathSemantics::Descriptor)
        fail("traced path was deleted before admission: " + path);
      path.resize(path.size() - std::string_view(" (deleted)").size());
    }
    path = lexicalAbsolute(path);
    if (success) {
      auto exactArgument = allowlist.exact.find(path);
      if (exactArgument != allowlist.exact.end()) {
        if (canonicalExisting(path, "exact traced input") != path)
          fail("exact traced input changed identity: " + path);
        const std::string retainedPath = evidencePath(path);
        canonicalRows.insert("F\t" + origin + "\t" + exactArgument->second +
                             "\t" + retainedPath);
        inputRows.insert("F\t" + exactArgument->second + "\t" + retainedPath);
        return;
      }
    }
    if (const auto output = outputFor(path)) {
      if (success)
        addOutput(origin, path, *output, semantics);
      else
        canonicalRows.insert("N\t" + origin + "\t" + output->first + "\t$" +
                             output->first +
                             path.substr(output->second.size()) + "\t" + error);
      return;
    }
    if (!success) {
      auto exactKernelAttempt = allowlist.kernel.find(path);
      if (exactKernelAttempt != allowlist.kernel.end()) {
        canonicalRows.insert("N\t" + origin + "\t" +
                             exactKernelAttempt->second + "\t" + path + "\t" +
                             error);
        return;
      }
      std::error_code canonicalError;
      const std::string weak =
          fs::weakly_canonical(path, canonicalError).string();
      if (canonicalError || weak.empty() || weak.front() != '/')
        fail("failed probe cannot be weakly canonicalized: " + path);
      path = lexicalAbsolute(weak);
      auto exactAttempt = allowlist.exact.find(path);
      if (exactAttempt != allowlist.exact.end()) {
        canonicalRows.insert("N\t" + origin + "\t" + exactAttempt->second +
                             "\t" + path + "\t" + error);
        inputRows.insert("N\t" + exactAttempt->second + "\t" + path);
        return;
      }
      auto kernelAttempt = allowlist.kernel.find(path);
      if (kernelAttempt != allowlist.kernel.end()) {
        canonicalRows.insert("N\t" + origin + "\t" + kernelAttempt->second +
                             "\t" + path + "\t" + error);
        inputRows.insert("N\t" + kernelAttempt->second + "\t" + path);
        return;
      }
      for (const auto &[label, root] : allowlist.roots) {
        if (path == root || path.starts_with(root + "/")) {
          canonicalRows.insert("N\t" + origin + "\t" + label + "\t" + path +
                               "\t" + error);
          inputRows.insert("N\t" + label + "\t" + path);
          return;
        }
      }
      for (const auto &[label, root] : allowlist.absenceRoots) {
        if (path == root || path.starts_with(root + "/")) {
          canonicalRows.insert("N\t" + origin + "\t" + label + "\t" + path +
                               "\t" + error);
          return;
        }
      }
      for (const auto &[label, prefix] : allowlist.absencePrefixes) {
        if (path == prefix || path.starts_with(prefix + "/")) {
          canonicalRows.insert("N\t" + origin + "\t" + label + "\t" + path +
                               "\t" + error);
          return;
        }
      }
      fail("failed probe is outside a guarded root or output tree: " + path);
    }
    addCanonicalInput(origin, canonicalExisting(path, "traced input"), rawPath);
  }
};

struct ProcessTrace {
  uint64_t pid = 0;
  std::vector<Record> records;
  std::string path;
  uint64_t device = 0;
  uint64_t inode = 0;
  uint64_t length = 0;
  std::string digest;
};

bool sameFileState(const struct stat &first, const struct stat &second) {
  return first.st_dev == second.st_dev && first.st_ino == second.st_ino &&
         first.st_mode == second.st_mode && first.st_nlink == second.st_nlink &&
         first.st_size == second.st_size &&
         first.st_mtim.tv_sec == second.st_mtim.tv_sec &&
         first.st_mtim.tv_nsec == second.st_mtim.tv_nsec &&
         first.st_ctim.tv_sec == second.st_ctim.tv_sec &&
         first.st_ctim.tv_nsec == second.st_ctim.tv_nsec;
}

struct RawBytes {
  std::string content;
  std::string digest;
  uint64_t device = 0;
  uint64_t inode = 0;
  uint64_t length = 0;
};

RawBytes readRawBytes(const std::string &path, uint64_t aggregateBytes) {
  Fd fd(::open(path.c_str(), O_RDONLY | O_CLOEXEC | O_NOFOLLOW));
  if (!fd)
    fail("cannot open per-PID trace through a non-following descriptor: " +
         path);
  struct stat before{};
  if (::fstat(fd.get(), &before) != 0 || !S_ISREG(before.st_mode) ||
      before.st_size < 0)
    fail("per-PID trace is not a regular descriptor-bound file");
  const uint64_t length = static_cast<uint64_t>(before.st_size);
  if (length > kPerTraceByteBound ||
      aggregateBytes > kAggregateTraceByteBound - length)
    fail("per-PID or aggregate trace byte bound exceeded");
  std::string content(static_cast<size_t>(length), '\0');
  Sha256 digest;
  uint64_t offset = 0;
  while (offset < length) {
    const size_t requested =
        static_cast<size_t>(std::min<uint64_t>(65536, length - offset));
    const ssize_t count =
        ::pread(fd.get(), content.data() + static_cast<size_t>(offset),
                requested, static_cast<off_t>(offset));
    if (count < 0)
      fail("cannot read descriptor-bound per-PID trace");
    if (count == 0)
      fail("per-PID trace shortened while checking");
    digest.update(content.data() + static_cast<size_t>(offset),
                  static_cast<size_t>(count));
    offset += static_cast<uint64_t>(count);
  }
  struct stat after{};
  struct stat named{};
  if (::fstat(fd.get(), &after) != 0 || ::lstat(path.c_str(), &named) != 0 ||
      !sameFileState(before, after) || after.st_dev != named.st_dev ||
      after.st_ino != named.st_ino || after.st_mode != named.st_mode ||
      after.st_size != named.st_size)
    fail("per-PID trace changed while checking");
  return {std::move(content), digest.finish(),
          static_cast<uint64_t>(after.st_dev),
          static_cast<uint64_t>(after.st_ino), length};
}

std::map<uint64_t, ProcessTrace> readTraces(const std::string &prefix) {
  const fs::path prefixPath(prefix);
  const fs::path parent = prefixPath.parent_path();
  const std::string prefixName = prefixPath.filename().string();
  if (!canonicalTracePrefixName(prefixName))
    fail("trace prefix has a noncanonical basename");
  const std::string stem = prefixName + ".";
  std::map<uint64_t, ProcessTrace> traces;
  uint64_t totalBytes = 0;
  for (const fs::directory_entry &entry : fs::directory_iterator(parent)) {
    const std::string name = entry.path().filename().string();
    if (!name.starts_with(stem))
      continue;
    const std::string pidText = name.substr(stem.size());
    if (pidText.empty() ||
        !std::all_of(pidText.begin(), pidText.end(), [](unsigned char item) {
          return std::isdigit(static_cast<unsigned char>(item)) != 0;
        }))
      fail("noncanonical per-PID trace filename: " + name);
    const uint64_t pid = parseUnsigned(pidText, "trace PID");
    const std::string tracePath = entry.path().string();
    RawBytes raw = readRawBytes(tracePath, totalBytes);
    totalBytes += raw.length;
    std::istringstream input(raw.content);
    ProcessTrace trace;
    trace.pid = pid;
    trace.path = tracePath;
    trace.device = raw.device;
    trace.inode = raw.inode;
    trace.length = raw.length;
    trace.digest = std::move(raw.digest);
    std::string line;
    size_t lineNumber = 0;
    while (std::getline(input, line)) {
      ++lineNumber;
      if (auto record = parseRecord(line, lineNumber))
        trace.records.push_back(std::move(*record));
    }
    if (trace.records.empty())
      fail("per-PID trace contains no parsed syscall records");
    if (!traces.emplace(pid, std::move(trace)).second)
      fail("duplicate per-PID trace");
  }
  if (traces.empty())
    fail("no per-PID traces were found");
  return traces;
}

std::optional<uint64_t> childResult(const Record &record) {
  static const std::set<std::string> spawn = {"clone", "clone3", "fork",
                                              "vfork"};
  if (!spawn.contains(record.name) || !record.success)
    return std::nullopt;
  const size_t end = record.result.find_first_not_of("0123456789");
  const std::string value = record.result.substr(0, end);
  if (value.empty())
    fail("spawn syscall has a non-PID result");
  return parseUnsigned(value, "spawned PID");
}

std::string resolvePath(const std::string &value, const std::string &base) {
  if (value.empty())
    return lexicalAbsolute(base);
  if (value.front() == '/')
    return lexicalAbsolute(value);
  return lexicalAbsolute(base + "/" + value);
}

std::string dirfdBase(const std::string &argument, const std::string &cwd) {
  if (auto value = annotation(argument)) {
    if (value->starts_with("/"))
      return lexicalAbsolute(*value);
    fail("directory descriptor has a non-path annotation: " + *value);
  }
  if (argument == "AT_FDCWD")
    return cwd;
  fail("relative path uses an unresolved directory descriptor: " + argument);
}

std::optional<std::string> descriptorPath(const std::string &argument) {
  auto value = annotation(argument);
  if (!value)
    fail("successful descriptor syscall lacks a decoded descriptor path");
  if (value->starts_with("/"))
    return lexicalAbsolute(*value);
  if (value->starts_with("pipe:[") || value->starts_with("socket:[") ||
      value->starts_with("anon_inode:") || value->starts_with("eventfd:") ||
      value->starts_with("inotify"))
    return std::nullopt;
  fail("successful descriptor syscall has an unknown descriptor annotation: " +
       *value);
}

void requireArguments(const Record &record, size_t count) {
  if (record.arguments.size() < count)
    fail("too few arguments for successful " + record.name);
}

class TraceProcessor {
public:
  TraceProcessor(std::map<uint64_t, ProcessTrace> &traces, Admission &admission,
                 std::string initialCwd)
      : traces_(traces), admission_(admission),
        initialCwd_(std::move(initialCwd)) {}

  void run() {
    std::set<uint64_t> children;
    for (const auto &[pid, trace] : traces_) {
      (void)pid;
      for (const Record &record : trace.records)
        if (auto child = childResult(record)) {
          if (!children.insert(*child).second)
            fail("spawned PID appears more than once");
          if (!traces_.contains(*child))
            fail("spawned PID has no per-PID trace: " + std::to_string(*child));
        }
    }
    std::vector<uint64_t> roots;
    for (const auto &[pid, trace] : traces_) {
      (void)trace;
      if (!children.contains(pid))
        roots.push_back(pid);
    }
    if (roots.size() != 1)
      fail("per-PID traces do not form one represented process tree");
    process(roots.front(), initialCwd_, 0);
    if (processed_.size() != traces_.size())
      fail("one or more per-PID traces were not represented by a spawn");
  }

  const std::map<uint64_t, std::pair<uint64_t, size_t>> &
  processEvidence() const {
    return processEvidence_;
  }

private:
  void addPath(const Record &record, size_t index, const std::string &cwd,
               std::optional<size_t> dirfd = std::nullopt,
               PathSemantics semantics = PathSemantics::Follow) {
    requireArguments(record, index + 1);
    if (record.arguments[index].find('"') == std::string::npos)
      fail("successful " + record.name +
           " has an unquoted path argument: " + record.arguments[index]);
    const std::string value = decodeQuoted(record.arguments[index]);
    if (value.empty()) {
      admission_.addEmptyPath(record.name, record.success, record.error);
      return;
    }
    const std::string base =
        dirfd ? dirfdBase(record.arguments[*dirfd], cwd) : cwd;
    admission_.add(record.name, resolvePath(value, base), record.success,
                   record.error, semantics);
  }

  void addOpenPath(const Record &record, size_t index, const std::string &cwd,
                   std::optional<size_t> dirfd = std::nullopt) {
    addPath(record, index, cwd, dirfd, PathSemantics::OutputLexical);
    if (!record.success)
      return;
    const auto actual = descriptorPath(record.result);
    if (!actual)
      fail("successful open returned a non-path descriptor");
    admission_.add(record.name + "-result", *actual, true, {},
                   PathSemantics::Descriptor);
  }

  void addFd(const Record &record, size_t index) {
    requireArguments(record, index + 1);
    if (auto path = descriptorPath(record.arguments[index]))
      admission_.add(record.name, *path, record.success, record.error,
                     PathSemantics::Descriptor);
  }

  static bool hasFlag(const Record &record, size_t index,
                      std::string_view flag) {
    requireArguments(record, index + 1);
    return record.arguments[index].find(flag) != std::string::npos;
  }

  void process(uint64_t pid, std::string cwd, uint64_t parent) {
    if (!processed_.insert(pid).second)
      fail("process trace is cyclic or repeated");
    ProcessTrace &trace = traces_.at(pid);
    size_t execCount = 0;
    for (const Record &record : trace.records) {
      const std::string &name = record.name;
      if (name == "clone" || name == "clone3" || name == "fork" ||
          name == "vfork") {
        if (record.success) {
          const uint64_t child = *childResult(record);
          process(child, cwd, pid);
        }
      } else if (name == "execve") {
        addPath(record, 0, cwd);
        if (record.success)
          ++execCount;
      } else if (name == "execveat") {
        addPath(record, 1, cwd, 0);
        if (record.success)
          ++execCount;
      } else if (name == "chdir") {
        requireArguments(record, 1);
        const std::string value = decodeQuoted(record.arguments[0]);
        if (value.empty()) {
          admission_.addEmptyPath(name, record.success, record.error);
          continue;
        }
        const std::string path = resolvePath(value, cwd);
        admission_.add(name, path, record.success, record.error);
        if (record.success)
          cwd = path;
      } else if (name == "fchdir") {
        requireArguments(record, 1);
        const auto path = descriptorPath(record.arguments[0]);
        if (!path)
          fail("fchdir used a non-path descriptor");
        admission_.add(name, *path, record.success, record.error);
        if (record.success)
          cwd = *path;
      } else if (name == "open" || name == "creat") {
        addOpenPath(record, 0, cwd);
      } else if (name == "access" || name == "stat" || name == "statfs" ||
                 name == "truncate" || name == "chmod" || name == "chown" ||
                 name == "utime" || name == "utimes") {
        addPath(record, 0, cwd);
      } else if (name == "lstat" || name == "readlink" || name == "lchown" ||
                 name == "mkdir" || name == "mknod" || name == "rmdir" ||
                 name == "unlink") {
        addPath(record, 0, cwd, std::nullopt, PathSemantics::NoFollow);
      } else if (name == "openat" || name == "openat2") {
        addOpenPath(record, 1, cwd, 0);
      } else if (name == "faccessat") {
        addPath(record, 1, cwd, 0);
      } else if (name == "faccessat2") {
        addPath(record, 1, cwd, 0,
                hasFlag(record, 3, "AT_SYMLINK_NOFOLLOW")
                    ? PathSemantics::NoFollow
                    : PathSemantics::Follow);
      } else if (name == "newfstatat") {
        addPath(record, 1, cwd, 0,
                hasFlag(record, 3, "AT_SYMLINK_NOFOLLOW")
                    ? PathSemantics::NoFollow
                    : PathSemantics::Follow);
      } else if (name == "statx") {
        addPath(record, 1, cwd, 0,
                hasFlag(record, 2, "AT_SYMLINK_NOFOLLOW")
                    ? PathSemantics::NoFollow
                    : PathSemantics::Follow);
      } else if (name == "readlinkat" || name == "mkdirat" ||
                 name == "mknodat" || name == "unlinkat") {
        addPath(record, 1, cwd, 0, PathSemantics::NoFollow);
      } else if (name == "fchmodat" || name == "fchmodat2") {
        const size_t flags = name == "fchmodat2" ? 3 : 3;
        addPath(record, 1, cwd, 0,
                record.arguments.size() > flags &&
                        hasFlag(record, flags, "AT_SYMLINK_NOFOLLOW")
                    ? PathSemantics::NoFollow
                    : PathSemantics::Follow);
      } else if (name == "fchownat") {
        addPath(record, 1, cwd, 0,
                hasFlag(record, 4, "AT_SYMLINK_NOFOLLOW")
                    ? PathSemantics::NoFollow
                    : PathSemantics::Follow);
      } else if (name == "utimensat") {
        addPath(record, 1, cwd, 0,
                hasFlag(record, 3, "AT_SYMLINK_NOFOLLOW")
                    ? PathSemantics::NoFollow
                    : PathSemantics::Follow);
      } else if (name == "rename" || name == "link") {
        addPath(record, 0, cwd, std::nullopt, PathSemantics::NoFollow);
        addPath(record, 1, cwd, std::nullopt, PathSemantics::NoFollow);
      } else if (name == "renameat" || name == "renameat2") {
        addPath(record, 1, cwd, 0, PathSemantics::NoFollow);
        addPath(record, 3, cwd, 2, PathSemantics::NoFollow);
      } else if (name == "linkat") {
        addPath(record, 1, cwd, 0,
                hasFlag(record, 4, "AT_SYMLINK_FOLLOW")
                    ? PathSemantics::Follow
                    : PathSemantics::NoFollow);
        addPath(record, 3, cwd, 2, PathSemantics::NoFollow);
      } else if (name == "symlink") {
        addPath(record, 1, cwd, std::nullopt, PathSemantics::SymlinkCreation);
      } else if (name == "symlinkat") {
        addPath(record, 2, cwd, 1, PathSemantics::SymlinkCreation);
      } else if (name == "inotify_add_watch") {
        addPath(record, 1, cwd);
      } else if (name == "mmap" || name == "mmap2") {
        requireArguments(record, 5);
        if (record.arguments[4] != "-1")
          addFd(record, 4);
      } else if (name == "read" || name == "pread64" || name == "readv" ||
                 name == "preadv" || name == "preadv2") {
        addFd(record, 0);
      } else if (name == "fstat" || name == "fstatfs" || name == "ftruncate" ||
                 name == "fchmod" || name == "fchown") {
        addFd(record, 0);
      } else if (name == "getcwd") {
        if (record.success) {
          requireArguments(record, 1);
          const std::string resultPath = decodeQuoted(record.arguments[0]);
          cwd = lexicalAbsolute(resultPath);
          admission_.add(name, cwd, true);
        }
      } else if (name == "wait4" || name == "waitid" || name == "exit" ||
                 name == "exit_group") {
        // Process-only records carry no path source.
      } else {
        fail("unparsed successful path/process/mmap syscall: " + name +
             " at line " + std::to_string(record.line));
      }
    }
    processEvidence_[pid] = {parent, execCount};
  }

  std::map<uint64_t, ProcessTrace> &traces_;
  Admission &admission_;
  std::string initialCwd_;
  std::set<uint64_t> processed_;
  std::map<uint64_t, std::pair<uint64_t, size_t>> processEvidence_;
};

void writeAll(int fd, const char *data, size_t length) {
  size_t offset = 0;
  while (offset < length) {
    const ssize_t count = ::write(fd, data + offset, length - offset);
    if (count < 0) {
      if (errno == EINTR)
        continue;
      fail("cannot write descriptor-bound evidence");
    }
    if (count == 0)
      fail("descriptor-bound evidence write made no progress");
    offset += static_cast<size_t>(count);
  }
}

void writeExclusive(const std::string &path, const std::string &content) {
  if (fs::exists(path) || fs::is_symlink(path))
    fail("trace evidence output already exists: " + path);
  Fd output(::open(path.c_str(),
                   O_WRONLY | O_CREAT | O_EXCL | O_CLOEXEC | O_NOFOLLOW, 0600));
  if (!output)
    fail("cannot create trace evidence output: " + path);
  writeAll(output.get(), content.data(), content.size());
  if (::fchmod(output.get(), 0444) != 0 || ::fsync(output.get()) != 0)
    fail("cannot seal trace evidence output: " + path);
  struct stat retained{};
  struct stat named{};
  if (::fstat(output.get(), &retained) != 0 ||
      ::lstat(path.c_str(), &named) != 0 || !S_ISREG(retained.st_mode) ||
      retained.st_dev != named.st_dev || retained.st_ino != named.st_ino ||
      retained.st_size < 0 ||
      static_cast<uint64_t>(retained.st_size) != content.size() ||
      (retained.st_mode & 07777) != 0444)
    fail("sealed trace evidence output changed identity: " + path);
}

void verifyRawEvidence(const std::map<uint64_t, ProcessTrace> &traces) {
  uint64_t aggregate = 0;
  for (const auto &[pid, trace] : traces) {
    (void)pid;
    RawBytes observed = readRawBytes(trace.path, aggregate);
    if (observed.device != trace.device || observed.inode != trace.inode ||
        observed.length != trace.length || observed.digest != trace.digest)
      fail("per-PID trace changed after admission checking: " + trace.path);
    aggregate += observed.length;
  }
}

struct CheckedRawEntry {
  uint64_t pid = 0;
  uint64_t device = 0;
  uint64_t inode = 0;
  uint64_t length = 0;
  std::string digest;
  std::string path;
};

struct CheckedRawRecord {
  std::string prefix;
  std::vector<CheckedRawEntry> entries;
  std::string canonicalDigest;
  std::string inputsDigest;
  uint64_t totalBytes = 0;
  RawBytes backing;
};

struct RetentionPhase {
  std::string name;
  uint64_t files = 0;
  uint64_t bytes = 0;
};

struct RetentionLedger {
  std::vector<RetentionPhase> phases;
  uint64_t files = 0;
  uint64_t bytes = 0;
};

std::vector<std::string> splitTabs(const std::string &line) {
  std::vector<std::string> fields;
  size_t begin = 0;
  while (true) {
    const size_t end = line.find('\t', begin);
    fields.push_back(line.substr(
        begin, end == std::string::npos ? line.size() - begin : end - begin));
    if (end == std::string::npos)
      return fields;
    begin = end + 1;
  }
}

CheckedRawRecord readCheckedRawRecord(const std::string &path) {
  RawBytes backing = readRawBytes(path, 0);
  std::istringstream input(backing.content);
  auto next = [&]() {
    std::string line;
    if (!std::getline(input, line))
      fail("checked raw trace record is truncated");
    return line;
  };
  if (next() != "FORMAT=fe2o3-static-host-lld-checked-raw-traces-v1" ||
      next() != "STATUS=descriptor-bound-checked-bytes")
    fail("checked raw trace record has the wrong header");
  CheckedRawRecord result;
  const std::string prefix = next();
  if (!prefix.starts_with("PREFIX=") || prefix.size() == 7)
    fail("checked raw trace record has no prefix");
  result.prefix = prefix.substr(7);
  if (next() != "PER_FILE_BYTE_BOUND=" + std::to_string(kPerTraceByteBound) ||
      next() !=
          "AGGREGATE_BYTE_BOUND=" + std::to_string(kAggregateTraceByteBound))
    fail("checked raw trace record has the wrong byte bounds");
  uint64_t aggregate = 0;
  std::set<uint64_t> pids;
  std::string line = next();
  while (line.starts_with("F\t")) {
    const std::vector<std::string> fields = splitTabs(line);
    if (fields.size() != 7 || fields[0] != "F" || fields[5].size() != 64 ||
        !std::all_of(
            fields[5].begin(), fields[5].end(), [](unsigned char item) {
              return std::isdigit(static_cast<unsigned char>(item)) != 0 ||
                     (item >= 'a' && item <= 'f');
            }))
      fail("checked raw trace record has a malformed file row");
    CheckedRawEntry entry{parseUnsigned(fields[1], "checked trace PID"),
                          parseUnsigned(fields[2], "checked trace device"),
                          parseUnsigned(fields[3], "checked trace inode"),
                          parseUnsigned(fields[4], "checked trace length"),
                          fields[5],
                          fields[6]};
    if (!pids.insert(entry.pid).second || entry.length > kPerTraceByteBound ||
        aggregate > kAggregateTraceByteBound - entry.length ||
        entry.path != result.prefix + "." + std::to_string(entry.pid))
      fail("checked raw trace record file row violates its closure");
    aggregate += entry.length;
    result.entries.push_back(std::move(entry));
    line = next();
  }
  if (line != "FILES=" + std::to_string(result.entries.size()) ||
      next() != "TOTAL_BYTES=" + std::to_string(aggregate))
    fail("checked raw trace record count or aggregate differs");
  line = next();
  if (!line.starts_with("CANONICAL_SHA256=") || line.size() != 81)
    fail("checked raw trace record canonical digest is malformed");
  result.canonicalDigest = line.substr(17);
  line = next();
  if (!line.starts_with("INPUTS_SHA256=") || line.size() != 78)
    fail("checked raw trace record input digest is malformed");
  result.inputsDigest = line.substr(14);
  if (next() != "TERMINAL=fe2o3-static-host-lld-checked-raw-traces-v1-end")
    fail("checked raw trace record has no terminal marker");
  if (std::string extra; std::getline(input, extra))
    fail("checked raw trace record has trailing content");
  if (result.entries.empty())
    fail("checked raw trace record is empty");
  result.totalBytes = aggregate;
  result.backing = std::move(backing);
  return result;
}

bool canonicalPhase(std::string_view phase) {
  return !phase.empty() && phase.size() <= 64 &&
         std::all_of(phase.begin(), phase.end(), [](unsigned char item) {
           return (item >= 'a' && item <= 'z') ||
                  (item >= '0' && item <= '9') || item == '-';
         });
}

std::string readLockedLedger(int fd) {
  struct stat before{};
  if (::fstat(fd, &before) != 0 || !S_ISREG(before.st_mode) ||
      before.st_nlink != 1 || before.st_size < 0 ||
      static_cast<uint64_t>(before.st_size) > 16ULL * 1024ULL * 1024ULL)
    fail("global retention ledger has an invalid descriptor identity");
  const auto length = static_cast<size_t>(before.st_size);
  std::string content(length, '\0');
  size_t offset = 0;
  while (offset < length) {
    const ssize_t count = ::pread(fd, content.data() + offset, length - offset,
                                  static_cast<off_t>(offset));
    if (count < 0) {
      if (errno == EINTR)
        continue;
      fail("cannot read global retention ledger");
    }
    if (count == 0)
      fail("global retention ledger shortened while reading");
    offset += static_cast<size_t>(count);
  }
  struct stat after{};
  if (::fstat(fd, &after) != 0 || !sameFileState(before, after))
    fail("global retention ledger changed while reading");
  return content;
}

RetentionLedger parseRetentionLedger(const std::string &content,
                                     uint64_t fileBound, uint64_t byteBound) {
  RetentionLedger result;
  if (content.empty())
    return result;
  std::istringstream input(content);
  auto next = [&]() {
    std::string line;
    if (!std::getline(input, line))
      fail("global retention ledger is truncated");
    return line;
  };
  if (next() != "FORMAT=fe2o3-static-host-lld-global-retention-ledger-v1" ||
      next() != "STATUS=global-precopy-budget-accounting" ||
      next() != "GLOBAL_FILE_BOUND=" + std::to_string(fileBound) ||
      next() != "GLOBAL_BYTE_BOUND=" + std::to_string(byteBound))
    fail("global retention ledger has the wrong header or bounds");
  std::set<std::string> seen;
  std::string line = next();
  while (line.starts_with("P\t")) {
    const std::vector<std::string> fields = splitTabs(line);
    if (fields.size() != 4 || fields[0] != "P" || !canonicalPhase(fields[1]) ||
        !seen.insert(fields[1]).second)
      fail("global retention ledger has a malformed phase row");
    const uint64_t files = parseUnsigned(fields[2], "retention phase files");
    const uint64_t bytes = parseUnsigned(fields[3], "retention phase bytes");
    if (files == 0 || files > fileBound - result.files ||
        bytes > byteBound - result.bytes)
      fail("global retention ledger exceeds its cumulative bounds");
    result.phases.push_back({fields[1], files, bytes});
    result.files += files;
    result.bytes += bytes;
    line = next();
  }
  if (line != "FILES=" + std::to_string(result.files) ||
      next() != "TOTAL_BYTES=" + std::to_string(result.bytes) ||
      next() != "TERMINAL=fe2o3-static-host-lld-global-retention-ledger-v1-end")
    fail("global retention ledger totals or terminal differ");
  if (std::string extra; std::getline(input, extra))
    fail("global retention ledger has trailing content");
  return result;
}

std::string serializeRetentionLedger(const RetentionLedger &ledger,
                                     uint64_t fileBound, uint64_t byteBound) {
  std::ostringstream output;
  output << "FORMAT=fe2o3-static-host-lld-global-retention-ledger-v1\n"
         << "STATUS=global-precopy-budget-accounting\n"
         << "GLOBAL_FILE_BOUND=" << fileBound << "\n"
         << "GLOBAL_BYTE_BOUND=" << byteBound << "\n";
  for (const RetentionPhase &phase : ledger.phases)
    output << "P\t" << phase.name << '\t' << phase.files << '\t' << phase.bytes
           << '\n';
  output << "FILES=" << ledger.files << "\n"
         << "TOTAL_BYTES=" << ledger.bytes << "\n"
         << "TERMINAL=fe2o3-static-host-lld-global-retention-ledger-v1-end\n";
  return output.str();
}

Fd openLockedRetentionLedger(const std::string &path) {
  int descriptor = ::open(path.c_str(), O_RDWR | O_CLOEXEC | O_NOFOLLOW);
  if (descriptor < 0 && errno == ENOENT)
    descriptor = ::open(
        path.c_str(), O_RDWR | O_CREAT | O_EXCL | O_CLOEXEC | O_NOFOLLOW, 0600);
  Fd ledger(descriptor);
  if (!ledger || ::flock(ledger.get(), LOCK_EX) != 0)
    fail("cannot open and lock global retention ledger");
  struct stat fdState{};
  struct stat namedState{};
  if (::fstat(ledger.get(), &fdState) != 0 ||
      ::lstat(path.c_str(), &namedState) != 0 || !S_ISREG(fdState.st_mode) ||
      fdState.st_nlink != 1 || fdState.st_dev != namedState.st_dev ||
      fdState.st_ino != namedState.st_ino || (fdState.st_mode & 07777) != 0600)
    fail("global retention ledger path or mode changed");
  return ledger;
}

void writeLockedRetentionLedger(int fd, const std::string &path,
                                const std::string &content) {
  if (::ftruncate(fd, 0) != 0 || ::lseek(fd, 0, SEEK_SET) != 0)
    fail("cannot reset global retention ledger");
  writeAll(fd, content.data(), content.size());
  if (::fsync(fd) != 0)
    fail("cannot synchronize global retention ledger");
  struct stat fdState{};
  struct stat namedState{};
  if (::fstat(fd, &fdState) != 0 || ::lstat(path.c_str(), &namedState) != 0 ||
      !S_ISREG(fdState.st_mode) || fdState.st_nlink != 1 ||
      fdState.st_dev != namedState.st_dev ||
      fdState.st_ino != namedState.st_ino || fdState.st_size < 0 ||
      static_cast<uint64_t>(fdState.st_size) != content.size() ||
      (fdState.st_mode & 07777) != 0600 || readLockedLedger(fd) != content)
    fail("global retention ledger changed after update");
}

void retainRawEntry(const CheckedRawEntry &entry,
                    const std::string &destination) {
  Fd source(::open(entry.path.c_str(), O_RDONLY | O_CLOEXEC | O_NOFOLLOW));
  if (!source)
    fail("cannot open checked raw trace for retention");
  struct stat before{};
  if (::fstat(source.get(), &before) != 0 || !S_ISREG(before.st_mode) ||
      static_cast<uint64_t>(before.st_dev) != entry.device ||
      static_cast<uint64_t>(before.st_ino) != entry.inode ||
      before.st_size < 0 ||
      static_cast<uint64_t>(before.st_size) != entry.length)
    fail("checked raw trace identity changed before retention");
  Fd output(::open(destination.c_str(),
                   O_RDWR | O_CREAT | O_EXCL | O_CLOEXEC | O_NOFOLLOW, 0600));
  if (!output)
    fail("cannot create retained raw trace");
  Sha256 digest;
  std::array<char, 65536> buffer{};
  uint64_t offset = 0;
  while (offset < entry.length) {
    const size_t requested = static_cast<size_t>(
        std::min<uint64_t>(buffer.size(), entry.length - offset));
    const ssize_t count = ::pread(source.get(), buffer.data(), requested,
                                  static_cast<off_t>(offset));
    if (count < 0)
      fail("cannot read checked raw trace during retention");
    if (count == 0)
      fail("checked raw trace shortened during retention");
    digest.update(buffer.data(), static_cast<size_t>(count));
    writeAll(output.get(), buffer.data(), static_cast<size_t>(count));
    offset += static_cast<uint64_t>(count);
  }
  struct stat after{};
  struct stat named{};
  if (::fstat(source.get(), &after) != 0 ||
      ::lstat(entry.path.c_str(), &named) != 0 ||
      !sameFileState(before, after) || after.st_dev != named.st_dev ||
      after.st_ino != named.st_ino || digest.finish() != entry.digest)
    fail("checked raw trace changed during retention");
  if (::fchmod(output.get(), 0444) != 0 || ::fsync(output.get()) != 0)
    fail("cannot seal retained raw trace mode and bytes");
  struct stat retained{};
  struct stat retainedAfter{};
  struct stat retainedNamed{};
  if (::fstat(output.get(), &retained) != 0 || !S_ISREG(retained.st_mode) ||
      retained.st_size < 0 ||
      static_cast<uint64_t>(retained.st_size) != entry.length ||
      (retained.st_mode & 07777) != 0444)
    fail("retained raw trace has the wrong identity");
  RawBytes retainedBytes = readRawBytes(destination, 0);
  if (::fstat(output.get(), &retainedAfter) != 0 ||
      ::lstat(destination.c_str(), &retainedNamed) != 0 ||
      !sameFileState(retained, retainedAfter) ||
      retainedAfter.st_dev != retainedNamed.st_dev ||
      retainedAfter.st_ino != retainedNamed.st_ino ||
      retainedBytes.device != static_cast<uint64_t>(retained.st_dev) ||
      retainedBytes.inode != static_cast<uint64_t>(retained.st_ino) ||
      retainedBytes.length != entry.length ||
      retainedBytes.digest != entry.digest)
    fail("retained raw trace bytes or identity changed");
}

int retainMain(int argc, char **argv) {
  if (argc != 11 || std::string_view(argv[1]) != "--retain")
    fail("usage: TRACE_CHECK --retain CHECKED_RAW_RECORD SOURCE_PREFIX "
         "DESTINATION_PREFIX CANONICAL INPUT_SET GLOBAL_LEDGER PHASE "
         "GLOBAL_FILE_BOUND GLOBAL_BYTE_BOUND");
  const std::string checkedRecord = argv[2];
  const std::string sourcePrefix = argv[3];
  const std::string destinationPrefix = argv[4];
  const std::string ledgerPath = argv[7];
  const std::string phase = argv[8];
  const uint64_t fileBound = parseUnsigned(argv[9], "global file bound");
  const uint64_t byteBound = parseUnsigned(argv[10], "global byte bound");
  if (!canonicalPhase(phase) || fileBound == 0 ||
      fileBound > kGlobalRetentionFileBound || byteBound == 0 ||
      byteBound > kGlobalRetentionByteBound)
    fail("global retention phase or bounds are invalid");
  CheckedRawRecord record = readCheckedRawRecord(checkedRecord);
  if (record.prefix != sourcePrefix)
    fail("checked raw trace record prefix differs during retention");
  RawBytes canonical = readRawBytes(argv[5], 0);
  RawBytes inputs = readRawBytes(argv[6], canonical.length);
  if (canonical.digest != record.canonicalDigest ||
      inputs.digest != record.inputsDigest)
    fail("canonical or input evidence differs from its checked digest");
  const fs::path destinationParent = fs::path(destinationPrefix).parent_path();
  if (!canonicalTracePrefixName(fs::path(sourcePrefix).filename().string()) ||
      !canonicalTracePrefixName(
          fs::path(destinationPrefix).filename().string()))
    fail("retained trace prefix has a noncanonical basename");
  if (canonicalExisting(destinationParent.string(), "retained trace parent") !=
      destinationParent.string())
    fail("retained trace parent is not canonical");
  const fs::path ledgerParent = fs::path(ledgerPath).parent_path();
  if (canonicalExisting(ledgerParent.string(), "retention ledger parent") !=
      ledgerParent.string())
    fail("global retention ledger parent is not canonical");
  std::set<uint64_t> discovered;
  const fs::path sourceParent = fs::path(sourcePrefix).parent_path();
  const std::string sourceStem =
      fs::path(sourcePrefix).filename().string() + ".";
  for (const fs::directory_entry &item : fs::directory_iterator(sourceParent)) {
    const std::string name = item.path().filename().string();
    if (!name.starts_with(sourceStem))
      continue;
    const std::string pidText = name.substr(sourceStem.size());
    if (pidText.empty() ||
        !std::all_of(pidText.begin(), pidText.end(),
                     [](unsigned char item) {
                       return std::isdigit(static_cast<unsigned char>(item)) !=
                              0;
                     }) ||
        !discovered.insert(parseUnsigned(pidText, "retained trace PID")).second)
      fail("source trace set has a noncanonical entry during retention");
  }
  if (discovered.size() != record.entries.size())
    fail("source trace set changed after checking");
  Fd ledger = openLockedRetentionLedger(ledgerPath);
  RetentionLedger retention = parseRetentionLedger(
      readLockedLedger(ledger.get()), fileBound, byteBound);
  if (std::any_of(
          retention.phases.begin(), retention.phases.end(),
          [&](const RetentionPhase &item) { return item.name == phase; }))
    fail("global retention ledger repeats a phase");
  const uint64_t phaseFiles = static_cast<uint64_t>(record.entries.size());
  if (phaseFiles > fileBound - retention.files ||
      record.totalBytes > byteBound - retention.bytes)
    fail("global raw trace retention budget exceeded before copying");
  for (const CheckedRawEntry &entry : record.entries) {
    if (!discovered.contains(entry.pid))
      fail("source trace set omitted a checked PID during retention");
    const std::string destination =
        destinationPrefix + "." + std::to_string(entry.pid);
    struct stat existing{};
    if (::lstat(destination.c_str(), &existing) == 0 || errno != ENOENT)
      fail("retained raw trace destination already exists");
  }
  for (const CheckedRawEntry &entry : record.entries) {
    retainRawEntry(entry, destinationPrefix + "." + std::to_string(entry.pid));
  }
  RawBytes recordAfter = readRawBytes(checkedRecord, 0);
  RawBytes canonicalAfter = readRawBytes(argv[5], 0);
  RawBytes inputsAfter = readRawBytes(argv[6], canonicalAfter.length);
  if (recordAfter.device != record.backing.device ||
      recordAfter.inode != record.backing.inode ||
      recordAfter.length != record.backing.length ||
      recordAfter.digest != record.backing.digest ||
      canonicalAfter.device != canonical.device ||
      canonicalAfter.inode != canonical.inode ||
      canonicalAfter.length != canonical.length ||
      canonicalAfter.digest != canonical.digest ||
      inputsAfter.device != inputs.device ||
      inputsAfter.inode != inputs.inode ||
      inputsAfter.length != inputs.length ||
      inputsAfter.digest != inputs.digest)
    fail("checked trace evidence changed during retention");
  retention.phases.push_back({phase, phaseFiles, record.totalBytes});
  retention.files += phaseFiles;
  retention.bytes += record.totalBytes;
  writeLockedRetentionLedger(
      ledger.get(), ledgerPath,
      serializeRetentionLedger(retention, fileBound, byteBound));
  return 0;
}

int checkMain(int argc, char **argv) {
  if (argc != 8 || std::string_view(argv[1]) != "--check")
    fail("usage: TRACE_CHECK --check PREFIX CANONICAL INPUT_SET ALLOWLIST "
         "INITIAL_CWD CHECKED_RAW_RECORD");
  const std::string prefix = argv[2];
  const std::string canonicalOutput = argv[3];
  const std::string inputOutput = argv[4];
  const std::string allowlistPath =
      canonicalExisting(argv[5], "trace allowlist");
  const std::string initialCwd = canonicalExisting(argv[6], "initial cwd");
  const std::string checkedRawOutput = argv[7];
  if (canonicalExisting(fs::path(prefix).parent_path().string(),
                        "trace parent") !=
      fs::path(prefix).parent_path().string())
    fail("trace prefix parent is not canonical");

  Admission admission{readAllowlist(allowlistPath), {}, {}, {}};
  auto traces = readTraces(prefix);
  TraceProcessor processor(traces, admission, initialCwd);
  processor.run();

  std::ostringstream canonical;
  canonical << "FORMAT=fe2o3-static-host-lld-canonical-trace-v1\n"
            << "STATUS=measured-observational-admission\n"
            << "ENFORCEMENT=observational-gap-detector-landlock-status-"
               "retained-separately\n"
            << "OUTPUT_ACCESS=follow-resolved-and-no-follow-output-"
               "observation-under-landlock-not-an-input\n"
            << "PIDS=" << processor.processEvidence().size() << "\n";
  for (const auto &[pid, evidence] : processor.processEvidence())
    canonical << "P\t" << pid << '\t' << evidence.first << '\t'
              << evidence.second << "\n";
  for (const std::string &row : admission.canonicalRows)
    canonical << row << '\n';

  std::ostringstream inputs;
  inputs << "FORMAT=fe2o3-static-host-lld-admitted-input-set-v1\n"
         << "STATUS=measured-observational-admission\n";
  for (const std::string &row : admission.inputRows)
    inputs << row << '\n';
  verifyRawEvidence(traces);

  uint64_t totalBytes = 0;
  std::ostringstream checkedRaw;
  checkedRaw << "FORMAT=fe2o3-static-host-lld-checked-raw-traces-v1\n"
             << "STATUS=descriptor-bound-checked-bytes\n"
             << "PREFIX=" << prefix << "\n"
             << "PER_FILE_BYTE_BOUND=" << kPerTraceByteBound << "\n"
             << "AGGREGATE_BYTE_BOUND=" << kAggregateTraceByteBound << "\n";
  for (const auto &[pid, trace] : traces) {
    totalBytes += trace.length;
    checkedRaw << "F\t" << pid << '\t' << trace.device << '\t' << trace.inode
               << '\t' << trace.length << '\t' << trace.digest << '\t'
               << trace.path << '\n';
  }
  checkedRaw << "FILES=" << traces.size() << "\n"
             << "TOTAL_BYTES=" << totalBytes << "\n"
             << "CANONICAL_SHA256=" << sha256(canonical.str()) << "\n"
             << "INPUTS_SHA256=" << sha256(inputs.str()) << "\n"
             << "TERMINAL=fe2o3-static-host-lld-checked-raw-traces-v1-end\n";
  writeExclusive(canonicalOutput, canonical.str());
  writeExclusive(inputOutput, inputs.str());
  writeExclusive(checkedRawOutput, checkedRaw.str());
  return 0;
}

} // namespace

int main(int argc, char **argv) {
  try {
    if (argc > 1 && std::string_view(argv[1]) == "--retain")
      return retainMain(argc, argv);
    return checkMain(argc, argv);
  } catch (const std::exception &error) {
    std::cerr << "fe2o3-static-host-lld-build-trace-check: " << error.what()
              << '\n';
    return 70;
  }
}
