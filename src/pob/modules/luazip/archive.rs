use std::{
    cell::RefCell,
    io::{self, Read, Seek, SeekFrom, Take},
    rc::Rc,
};

use flate2::read::DeflateDecoder;

const EOCD_LEN: usize = 22;
const EOCD_MAGIC_BYTES: &[u8] = &[0x50, 0x4B, 0x05, 0x06];
const EOCD_MAX_SIZE: usize = EOCD_LEN + u16::MAX as usize; // fixed fields + variable-sized comment

const EOCD64_LEN: usize = 56;
const EOCD64_MAGIC_BYTES: &[u8] = &[0x50, 0x4B, 0x06, 0x06];

const EOCD64_LOCATOR_LEN: usize = 20;
const EOCD64_LOCATOR_MAGIC_BYTES: &[u8] = &[0x50, 0x4B, 0x06, 0x07];

const CDFH_LEN: usize = 46;
const CDFH_MAGIC_BYTES: &[u8] = &[0x50, 0x4B, 0x01, 0x02];

const LFH_LEN: usize = 30;
const LFH_MAGIC_BYTES: &[u8] = &[0x50, 0x4B, 0x03, 0x04];

// 0 = encrypted | 6 = strong encryption | 13 = header masked
const ENCRYPTED_FLAGS: u16 = (1 << 0) | (1 << 6) | (1 << 13);

#[inline]
fn parse_u16(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(b[at..at.saturating_add(2)].try_into().unwrap())
}

#[inline]
fn parse_u32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at.saturating_add(4)].try_into().unwrap())
}

#[inline]
fn parse_u64(b: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(b[at..at.saturating_add(8)].try_into().unwrap())
}

#[inline]
fn read_u8_slice<R: Read>(reader: &mut R, size: usize) -> io::Result<Box<[u8]>> {
    let mut buf = vec![0; size];
    reader.read_exact(&mut buf)?;
    Ok(buf.into_boxed_slice())
}

#[inline]
fn take_u32(data: &[u8], at: &mut usize) -> io::Result<u32> {
    let bytes = data
        .get(*at..at.saturating_add(4))
        .ok_or_else(|| invalid("truncated extra field"))?;

    *at += 4;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}

#[inline]
fn take_u64(data: &[u8], at: &mut usize) -> io::Result<u64> {
    let bytes = data
        .get(*at..at.saturating_add(8))
        .ok_or_else(|| invalid("truncated extra field"))?;

    *at += 8;
    Ok(u64::from_le_bytes(bytes.try_into().unwrap()))
}

#[inline]
fn invalid(msg: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

#[inline]
fn unsupported(msg: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> io::Error {
    io::Error::new(io::ErrorKind::Unsupported, msg)
}

struct SharedReader<R> {
    inner: Rc<RefCell<R>>,
    pos: u64,
}

impl<R: Read + Seek> Read for SharedReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut r = self.inner.borrow_mut();
        r.seek(SeekFrom::Start(self.pos))?;
        let n = r.read(buf)?;
        self.pos += n as u64;
        Ok(n)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum CompressionMethod {
    Stored,
    Deflate,
    Other(u16),
}

impl From<u16> for CompressionMethod {
    fn from(value: u16) -> Self {
        match value {
            0 => Self::Stored,
            8 => Self::Deflate,
            other => Self::Other(other),
        }
    }
}

#[derive(Debug)]
pub struct ZipArchive<R> {
    reader: Rc<RefCell<R>>,
    entries: Vec<ZipEntry>,
}

impl<R: Read + Seek> ZipArchive<R> {
    pub fn new(mut reader: R) -> io::Result<Self> {
        let cd_info = read_central_directory_info(&mut reader)?;
        let entries = parse_central_directory(
            &mut reader,
            cd_info.cd_offset,
            cd_info.cd_size,
            cd_info.cd_records_on_disk as usize,
        )?;

        Ok(Self {
            reader: Rc::new(RefCell::new(reader)),
            entries,
        })
    }

    pub fn entries(&self) -> &[ZipEntry] {
        &self.entries
    }

    pub fn by_index(&self, index: usize) -> io::Result<ZipFile<R>> {
        let entry = self
            .entries
            .get(index)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "entry not found"))?;

        // reject unsupported entries
        if entry.flags & ENCRYPTED_FLAGS != 0 {
            return Err(unsupported("encrypted entries are not supported"));
        }
        let deflated = match entry.compression {
            CompressionMethod::Stored => {
                if entry.compressed_size != entry.uncompressed_size {
                    return Err(invalid("uncompressed entry has mismatched sizes"));
                }
                false
            }
            CompressionMethod::Deflate => true,
            CompressionMethod::Other(method) => {
                return Err(unsupported(format!(
                    "unsupported compression method {method}"
                )));
            }
        };

        let start = data_start(&mut *self.reader.borrow_mut(), entry)?;
        let limited = SharedReader {
            inner: Rc::clone(&self.reader),
            pos: start,
        }
        .take(entry.compressed_size);

        Ok(ZipFile {
            inner: if deflated {
                Inner::Deflate(DeflateDecoder::new(limited))
            } else {
                Inner::Stored(limited)
            },
            expected_size: entry.uncompressed_size,
            produced: 0,
        })
    }
}

/// Entry metadata
#[derive(Debug)]
pub struct ZipEntry {
    name: Box<[u8]>,
    flags: u16,
    compression: CompressionMethod,
    _crc32: u32,
    compressed_size: u64,
    uncompressed_size: u64,
    file_start_disk: u32,
    local_header_offset: u64,
    extra_fields: Box<[ExtraField]>,
}

impl ZipEntry {
    pub fn name(&self) -> &[u8] {
        &self.name
    }

    pub fn uncompressed_size(&self) -> u64 {
        self.uncompressed_size
    }
}

impl From<CDFH> for ZipEntry {
    fn from(cdfh: CDFH) -> Self {
        Self {
            name: cdfh.file_name,
            flags: cdfh.general_purpose_bit_flag,
            compression: CompressionMethod::from(cdfh.compression_method),
            _crc32: cdfh.crc32,
            compressed_size: u64::from(cdfh.compressed_size),
            uncompressed_size: u64::from(cdfh.uncompressed_size),
            local_header_offset: u64::from(cdfh.local_file_header_offset),
            file_start_disk: u32::from(cdfh.file_start_disk),
            extra_fields: cdfh.extra_fields,
        }
    }
}

enum Inner<R: Read + Seek> {
    Stored(Take<SharedReader<R>>),
    Deflate(DeflateDecoder<Take<SharedReader<R>>>),
}

pub struct ZipFile<R: Read + Seek> {
    inner: Inner<R>,
    expected_size: u64,
    produced: u64,
}

impl<R: Read + Seek> ZipFile<R> {
    pub fn size(&self) -> u64 {
        self.expected_size
    }
}

impl<R: Read + Seek> Read for ZipFile<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        // don't hand out more than one byte past the declared size to protect against malicious
        // archives
        let remaining = self.expected_size - self.produced.min(self.expected_size);
        let max = remaining.saturating_add(1).min(buf.len() as u64) as usize;
        let buf = &mut buf[..max];

        let n = match &mut self.inner {
            Inner::Stored(r) => r.read(buf)?,
            Inner::Deflate(r) => r.read(buf)?,
        };

        if n > 0 {
            self.produced += n as u64;
            if self.produced > self.expected_size {
                return Err(invalid("entry is larger than declared"));
            }
        } else {
            if self.produced != self.expected_size {
                return Err(invalid("entry is smaller than declared"));
            }
        }

        Ok(n)
    }
}

/// Returns the start position of an entrys payload.
fn data_start<R: Read + Seek>(r: &mut R, entry: &ZipEntry) -> io::Result<u64> {
    r.seek(SeekFrom::Start(entry.local_header_offset))?;

    let mut buf = [0u8; LFH_LEN];
    r.read_exact(&mut buf)?;

    if [buf[0], buf[1], buf[2], buf[3]] != LFH_MAGIC_BYTES {
        return Err(invalid("Invalid LFH magic number"));
    }

    let name_len = parse_u16(&buf, 26) as u64;
    let extra_len = parse_u16(&buf, 28) as u64;
    Ok(entry.local_header_offset + LFH_LEN as u64 + name_len + extra_len)
}

fn parse_central_directory<R: Read + Seek>(
    reader: &mut R,
    start: u64,
    size: u64,
    num_of_entries: usize,
) -> io::Result<Vec<ZipEntry>> {
    reader.seek(SeekFrom::Start(start))?;
    let end = start
        .checked_add(size)
        .ok_or_else(|| invalid("central directory too big"))?;
    let mut entries = Vec::new();

    for _ in 0..num_of_entries {
        if reader.stream_position()? > end {
            return Err(invalid("invalid number of entries"));
        }

        let cdfh = parse_cdfh(reader)?;

        // check for ZIP64 sentinel values
        let need_uncompressed = cdfh.uncompressed_size == u32::MAX;
        let need_compressed = cdfh.compressed_size == u32::MAX;
        let need_offset = cdfh.local_file_header_offset == u32::MAX;
        let need_start_disk = cdfh.file_start_disk == u16::MAX;

        let mut entry = ZipEntry::from(cdfh);

        // update entry with info from ZIP64 extra field
        if need_compressed || need_uncompressed || need_offset || need_start_disk {
            let zip64_extra = entry
                .extra_fields
                .iter()
                .find(|extra| extra.header == 0x0001)
                .ok_or_else(|| invalid("ZIP64 extra field missing"))?;

            let data = &zip64_extra.data[..];
            let mut at = 0;

            if need_uncompressed {
                entry.uncompressed_size = take_u64(data, &mut at)?;
            }
            if need_compressed {
                entry.compressed_size = take_u64(data, &mut at)?;
            }
            if need_offset {
                entry.local_header_offset = take_u64(data, &mut at)?;
            }
            if need_start_disk {
                entry.file_start_disk = take_u32(data, &mut at)?;
            }
        }

        if entry.file_start_disk != 0 {
            return Err(unsupported("split archives are not supported"));
        }

        entries.push(entry);
    }

    Ok(entries)
}

/// Central Directory File Header (CDFH).
#[derive(Debug)]
#[allow(clippy::upper_case_acronyms)]
struct CDFH {
    /// Version made by.
    _version_made_by: u16,
    /// Version needed to extract (minimum).
    _version_needed_to_extract: u16,
    /// General purpose bit flag.
    general_purpose_bit_flag: u16,
    /// Compression method.
    compression_method: u16,
    /// File last modification time.
    _last_modification_time: u16,
    /// File last modification date.
    _last_modification_date: u16,
    /// CRC-32 of uncompressed data.
    crc32: u32,
    /// Compressed size (or FF FF FF FF for ZIP64).
    compressed_size: u32,
    /// Uncompressed size (or FF FF FF FF for ZIP64).
    uncompressed_size: u32,
    /// Disk number where file starts (or FF FF for ZIP64).
    file_start_disk: u16,
    /// Internal file attributes.
    _internal_file_attributes: u16,
    /// External file attributes.
    _external_file_attributes: u32,
    /// Relative offset of local file header (or FF FF FF FF for ZIP64). This is the number of
    /// bytes between the start of the first disk on which the file occurs, and the start of the
    /// local file header. This allows software reading the central directory to locate the
    /// position of the file inside the ZIP file.
    local_file_header_offset: u32,
    /// File name.
    file_name: Box<[u8]>,
    /// Extra field.
    extra_fields: Box<[ExtraField]>,
    /// File comment.
    _file_comment: Box<[u8]>,
}

/// Parses a CDFH. `reader` must be positioned at the start of a CDFH.
fn parse_cdfh<R: Read>(reader: &mut R) -> io::Result<CDFH> {
    let mut buf = [0u8; CDFH_LEN];
    reader.read_exact(&mut buf)?;

    if [buf[0], buf[1], buf[2], buf[3]] != CDFH_MAGIC_BYTES {
        return Err(invalid("Invalid CDFH magic number"));
    }

    let file_name_length = parse_u16(&buf, 28) as usize;
    let extra_fields_length = parse_u16(&buf, 30);
    let file_comment_length = parse_u16(&buf, 32) as usize;

    Ok(CDFH {
        _version_made_by: parse_u16(&buf, 4),
        _version_needed_to_extract: parse_u16(&buf, 6),
        general_purpose_bit_flag: parse_u16(&buf, 8),
        compression_method: parse_u16(&buf, 10),
        _last_modification_time: parse_u16(&buf, 12),
        _last_modification_date: parse_u16(&buf, 14),
        crc32: parse_u32(&buf, 16),
        compressed_size: parse_u32(&buf, 20),
        uncompressed_size: parse_u32(&buf, 24),
        file_start_disk: parse_u16(&buf, 34),
        _internal_file_attributes: parse_u16(&buf, 36),
        _external_file_attributes: parse_u32(&buf, 38),
        local_file_header_offset: parse_u32(&buf, 42),
        file_name: read_u8_slice(reader, file_name_length)?,
        extra_fields: read_extra_fields(reader, extra_fields_length)?,
        _file_comment: read_u8_slice(reader, file_comment_length)?,
    })
}

#[derive(Debug)]
struct ExtraField {
    pub header: u16,
    pub data: Box<[u8]>,
}

fn read_extra_fields<R: Read>(reader: &mut R, size: u16) -> io::Result<Box<[ExtraField]>> {
    let mut remaining = usize::from(size);
    let mut fields = Vec::new();

    while remaining > 0 {
        if remaining < 4 {
            return Err(invalid("truncated ZIP extra field"));
        }

        let mut header = [0u8; 4];
        reader.read_exact(&mut header)?;

        let id = parse_u16(&header, 0);
        let data_size = usize::from(parse_u16(&header, 2));

        remaining -= 4;

        if data_size > remaining {
            return Err(invalid("ZIP extra field data exceeds remaining length"));
        }

        let mut data = vec![0u8; data_size];
        reader.read_exact(&mut data)?;

        fields.push(ExtraField {
            header: id,
            data: data.into_boxed_slice(),
        });

        remaining -= data_size;
    }

    Ok(fields.into_boxed_slice())
}

/// Central directory metadata.
#[derive(Debug)]
struct CentralDirectoryInfo {
    /// Number of this disk.
    disk_number: u32,
    /// Disk where central directory starts.
    cd_start_disk: u32,
    /// Number of central directory records on this disk.
    cd_records_on_disk: u64,
    /// Total number of central directory records.
    cd_records_total: u64,
    /// Size of central directory in bytes.
    cd_size: u64,
    /// Offset of start of central directory, relative to start of archive.
    cd_offset: u64,
    /// Comment
    _comment: Box<[u8]>,
}

impl CentralDirectoryInfo {
    fn validate_single_disk(&self) -> io::Result<()> {
        if self.disk_number != 0
            || self.cd_start_disk != 0
            || self.cd_records_on_disk != self.cd_records_total
        {
            return Err(unsupported("split archives are not supported"));
        }
        Ok(())
    }
}

impl From<EOCD> for CentralDirectoryInfo {
    fn from(eocd: EOCD) -> Self {
        Self {
            disk_number: u32::from(eocd.disk_number),
            cd_start_disk: u32::from(eocd.cd_start_disk),
            cd_records_on_disk: u64::from(eocd.cd_records_on_disk),
            cd_records_total: u64::from(eocd.cd_records_total),
            cd_size: u64::from(eocd.cd_size),
            cd_offset: u64::from(eocd.cd_offset),
            _comment: eocd.comment,
        }
    }
}

impl From<EOCD64> for CentralDirectoryInfo {
    fn from(eocd: EOCD64) -> Self {
        Self {
            disk_number: eocd.disk_number,
            cd_start_disk: eocd.cd_start_disk,
            cd_records_on_disk: eocd.cd_records_on_disk,
            cd_records_total: eocd.cd_records_total,
            cd_size: eocd.cd_size,
            cd_offset: eocd.cd_offset,
            _comment: eocd.comment,
        }
    }
}

fn read_central_directory_info<R: Read + Seek>(reader: &mut R) -> io::Result<CentralDirectoryInfo> {
    let eocd_start = find_eocd(reader)?;
    reader.seek(SeekFrom::Start(eocd_start))?;
    let eocd = parse_eocd(reader)?;

    let needs_zip64 = eocd.has_zip64_sentinels();

    // check for presence of EOCD64 Locator which sits right before the EOCD
    let locator_start = eocd_start.saturating_sub(EOCD64_LOCATOR_LEN as u64);
    reader.seek(SeekFrom::Start(locator_start))?;
    let eocd64_locator = parse_eocd64_locator(reader);

    let info = match eocd64_locator {
        Ok(locator) => {
            if locator.cd_start_disk != 0 || locator.disks_total != 1 {
                return Err(unsupported("split archives are not supported"));
            }

            // seek to and parse EOCD64
            reader.seek(SeekFrom::Start(locator.cd_offset))?;
            let eocd64 = parse_eocd64(reader)?;
            CentralDirectoryInfo::from(eocd64)
        }
        Err(_) if needs_zip64 => return Err(invalid("missing EOCD64")),
        Err(_) => CentralDirectoryInfo::from(eocd),
    };

    info.validate_single_disk()?;
    Ok(info)
}

#[derive(Debug)]
struct EOCD64 {
    /// Version made by.
    _version_made_by: u16,
    /// Version needed to extract (minimum).
    _version_needed_to_extract: u16,
    /// Number of this disk.
    disk_number: u32,
    /// Disk where central directory starts.
    cd_start_disk: u32,
    /// Number of central directory records on this disk.
    cd_records_on_disk: u64,
    /// Total number of central directory records.
    cd_records_total: u64,
    /// Size of central directory in bytes.
    cd_size: u64,
    /// Offset of start of central directory, relative to start of archive.
    cd_offset: u64,
    /// Comment.
    comment: Box<[u8]>,
}

/// Parses the EOCD64. `reader` must be positioned at the start of the EOCD64.
fn parse_eocd64<R: Read>(reader: &mut R) -> io::Result<EOCD64> {
    let mut buf = [0u8; EOCD64_LEN];
    reader.read_exact(&mut buf)?;

    if [buf[0], buf[1], buf[2], buf[3]] != EOCD64_MAGIC_BYTES {
        return Err(invalid("Invalid EOCD64 magic number"));
    }

    let size_minus_12 = parse_u64(&buf, 4) + 12;
    let comment_length = size_minus_12 as usize - (EOCD64_LEN - 12);

    Ok(EOCD64 {
        _version_made_by: parse_u16(&buf, 12),
        _version_needed_to_extract: parse_u16(&buf, 14),
        disk_number: parse_u32(&buf, 16),
        cd_start_disk: parse_u32(&buf, 20),
        cd_records_on_disk: parse_u64(&buf, 24),
        cd_records_total: parse_u64(&buf, 32),
        cd_size: parse_u64(&buf, 40),
        cd_offset: parse_u64(&buf, 48),
        comment: read_u8_slice(reader, comment_length)?,
    })
}

#[derive(Debug)]
struct EOCD64Locator {
    /// Disk where EOCD64 starts.
    cd_start_disk: u32,
    /// Offset to start of EOCD64, relative to start of archive.
    cd_offset: u64,
    /// Total number of disks.
    disks_total: u32,
}

/// Parses the EOCD64 Locator. `reader` must be positioned at the start of the EOCD64 Locator.
fn parse_eocd64_locator<R: Read>(reader: &mut R) -> io::Result<EOCD64Locator> {
    let mut buf = [0u8; EOCD64_LOCATOR_LEN];
    reader.read_exact(&mut buf)?;

    if [buf[0], buf[1], buf[2], buf[3]] != EOCD64_LOCATOR_MAGIC_BYTES {
        return Err(invalid("Invalid EOCD64 Locator magic number"));
    }

    Ok(EOCD64Locator {
        cd_start_disk: parse_u32(&buf, 4),
        cd_offset: parse_u64(&buf, 8),
        disks_total: parse_u32(&buf, 16),
    })
}

/// End Of Central Directory record (EOCD).
#[derive(Debug)]
#[allow(clippy::upper_case_acronyms)]
struct EOCD {
    /// Number of this disk (or FF FF for ZIP64).
    disk_number: u16,
    /// Disk where central directory starts (or FF FF for ZIP64).
    cd_start_disk: u16,
    /// Number of central directory records on this disk (or FF FF for ZIP64).
    cd_records_on_disk: u16,
    /// Total number of central directory records (or FF FF for ZIP64).
    cd_records_total: u16,
    /// Size of central directory in bytes (or FF FF FF FF for ZIP64).
    cd_size: u32,
    /// Offset of start of central directory, relative to start of archive (or FF FF FF FF for ZIP64).
    cd_offset: u32,
    /// Comment.
    comment: Box<[u8]>,
}

impl EOCD {
    fn has_zip64_sentinels(&self) -> bool {
        self.disk_number == u16::MAX
            || self.cd_start_disk == u16::MAX
            || self.cd_records_on_disk == u16::MAX
            || self.cd_records_total == u16::MAX
            || self.cd_size == u32::MAX
            || self.cd_offset == u32::MAX
    }
}

/// Returns the start byte position of the EOCD.
fn find_eocd<R: Read + Seek>(reader: &mut R) -> io::Result<u64> {
    // the EOCD is located at the end of the file and is at most `EOCD_MAX_SIZE` bytes long so the
    // search is restricted to that region.
    let file_len = reader.seek(SeekFrom::End(0))?;
    let search_start = file_len.saturating_sub(EOCD_MAX_SIZE as u64);
    reader.seek(SeekFrom::Start(search_start))?;

    let mut search_area = Vec::with_capacity((file_len - search_start) as usize);
    reader.read_to_end(&mut search_area)?;

    let eocd_start_in_search_area = search_area
        .windows(EOCD_MAGIC_BYTES.len())
        .enumerate()
        .rev()
        .find_map(|(pos, window)| {
            if window != EOCD_MAGIC_BYTES {
                return None;
            }

            // we found the magic bytes!
            // but they can theoretically appear in the comment so we need to verify that this
            // candidate header ends exactly at the end of the search area.
            let header_end = pos.checked_add(EOCD_LEN)?;
            let header = search_area.get(pos..header_end)?;
            let comment_length = u16::from_le_bytes([header[20], header[21]]) as usize;
            if header_end.checked_add(comment_length)? == search_area.len() {
                Some(pos)
            } else {
                None
            }
        })
        .ok_or_else(|| invalid("EOCD not found"))?;

    let eocd_start = search_start + eocd_start_in_search_area as u64;
    Ok(eocd_start)
}

/// Parses the EOCD. `reader` must be positioned at the start of the EOCD.
fn parse_eocd<R: Read>(reader: &mut R) -> io::Result<EOCD> {
    let mut buf = [0u8; EOCD_LEN];
    reader.read_exact(&mut buf)?;

    if [buf[0], buf[1], buf[2], buf[3]] != EOCD_MAGIC_BYTES {
        return Err(invalid("Invalid EOCD magic number"));
    }

    let comment_length = parse_u16(&buf, 20) as usize;

    Ok(EOCD {
        disk_number: parse_u16(&buf, 4),
        cd_start_disk: parse_u16(&buf, 6),
        cd_records_on_disk: parse_u16(&buf, 8),
        cd_records_total: parse_u16(&buf, 10),
        cd_size: parse_u32(&buf, 12),
        cd_offset: parse_u32(&buf, 16),
        comment: read_u8_slice(reader, comment_length)?,
    })
}
