use drcov::{from_reader, to_writer, CoverageData, ModuleTableVersion};
use std::io::Cursor;
use tempfile::NamedTempFile;

#[test]
fn test_file_roundtrip() {
    let original = CoverageData::builder()
        .flavor("integration_test")
        .module_version(ModuleTableVersion::V4)
        .add_module("/usr/bin/test", 0x400000, 0x500000)
        .add_module(
            "/lib/x86_64-linux-gnu/libc.so.6",
            0x7ffff7a00000,
            0x7ffff7c00000,
        )
        .add_coverage(0, 0x1000, 32)
        .add_coverage(0, 0x2000, 16)
        .add_coverage(1, 0x10000, 64)
        .build()
        .unwrap();

    // Write to temporary file
    let temp_file = NamedTempFile::new().unwrap();
    drcov::to_file(&original, temp_file.path()).unwrap();

    // Read back from file
    let parsed = drcov::from_file(temp_file.path()).unwrap();

    assert_eq!(original.header.version, parsed.header.version);
    assert_eq!(original.header.flavor, parsed.header.flavor);
    assert_eq!(original.module_version, parsed.module_version);
    assert_eq!(original.modules.len(), parsed.modules.len());
    assert_eq!(original.basic_blocks.len(), parsed.basic_blocks.len());

    for (orig_mod, parsed_mod) in original.modules.iter().zip(parsed.modules.iter()) {
        assert_eq!(orig_mod.id, parsed_mod.id);
        assert_eq!(orig_mod.base, parsed_mod.base);
        assert_eq!(orig_mod.end, parsed_mod.end);
        assert_eq!(orig_mod.path, parsed_mod.path);
    }

    for (orig_bb, parsed_bb) in original.basic_blocks.iter().zip(parsed.basic_blocks.iter()) {
        assert_eq!(orig_bb.start, parsed_bb.start);
        assert_eq!(orig_bb.size, parsed_bb.size);
        assert_eq!(orig_bb.module_id, parsed_bb.module_id);
    }
}

#[test]
fn test_legacy_format_support() {
    let legacy_drcov = "DRCOV VERSION: 2
DRCOV FLAVOR: legacy_test
Module Table: 2
0, 0x0000000000400000, 0x0000000000500000, 0x0000000000401000, /usr/bin/test
1, 0x00007ffff7a00000, 0x00007ffff7c00000, 0x00007ffff7a01000, /lib/x86_64-linux-gnu/libc.so.6
BB Table: 1 bbs
";

    let mut data = Vec::new();
    data.extend_from_slice(legacy_drcov.as_bytes());

    // Add one basic block
    data.extend_from_slice(&0x1000u32.to_le_bytes()); // start
    data.extend_from_slice(&32u16.to_le_bytes()); // size
    data.extend_from_slice(&0u16.to_le_bytes()); // module_id

    let coverage = from_reader(Cursor::new(data)).unwrap();

    assert_eq!(coverage.header.flavor, "legacy_test");
    assert_eq!(coverage.module_version, ModuleTableVersion::Legacy);
    assert_eq!(coverage.modules.len(), 2);
    assert_eq!(coverage.basic_blocks.len(), 1);
    assert_eq!(coverage.modules[0].path, "/usr/bin/test");
    assert_eq!(coverage.modules[1].path, "/lib/x86_64-linux-gnu/libc.so.6");
}

#[test]
fn test_v2_module_table() {
    let v2_drcov = "DRCOV VERSION: 2
DRCOV FLAVOR: v2_test
Module Table: version 2, count 1
Columns: id, base, end, entry, checksum, timestamp, path
0, 0x0000000000400000, 0x0000000000500000, 0x0000000000401000, 0x12345678, 0x87654321, /usr/bin/test
BB Table: 0 bbs
";

    let coverage = from_reader(Cursor::new(v2_drcov)).unwrap();
    assert_eq!(coverage.module_version, ModuleTableVersion::V2);
    assert_eq!(coverage.modules[0].checksum, Some(0x12345678));
    assert_eq!(coverage.modules[0].timestamp, Some(0x87654321));
}

#[test]
fn test_v3_module_table() {
    let v3_drcov = "DRCOV VERSION: 2
DRCOV FLAVOR: v3_test
Module Table: version 3, count 1
Columns: id, containing_id, start, end, entry, path
0, -1, 0x0000000000400000, 0x0000000000500000, 0x0000000000401000, /usr/bin/test
BB Table: 0 bbs
";

    let coverage = from_reader(Cursor::new(v3_drcov)).unwrap();
    assert_eq!(coverage.module_version, ModuleTableVersion::V3);
    assert_eq!(coverage.modules[0].containing_id, Some(-1));
}

#[test]
fn test_large_file_handling() {
    // Create a large coverage file with many modules and basic blocks
    let mut builder = CoverageData::builder()
        .flavor("large_test")
        .module_version(ModuleTableVersion::V4);

    // Add 100 modules
    for i in 0..100 {
        let base = 0x400000 + i * 0x100000;
        builder = builder.add_module(&format!("/usr/lib/module_{i}.so"), base, base + 0x50000);
    }

    // Add 1000 basic blocks distributed across modules
    for i in 0..1000 {
        let module_id = (i % 100) as u16;
        let offset = 0x1000 + (i / 100) * 0x100;
        builder = builder.add_coverage(module_id, offset as u32, 32);
    }

    let coverage = builder.build().unwrap();
    assert_eq!(coverage.modules.len(), 100);
    assert_eq!(coverage.basic_blocks.len(), 1000);

    // Test roundtrip with large file
    let mut buffer = Vec::new();
    to_writer(&coverage, &mut buffer).unwrap();
    let parsed = from_reader(Cursor::new(buffer)).unwrap();

    assert_eq!(coverage.modules.len(), parsed.modules.len());
    assert_eq!(coverage.basic_blocks.len(), parsed.basic_blocks.len());
}

#[test]
fn test_empty_basic_blocks() {
    let empty_bb_drcov = "DRCOV VERSION: 2
DRCOV FLAVOR: empty_test
Module Table: 1
0, 0x0000000000400000, 0x0000000000500000, 0x0000000000401000, /usr/bin/test
BB Table: 0 bbs
";

    let coverage = from_reader(Cursor::new(empty_bb_drcov)).unwrap();
    assert_eq!(coverage.basic_blocks.len(), 0);
    assert_eq!(coverage.modules.len(), 1);
}

#[test]
fn test_error_handling() {
    // Test invalid version
    let invalid_version = "DRCOV VERSION: 999
DRCOV FLAVOR: test
";
    assert!(from_reader(Cursor::new(invalid_version)).is_err());

    // Test missing header
    let no_header = "NOT A DRCOV FILE
";
    assert!(from_reader(Cursor::new(no_header)).is_err());

    // Test truncated file
    let truncated = "DRCOV VERSION: 2
DRCOV FLAVOR: test
Module Table: 1
";
    assert!(from_reader(Cursor::new(truncated)).is_err());
}

#[test]
fn test_coverage_analysis() {
    let coverage = CoverageData::builder()
        .add_module("/bin/main", 0x400000, 0x500000)
        .add_module("/lib/helper.so", 0x7fff00000000, 0x7fff00100000)
        .add_coverage(0, 0x1000, 32)
        .add_coverage(0, 0x2000, 16)
        .add_coverage(0, 0x3000, 64)
        .add_coverage(1, 0x5000, 8)
        .build()
        .unwrap();

    let stats = coverage.get_coverage_stats();
    assert_eq!(stats.get(&0), Some(&3)); // 3 blocks in module 0
    assert_eq!(stats.get(&1), Some(&1)); // 1 block in module 1

    // Test address lookup
    let main_module = coverage.find_module_by_address(0x420000).unwrap();
    assert_eq!(main_module.path, "/bin/main");

    let helper_module = coverage.find_module_by_address(0x7fff00050000).unwrap();
    assert_eq!(helper_module.path, "/lib/helper.so");

    assert!(coverage.find_module_by_address(0x600000).is_none());
}

#[test]
fn test_builder_validation() {
    // Test that builder validates data
    let invalid_coverage = CoverageData::builder()
        .add_module("/bin/test", 0x400000, 0x500000)
        .add_coverage(1, 0x1000, 32) // References non-existent module 1
        .build();

    assert!(invalid_coverage.is_err());
}
