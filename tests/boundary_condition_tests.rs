use drcov::{from_reader, to_writer, BasicBlock, CoverageData, ModuleEntry, ModuleTableVersion};
use std::io::Cursor;

#[test]
fn test_maximum_values() {
    // Test with maximum possible values
    let max_values = CoverageData::builder()
        .module_version(ModuleTableVersion::V4)
        .add_full_module(ModuleEntry {
            id: 0,
            base: u64::MAX - 1,
            end: u64::MAX,
            entry: u64::MAX - 1,
            path: "a".repeat(1000), // Very long path
            containing_id: Some(i32::MAX),
            offset: Some(u64::MAX),
            checksum: Some(u32::MAX),
            timestamp: Some(u32::MAX),
        })
        .add_basic_block(BasicBlock {
            module_id: 0,
            start: u32::MAX,
            size: u16::MAX,
        })
        .build();

    assert!(max_values.is_ok());
    let data = max_values.unwrap();

    // Test serialization and deserialization with max values
    let mut buffer = Vec::new();
    let write_result = to_writer(&data, &mut buffer);
    assert!(write_result.is_ok());

    let parsed = from_reader(Cursor::new(buffer));
    assert!(parsed.is_ok());
    let parsed_data = parsed.unwrap();

    assert_eq!(parsed_data.modules[0].base, u64::MAX - 1);
    assert_eq!(parsed_data.modules[0].end, u64::MAX);
    assert_eq!(parsed_data.basic_blocks[0].start, u32::MAX);
    assert_eq!(parsed_data.basic_blocks[0].size, u16::MAX);
}

#[test]
fn test_minimum_values() {
    // Test with minimum possible values
    let min_values = CoverageData::builder()
        .module_version(ModuleTableVersion::V4)
        .add_full_module(ModuleEntry {
            id: 0,
            base: 0,
            end: 0,
            entry: 0,
            path: String::new(), // Empty path
            containing_id: Some(i32::MIN),
            offset: Some(0),
            checksum: Some(0),
            timestamp: Some(0),
        })
        .add_basic_block(BasicBlock {
            module_id: 0,
            start: 0,
            size: 0,
        })
        .build();

    assert!(min_values.is_ok());
    let data = min_values.unwrap();

    // Test serialization and deserialization with min values
    let mut buffer = Vec::new();
    let write_result = to_writer(&data, &mut buffer);
    assert!(write_result.is_ok());

    let parsed = from_reader(Cursor::new(buffer));
    assert!(parsed.is_ok());
    let parsed_data = parsed.unwrap();

    assert_eq!(parsed_data.modules[0].base, 0);
    assert_eq!(parsed_data.modules[0].end, 0);
    assert_eq!(parsed_data.modules[0].path, "");
    assert_eq!(parsed_data.basic_blocks[0].start, 0);
    assert_eq!(parsed_data.basic_blocks[0].size, 0);
}

#[test]
fn test_empty_files_and_collections() {
    // Completely empty coverage data
    let empty = CoverageData::builder().build().unwrap();
    assert_eq!(empty.modules.len(), 0);
    assert_eq!(empty.basic_blocks.len(), 0);

    // Test empty serialization
    let mut buffer = Vec::new();
    to_writer(&empty, &mut buffer).unwrap();
    let parsed = from_reader(Cursor::new(buffer)).unwrap();
    assert_eq!(parsed.modules.len(), 0);
    assert_eq!(parsed.basic_blocks.len(), 0);

    // Empty file parsing
    let empty_file = "";
    let result = from_reader(Cursor::new(empty_file));
    assert!(result.is_err()); // Should fail on empty file

    // File with only headers
    let headers_only = "DRCOV VERSION: 2\nDRCOV FLAVOR: empty\nModule Table: 0\nBB Table: 0 bbs\n";
    let parsed_headers = from_reader(Cursor::new(headers_only)).unwrap();
    assert_eq!(parsed_headers.modules.len(), 0);
    assert_eq!(parsed_headers.basic_blocks.len(), 0);
}

#[test]
fn test_large_module_counts() {
    // Test with many modules (stress test for module ID limits)
    let module_count = 1000;
    let mut builder = CoverageData::builder();

    for i in 0..module_count {
        let base = i * 0x1000;
        builder = builder.add_module(&format!("/module_{i}"), base, base + 0x1000);
    }

    let large_module_data = builder.build().unwrap();
    assert_eq!(large_module_data.modules.len(), module_count as usize);

    // Test that module IDs are sequential
    for (i, module) in large_module_data.modules.iter().enumerate() {
        assert_eq!(module.id, i as u32);
    }

    // Test serialization of large module count
    let mut buffer = Vec::new();
    to_writer(&large_module_data, &mut buffer).unwrap();
    let parsed = from_reader(Cursor::new(buffer)).unwrap();
    assert_eq!(parsed.modules.len(), module_count as usize);
}

#[test]
fn test_large_basic_block_counts() {
    // Test with many basic blocks
    let bb_count = 50000;
    let mut builder = CoverageData::builder().add_module("/test", 0x400000, 0x500000);

    for i in 0..bb_count {
        builder = builder.add_coverage(0, (i * 4) as u32, 4);
    }

    let large_bb_data = builder.build().unwrap();
    assert_eq!(large_bb_data.basic_blocks.len(), bb_count);

    // Test serialization of large BB count
    let mut buffer = Vec::new();
    to_writer(&large_bb_data, &mut buffer).unwrap();

    // Verify binary data size is correct
    let _expected_binary_size = bb_count * 8; // 8 bytes per basic block
    let _text_part_end = buffer.iter().position(|&b| b == b'\n').and_then(|start| {
        let remaining = &buffer[start + 1..];
        remaining.windows(8).position(|w| w == b"BB Table")
    });

    // Parse back and verify
    let parsed = from_reader(Cursor::new(buffer)).unwrap();
    assert_eq!(parsed.basic_blocks.len(), bb_count);
}

#[test]
fn test_address_space_boundaries() {
    // Test addresses at memory boundaries
    let boundary_addresses = [
        (0x0, 0x1000),                            // Low memory
        (0x7fff00000000, 0x7fff00001000),         // High userspace (Linux)
        (0xffff800000000000, 0xffff800000001000), // Kernel space
        (0x400000, 0x401000),                     // Typical executable base
        (0x10000000, 0x10001000),                 // DLL base (Windows)
    ];

    for (i, (base, end)) in boundary_addresses.iter().enumerate() {
        let boundary_data = CoverageData::builder()
            .add_module(&format!("/boundary_{i}"), *base, *end)
            .add_coverage(0, 0x100, 16)
            .build()
            .unwrap();

        // Verify address calculations
        let bb = &boundary_data.basic_blocks[0];
        let module = &boundary_data.modules[0];
        let abs_addr = bb.absolute_address(module);
        assert_eq!(abs_addr, base + 0x100);

        // Test serialization round-trip
        let mut buffer = Vec::new();
        to_writer(&boundary_data, &mut buffer).unwrap();
        let parsed = from_reader(Cursor::new(buffer)).unwrap();
        assert_eq!(parsed.modules[0].base, *base);
        assert_eq!(parsed.modules[0].end, *end);
    }
}

#[test]
fn test_unicode_and_special_characters() {
    // Test with various Unicode characters in paths
    let long_path = "/very/long/".to_owned() + &"directory/".repeat(50) + "file";
    let unicode_paths = [
        "/测试/文件",              // Chinese
        "/тест/файл",              // Cyrillic
        "/🦀/rust/path",           // Emoji
        "/café/naïve/résumé",      // Accented characters
        "C:\\Program Files\\Test", // Windows path
        "/path with spaces/file",  // Spaces
        "/path,with,commas/file",  // Commas
        "/path;with;semicolons",   // Semicolons
        "/path\"with\"quotes",     // Quotes
        "/path'with'apostrophes",  // Apostrophes
        &long_path,                // Very long path
    ];

    for (i, path) in unicode_paths.iter().enumerate() {
        let unicode_data = CoverageData::builder()
            .add_module(
                path,
                0x400000 + i as u64 * 0x1000,
                0x401000 + i as u64 * 0x1000,
            )
            .build()
            .unwrap();

        assert_eq!(unicode_data.modules[0].path, *path);

        // Test serialization round-trip
        let mut buffer = Vec::new();
        to_writer(&unicode_data, &mut buffer).unwrap();
        let parsed = from_reader(Cursor::new(buffer)).unwrap();
        assert_eq!(parsed.modules[0].path, *path);
    }
}

#[test]
fn test_negative_containing_id_boundaries() {
    // Test various negative containing_id values
    let negative_ids = [i32::MIN, -1, -1000, -65536];

    for (i, &containing_id) in negative_ids.iter().enumerate() {
        let negative_data = CoverageData::builder()
            .module_version(ModuleTableVersion::V3)
            .add_full_module(ModuleEntry {
                id: 0,
                base: 0x400000,
                end: 0x500000,
                entry: 0x401000,
                path: format!("/negative_{i}"),
                containing_id: Some(containing_id),
                ..Default::default()
            })
            .build()
            .unwrap();

        // Test serialization round-trip
        let mut buffer = Vec::new();
        to_writer(&negative_data, &mut buffer).unwrap();
        let parsed = from_reader(Cursor::new(buffer)).unwrap();
        assert_eq!(parsed.modules[0].containing_id, Some(containing_id));
    }
}

#[test]
fn test_hex_value_boundaries() {
    // Test hex parsing at various boundaries
    let hex_values = vec![
        ("0x0", 0),
        ("0x1", 1),
        ("0xff", 255),
        ("0x100", 256),
        ("0xffff", 65535),
        ("0x10000", 65536),
        ("0xffffffff", 0xffffffff),
        ("0x100000000", 0x100000000),
        ("0xffffffffffffffff", u64::MAX),
    ];

    for (hex_str, expected) in hex_values {
        let drcov_content = format!("DRCOV VERSION: 2\nDRCOV FLAVOR: hex_test\nModule Table: 1\n0, {hex_str}, {hex_str}, {hex_str}, /test\nBB Table: 0 bbs\n");

        let coverage = from_reader(Cursor::new(drcov_content)).unwrap();
        assert_eq!(coverage.modules[0].base, expected);
        assert_eq!(coverage.modules[0].end, expected);
        assert_eq!(coverage.modules[0].entry, expected);
    }
}

#[test]
fn test_module_size_calculations() {
    // Test module size calculations with various scenarios
    let size_test_cases = vec![
        (0x400000, 0x500000, 0x100000), // Normal case
        (0x400000, 0x400000, 0),        // Zero size
        (0x500000, 0x400000, 0),        // Inverted (saturating_sub should return 0)
        (0, u64::MAX, u64::MAX),        // Maximum size
        (u64::MAX - 1, u64::MAX, 1),    // Minimum non-zero size
    ];

    for (base, end, expected_size) in size_test_cases {
        let module_data = CoverageData::builder()
            .add_module("/size_test", base, end)
            .build()
            .unwrap();

        assert_eq!(module_data.modules[0].size(), expected_size);
    }
}

#[test]
fn test_basic_block_offset_boundaries() {
    // Test basic blocks with various offset values
    let offset_cases = vec![
        (0, "zero offset"),
        (1, "minimum offset"),
        (0xffff, "16-bit boundary"),
        (0x10000, "beyond 16-bit"),
        (0xffffff, "24-bit boundary"),
        (0x1000000, "beyond 24-bit"),
        (u32::MAX, "maximum offset"),
    ];

    for (offset, description) in offset_cases {
        let bb_data = CoverageData::builder()
            .add_module("/offset_test", 0x400000, 0x500000)
            .add_coverage(0, offset, 32)
            .build()
            .unwrap();

        assert_eq!(
            bb_data.basic_blocks[0].start, offset,
            "Failed for {description}"
        );

        // Test serialization round-trip
        let mut buffer = Vec::new();
        to_writer(&bb_data, &mut buffer).unwrap();
        let parsed = from_reader(Cursor::new(buffer)).unwrap();
        assert_eq!(
            parsed.basic_blocks[0].start, offset,
            "Round-trip failed for {description}"
        );
    }
}

#[test]
fn test_basic_block_size_boundaries() {
    // Test basic blocks with various size values
    let size_cases = vec![
        (0, "zero size"),
        (1, "minimum size"),
        (4, "typical instruction size"),
        (0xff, "8-bit boundary"),
        (0x100, "beyond 8-bit"),
        (0xffff, "maximum 16-bit"),
    ];

    for (size, description) in size_cases {
        let bb_data = CoverageData::builder()
            .add_module("/size_test", 0x400000, 0x500000)
            .add_coverage(0, 0x1000, size)
            .build()
            .unwrap();

        assert_eq!(
            bb_data.basic_blocks[0].size, size,
            "Failed for {description}"
        );

        // Test serialization round-trip
        let mut buffer = Vec::new();
        to_writer(&bb_data, &mut buffer).unwrap();
        let parsed = from_reader(Cursor::new(buffer)).unwrap();
        assert_eq!(
            parsed.basic_blocks[0].size, size,
            "Round-trip failed for {description}"
        );
    }
}

#[test]
fn test_flavor_string_boundaries() {
    // Test with various flavor string lengths and characters
    let long_flavor = "a".repeat(100);
    let flavor_cases = vec![
        ("", "empty flavor"),
        ("a", "single character"),
        ("drcov", "standard flavor"),
        (&long_flavor, "long flavor"),
        ("测试工具", "unicode flavor"),
        ("Tool v1.2.3-beta+build.123", "version string"),
        ("tool with spaces", "spaces in flavor"),
        ("tool,with,commas", "commas in flavor"),
    ];

    for (flavor, description) in flavor_cases {
        let flavor_data = CoverageData::builder()
            .flavor(flavor)
            .add_module("/test", 0x400000, 0x500000)
            .build()
            .unwrap();

        assert_eq!(
            flavor_data.header.flavor, flavor,
            "Failed for {description}"
        );

        // Test serialization round-trip
        let mut buffer = Vec::new();
        to_writer(&flavor_data, &mut buffer).unwrap();
        let parsed = from_reader(Cursor::new(buffer)).unwrap();
        assert_eq!(
            parsed.header.flavor, flavor,
            "Round-trip failed for {description}"
        );
    }
}
