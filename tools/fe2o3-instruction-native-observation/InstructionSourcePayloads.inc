// Task-private evidence retention, NOT production artifact publication.
// A failed write may leave a partial new file; never overwrite/delete/retry.
class RetainedInstructionPayloads {
  struct Pin {
    std::string Name;
    std::array<uint8_t, 32> Digest;
    struct stat Snapshot;
  };
  std::string Path;
  int Directory = -1;
  struct stat DirectoryIdentity {};
  std::vector<Pin> Pins;

  void directoryUnchanged() const {
    struct stat Current {}, Named {};
    require(fstat(Directory, &Current) == 0 && lstat(Path.c_str(), &Named) == 0 &&
        S_ISDIR(Current.st_mode) && Current.st_dev == DirectoryIdentity.st_dev &&
        Current.st_ino == DirectoryIdentity.st_ino && Current.st_mode == DirectoryIdentity.st_mode &&
        Named.st_dev == Current.st_dev && Named.st_ino == Current.st_ino &&
        Named.st_mode == Current.st_mode, "payload directory identity changed");
    char *Resolved = realpath(Path.c_str(), nullptr);
    require(Resolved != nullptr, "payload directory disappeared");
    const bool Equal = Path == Resolved; free(Resolved);
    require(Equal, "payload directory path redirected");
  }
  static std::array<uint8_t, 32> readDigest(int File, size_t Size) {
    require(Size > 0 && Size <= PayloadByteLimit, "payload retained read bound");
    std::vector<uint8_t> Bytes(Size); size_t Offset = 0;
    while (Offset < Size) {
      const ssize_t Count = pread(File, Bytes.data() + Offset, Size - Offset,
          static_cast<off_t>(Offset));
      if (Count < 0 && errno == EINTR) continue;
      require(Count > 0, "retained payload shrank or became unreadable");
      Offset += static_cast<size_t>(Count);
    }
    uint8_t Extra = 0; ssize_t Count;
    do { Count = pread(File, &Extra, 1, static_cast<off_t>(Size)); }
    while (Count < 0 && errno == EINTR);
    require(Count == 0, "retained payload grew");
    return SHA256::hash(Bytes);
  }
public:
  explicit RetainedInstructionPayloads(StringRef Name) : Path(Name.str()) {
    require(Name.starts_with("/") && Name.size() <= 4096 && !Name.ends_with("/") &&
        llvm::none_of(Name, [](char C) {
          return static_cast<unsigned char>(C) < 32 || C == 127;
        }), "fresh bounded absolute payload directory required");
    const auto Slash = Path.find_last_of('/');
    const std::string Parent = Slash == 0 ? "/" : Path.substr(0, Slash);
    char *Resolved = realpath(Parent.c_str(), nullptr);
    require(Resolved != nullptr, "payload directory parent is unavailable");
    const bool Equal = Parent == Resolved; free(Resolved);
    require(Equal && Path.substr(Slash + 1) != "." && Path.substr(Slash + 1) != "..",
        "payload directory parent must be canonical");
    require(mkdir(Path.c_str(), 0700) == 0, "payload directory must be new; no reuse");
    Directory = open(Path.c_str(), O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC);
    require(Directory >= 0 && fstat(Directory, &DirectoryIdentity) == 0 &&
        S_ISDIR(DirectoryIdentity.st_mode), "cannot retain fresh payload directory");
    Pins.reserve(2);
    directoryUnchanged();
  }
  RetainedInstructionPayloads(const RetainedInstructionPayloads &) = delete;
  RetainedInstructionPayloads &operator=(const RetainedInstructionPayloads &) = delete;
  ~RetainedInstructionPayloads() { if (Directory >= 0) close(Directory); }

  json::Object publish(OptimizationLevel Level, const Output &Payload) {
    directoryUnchanged();
    const std::string Name = Level == OptimizationLevel::O0 ? "O0.hsaco" : "O3.hsaco";
    require((Pins.empty() && Level == OptimizationLevel::O0) ||
        (Pins.size() == 1 && Level == OptimizationLevel::O3), "exact ordered output names");
    require(!Payload.Bytes.empty() && Payload.Bytes.size() <= PayloadByteLimit &&
        Payload.Digest == SHA256::hash(Payload.Bytes), "exact linked payload identity");
    const int File = openat(Directory, Name.c_str(),
        O_RDWR | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC, 0600);
    require(File >= 0, "create-new payload file failed; no replacement");
    size_t Offset = 0;
    while (Offset < Payload.Bytes.size()) {
      const ssize_t Count = write(File, Payload.Bytes.data() + Offset, Payload.Bytes.size() - Offset);
      if (Count < 0 && errno == EINTR) continue;
      require(Count > 0, "payload write failed; partial file may remain");
      Offset += static_cast<size_t>(Count);
    }
    require(fsync(File) == 0, "payload fsync failed; complete file may remain");
    struct stat Before {}, After {}, Named {};
    require(fstat(File, &Before) == 0 && S_ISREG(Before.st_mode) &&
        Before.st_size > 0 && static_cast<size_t>(Before.st_size) == Payload.Bytes.size() &&
        readDigest(File, Payload.Bytes.size()) == Payload.Digest &&
        fstat(File, &After) == 0 && fstatat(Directory, Name.c_str(), &Named, AT_SYMLINK_NOFOLLOW) == 0 &&
        sameInputStat(Before, After) && sameInputStat(Before, Named),
        "published payload file differs from linked bytes");
    require(close(File) == 0 && fsync(Directory) == 0, "payload publication sync/close failed");
    Pins.push_back(Pin{Name, Payload.Digest, Before});
    unchanged();
    return json::Object{{"path", Path + "/" + Name}, {"bytes", Payload.Bytes.size()},
        {"sha256", hex(Payload.Digest)}, {"matches_linked_worker_payload", true},
        {"create_new_only", true}, {"production_artifact_authority", false}};
  }
  void unchanged() const {
    directoryUnchanged();
    require(Pins.size() <= 2, "payload file count bound");
    for (const auto &Pin : Pins) {
      const int File = openat(Directory, Pin.Name.c_str(), O_RDONLY | O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC);
      struct stat Before {}, After {}, Named {};
      require(File >= 0 && fstat(File, &Before) == 0 && sameInputStat(Pin.Snapshot, Before) &&
          readDigest(File, static_cast<size_t>(Before.st_size)) == Pin.Digest &&
          fstat(File, &After) == 0 &&
          fstatat(Directory, Pin.Name.c_str(), &Named, AT_SYMLINK_NOFOLLOW) == 0 &&
          sameInputStat(Pin.Snapshot, After) && sameInputStat(Pin.Snapshot, Named),
          "retained linked payload changed after publication");
      require(close(File) == 0, "retained payload close failed");
    }
    directoryUnchanged();
  }
};
