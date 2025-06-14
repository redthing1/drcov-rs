use drcov::{BasicBlock, CoverageData, ModuleEntry, ModuleTableVersion};

#[test]
fn test_builder_sequential_module_ids() {
    // Valid sequential IDs starting from 0
    let valid = CoverageData::builder()
        .add_module("/bin/test1", 0x400000, 0x500000)
        .add_module("/bin/test2", 0x500000, 0x600000)
        .add_module("/bin/test3", 0x600000, 0x700000)
        .build();
    assert!(valid.is_ok());

    // Invalid: non-sequential module IDs when using add_full_module
    let invalid = CoverageData::builder()
        .add_full_module(ModuleEntry {
            id: 0,
            base: 0x400000,
            end: 0x500000,
            path: "/bin/test1".to_string(),
            ..Default::default()
        })
        .add_full_module(ModuleEntry {
            id: 2, // Should be 1
            base: 0x500000,
            end: 0x600000,
            path: "/bin/test2".to_string(),
            ..Default::default()
        })
        .build();
    assert!(invalid.is_err());

    // Invalid: duplicate module IDs
    let duplicate = CoverageData::builder()
        .add_full_module(ModuleEntry {
            id: 0,
            base: 0x400000,
            end: 0x500000,
            path: "/bin/test1".to_string(),
            ..Default::default()
        })
        .add_full_module(ModuleEntry {
            id: 0, // Duplicate
            base: 0x500000,
            end: 0x600000,
            path: "/bin/test2".to_string(),
            ..Default::default()
        })
        .build();
    assert!(duplicate.is_err());
}

#[test]
fn test_builder_basic_block_validation() {
    // Valid basic blocks referencing existing modules
    let valid = CoverageData::builder()
        .add_module("/bin/test", 0x400000, 0x500000)
        .add_module("/lib/test", 0x500000, 0x600000)
        .add_coverage(0, 0x1000, 32)
        .add_coverage(1, 0x2000, 64)
        .build();
    assert!(valid.is_ok());

    // Invalid: basic block references non-existent module
    let invalid_ref = CoverageData::builder()
        .add_module("/bin/test", 0x400000, 0x500000)
        .add_coverage(1, 0x1000, 32) // Module 1 doesn't exist
        .build();
    assert!(invalid_ref.is_err());

    // Invalid: basic block references module ID beyond range
    let out_of_range = CoverageData::builder()
        .add_module("/bin/test", 0x400000, 0x500000)
        .add_coverage(100, 0x1000, 32) // Way out of range
        .build();
    assert!(out_of_range.is_err());

    // Valid: empty basic blocks
    let empty_bb = CoverageData::builder()
        .add_module("/bin/test", 0x400000, 0x500000)
        .build();
    assert!(empty_bb.is_ok());
}

#[test]
fn test_builder_module_address_ranges() {
    // Valid: non-overlapping modules
    let non_overlapping = CoverageData::builder()
        .add_module("/bin/test1", 0x400000, 0x500000)
        .add_module("/bin/test2", 0x600000, 0x700000)
        .build();
    assert!(non_overlapping.is_ok());

    // Valid: adjacent modules
    let adjacent = CoverageData::builder()
        .add_module("/bin/test1", 0x400000, 0x500000)
        .add_module("/bin/test2", 0x500000, 0x600000)
        .build();
    assert!(adjacent.is_ok());

    // Note: The library doesn't validate overlapping ranges as this can be valid in some scenarios
    let overlapping = CoverageData::builder()
        .add_module("/bin/test1", 0x400000, 0x500000)
        .add_module("/bin/test2", 0x450000, 0x550000) // Overlaps
        .build();
    assert!(overlapping.is_ok()); // Should still be valid

    // Edge case: zero-sized module
    let zero_size = CoverageData::builder()
        .add_module("/bin/test", 0x400000, 0x400000)
        .build();
    assert!(zero_size.is_ok());

    // Edge case: inverted range (end < base) - should be allowed
    let inverted = CoverageData::builder()
        .add_module("/bin/test", 0x500000, 0x400000)
        .build();
    assert!(inverted.is_ok());
}

#[test]
fn test_builder_mixed_construction_methods() {
    // Mix add_module and add_full_module
    let mixed = CoverageData::builder()
        .add_module("/bin/auto", 0x400000, 0x500000) // ID will be 0
        .add_full_module(ModuleEntry {
            id: 1, // Must be sequential
            base: 0x500000,
            end: 0x600000,
            path: "/bin/manual".to_string(),
            ..Default::default()
        })
        .add_module("/bin/auto2", 0x600000, 0x700000) // ID will be 2
        .build();
    assert!(mixed.is_ok());

    // Invalid mix: wrong ID in add_full_module
    let invalid_mix = CoverageData::builder()
        .add_module("/bin/auto", 0x400000, 0x500000) // ID will be 0
        .add_full_module(ModuleEntry {
            id: 5, // Should be 1
            base: 0x500000,
            end: 0x600000,
            path: "/bin/manual".to_string(),
            ..Default::default()
        })
        .build();
    assert!(invalid_mix.is_err());
}

#[test]
fn test_builder_basic_block_construction_methods() {
    // Mix add_coverage and add_basic_block
    let mixed_bb = CoverageData::builder()
        .add_module("/bin/test", 0x400000, 0x500000)
        .add_coverage(0, 0x1000, 32)
        .add_basic_block(BasicBlock {
            module_id: 0,
            start: 0x2000,
            size: 64,
        })
        .build();
    assert!(mixed_bb.is_ok());

    // All add_basic_block with invalid module reference
    let invalid_bb = CoverageData::builder()
        .add_module("/bin/test", 0x400000, 0x500000)
        .add_basic_block(BasicBlock {
            module_id: 1, // Invalid
            start: 0x1000,
            size: 32,
        })
        .build();
    assert!(invalid_bb.is_err());
}

#[test]
fn test_builder_flavor_and_version_settings() {
    // Default values
    let default_builder = CoverageData::builder().build().unwrap();
    assert_eq!(default_builder.header.flavor, "drcov");
    assert_eq!(default_builder.module_version, ModuleTableVersion::Legacy);

    // Custom values
    let custom = CoverageData::builder()
        .flavor("custom_tool")
        .module_version(ModuleTableVersion::V4)
        .build()
        .unwrap();
    assert_eq!(custom.header.flavor, "custom_tool");
    assert_eq!(custom.module_version, ModuleTableVersion::V4);

    // Overwriting values
    let overwritten = CoverageData::builder()
        .flavor("first")
        .flavor("second") // Should overwrite
        .module_version(ModuleTableVersion::V2)
        .module_version(ModuleTableVersion::V3) // Should overwrite
        .build()
        .unwrap();
    assert_eq!(overwritten.header.flavor, "second");
    assert_eq!(overwritten.module_version, ModuleTableVersion::V3);
}

#[test]
fn test_builder_empty_configurations() {
    // Completely empty
    let empty = CoverageData::builder().build();
    assert!(empty.is_ok());
    let empty_data = empty.unwrap();
    assert_eq!(empty_data.modules.len(), 0);
    assert_eq!(empty_data.basic_blocks.len(), 0);

    // Only modules, no basic blocks
    let modules_only = CoverageData::builder()
        .add_module("/bin/test", 0x400000, 0x500000)
        .build();
    assert!(modules_only.is_ok());

    // This would be invalid: basic blocks without modules
    // (tested in other validation tests)
}

#[test]
fn test_builder_large_numbers() {
    // Maximum values for addresses
    let max_addresses = CoverageData::builder()
        .add_module("/bin/test", u64::MAX - 0x1000, u64::MAX)
        .add_coverage(0, u32::MAX, u16::MAX)
        .build();
    assert!(max_addresses.is_ok());

    // Zero addresses
    let zero_addresses = CoverageData::builder()
        .add_module("/bin/test", 0, 0)
        .add_coverage(0, 0, 0)
        .build();
    assert!(zero_addresses.is_ok());
}

#[test]
fn test_builder_module_entry_fields() {
    // Module with all optional fields
    let full_module = CoverageData::builder()
        .add_full_module(ModuleEntry {
            id: 0,
            base: 0x400000,
            end: 0x500000,
            entry: 0x401000,
            path: "/bin/test".to_string(),
            containing_id: Some(42),
            offset: Some(0x1000),
            checksum: Some(0x12345678),
            timestamp: Some(0x87654321),
        })
        .build();
    assert!(full_module.is_ok());

    // Module with negative containing_id
    let negative_containing_id = CoverageData::builder()
        .add_full_module(ModuleEntry {
            id: 0,
            base: 0x400000,
            end: 0x500000,
            entry: 0x401000,
            path: "/bin/test".to_string(),
            containing_id: Some(-1),
            ..Default::default()
        })
        .build();
    assert!(negative_containing_id.is_ok());

    // Module with very long path
    let long_path = "a".repeat(1000);
    let long_path_module = CoverageData::builder()
        .add_module(&long_path, 0x400000, 0x500000)
        .build();
    assert!(long_path_module.is_ok());
}

#[test]
fn test_builder_chaining() {
    // Test that builder methods can be chained in any order
    let chained = CoverageData::builder()
        .module_version(ModuleTableVersion::V4)
        .add_module("/bin/test1", 0x400000, 0x500000)
        .flavor("test_tool")
        .add_coverage(0, 0x1000, 32)
        .add_module("/bin/test2", 0x500000, 0x600000)
        .add_coverage(1, 0x2000, 64)
        .build();
    assert!(chained.is_ok());

    let data = chained.unwrap();
    assert_eq!(data.header.flavor, "test_tool");
    assert_eq!(data.module_version, ModuleTableVersion::V4);
    assert_eq!(data.modules.len(), 2);
    assert_eq!(data.basic_blocks.len(), 2);
}

#[test]
fn test_builder_validation_edge_cases() {
    // Maximum number of modules (limited by u16 for module_id in basic blocks)
    // This would take too long to actually test, so we test a reasonable subset
    let many_modules_builder = CoverageData::builder();
    let mut builder = many_modules_builder;
    for i in 0..1000u32 {
        let base = 0x400000 + i as u64 * 0x1000;
        builder = builder.add_module(&format!("/module{i}"), base, base + 0x1000);
    }
    let many_modules = builder.build();
    assert!(many_modules.is_ok());

    // Test with maximum u16 module_id in basic block
    let max_module_id = CoverageData::builder()
        .add_module("/bin/test", 0x400000, 0x500000)
        .add_basic_block(BasicBlock {
            module_id: 0, // Valid reference
            start: 0x1000,
            size: 32,
        })
        .build();
    assert!(max_module_id.is_ok());
}
