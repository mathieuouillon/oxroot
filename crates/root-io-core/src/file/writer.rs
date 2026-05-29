//! Writing a minimal, ROOT-compatible TFile container.
//!
//! Produces a small-format (32-bit) file: header, the root directory's name
//! key + `TDirectory` record, one `TKey` + object per supplied object, an
//! optional streamer-info record, and the directory key list. The free list is
//! kept empty. All offsets are back-patched once the layout is known.
//! [`update_root_file`] re-reads an existing file and rewrites it with extra
//! objects appended.

use crate::buffer::WBuffer;
use crate::error::{Error, Result};

use super::key::TKey;
use super::rfile::RFile;

/// A fixed creation/modification timestamp (`TDatime`); readers don't validate it.
const DATIME: u32 = 0x7d7a_79ca;
/// On-disk file version (small format, < 1_000_000).
const FILE_VERSION: u32 = 62400;
/// Class used for the directory name key and key-list key.
const DIR_CLASS: &str = "TFile";

/// One object to store in the file: its class, name, title, and already-streamed
/// object bytes (including the object's own byte-count/version header).
pub struct ObjectRecord {
    /// ROOT class name (e.g. `"TH1D"`).
    pub class_name: String,
    /// Object name (the key name).
    pub name: String,
    /// Object title.
    pub title: String,
    /// Streamed object bytes.
    pub object: Vec<u8>,
}

/// Length of a small-format `TKey` header for the given strings.
pub fn key_len(class: &str, name: &str, title: &str) -> u16 {
    // Nbytes(4)+version(2)+ObjLen(4)+Datime(4)+KeyLen(2)+Cycle(2)+SeekKey(4)
    // +SeekPdir(4) = 26, then three length-prefixed strings.
    (26 + (1 + class.len()) + (1 + name.len()) + (1 + title.len())) as u16
}

/// Write a small-format (32-bit seek) `TKey` header (no payload). `obj_len` is
/// the uncompressed object size (`ObjLen`); `payload_len` is the on-disk payload
/// size (equal to `obj_len` when stored uncompressed). `Nbytes = KeyLen +
/// payload_len`.
#[allow(clippy::too_many_arguments)]
pub fn write_key_header(
    w: &mut WBuffer,
    class: &str,
    name: &str,
    title: &str,
    obj_len: u32,
    payload_len: u32,
    seek_key: u64,
    seek_pdir: u64,
) {
    write_key_header_cycle(
        w,
        class,
        name,
        title,
        obj_len,
        payload_len,
        seek_key,
        seek_pdir,
        1,
    );
}

/// Like [`write_key_header`], but with an explicit `cycle` (ROOT bumps the cycle
/// when an object is rewritten under an existing name; the highest cycle wins).
#[allow(clippy::too_many_arguments)]
pub fn write_key_header_cycle(
    w: &mut WBuffer,
    class: &str,
    name: &str,
    title: &str,
    obj_len: u32,
    payload_len: u32,
    seek_key: u64,
    seek_pdir: u64,
    cycle: u16,
) {
    let klen = key_len(class, name, title);
    w.be_i32((klen as u32 + payload_len) as i32); // Nbytes = KeyLen + on-disk payload
    w.be_u16(4); // key version (small format)
    w.be_u32(obj_len);
    w.be_u32(DATIME);
    w.be_u16(klen);
    w.be_u16(cycle);
    w.be_u32(seek_key as u32);
    w.be_u32(seek_pdir as u32);
    w.string(class);
    w.string(name);
    w.string(title);
}

/// The on-disk payload for an object: compressed when `compression != 0` and the
/// result is actually smaller, otherwise the raw object bytes.
fn on_disk_payload(object: &[u8], compression: u32) -> Vec<u8> {
    if compression == 0 {
        return object.to_vec();
    }
    match root_compress::compress(object, compression) {
        Ok(compressed) if compressed.len() < object.len() => compressed,
        _ => object.to_vec(),
    }
}

/// Class/name/title of the streamer-info key. The fixed strings give the key a
/// `KeyLen` of 64 bytes, which the baked `TList` blob's internal class-tag
/// references depend on (ROOT resolves them relative to `-KeyLen`).
const STREAMER_INFO_NAME: &str = "StreamerInfo";
const STREAMER_INFO_TITLE: &str = "Doubly linked list";
const TLIST_CLASS: &str = "TList";

/// Build a complete TFile holding `objects` in its root directory, optionally
/// compressing object payloads (`compression` = `algorithm*100 + level`, 0 = none).
pub fn write_root_file(file_name: &str, objects: &[ObjectRecord], compression: u32) -> Vec<u8> {
    write_root_file_with_streamers(file_name, objects, compression, None)
}

/// Like [`write_root_file`], but also embeds `streamer_info` (the already-streamed
/// `TList<TStreamerInfo>` object bytes) as the file's streamer-info record at
/// `fSeekInfo`, making the file self-describing for any ROOT reader.
pub fn write_root_file_with_streamers(
    file_name: &str,
    objects: &[ObjectRecord],
    compression: u32,
    streamer_info: Option<&[u8]>,
) -> Vec<u8> {
    let payloads: Vec<Vec<u8>> = objects
        .iter()
        .map(|o| on_disk_payload(&o.object, compression))
        .collect();
    let streamer_payload = streamer_info.map(|si| on_disk_payload(si, compression));

    // Per-object cycle: the n-th object sharing a name gets cycle n (1-based), so
    // re-adding an existing name yields a higher, newer cycle (as ROOT does).
    let mut seen: std::collections::HashMap<&str, u16> = std::collections::HashMap::new();
    let cycles: Vec<u16> = objects
        .iter()
        .map(|o| {
            let c = seen.entry(o.name.as_str()).or_insert(0);
            *c += 1;
            *c
        })
        .collect();

    let mut w = WBuffer::new();

    // --- File header (100 bytes; pointers patched at the end). ---
    w.bytes(b"root");
    w.be_u32(FILE_VERSION);
    w.be_u32(100); // fBEGIN
    let p_end = w.reserve(4);
    let p_seek_free = w.reserve(4);
    let p_nbytes_free = w.reserve(4);
    let p_nfree = w.reserve(4);
    let p_nbytes_name = w.reserve(4);
    w.u8(4); // fUnits
    w.be_u32(compression); // fCompress
    let p_seek_info = w.reserve(4);
    let p_nbytes_info = w.reserve(4);
    w.be_u16(1); // fUUID version
    w.bytes(&[0u8; 16]); // fUUID
    while w.len() < 100 {
        w.u8(0);
    }

    // --- Root directory name key + object (at fBEGIN = 100). ---
    let first_klen = key_len(DIR_CLASS, file_name, "");
    let name_title_len = (1 + file_name.len()) + 1; // object name=file_name, title=""
    let f_nbytes_name = first_klen as usize + name_title_len; // dir record starts here
    let dir_record_len = 30 + 18; // TDirectory fields (30) + UUID (18)
    let first_obj_len = (name_title_len + dir_record_len) as u32;

    write_key_header(
        &mut w,
        DIR_CLASS,
        file_name,
        "",
        first_obj_len,
        first_obj_len,
        100,
        0,
    );
    w.string(file_name); // object: name
    w.string(""); // object: title
                  // TDirectory record.
    w.be_i16(5); // version
    w.be_u32(DATIME); // fDatimeC
    w.be_u32(DATIME); // fDatimeM
    let p_dir_nbytes_keys = w.reserve(4);
    w.be_i32(f_nbytes_name as i32); // fNbytesName
    w.be_u32(100); // fSeekDir
    w.be_u32(0); // fSeekParent
    let p_dir_seek_keys = w.reserve(4);
    w.be_u16(1); // UUID version
    w.bytes(&[0u8; 16]); // UUID

    // --- One key + object per stored object. ---
    let mut seeks = Vec::with_capacity(objects.len());
    for (i, obj) in objects.iter().enumerate() {
        let seek = w.len();
        write_key_header_cycle(
            &mut w,
            &obj.class_name,
            &obj.name,
            &obj.title,
            obj.object.len() as u32,
            payloads[i].len() as u32,
            seek as u64,
            100,
            cycles[i],
        );
        w.bytes(&payloads[i]);
        seeks.push(seek);
    }

    // --- Streamer-info record (TList<TStreamerInfo>), referenced by fSeekInfo
    // only (not listed as a directory key). ---
    let (seek_info, nbytes_info) = match (streamer_info, &streamer_payload) {
        (Some(object), Some(payload)) => {
            let seek = w.len();
            write_key_header(
                &mut w,
                TLIST_CLASS,
                STREAMER_INFO_NAME,
                STREAMER_INFO_TITLE,
                object.len() as u32,
                payload.len() as u32,
                seek as u64,
                100,
            );
            w.bytes(payload);
            let klen = key_len(TLIST_CLASS, STREAMER_INFO_NAME, STREAMER_INFO_TITLE) as u32;
            (seek as u32, klen + payload.len() as u32)
        }
        _ => (0, 0),
    };

    // --- Directory key list: a key, then nkeys, then a header per object. ---
    let keylist_seek = w.len();
    let keylist_obj_len = {
        let headers: usize = objects
            .iter()
            .map(|o| key_len(&o.class_name, &o.name, &o.title) as usize)
            .sum();
        (4 + headers) as u32
    };
    write_key_header(
        &mut w,
        DIR_CLASS,
        file_name,
        "",
        keylist_obj_len,
        keylist_obj_len,
        keylist_seek as u64,
        100,
    );
    w.be_i32(objects.len() as i32); // nkeys
    for (i, obj) in objects.iter().enumerate() {
        write_key_header_cycle(
            &mut w,
            &obj.class_name,
            &obj.name,
            &obj.title,
            obj.object.len() as u32,
            payloads[i].len() as u32,
            seeks[i] as u64,
            100,
            cycles[i],
        );
    }
    let keylist_nbytes = key_len(DIR_CLASS, file_name, "") as u32 + keylist_obj_len;
    let f_end = w.len() as u32;

    // --- Back-patch header + directory pointers. ---
    w.patch_be_u32(p_end, f_end);
    w.patch_be_u32(p_seek_free, 0);
    w.patch_be_u32(p_nbytes_free, 0);
    w.patch_be_u32(p_nfree, 0);
    w.patch_be_u32(p_nbytes_name, f_nbytes_name as u32);
    w.patch_be_u32(p_seek_info, seek_info);
    w.patch_be_u32(p_nbytes_info, nbytes_info);
    w.patch_be_u32(p_dir_nbytes_keys, keylist_nbytes);
    w.patch_be_u32(p_dir_seek_keys, keylist_seek as u32);

    w.into_vec()
}

/// Append `new_objects` to an existing ROOT file (`existing` bytes), returning a
/// new file holding the existing objects plus the new ones. `file_name` is the
/// root directory's name; `compression` applies to all object payloads.
///
/// Existing objects are copied (decompressed, then re-emitted); an added object
/// whose name matches an existing one gets a higher, newer cycle, as ROOT does.
/// The file's existing streamer info is preserved unless `streamer_info` is
/// given, in which case that replaces it.
///
/// This rewrites the whole file rather than appending in place, so it does not
/// support files containing an RNTuple (whose anchor stores absolute file
/// offsets that a rewrite would invalidate); such files return an error.
pub fn update_root_file(
    existing: &[u8],
    file_name: &str,
    new_objects: &[ObjectRecord],
    compression: u32,
    streamer_info: Option<&[u8]>,
) -> Result<Vec<u8>> {
    let file = RFile::from_bytes(existing.to_vec())?;

    // Copy existing objects, ordered by ascending cycle so the cycle-by-position
    // assignment in the rewrite reproduces (or extends) their cycles.
    let mut keys: Vec<&TKey> = file.keys().iter().collect();
    keys.sort_by_key(|k| k.cycle);

    let mut objects: Vec<ObjectRecord> = Vec::with_capacity(keys.len() + new_objects.len());
    for key in keys {
        if key.class_name == "ROOT::RNTuple" {
            return Err(Error::Format(
                "updating a file that contains an RNTuple is not supported \
                 (its anchor holds absolute file offsets)"
                    .into(),
            ));
        }
        let payload = &file.data()[key.payload_range()];
        let object = root_compress::decompress(payload, key.obj_len as usize)
            .map_err(|e| Error::Format(format!("decompressing {:?}: {e}", key.name)))?;
        objects.push(ObjectRecord {
            class_name: key.class_name.clone(),
            name: key.name.clone(),
            title: key.title.clone(),
            object,
        });
    }
    for o in new_objects {
        objects.push(ObjectRecord {
            class_name: o.class_name.clone(),
            name: o.name.clone(),
            title: o.title.clone(),
            object: o.object.clone(),
        });
    }

    let existing_si = file.streamer_info_object()?;
    let si = streamer_info.or(existing_si.as_deref());

    Ok(write_root_file_with_streamers(
        file_name,
        &objects,
        compression,
        si,
    ))
}
