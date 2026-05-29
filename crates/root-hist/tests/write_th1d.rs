//! M4 (chunk 1): serializing a `TH1D` reproduces ROOT's object bytes exactly.
//!
//! Since the histogram is obtained by reading the fixture and then written
//! back, equality with the fixture's object bytes proves a full byte-level
//! round-trip (write ∘ read == identity) against real ROOT output.

use std::path::PathBuf;

use root_hist::{read_th1d, th1d_to_bytes};
use root_io_core::RFile;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}

#[test]
fn serializes_th1d_byte_identical_to_root() {
    let f = RFile::open(fixture("th1d_uncompressed.root")).expect("open fixture");
    let key = f.key("h1").expect("h1 key");
    assert!(key.is_uncompressed());
    let expected: &[u8] = &f.data()[key.payload_range()];

    let h = read_th1d(&f, "h1").expect("read TH1D");
    let written = th1d_to_bytes(&h);

    assert_eq!(written.len(), expected.len(), "serialized length differs");
    assert_eq!(
        written, expected,
        "serialized TH1D must match ROOT byte-for-byte"
    );
}
