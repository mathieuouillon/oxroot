//! The ROOT (TFile) on-disk container: header, keys, directories, free list,
//! and the [`RFile`] reading entry point.

mod directory;
mod free;
mod header;
mod key;
mod rfile;
mod writer;

pub use directory::Directory;
pub use free::{read_free, FreeSegment};
pub use header::{FileHeader, TUuid, BIG_FILE_VERSION, MAGIC};
pub use key::{TDatime, TKey};
pub use rfile::RFile;
pub use writer::{write_root_file, ObjectRecord};
