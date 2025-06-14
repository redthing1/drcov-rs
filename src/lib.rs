//! # drcov
//!
//! A high-quality, idiomatic, and self-contained Rust library for parsing and
//! writing DrCov coverage files.
//!
//! This library provides a complete implementation for reading and writing
//! DrCov coverage files, supporting format version 2 with module table versions
//! 2-4 and legacy module tables.
//!
//! ## References
//!
//! - DrCov format analysis: <https://www.ayrx.me/drcov-file-format/>
//! - DynamoRIO drcov tool: <https://dynamorio.org/>
//! - Lighthouse plugin: <https://github.com/gaasedelen/lighthouse>
//!
//! ## Example Usage
//!
//! ```no_run
//! use drcov::{CoverageData, ModuleTableVersion};
//!
//! // Reading a file
//! let coverage = drcov::from_file("coverage.drcov").unwrap();
//!
//! // Creating coverage data using the builder
//! let new_coverage = CoverageData::builder()
//!     .flavor("my_tool")
//!     .module_version(ModuleTableVersion::V4)
//!     .add_module("/bin/program", 0x400000, 0x450000)
//!     .add_module("/lib/libc.so", 0x7fff00000000, 0x7fff00100000)
//!     .add_coverage(0, 0x1000, 32) // module 0, offset 0x1000, size 32
//!     .build()
//!     .unwrap();
//!
//! // Writing to a file
//! drcov::to_file(&new_coverage, "output.drcov").unwrap();
//! ```

use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::Path;

/// A specialized `Result` type for drcov operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Represents errors that can occur during drcov file processing.
#[derive(Debug)]
pub enum Error {
    /// An I/O error occurred while reading or writing.
    Io(io::Error),
    /// The file format is invalid or malformed.
    InvalidFormat(String),
    /// The drcov file version is not supported.
    UnsupportedVersion(u32),
    /// The module table is invalid or malformed.
    InvalidModuleTable(String),
    /// The basic block table is invalid or malformed.
    InvalidBbTable(String),
    /// The data failed a validation check (e.g., inconsistent IDs).
    ValidationError(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "I/O error: {e}"),
            Error::InvalidFormat(msg) => write!(f, "Invalid format: {msg}"),
            Error::UnsupportedVersion(v) => write!(f, "Unsupported drcov version: {v}"),
            Error::InvalidModuleTable(msg) => write!(f, "Invalid module table: {msg}"),
            Error::InvalidBbTable(msg) => write!(f, "Invalid basic block table: {msg}"),
            Error::ValidationError(msg) => write!(f, "Validation error: {msg}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::Io(err)
    }
}

/// Constants used throughout the library.
mod consts {
    pub(crate) const SUPPORTED_FILE_VERSION: u32 = 2;
    pub(crate) const BB_ENTRY_SIZE: usize = 8;
    pub(crate) const VERSION_PREFIX: &str = "DRCOV VERSION: ";
    pub(crate) const FLAVOR_PREFIX: &str = "DRCOV FLAVOR: ";
    pub(crate) const MODULE_TABLE_PREFIX: &str = "Module Table: ";
    pub(crate) const BB_TABLE_PREFIX: &str = "BB Table: ";
    pub(crate) const COLUMNS_PREFIX: &str = "Columns: ";
}

/// DrCov file header containing version and tool information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileHeader {
    pub version: u32,
    pub flavor: String,
}

impl Default for FileHeader {
    fn default() -> Self {
        Self {
            version: consts::SUPPORTED_FILE_VERSION,
            flavor: "drcov".to_string(),
        }
    }
}

/// Module table format versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum ModuleTableVersion {
    #[default]
    Legacy = 1,
    V2 = 2,
    V3 = 3,
    V4 = 4,
}

/// Represents a loaded module/library in the traced process.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModuleEntry {
    pub id: u32,
    pub base: u64,
    pub end: u64,
    pub entry: u64,
    pub path: String,
    pub containing_id: Option<i32>,
    pub offset: Option<u64>,
    pub checksum: Option<u32>,
    pub timestamp: Option<u32>,
}

impl ModuleEntry {
    /// Returns the size of the module in bytes.
    pub fn size(&self) -> u64 {
        self.end.saturating_sub(self.base)
    }

    /// Checks if a given memory address is within this module's range.
    pub fn contains_address(&self, addr: u64) -> bool {
        addr >= self.base && addr < self.end
    }
}

/// Represents an executed basic block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BasicBlock {
    /// Offset of the basic block start from the image base.
    pub start: u32,
    /// Size of the basic block in bytes.
    pub size: u16,
    /// ID of the module where the basic block is located.
    pub module_id: u16,
}

impl BasicBlock {
    /// Calculates the absolute memory address of the basic block.
    pub fn absolute_address(&self, module: &ModuleEntry) -> u64 {
        module.base + self.start as u64
    }
}

/// A builder for creating `CoverageData` instances.
#[derive(Debug, Default)]
pub struct CoverageBuilder {
    data: CoverageData,
}

impl CoverageBuilder {
    /// Sets the tool flavor string.
    pub fn flavor(mut self, flavor: &str) -> Self {
        self.data.header.flavor = flavor.to_string();
        self
    }

    /// Sets the version of the module table to be generated.
    pub fn module_version(mut self, version: ModuleTableVersion) -> Self {
        self.data.module_version = version;
        self
    }

    /// Adds a new module to the coverage data.
    /// The module ID will be assigned sequentially.
    pub fn add_module(mut self, path: &str, base: u64, end: u64) -> Self {
        let id = self.data.modules.len() as u32;
        self.data.modules.push(ModuleEntry {
            id,
            path: path.to_string(),
            base,
            end,
            ..Default::default()
        });
        self
    }

    /// Adds a fully-specified module entry.
    pub fn add_full_module(mut self, module: ModuleEntry) -> Self {
        self.data.modules.push(module);
        self
    }

    /// Adds a new basic block to the coverage data.
    pub fn add_coverage(mut self, module_id: u16, offset: u32, size: u16) -> Self {
        self.data.basic_blocks.push(BasicBlock {
            module_id,
            start: offset,
            size,
        });
        self
    }

    /// Adds a `BasicBlock` struct directly.
    pub fn add_basic_block(mut self, block: BasicBlock) -> Self {
        self.data.basic_blocks.push(block);
        self
    }

    /// Consumes the builder and returns the final `CoverageData`.
    ///
    /// # Errors
    /// Returns a `ValidationError` if the constructed data is inconsistent.
    pub fn build(self) -> Result<CoverageData> {
        self.data.validate()?;
        Ok(self.data)
    }
}

/// Complete drcov coverage data structure.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CoverageData {
    pub header: FileHeader,
    pub module_version: ModuleTableVersion,
    pub modules: Vec<ModuleEntry>,
    pub basic_blocks: Vec<BasicBlock>,
}

impl CoverageData {
    /// Creates a new `CoverageBuilder` to construct `CoverageData`.
    pub fn builder() -> CoverageBuilder {
        CoverageBuilder::default()
    }

    /// Validates the integrity of the coverage data.
    /// Checks for sequential module IDs and valid basic block references.
    pub fn validate(&self) -> Result<()> {
        for (i, module) in self.modules.iter().enumerate() {
            if module.id != i as u32 {
                return Err(Error::ValidationError(format!(
                    "Non-sequential module ID {} at index {}",
                    module.id, i
                )));
            }
        }

        let num_modules = self.modules.len();
        for bb in &self.basic_blocks {
            if bb.module_id as usize >= num_modules {
                return Err(Error::ValidationError(format!(
                    "Basic block references invalid module ID: {}",
                    bb.module_id
                )));
            }
        }
        Ok(())
    }

    /// Finds a module by its ID.
    pub fn find_module(&self, id: u16) -> Option<&ModuleEntry> {
        self.modules.get(id as usize).filter(|m| m.id == id as u32)
    }

    /// Finds a module that contains a given memory address.
    pub fn find_module_by_address(&self, addr: u64) -> Option<&ModuleEntry> {
        self.modules.iter().find(|m| m.contains_address(addr))
    }

    /// Calculates coverage statistics, returning a map of module ID to basic block count.
    pub fn get_coverage_stats(&self) -> HashMap<u16, usize> {
        let mut stats = HashMap::new();
        for bb in &self.basic_blocks {
            *stats.entry(bb.module_id).or_insert(0) += 1;
        }
        stats
    }
}

/// Parses a drcov file from a file path.
pub fn from_file<P: AsRef<Path>>(path: P) -> Result<CoverageData> {
    from_reader(File::open(path)?)
}

/// Parses a drcov file from any reader.
pub fn from_reader<R: Read>(reader: R) -> Result<CoverageData> {
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    // Parse Header
    let version = parse_header_line(&mut reader, &mut line, consts::VERSION_PREFIX)?
        .parse()
        .map_err(|_| Error::InvalidFormat("Malformed version number".into()))?;

    if version != consts::SUPPORTED_FILE_VERSION {
        return Err(Error::UnsupportedVersion(version));
    }

    let flavor = parse_header_line(&mut reader, &mut line, consts::FLAVOR_PREFIX)?.to_string();
    let header = FileHeader { version, flavor };

    // Parse Module Table
    let (modules, module_version) = parse_module_table(&mut reader, &mut line)?;

    // Parse Basic Block Table
    let basic_blocks = parse_bb_table(&mut reader, &mut line)?;

    let data = CoverageData {
        header,
        module_version,
        modules,
        basic_blocks,
    };
    data.validate()?;
    Ok(data)
}

fn parse_header_line<'a>(
    reader: &mut impl BufRead,
    line: &'a mut String,
    prefix: &str,
) -> Result<&'a str> {
    line.clear();
    if reader.read_line(line)? == 0 {
        return Err(Error::InvalidFormat(format!(
            "Expected header line with prefix '{prefix}', but found EOF"
        )));
    }
    line.strip_suffix('\n')
        .unwrap_or(line.as_str())
        .strip_prefix(prefix)
        .ok_or_else(|| {
            Error::InvalidFormat(format!(
                "Invalid header line format, expected prefix '{prefix}'"
            ))
        })
}

fn parse_module_table(
    reader: &mut impl BufRead,
    line: &mut String,
) -> Result<(Vec<ModuleEntry>, ModuleTableVersion)> {
    line.clear();
    reader.read_line(line)?;
    let content = line
        .trim()
        .strip_prefix(consts::MODULE_TABLE_PREFIX)
        .ok_or_else(|| Error::InvalidModuleTable("Missing or malformed header".to_string()))?;

    let (version, count) = if let Some(version_part) = content.strip_prefix("version ") {
        let parts: Vec<_> = version_part.split(',').collect();
        if parts.len() != 2 {
            return Err(Error::InvalidModuleTable(
                "Invalid versioned header format".to_string(),
            ));
        }
        let ver_num = parts[0]
            .trim()
            .parse::<u32>()
            .map_err(|_| Error::InvalidModuleTable("Invalid version number".to_string()))?;
        let count_str = parts[1]
            .trim()
            .strip_prefix("count ")
            .ok_or_else(|| Error::InvalidModuleTable("Missing count".to_string()))?;
        let count = count_str
            .parse::<usize>()
            .map_err(|_| Error::InvalidModuleTable("Invalid count value".to_string()))?;
        (
            match ver_num {
                2 => ModuleTableVersion::V2,
                3 => ModuleTableVersion::V3,
                4 => ModuleTableVersion::V4,
                _ => {
                    return Err(Error::InvalidModuleTable(format!(
                        "Unsupported module table version: {ver_num}"
                    )))
                }
            },
            count,
        )
    } else {
        (
            ModuleTableVersion::Legacy,
            content
                .parse::<usize>()
                .map_err(|_| Error::InvalidModuleTable("Invalid legacy count".to_string()))?,
        )
    };

    let columns = if version != ModuleTableVersion::Legacy {
        line.clear();
        reader.read_line(line)?;
        let columns_str = line
            .trim()
            .strip_prefix(consts::COLUMNS_PREFIX)
            .ok_or_else(|| Error::InvalidModuleTable("Missing columns header".to_string()))?;
        columns_str
            .split(',')
            .map(|s| s.trim().to_string())
            .collect::<Vec<_>>()
    } else {
        vec![
            "id".to_string(),
            "base".to_string(),
            "end".to_string(),
            "entry".to_string(),
            "path".to_string(),
        ]
    };

    let mut modules = Vec::with_capacity(count);
    for i in 0..count {
        line.clear();
        reader.read_line(line)?;
        let module = parse_module_entry(line.trim(), &columns)?;
        if module.id != i as u32 {
            return Err(Error::InvalidModuleTable(format!(
                "Non-sequential module ID. Expected {i}, got {}",
                module.id
            )));
        }
        // No normalization needed - 'start' is already mapped to 'base' in parse_module_entry
        modules.push(module);
    }

    Ok((modules, version))
}

fn parse_module_entry(line: &str, columns: &[String]) -> Result<ModuleEntry> {
    let values: Vec<_> = line.splitn(columns.len(), ',').map(|s| s.trim()).collect();
    if values.len() != columns.len() {
        return Err(Error::InvalidModuleTable(format!(
            "Column count mismatch in line: {line}"
        )));
    }

    let map: HashMap<_, _> = columns.iter().zip(values.iter()).collect();
    let mut entry = ModuleEntry::default();

    let parse_u64 = |key: &str| {
        map.get(&key.to_string())
            .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
    };
    let parse_u32 = |key: &str| {
        map.get(&key.to_string())
            .and_then(|s| u32::from_str_radix(s.trim_start_matches("0x"), 16).ok())
    };

    entry.id = map
        .get(&"id".to_string())
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| Error::InvalidModuleTable("Missing or invalid 'id'".to_string()))?;
    entry.base = parse_u64("base")
        .or_else(|| parse_u64("start"))
        .unwrap_or(0);
    entry.end = parse_u64("end").unwrap_or(0);
    entry.entry = parse_u64("entry").unwrap_or(0);
    entry.path = map
        .get(&"path".to_string())
        .map(|s| s.to_string())
        .unwrap_or_default();
    entry.containing_id = map
        .get(&"containing_id".to_string())
        .and_then(|s| s.parse().ok());
    entry.offset = parse_u64("offset");
    entry.checksum = parse_u32("checksum");
    entry.timestamp = parse_u32("timestamp");

    Ok(entry)
}

fn parse_bb_table(reader: &mut impl BufRead, line: &mut String) -> Result<Vec<BasicBlock>> {
    line.clear();
    // It's possible for the BB table to be missing if there are no blocks
    if reader.read_line(line)? == 0 {
        return Ok(Vec::new());
    }
    let content = line
        .trim()
        .strip_prefix(consts::BB_TABLE_PREFIX)
        .ok_or_else(|| Error::InvalidBbTable("Missing or malformed header".to_string()))?;

    let count = content
        .split_whitespace()
        .next()
        .unwrap_or("0")
        .parse::<usize>()
        .map_err(|_| Error::InvalidBbTable("Invalid block count".to_string()))?;

    if count == 0 {
        return Ok(Vec::new());
    }

    let mut binary_data = vec![0u8; count * consts::BB_ENTRY_SIZE];
    reader.read_exact(&mut binary_data)?;

    let blocks = binary_data
        .chunks_exact(consts::BB_ENTRY_SIZE)
        .map(|chunk| BasicBlock {
            start: u32::from_le_bytes(chunk[0..4].try_into().unwrap()),
            size: u16::from_le_bytes(chunk[4..6].try_into().unwrap()),
            module_id: u16::from_le_bytes(chunk[6..8].try_into().unwrap()),
        })
        .collect();

    Ok(blocks)
}

/// Writes coverage data to a file path.
pub fn to_file<P: AsRef<Path>>(data: &CoverageData, path: P) -> Result<()> {
    to_writer(data, &mut File::create(path)?)
}

/// Writes coverage data to any writer.
pub fn to_writer<W: Write>(data: &CoverageData, writer: &mut W) -> Result<()> {
    data.validate()?;

    // Write header
    writeln!(writer, "{}{}", consts::VERSION_PREFIX, data.header.version)?;
    writeln!(writer, "{}{}", consts::FLAVOR_PREFIX, data.header.flavor)?;

    // Write module table
    if data.module_version == ModuleTableVersion::Legacy {
        writeln!(
            writer,
            "{}{}",
            consts::MODULE_TABLE_PREFIX,
            data.modules.len()
        )?;
    } else {
        writeln!(
            writer,
            "{}version {}, count {}",
            consts::MODULE_TABLE_PREFIX,
            data.module_version as u32,
            data.modules.len()
        )?;

        let has_windows_fields = data
            .modules
            .iter()
            .any(|m| m.checksum.is_some() || m.timestamp.is_some());
        let columns = match data.module_version {
            ModuleTableVersion::Legacy => "id, base, end, entry, path", // Should be unreachable
            ModuleTableVersion::V2 => {
                if has_windows_fields {
                    "id, base, end, entry, checksum, timestamp, path"
                } else {
                    "id, base, end, entry, path"
                }
            }
            ModuleTableVersion::V3 => {
                if has_windows_fields {
                    "id, containing_id, start, end, entry, checksum, timestamp, path"
                } else {
                    "id, containing_id, start, end, entry, path"
                }
            }
            ModuleTableVersion::V4 => {
                if has_windows_fields {
                    "id, containing_id, start, end, entry, offset, checksum, timestamp, path"
                } else {
                    "id, containing_id, start, end, entry, offset, path"
                }
            }
        };
        writeln!(writer, "{}{}", consts::COLUMNS_PREFIX, columns)?;
    }

    for module in &data.modules {
        write_module_line(writer, module, data.module_version)?;
    }

    // Write basic block table
    writeln!(
        writer,
        "{} {} bbs",
        consts::BB_TABLE_PREFIX,
        data.basic_blocks.len()
    )?;
    if !data.basic_blocks.is_empty() {
        let mut binary_data = Vec::with_capacity(data.basic_blocks.len() * consts::BB_ENTRY_SIZE);
        for bb in &data.basic_blocks {
            binary_data.extend_from_slice(&bb.start.to_le_bytes());
            binary_data.extend_from_slice(&bb.size.to_le_bytes());
            binary_data.extend_from_slice(&bb.module_id.to_le_bytes());
        }
        writer.write_all(&binary_data)?;
    }

    Ok(())
}

fn write_module_line(
    writer: &mut impl Write,
    module: &ModuleEntry,
    version: ModuleTableVersion,
) -> Result<()> {
    let mut parts = vec![module.id.to_string()];
    let has_windows_fields = module.checksum.is_some() || module.timestamp.is_some();

    if version >= ModuleTableVersion::V3 {
        parts.push(
            module
                .containing_id
                .map_or_else(|| "-1".to_string(), |id| id.to_string()),
        );
    }

    parts.push(format!("0x{:016x}", module.base));
    parts.push(format!("0x{:016x}", module.end));
    parts.push(format!("0x{:016x}", module.entry));

    if version >= ModuleTableVersion::V4 {
        parts.push(format!("0x{:x}", module.offset.unwrap_or(0)));
    }

    let use_windows_cols = match version {
        ModuleTableVersion::V2 | ModuleTableVersion::V3 | ModuleTableVersion::V4 => {
            has_windows_fields
        }
        _ => false,
    };

    if use_windows_cols {
        parts.push(format!("0x{:08x}", module.checksum.unwrap_or(0)));
        parts.push(format!("0x{:08x}", module.timestamp.unwrap_or(0)));
    }

    parts.push(module.path.clone());

    writeln!(writer, "{}", parts.join(", "))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_error_display() {
        let io_err = Error::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));
        assert!(io_err.to_string().contains("I/O error"));

        let format_err = Error::InvalidFormat("bad format".to_string());
        assert_eq!(format_err.to_string(), "Invalid format: bad format");

        let version_err = Error::UnsupportedVersion(3);
        assert_eq!(version_err.to_string(), "Unsupported drcov version: 3");
    }

    #[test]
    fn test_file_header_default() {
        let header = FileHeader::default();
        assert_eq!(header.version, 2);
        assert_eq!(header.flavor, "drcov");
    }

    #[test]
    fn test_module_entry_methods() {
        let module = ModuleEntry {
            id: 0,
            base: 0x400000,
            end: 0x450000,
            entry: 0x401000,
            path: "/bin/test".to_string(),
            ..Default::default()
        };

        assert_eq!(module.size(), 0x50000);
        assert!(module.contains_address(0x420000));
        assert!(!module.contains_address(0x300000));
        assert!(!module.contains_address(0x460000));
    }

    #[test]
    fn test_basic_block_absolute_address() {
        let module = ModuleEntry {
            id: 0,
            base: 0x400000,
            end: 0x450000,
            entry: 0x401000,
            path: "/bin/test".to_string(),
            ..Default::default()
        };

        let bb = BasicBlock {
            start: 0x1000,
            size: 32,
            module_id: 0,
        };

        assert_eq!(bb.absolute_address(&module), 0x401000);
    }

    #[test]
    fn test_coverage_builder() {
        let coverage = CoverageData::builder()
            .flavor("test_tool")
            .module_version(ModuleTableVersion::V4)
            .add_module("/bin/test", 0x400000, 0x450000)
            .add_module("/lib/libc.so", 0x7fff00000000, 0x7fff00100000)
            .add_coverage(0, 0x1000, 32)
            .add_coverage(1, 0x2000, 16)
            .build()
            .unwrap();

        assert_eq!(coverage.header.flavor, "test_tool");
        assert_eq!(coverage.module_version, ModuleTableVersion::V4);
        assert_eq!(coverage.modules.len(), 2);
        assert_eq!(coverage.basic_blocks.len(), 2);

        assert_eq!(coverage.modules[0].path, "/bin/test");
        assert_eq!(coverage.modules[1].path, "/lib/libc.so");
    }

    #[test]
    fn test_coverage_validation() {
        // Test non-sequential module IDs
        let mut coverage = CoverageData::default();
        coverage.modules.push(ModuleEntry {
            id: 1,
            ..Default::default()
        });
        assert!(coverage.validate().is_err());

        // Test invalid basic block module reference
        let mut coverage = CoverageData::default();
        coverage.modules.push(ModuleEntry {
            id: 0,
            ..Default::default()
        });
        coverage.basic_blocks.push(BasicBlock {
            module_id: 1,
            start: 0,
            size: 0,
        });
        assert!(coverage.validate().is_err());
    }

    #[test]
    fn test_coverage_find_methods() {
        let coverage = CoverageData::builder()
            .add_module("/bin/test", 0x400000, 0x450000)
            .add_module("/lib/libc.so", 0x7fff00000000, 0x7fff00100000)
            .build()
            .unwrap();

        assert!(coverage.find_module(0).is_some());
        assert!(coverage.find_module(2).is_none());

        assert!(coverage.find_module_by_address(0x420000).is_some());
        assert_eq!(
            coverage.find_module_by_address(0x420000).unwrap().path,
            "/bin/test"
        );
        assert!(coverage.find_module_by_address(0x300000).is_none());
    }

    #[test]
    fn test_coverage_stats() {
        let coverage = CoverageData::builder()
            .add_module("/bin/test", 0x400000, 0x450000)
            .add_module("/lib/libc.so", 0x7fff00000000, 0x7fff00100000)
            .add_coverage(0, 0x1000, 32)
            .add_coverage(0, 0x2000, 16)
            .add_coverage(1, 0x3000, 8)
            .build()
            .unwrap();

        let stats = coverage.get_coverage_stats();
        assert_eq!(stats.get(&0), Some(&2));
        assert_eq!(stats.get(&1), Some(&1));
        assert_eq!(stats.get(&2), None);
    }

    #[test]
    fn test_parse_simple_drcov() {
        let drcov_content = "DRCOV VERSION: 2\nDRCOV FLAVOR: test\nModule Table: 1\n0, 0x0000000000400000, 0x0000000000450000, 0x0000000000401000, /bin/test\nBB Table: 0 bbs\n";

        let coverage = from_reader(Cursor::new(drcov_content)).unwrap();
        assert_eq!(coverage.header.version, 2);
        assert_eq!(coverage.header.flavor, "test");
        assert_eq!(coverage.modules.len(), 1);
        assert_eq!(coverage.basic_blocks.len(), 0);
        assert_eq!(coverage.modules[0].path, "/bin/test");
    }

    #[test]
    fn test_parse_versioned_module_table() {
        let drcov_content = "DRCOV VERSION: 2\nDRCOV FLAVOR: test\nModule Table: version 4, count 1\nColumns: id, containing_id, start, end, entry, offset, path\n0, -1, 0x0000000000400000, 0x0000000000450000, 0x0000000000401000, 0x0, /bin/test\nBB Table: 0 bbs\n";

        let coverage = from_reader(Cursor::new(drcov_content)).unwrap();
        assert_eq!(coverage.module_version, ModuleTableVersion::V4);
        assert_eq!(coverage.modules.len(), 1);
        assert_eq!(coverage.modules[0].containing_id, Some(-1));
    }

    #[test]
    fn test_write_and_read_roundtrip() {
        let original = CoverageData::builder()
            .flavor("roundtrip_test")
            .module_version(ModuleTableVersion::V3)
            .add_module("/bin/test", 0x400000, 0x450000)
            .add_coverage(0, 0x1000, 32)
            .build()
            .unwrap();

        let mut buffer = Vec::new();
        to_writer(&original, &mut buffer).unwrap();

        let parsed = from_reader(Cursor::new(buffer)).unwrap();
        assert_eq!(original.header, parsed.header);
        assert_eq!(original.module_version, parsed.module_version);
        assert_eq!(original.modules.len(), parsed.modules.len());
        assert_eq!(original.basic_blocks.len(), parsed.basic_blocks.len());
    }

    #[test]
    fn test_invalid_version() {
        let drcov_content = "DRCOV VERSION: 3\nDRCOV FLAVOR: test\n";
        let result = from_reader(Cursor::new(drcov_content));
        assert!(matches!(result, Err(Error::UnsupportedVersion(3))));
    }

    #[test]
    fn test_malformed_header() {
        let drcov_content = "INVALID HEADER\n";
        let result = from_reader(Cursor::new(drcov_content));
        assert!(matches!(result, Err(Error::InvalidFormat(_))));
    }

    #[test]
    fn test_empty_file() {
        let result = from_reader(Cursor::new(""));
        assert!(matches!(result, Err(Error::InvalidFormat(_))));
    }

    #[test]
    fn test_module_table_version_edge_cases() {
        // Test unsupported module table version
        let drcov_content =
            "DRCOV VERSION: 2\nDRCOV FLAVOR: test\nModule Table: version 99, count 0\n";
        let result = from_reader(Cursor::new(drcov_content));
        assert!(matches!(result, Err(Error::InvalidModuleTable(_))));
    }

    #[test]
    fn test_basic_block_parsing() {
        // Create a drcov with basic blocks
        let header = "DRCOV VERSION: 2\nDRCOV FLAVOR: test\nModule Table: 1\n0, 0x0000000000400000, 0x0000000000450000, 0x0000000000401000, /bin/test\nBB Table: 2 bbs\n";

        let mut data = Vec::new();
        data.extend_from_slice(header.as_bytes());

        // Add two basic blocks in binary format
        data.extend_from_slice(&0x1000u32.to_le_bytes()); // start
        data.extend_from_slice(&32u16.to_le_bytes()); // size
        data.extend_from_slice(&0u16.to_le_bytes()); // module_id

        data.extend_from_slice(&0x2000u32.to_le_bytes()); // start
        data.extend_from_slice(&16u16.to_le_bytes()); // size
        data.extend_from_slice(&0u16.to_le_bytes()); // module_id

        let coverage = from_reader(Cursor::new(data)).unwrap();
        assert_eq!(coverage.basic_blocks.len(), 2);
        assert_eq!(coverage.basic_blocks[0].start, 0x1000);
        assert_eq!(coverage.basic_blocks[0].size, 32);
        assert_eq!(coverage.basic_blocks[1].start, 0x2000);
        assert_eq!(coverage.basic_blocks[1].size, 16);
    }

    #[test]
    fn test_windows_fields_in_modules() {
        let coverage = CoverageData::builder()
            .module_version(ModuleTableVersion::V2)
            .add_full_module(ModuleEntry {
                id: 0,
                base: 0x400000,
                end: 0x450000,
                entry: 0x401000,
                path: "/bin/test".to_string(),
                checksum: Some(0x12345678),
                timestamp: Some(0x87654321),
                ..Default::default()
            })
            .build()
            .unwrap();

        let mut buffer = Vec::new();
        to_writer(&coverage, &mut buffer).unwrap();
        let output = String::from_utf8(buffer).unwrap();

        assert!(output.contains("checksum"));
        assert!(output.contains("timestamp"));
        assert!(output.contains("0x12345678"));
        assert!(output.contains("0x87654321"));
    }
}
