# fe2o3-runtime-protocol

Production, authority-aware runtime protocols shared by `cargo-fe2o3` and the
host runtime. The crate owns the Worker V3 load-envelope custody transition,
the application handoff wire, and sealed static-application identity.

Version suffixes identify frozen wire records. They are not selectable
compiler pipelines. Legacy Worker V2 codecs remain outside this crate.
