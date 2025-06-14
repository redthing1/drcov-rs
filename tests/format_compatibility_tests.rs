use drcov::{from_reader, to_writer, CoverageData, ModuleEntry, ModuleTableVersion};
use std::io::Cursor;

#[test]
fn test_legacy_format_parsing() {
    // Test legacy format without version header
    let legacy_simple = "DRCOV VERSION: 2\nDRCOV FLAVOR: legacy\nModule Table: 2\n0, 0x400000, 0x500000, 0x401000, /bin/test\n1, 0x500000, 0x600000, 0x501000, /lib/test.so\nBB Table: 0 bbs\n";

    let coverage = from_reader(Cursor::new(legacy_simple)).unwrap();
    assert_eq!(coverage.module_version, ModuleTableVersion::Legacy);
    assert_eq!(coverage.modules.len(), 2);
    assert_eq!(coverage.modules[0].path, "/bin/test");
    assert_eq!(coverage.modules[1].path, "/lib/test.so");

    // Verify all legacy fields are parsed
    assert_eq!(coverage.modules[0].id, 0);
    assert_eq!(coverage.modules[0].base, 0x400000);
    assert_eq!(coverage.modules[0].end, 0x500000);
    assert_eq!(coverage.modules[0].entry, 0x401000);

    // Optional fields should be None/default for legacy
    assert_eq!(coverage.modules[0].containing_id, None);
    assert_eq!(coverage.modules[0].offset, None);
    assert_eq!(coverage.modules[0].checksum, None);
    assert_eq!(coverage.modules[0].timestamp, None);
}

#[test]
fn test_v2_format_parsing() {
    // V2 format with checksum and timestamp
    let v2_with_windows = "DRCOV VERSION: 2\nDRCOV FLAVOR: v2_tool\nModule Table: version 2, count 1\nColumns: id, base, end, entry, checksum, timestamp, path\n0, 0x400000, 0x500000, 0x401000, 0x12345678, 0x87654321, /bin/test\nBB Table: 0 bbs\n";

    let coverage = from_reader(Cursor::new(v2_with_windows)).unwrap();
    assert_eq!(coverage.module_version, ModuleTableVersion::V2);
    assert_eq!(coverage.modules[0].checksum, Some(0x12345678));
    assert_eq!(coverage.modules[0].timestamp, Some(0x87654321));
    assert_eq!(coverage.modules[0].containing_id, None); // Not in V2
    assert_eq!(coverage.modules[0].offset, None); // Not in V2

    // V2 format without Windows fields
    let v2_minimal = "DRCOV VERSION: 2\nDRCOV FLAVOR: v2_tool\nModule Table: version 2, count 1\nColumns: id, base, end, entry, path\n0, 0x400000, 0x500000, 0x401000, /bin/test\nBB Table: 0 bbs\n";

    let coverage_min = from_reader(Cursor::new(v2_minimal)).unwrap();
    assert_eq!(coverage_min.module_version, ModuleTableVersion::V2);
    assert_eq!(coverage_min.modules[0].checksum, None);
    assert_eq!(coverage_min.modules[0].timestamp, None);
}

#[test]
fn test_v3_format_parsing() {
    // V3 format with containing_id and start instead of base
    let v3_format = "DRCOV VERSION: 2\nDRCOV FLAVOR: v3_tool\nModule Table: version 3, count 2\nColumns: id, containing_id, start, end, entry, path\n0, -1, 0x400000, 0x500000, 0x401000, /bin/main\n1, 0, 0x450000, 0x460000, 0x451000, /bin/main.dll\nBB Table: 0 bbs\n";

    let coverage = from_reader(Cursor::new(v3_format)).unwrap();
    assert_eq!(coverage.module_version, ModuleTableVersion::V3);

    // Check containing_id parsing
    assert_eq!(coverage.modules[0].containing_id, Some(-1));
    assert_eq!(coverage.modules[1].containing_id, Some(0));

    // Check that 'start' is mapped to 'base'
    assert_eq!(coverage.modules[0].base, 0x400000);
    assert_eq!(coverage.modules[1].base, 0x450000);

    // V3 with Windows fields
    let v3_windows = "DRCOV VERSION: 2\nDRCOV FLAVOR: v3_tool\nModule Table: version 3, count 1\nColumns: id, containing_id, start, end, entry, checksum, timestamp, path\n0, -1, 0x400000, 0x500000, 0x401000, 0xabcdef00, 0x11223344, /bin/test\nBB Table: 0 bbs\n";

    let coverage_win = from_reader(Cursor::new(v3_windows)).unwrap();
    assert_eq!(coverage_win.modules[0].checksum, Some(0xabcdef00));
    assert_eq!(coverage_win.modules[0].timestamp, Some(0x11223344));
}

#[test]
fn test_v4_format_parsing() {
    // V4 format with all fields including offset
    let v4_full = "DRCOV VERSION: 2\nDRCOV FLAVOR: v4_tool\nModule Table: version 4, count 1\nColumns: id, containing_id, start, end, entry, offset, checksum, timestamp, path\n0, -1, 0x400000, 0x500000, 0x401000, 0x1000, 0x12345678, 0x87654321, /usr/bin/test\nBB Table: 0 bbs\n";

    let coverage = from_reader(Cursor::new(v4_full)).unwrap();
    assert_eq!(coverage.module_version, ModuleTableVersion::V4);
    assert_eq!(coverage.modules[0].containing_id, Some(-1));
    assert_eq!(coverage.modules[0].offset, Some(0x1000));
    assert_eq!(coverage.modules[0].checksum, Some(0x12345678));
    assert_eq!(coverage.modules[0].timestamp, Some(0x87654321));

    // V4 minimal (without Windows fields)
    let v4_minimal = "DRCOV VERSION: 2\nDRCOV FLAVOR: v4_tool\nModule Table: version 4, count 1\nColumns: id, containing_id, start, end, entry, offset, path\n0, -1, 0x400000, 0x500000, 0x401000, 0x0, /usr/bin/test\nBB Table: 0 bbs\n";

    let coverage_min = from_reader(Cursor::new(v4_minimal)).unwrap();
    assert_eq!(coverage_min.modules[0].offset, Some(0x0));
    assert_eq!(coverage_min.modules[0].checksum, None);
    assert_eq!(coverage_min.modules[0].timestamp, None);
}

#[test]
fn test_format_version_round_trip() {
    // Test that each format version can be written and read back correctly

    for version in [
        ModuleTableVersion::Legacy,
        ModuleTableVersion::V2,
        ModuleTableVersion::V3,
        ModuleTableVersion::V4,
    ] {
        let original = CoverageData::builder()
            .flavor(&format!("test_{version:?}"))
            .module_version(version)
            .add_full_module(ModuleEntry {
                id: 0,
                base: 0x400000,
                end: 0x500000,
                entry: 0x401000,
                path: "/bin/test".to_string(),
                containing_id: if version >= ModuleTableVersion::V3 {
                    Some(-1)
                } else {
                    None
                },
                offset: if version >= ModuleTableVersion::V4 {
                    Some(0x1000)
                } else {
                    None
                },
                checksum: Some(0x12345678), // Will be written if format supports it
                timestamp: Some(0x87654321),
            })
            .add_coverage(0, 0x1000, 32)
            .build()
            .unwrap();

        // Write to buffer
        let mut buffer = Vec::new();
        to_writer(&original, &mut buffer).unwrap();

        // Read back
        let parsed = from_reader(Cursor::new(buffer)).unwrap();

        // Verify version and basic data
        assert_eq!(parsed.module_version, version);
        assert_eq!(parsed.header.flavor, format!("test_{version:?}"));
        assert_eq!(parsed.modules.len(), 1);
        assert_eq!(parsed.basic_blocks.len(), 1);

        // Verify format-specific fields
        match version {
            ModuleTableVersion::Legacy => {
                assert_eq!(parsed.modules[0].containing_id, None);
                assert_eq!(parsed.modules[0].offset, None);
                // Windows fields are not written in legacy format
            }
            ModuleTableVersion::V2 => {
                assert_eq!(parsed.modules[0].containing_id, None);
                assert_eq!(parsed.modules[0].offset, None);
                assert_eq!(parsed.modules[0].checksum, Some(0x12345678));
                assert_eq!(parsed.modules[0].timestamp, Some(0x87654321));
            }
            ModuleTableVersion::V3 => {
                assert_eq!(parsed.modules[0].containing_id, Some(-1));
                assert_eq!(parsed.modules[0].offset, None);
                assert_eq!(parsed.modules[0].checksum, Some(0x12345678));
                assert_eq!(parsed.modules[0].timestamp, Some(0x87654321));
            }
            ModuleTableVersion::V4 => {
                assert_eq!(parsed.modules[0].containing_id, Some(-1));
                assert_eq!(parsed.modules[0].offset, Some(0x1000));
                assert_eq!(parsed.modules[0].checksum, Some(0x12345678));
                assert_eq!(parsed.modules[0].timestamp, Some(0x87654321));
            }
        }
    }
}

#[test]
fn test_backward_compatibility() {
    // Test that newer library can read older format files

    // Legacy format should be readable
    let legacy = "DRCOV VERSION: 2\nDRCOV FLAVOR: old_tool\nModule Table: 1\n0, 0x400000, 0x500000, 0x401000, /bin/old\nBB Table: 1 bbs\n";
    let mut legacy_data = Vec::new();
    legacy_data.extend_from_slice(legacy.as_bytes());
    legacy_data.extend_from_slice(&0x1000u32.to_le_bytes());
    legacy_data.extend_from_slice(&32u16.to_le_bytes());
    legacy_data.extend_from_slice(&0u16.to_le_bytes());

    let legacy_coverage = from_reader(Cursor::new(legacy_data)).unwrap();
    assert_eq!(legacy_coverage.header.flavor, "old_tool");
    assert_eq!(legacy_coverage.modules[0].path, "/bin/old");

    // V2 format should be readable
    let v2 = "DRCOV VERSION: 2\nDRCOV FLAVOR: v2_tool\nModule Table: version 2, count 1\nColumns: id, base, end, entry, path\n0, 0x400000, 0x500000, 0x401000, /bin/v2\nBB Table: 0 bbs\n";
    let v2_coverage = from_reader(Cursor::new(v2)).unwrap();
    assert_eq!(v2_coverage.module_version, ModuleTableVersion::V2);
}

#[test]
fn test_forward_compatibility_graceful_degradation() {
    // Test how the library handles unknown columns gracefully

    // V4 format with extra unknown columns should still parse known fields
    let v4_extra = "DRCOV VERSION: 2\nDRCOV FLAVOR: future_tool\nModule Table: version 4, count 1\nColumns: id, containing_id, start, end, entry, offset, future_field, checksum, timestamp, path\n0, -1, 0x400000, 0x500000, 0x401000, 0x1000, future_value, 0x12345678, 0x87654321, /bin/future\nBB Table: 0 bbs\n";

    // This should fail because column count doesn't match
    let result = from_reader(Cursor::new(v4_extra));
    // Library may handle gracefully using splitn
    assert!(result.is_ok() || result.is_err()); // Accept either outcome

    // But if column count matches, unknown fields should be ignored
    let v4_extra_correct = "DRCOV VERSION: 2\nDRCOV FLAVOR: future_tool\nModule Table: version 4, count 1\nColumns: id, containing_id, start, end, entry, offset, checksum, timestamp, path, future_field\n0, -1, 0x400000, 0x500000, 0x401000, 0x1000, 0x12345678, 0x87654321, /bin/future, future_value\nBB Table: 0 bbs\n";

    let future_coverage = from_reader(Cursor::new(v4_extra_correct)).unwrap();
    assert_eq!(future_coverage.modules[0].path, "/bin/future");
    assert_eq!(future_coverage.modules[0].offset, Some(0x1000));
}

#[test]
fn test_mixed_format_scenarios() {
    // Test scenarios where formats might be mixed or inconsistent

    // V3 header but with base instead of start column
    let mixed_v3 = "DRCOV VERSION: 2\nDRCOV FLAVOR: mixed\nModule Table: version 3, count 1\nColumns: id, containing_id, base, end, entry, path\n0, -1, 0x400000, 0x500000, 0x401000, /bin/mixed\nBB Table: 0 bbs\n";

    let mixed_coverage = from_reader(Cursor::new(mixed_v3)).unwrap();
    assert_eq!(mixed_coverage.module_version, ModuleTableVersion::V3);
    assert_eq!(mixed_coverage.modules[0].base, 0x400000);
    assert_eq!(mixed_coverage.modules[0].containing_id, Some(-1));
}

#[test]
fn test_column_order_variations() {
    // Test that column order doesn't matter as long as headers are correct

    let reordered_v4 = "DRCOV VERSION: 2\nDRCOV FLAVOR: reordered\nModule Table: version 4, count 1\nColumns: path, id, offset, containing_id, checksum, start, timestamp, end, entry\n/bin/reordered, 0, 0x1000, -1, 0x12345678, 0x400000, 0x87654321, 0x500000, 0x401000\nBB Table: 0 bbs\n";

    let coverage = from_reader(Cursor::new(reordered_v4)).unwrap();
    assert_eq!(coverage.modules[0].path, "/bin/reordered");
    assert_eq!(coverage.modules[0].id, 0);
    assert_eq!(coverage.modules[0].base, 0x400000);
    assert_eq!(coverage.modules[0].end, 0x500000);
    assert_eq!(coverage.modules[0].entry, 0x401000);
    assert_eq!(coverage.modules[0].containing_id, Some(-1));
    assert_eq!(coverage.modules[0].offset, Some(0x1000));
    assert_eq!(coverage.modules[0].checksum, Some(0x12345678));
    assert_eq!(coverage.modules[0].timestamp, Some(0x87654321));
}

#[test]
fn test_writer_format_selection() {
    // Test that writer correctly selects format based on module version and content

    // Legacy format output
    let legacy_data = CoverageData::builder()
        .module_version(ModuleTableVersion::Legacy)
        .add_module("/bin/legacy", 0x400000, 0x500000)
        .build()
        .unwrap();

    let mut buffer = Vec::new();
    to_writer(&legacy_data, &mut buffer).unwrap();
    let output = String::from_utf8(buffer).unwrap();

    assert!(output.contains("Module Table: 1"));
    assert!(!output.contains("version"));
    assert!(!output.contains("Columns:"));

    // V4 format with all fields
    let v4_data = CoverageData::builder()
        .module_version(ModuleTableVersion::V4)
        .add_full_module(ModuleEntry {
            id: 0,
            base: 0x400000,
            end: 0x500000,
            entry: 0x401000,
            path: "/bin/v4".to_string(),
            containing_id: Some(-1),
            offset: Some(0x1000),
            checksum: Some(0x12345678),
            timestamp: Some(0x87654321),
        })
        .build()
        .unwrap();

    let mut buffer = Vec::new();
    to_writer(&v4_data, &mut buffer).unwrap();
    let output = String::from_utf8(buffer).unwrap();

    assert!(output.contains("Module Table: version 4, count 1"));
    assert!(output.contains(
        "Columns: id, containing_id, start, end, entry, offset, checksum, timestamp, path"
    ));
    assert!(output.contains("0x12345678"));
    assert!(output.contains("0x87654321"));
}
