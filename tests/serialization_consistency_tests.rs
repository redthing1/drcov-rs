use drcov::{from_reader, to_writer, CoverageData, ModuleEntry, ModuleTableVersion};
use std::io::Cursor;

#[test]
fn test_multiple_serialization_rounds() {
    // Test that multiple serialize/deserialize cycles maintain consistency
    let original = CoverageData::builder()
        .flavor("consistency_test")
        .module_version(ModuleTableVersion::V4)
        .add_full_module(ModuleEntry {
            id: 0,
            base: 0x400000,
            end: 0x500000,
            entry: 0x401000,
            path: "/bin/test".to_string(),
            containing_id: Some(-1),
            offset: Some(0x1000),
            checksum: Some(0x12345678),
            timestamp: Some(0x87654321),
        })
        .add_coverage(0, 0x1000, 32)
        .add_coverage(0, 0x2000, 64)
        .build()
        .unwrap();

    let mut current = original.clone();

    // Perform 10 rounds of serialization/deserialization
    for round in 0..10 {
        let mut buffer = Vec::new();
        to_writer(&current, &mut buffer).unwrap();

        let parsed = from_reader(Cursor::new(buffer)).unwrap();

        // Verify consistency after each round
        assert_eq!(
            current.header.version, parsed.header.version,
            "Round {round}: version mismatch"
        );
        assert_eq!(
            current.header.flavor, parsed.header.flavor,
            "Round {round}: flavor mismatch"
        );
        assert_eq!(
            current.module_version, parsed.module_version,
            "Round {round}: module_version mismatch"
        );
        assert_eq!(
            current.modules.len(),
            parsed.modules.len(),
            "Round {round}: modules length mismatch"
        );
        assert_eq!(
            current.basic_blocks.len(),
            parsed.basic_blocks.len(),
            "Round {round}: basic_blocks length mismatch"
        );

        // Verify module data consistency
        for (i, (orig_mod, parsed_mod)) in current
            .modules
            .iter()
            .zip(parsed.modules.iter())
            .enumerate()
        {
            assert_eq!(
                orig_mod.id, parsed_mod.id,
                "Round {round}, module {i}: id mismatch"
            );
            assert_eq!(
                orig_mod.base, parsed_mod.base,
                "Round {round}, module {i}: base mismatch"
            );
            assert_eq!(
                orig_mod.end, parsed_mod.end,
                "Round {round}, module {i}: end mismatch"
            );
            assert_eq!(
                orig_mod.entry, parsed_mod.entry,
                "Round {round}, module {i}: entry mismatch"
            );
            assert_eq!(
                orig_mod.path, parsed_mod.path,
                "Round {round}, module {i}: path mismatch"
            );
            assert_eq!(
                orig_mod.containing_id, parsed_mod.containing_id,
                "Round {round}, module {i}: containing_id mismatch"
            );
            assert_eq!(
                orig_mod.offset, parsed_mod.offset,
                "Round {round}, module {i}: offset mismatch"
            );
            assert_eq!(
                orig_mod.checksum, parsed_mod.checksum,
                "Round {round}, module {i}: checksum mismatch"
            );
            assert_eq!(
                orig_mod.timestamp, parsed_mod.timestamp,
                "Round {round}, module {i}: timestamp mismatch"
            );
        }

        // Verify basic block data consistency
        for (i, (orig_bb, parsed_bb)) in current
            .basic_blocks
            .iter()
            .zip(parsed.basic_blocks.iter())
            .enumerate()
        {
            assert_eq!(
                orig_bb.module_id, parsed_bb.module_id,
                "Round {round}, BB {i}: module_id mismatch"
            );
            assert_eq!(
                orig_bb.start, parsed_bb.start,
                "Round {round}, BB {i}: start mismatch"
            );
            assert_eq!(
                orig_bb.size, parsed_bb.size,
                "Round {round}, BB {i}: size mismatch"
            );
        }

        current = parsed;
    }

    // Final comparison with original
    assert_eq!(original.header, current.header);
    assert_eq!(original.module_version, current.module_version);
    assert_eq!(original.modules.len(), current.modules.len());
    assert_eq!(original.basic_blocks.len(), current.basic_blocks.len());
}

#[test]
fn test_binary_data_consistency() {
    // Test that binary basic block data is preserved exactly
    let test_cases = vec![
        (0x00000000, 0x0000, 0), // All zeros
        (0xffffffff, 0xffff, 0), // All ones (use valid module ID)
        (0x12345678, 0xabcd, 0), // Mixed values (use valid module ID)
        (0x00000001, 0x0001, 0), // Minimal non-zero (use valid module ID)
        (0xfffffffe, 0xfffe, 0), // Near maximum (use valid module ID)
    ];

    for (start, size, module_id) in test_cases {
        let coverage = CoverageData::builder()
            .add_module("/test", 0x400000, 0x500000)
            .add_basic_block(drcov::BasicBlock {
                module_id,
                start,
                size,
            })
            .build()
            .unwrap();

        let mut buffer = Vec::new();
        to_writer(&coverage, &mut buffer).unwrap();

        let parsed = from_reader(Cursor::new(buffer)).unwrap();

        assert_eq!(parsed.basic_blocks.len(), 1);
        assert_eq!(parsed.basic_blocks[0].module_id, module_id);
        assert_eq!(parsed.basic_blocks[0].start, start);
        assert_eq!(parsed.basic_blocks[0].size, size);
    }
}

#[test]
fn test_large_dataset_consistency() {
    // Test consistency with larger datasets
    let module_count = 100;
    let bb_count = 1000;

    let mut builder = CoverageData::builder()
        .flavor("large_consistency_test")
        .module_version(ModuleTableVersion::V3);

    // Add modules with varied properties
    for i in 0..module_count {
        let base = 0x400000 + i * 0x100000;
        builder = builder.add_full_module(ModuleEntry {
            id: i as u32,
            base,
            end: base + 0x50000,
            entry: base + 0x1000,
            path: format!("/lib/module_{i}.so"),
            containing_id: Some(if i % 2 == 0 { -1 } else { (i / 2) as i32 }),
            offset: None, // V3 doesn't support offset
            checksum: Some(0x12345678 + i as u32),
            timestamp: Some(0x87654321 - i as u32),
        });
    }

    // Add basic blocks with varied properties
    for i in 0..bb_count {
        let module_id = (i % module_count) as u16;
        let start = (i * 4) as u32;
        let size = ((i % 64) + 1) as u16; // Varied sizes
        builder = builder.add_basic_block(drcov::BasicBlock {
            module_id,
            start,
            size,
        });
    }

    let original = builder.build().unwrap();

    // Serialize and deserialize
    let mut buffer = Vec::new();
    to_writer(&original, &mut buffer).unwrap();
    let parsed = from_reader(Cursor::new(buffer)).unwrap();

    // Comprehensive verification
    assert_eq!(original.modules.len(), parsed.modules.len());
    assert_eq!(original.basic_blocks.len(), parsed.basic_blocks.len());

    for (orig, parsed) in original.modules.iter().zip(parsed.modules.iter()) {
        assert_eq!(orig.id, parsed.id);
        assert_eq!(orig.base, parsed.base);
        assert_eq!(orig.end, parsed.end);
        assert_eq!(orig.entry, parsed.entry);
        assert_eq!(orig.path, parsed.path);
        assert_eq!(orig.containing_id, parsed.containing_id);
        assert_eq!(orig.offset, parsed.offset);
        assert_eq!(orig.checksum, parsed.checksum);
        assert_eq!(orig.timestamp, parsed.timestamp);
    }

    for (orig, parsed) in original.basic_blocks.iter().zip(parsed.basic_blocks.iter()) {
        assert_eq!(orig.module_id, parsed.module_id);
        assert_eq!(orig.start, parsed.start);
        assert_eq!(orig.size, parsed.size);
    }
}

#[test]
fn test_format_version_consistency() {
    // Test that each format version maintains consistency
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
                path: "/bin/format_test".to_string(),
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
                checksum: Some(0x12345678),
                timestamp: Some(0x87654321),
            })
            .add_coverage(0, 0x1000, 32)
            .build()
            .unwrap();

        // Multiple rounds for each format
        let mut current = original.clone();
        for _ in 0..3 {
            let mut buffer = Vec::new();
            to_writer(&current, &mut buffer).unwrap();
            current = from_reader(Cursor::new(buffer)).unwrap();
        }

        // Verify version-specific consistency
        assert_eq!(original.module_version, current.module_version);
        assert_eq!(original.header.flavor, current.header.flavor);

        match version {
            ModuleTableVersion::Legacy => {
                // Legacy format should not preserve Windows fields
                assert_eq!(current.modules[0].containing_id, None);
                assert_eq!(current.modules[0].offset, None);
            }
            ModuleTableVersion::V2 => {
                // V2 preserves Windows fields but not containing_id/offset
                assert_eq!(current.modules[0].containing_id, None);
                assert_eq!(current.modules[0].offset, None);
                assert_eq!(current.modules[0].checksum, Some(0x12345678));
                assert_eq!(current.modules[0].timestamp, Some(0x87654321));
            }
            ModuleTableVersion::V3 => {
                // V3 preserves containing_id and Windows fields
                assert_eq!(current.modules[0].containing_id, Some(-1));
                assert_eq!(current.modules[0].offset, None);
                assert_eq!(current.modules[0].checksum, Some(0x12345678));
                assert_eq!(current.modules[0].timestamp, Some(0x87654321));
            }
            ModuleTableVersion::V4 => {
                // V4 preserves all fields
                assert_eq!(current.modules[0].containing_id, Some(-1));
                assert_eq!(current.modules[0].offset, Some(0x1000));
                assert_eq!(current.modules[0].checksum, Some(0x12345678));
                assert_eq!(current.modules[0].timestamp, Some(0x87654321));
            }
        }
    }
}

#[test]
fn test_special_character_consistency() {
    // Test that special characters in paths are preserved
    let very_long_path = "/very/".to_owned() + &"long/".repeat(20) + "path";
    let special_paths = [
        "/测试/文件",
        "/тест/файл",
        "/🦀/rust",
        "C:\\Program Files\\Test",
        "/path with spaces",
        "/path,with,commas",
        "/path\"with\"quotes",
        &very_long_path,
        "", // Empty path
    ];

    for (i, path) in special_paths.iter().enumerate() {
        let original = CoverageData::builder()
            .add_module(
                path,
                0x400000 + i as u64 * 0x1000,
                0x401000 + i as u64 * 0x1000,
            )
            .build()
            .unwrap();

        let mut buffer = Vec::new();
        to_writer(&original, &mut buffer).unwrap();
        let parsed = from_reader(Cursor::new(buffer)).unwrap();

        assert_eq!(
            parsed.modules[0].path, *path,
            "Path consistency failed for: {path}"
        );
    }
}

#[test]
fn test_field_order_independence() {
    // Test that field order in modules doesn't affect consistency
    // This primarily tests the parser's ability to handle reordered columns

    let reordered_v4 = "DRCOV VERSION: 2\nDRCOV FLAVOR: reorder_test\nModule Table: version 4, count 1\nColumns: path, checksum, id, offset, containing_id, timestamp, start, end, entry\n/bin/reordered, 0x12345678, 0, 0x1000, -1, 0x87654321, 0x400000, 0x500000, 0x401000\nBB Table: 1 bbs\n";

    let mut data = Vec::new();
    data.extend_from_slice(reordered_v4.as_bytes());
    // Add one basic block
    data.extend_from_slice(&0x1000u32.to_le_bytes());
    data.extend_from_slice(&32u16.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());

    let parsed = from_reader(Cursor::new(data)).unwrap();

    // Serialize back to standard format
    let mut buffer = Vec::new();
    to_writer(&parsed, &mut buffer).unwrap();

    // Parse again
    let reparsed = from_reader(Cursor::new(buffer)).unwrap();

    // Verify all data is preserved
    assert_eq!(parsed.modules[0].path, reparsed.modules[0].path);
    assert_eq!(parsed.modules[0].id, reparsed.modules[0].id);
    assert_eq!(parsed.modules[0].base, reparsed.modules[0].base);
    assert_eq!(parsed.modules[0].end, reparsed.modules[0].end);
    assert_eq!(parsed.modules[0].entry, reparsed.modules[0].entry);
    assert_eq!(
        parsed.modules[0].containing_id,
        reparsed.modules[0].containing_id
    );
    assert_eq!(parsed.modules[0].offset, reparsed.modules[0].offset);
    assert_eq!(parsed.modules[0].checksum, reparsed.modules[0].checksum);
    assert_eq!(parsed.modules[0].timestamp, reparsed.modules[0].timestamp);
    assert_eq!(parsed.basic_blocks[0].start, reparsed.basic_blocks[0].start);
    assert_eq!(parsed.basic_blocks[0].size, reparsed.basic_blocks[0].size);
    assert_eq!(
        parsed.basic_blocks[0].module_id,
        reparsed.basic_blocks[0].module_id
    );
}

#[test]
fn test_empty_data_consistency() {
    // Test consistency with various empty data scenarios
    let empty_cases = vec![
        // No modules, no basic blocks
        CoverageData::builder().build().unwrap(),
        // Modules but no basic blocks
        CoverageData::builder()
            .add_module("/empty", 0x400000, 0x500000)
            .build()
            .unwrap(),
        // Empty paths
        CoverageData::builder()
            .add_module("", 0x400000, 0x500000)
            .build()
            .unwrap(),
    ];

    for (i, original) in empty_cases.iter().enumerate() {
        let mut buffer = Vec::new();
        to_writer(original, &mut buffer).unwrap();
        let parsed = from_reader(Cursor::new(buffer)).unwrap();

        assert_eq!(
            original.modules.len(),
            parsed.modules.len(),
            "Case {i}: module count mismatch"
        );
        assert_eq!(
            original.basic_blocks.len(),
            parsed.basic_blocks.len(),
            "Case {i}: BB count mismatch"
        );

        for (orig_mod, parsed_mod) in original.modules.iter().zip(parsed.modules.iter()) {
            assert_eq!(orig_mod.path, parsed_mod.path, "Case {i}: path mismatch");
        }
    }
}

#[test]
fn test_validation_consistency() {
    // Test that validation results are consistent after serialization
    let valid_data = CoverageData::builder()
        .add_module("/valid", 0x400000, 0x500000)
        .add_coverage(0, 0x1000, 32)
        .build()
        .unwrap();

    // Original should be valid
    assert!(valid_data.validate().is_ok());

    // After serialization should still be valid
    let mut buffer = Vec::new();
    to_writer(&valid_data, &mut buffer).unwrap();
    let parsed = from_reader(Cursor::new(buffer)).unwrap();
    assert!(parsed.validate().is_ok());

    // Statistics should be consistent
    let orig_stats = valid_data.get_coverage_stats();
    let parsed_stats = parsed.get_coverage_stats();
    assert_eq!(orig_stats, parsed_stats);
}

#[test]
fn test_deterministic_output() {
    // Test that serialization produces deterministic output
    let data = CoverageData::builder()
        .flavor("deterministic_test")
        .module_version(ModuleTableVersion::V4)
        .add_full_module(ModuleEntry {
            id: 0,
            base: 0x400000,
            end: 0x500000,
            entry: 0x401000,
            path: "/bin/deterministic".to_string(),
            containing_id: Some(-1),
            offset: Some(0x1000),
            checksum: Some(0x12345678),
            timestamp: Some(0x87654321),
        })
        .add_coverage(0, 0x1000, 32)
        .add_coverage(0, 0x2000, 64)
        .build()
        .unwrap();

    // Serialize multiple times
    let mut outputs = Vec::new();
    for _ in 0..5 {
        let mut buffer = Vec::new();
        to_writer(&data, &mut buffer).unwrap();
        outputs.push(buffer);
    }

    // All outputs should be identical
    for (i, output) in outputs.iter().enumerate().skip(1) {
        assert_eq!(&outputs[0], output, "Output {i} differs from output 0");
    }
}
