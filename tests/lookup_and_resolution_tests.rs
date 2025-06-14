use drcov::{CoverageData, ModuleEntry};

#[test]
fn test_find_module_by_id() {
    let coverage = CoverageData::builder()
        .add_module("/bin/test1", 0x400000, 0x500000)
        .add_module("/bin/test2", 0x500000, 0x600000)
        .add_module("/bin/test3", 0x600000, 0x700000)
        .build()
        .unwrap();

    // Valid module IDs
    assert!(coverage.find_module(0).is_some());
    assert!(coverage.find_module(1).is_some());
    assert!(coverage.find_module(2).is_some());

    assert_eq!(coverage.find_module(0).unwrap().path, "/bin/test1");
    assert_eq!(coverage.find_module(1).unwrap().path, "/bin/test2");
    assert_eq!(coverage.find_module(2).unwrap().path, "/bin/test3");

    // Invalid module IDs
    assert!(coverage.find_module(3).is_none());
    assert!(coverage.find_module(100).is_none());
    assert!(coverage.find_module(u16::MAX).is_none());

    // Edge cases
    assert_eq!(coverage.find_module(0).unwrap().id, 0);
    assert_eq!(coverage.find_module(2).unwrap().id, 2);
}

#[test]
fn test_find_module_by_address() {
    let coverage = CoverageData::builder()
        .add_module("/bin/low", 0x400000, 0x500000)
        .add_module("/bin/mid", 0x600000, 0x700000) // Gap between modules
        .add_module("/bin/high", 0x800000, 0x900000)
        .build()
        .unwrap();

    // Addresses within modules
    assert!(coverage.find_module_by_address(0x400000).is_some());
    assert_eq!(
        coverage.find_module_by_address(0x400000).unwrap().path,
        "/bin/low"
    );

    assert!(coverage.find_module_by_address(0x450000).is_some());
    assert_eq!(
        coverage.find_module_by_address(0x450000).unwrap().path,
        "/bin/low"
    );

    assert!(coverage.find_module_by_address(0x4fffff).is_some());
    assert_eq!(
        coverage.find_module_by_address(0x4fffff).unwrap().path,
        "/bin/low"
    );

    assert!(coverage.find_module_by_address(0x600000).is_some());
    assert_eq!(
        coverage.find_module_by_address(0x600000).unwrap().path,
        "/bin/mid"
    );

    assert!(coverage.find_module_by_address(0x850000).is_some());
    assert_eq!(
        coverage.find_module_by_address(0x850000).unwrap().path,
        "/bin/high"
    );

    // Addresses outside modules
    assert!(coverage.find_module_by_address(0x300000).is_none()); // Before first module
    assert!(coverage.find_module_by_address(0x500000).is_none()); // At end boundary (exclusive)
    assert!(coverage.find_module_by_address(0x550000).is_none()); // In gap between modules
    assert!(coverage.find_module_by_address(0x900000).is_none()); // At end boundary (exclusive)
    assert!(coverage.find_module_by_address(0xa00000).is_none()); // After last module

    // Boundary conditions
    assert!(coverage.find_module_by_address(0x5fffff).is_none()); // Just before mid module
    assert!(coverage.find_module_by_address(0x6fffff).is_some()); // Just before end of mid module
    assert!(coverage.find_module_by_address(0x700000).is_none()); // At end of mid module (exclusive)
    assert!(coverage.find_module_by_address(0x7fffff).is_none()); // Just before high module
}

#[test]
fn test_address_resolution_with_overlapping_modules() {
    // Test scenario where modules might overlap (valid in some cases)
    let coverage = CoverageData::builder()
        .add_module("/bin/base", 0x400000, 0x500000)
        .add_module("/bin/overlay", 0x450000, 0x550000) // Overlaps with base
        .build()
        .unwrap();

    // Address in overlapping region should return the first matching module
    let addr_in_overlap = 0x480000;
    let found = coverage.find_module_by_address(addr_in_overlap);
    assert!(found.is_some());
    assert_eq!(found.unwrap().path, "/bin/base"); // Should find first match

    // Addresses in non-overlapping regions
    assert_eq!(
        coverage.find_module_by_address(0x420000).unwrap().path,
        "/bin/base"
    );
    assert_eq!(
        coverage.find_module_by_address(0x520000).unwrap().path,
        "/bin/overlay"
    );
}

#[test]
fn test_address_resolution_edge_cases() {
    // Zero-sized module
    let zero_size = CoverageData::builder()
        .add_module("/zero", 0x400000, 0x400000) // Zero size
        .build()
        .unwrap();

    assert!(zero_size.find_module_by_address(0x400000).is_none()); // Empty range

    // Adjacent modules
    let adjacent = CoverageData::builder()
        .add_module("/first", 0x400000, 0x500000)
        .add_module("/second", 0x500000, 0x600000) // Starts where first ends
        .build()
        .unwrap();

    assert_eq!(
        adjacent.find_module_by_address(0x4fffff).unwrap().path,
        "/first"
    );
    assert_eq!(
        adjacent.find_module_by_address(0x500000).unwrap().path,
        "/second"
    );
    assert!(adjacent.find_module_by_address(0x600000).is_none()); // End is exclusive

    // Maximum address values
    let max_addr = CoverageData::builder()
        .add_module("/max", u64::MAX - 0x1000, u64::MAX)
        .build()
        .unwrap();

    assert!(max_addr.find_module_by_address(u64::MAX - 1).is_some());
    assert!(max_addr.find_module_by_address(u64::MAX).is_none()); // End is exclusive
}

#[test]
fn test_module_contains_address() {
    let module = ModuleEntry {
        id: 0,
        base: 0x400000,
        end: 0x500000,
        entry: 0x401000,
        path: "/test".to_string(),
        ..Default::default()
    };

    // Addresses within range
    assert!(module.contains_address(0x400000)); // At start
    assert!(module.contains_address(0x450000)); // In middle
    assert!(module.contains_address(0x4fffff)); // Just before end

    // Addresses outside range
    assert!(!module.contains_address(0x3fffff)); // Just before start
    assert!(!module.contains_address(0x500000)); // At end (exclusive)
    assert!(!module.contains_address(0x500001)); // Just after end
    assert!(!module.contains_address(0)); // Zero
    assert!(!module.contains_address(u64::MAX)); // Maximum
}

#[test]
fn test_module_size_calculation() {
    let test_cases = vec![
        (0x400000, 0x500000, 0x100000), // Normal case
        (0x400000, 0x400001, 1),        // Minimum size
        (0x400000, 0x400000, 0),        // Zero size
        (0, u64::MAX, u64::MAX),        // Maximum size
        (u64::MAX - 1, u64::MAX, 1),    // Near maximum
        (0x500000, 0x400000, 0),        // Inverted (saturating_sub)
    ];

    for (base, end, expected_size) in test_cases {
        let module = ModuleEntry {
            id: 0,
            base,
            end,
            entry: base,
            path: "/test".to_string(),
            ..Default::default()
        };

        assert_eq!(
            module.size(),
            expected_size,
            "Size calculation failed for base=0x{base:x}, end=0x{end:x}"
        );
    }
}

#[test]
fn test_basic_block_absolute_address_calculation() {
    let module = ModuleEntry {
        id: 0,
        base: 0x400000,
        end: 0x500000,
        entry: 0x401000,
        path: "/test".to_string(),
        ..Default::default()
    };

    let test_cases = vec![
        (0x0, 0x400000),                                       // Zero offset
        (0x1000, 0x401000),                                    // Typical offset
        (0xffff, 0x40ffff),                                    // 16-bit boundary
        (0x100000, 0x500000),                                  // Large offset
        (u32::MAX, 0x400000u64.wrapping_add(u32::MAX as u64)), // Maximum offset
    ];

    for (offset, expected_addr) in test_cases {
        let bb = drcov::BasicBlock {
            module_id: 0,
            start: offset,
            size: 32,
        };

        assert_eq!(
            bb.absolute_address(&module),
            expected_addr,
            "Address calculation failed for offset=0x{offset:x}"
        );
    }
}

#[test]
fn test_address_resolution_with_maximum_values() {
    // Test with addresses near the maximum values
    let max_coverage = CoverageData::builder()
        .add_module("/max1", u64::MAX - 0x2000, u64::MAX - 0x1000)
        .add_module("/max2", u64::MAX - 0x1000, u64::MAX)
        .build()
        .unwrap();

    // Test lookups near maximum values
    assert!(max_coverage
        .find_module_by_address(u64::MAX - 0x1800)
        .is_some());
    assert_eq!(
        max_coverage
            .find_module_by_address(u64::MAX - 0x1800)
            .unwrap()
            .path,
        "/max1"
    );

    assert!(max_coverage
        .find_module_by_address(u64::MAX - 0x800)
        .is_some());
    assert_eq!(
        max_coverage
            .find_module_by_address(u64::MAX - 0x800)
            .unwrap()
            .path,
        "/max2"
    );

    assert!(max_coverage.find_module_by_address(u64::MAX - 1).is_some());
    assert_eq!(
        max_coverage
            .find_module_by_address(u64::MAX - 1)
            .unwrap()
            .path,
        "/max2"
    );

    assert!(max_coverage.find_module_by_address(u64::MAX).is_none()); // End is exclusive
}

#[test]
fn test_coverage_statistics_accuracy() {
    let coverage = CoverageData::builder()
        .add_module("/mod1", 0x400000, 0x500000)
        .add_module("/mod2", 0x500000, 0x600000)
        .add_module("/mod3", 0x600000, 0x700000) // Module with no coverage
        .add_coverage(0, 0x1000, 32) // 2 blocks in mod1
        .add_coverage(0, 0x2000, 64)
        .add_coverage(1, 0x3000, 16) // 1 block in mod2
        // No blocks in mod3
        .build()
        .unwrap();

    let stats = coverage.get_coverage_stats();

    // Verify statistics
    assert_eq!(stats.get(&0), Some(&2)); // mod1 has 2 blocks
    assert_eq!(stats.get(&1), Some(&1)); // mod2 has 1 block
    assert_eq!(stats.get(&2), None); // mod3 has no blocks (not in map)

    // Verify total count
    let total_blocks: usize = stats.values().sum();
    assert_eq!(total_blocks, 3);

    // Verify all modules are accounted for in coverage data
    assert_eq!(coverage.modules.len(), 3);
    assert_eq!(coverage.basic_blocks.len(), 3);
}

#[test]
fn test_lookup_performance_patterns() {
    // Create coverage data with patterns that might affect lookup performance
    let mut builder = CoverageData::builder();

    // Add modules with various patterns
    let patterns = vec![
        // Sequential addresses
        (0x400000, 0x500000, "/seq1"),
        (0x500000, 0x600000, "/seq2"),
        (0x600000, 0x700000, "/seq3"),
        // Gaps
        (0x800000, 0x900000, "/gap1"),
        (0xa00000, 0xb00000, "/gap2"),
        // High addresses
        (0x7fff00000000, 0x7fff00100000, "/high1"),
        (0x7fff00100000, 0x7fff00200000, "/high2"),
    ];

    for (base, end, path) in patterns {
        builder = builder.add_module(path, base, end);
    }

    let coverage = builder.build().unwrap();

    // Test various lookup patterns
    let test_addresses = vec![
        0x450000,       // Sequential region
        0x550000,       // Sequential region
        0x650000,       // Sequential region
        0x750000,       // Gap (should not find)
        0x850000,       // After gap
        0xa50000,       // After gap
        0x7fff00080000, // High address region
        0x7fff00180000, // High address region
    ];

    for addr in test_addresses {
        let result = coverage.find_module_by_address(addr);
        // Verify that lookup completes (performance test)
        // The actual result depends on whether address is in a module
        let _ = result;
    }

    // Test ID lookups for all modules
    for i in 0..coverage.modules.len() {
        let module = coverage.find_module(i as u16);
        assert!(module.is_some());
        assert_eq!(module.unwrap().id, i as u32);
    }
}

#[test]
fn test_module_lookup_with_non_sequential_scenarios() {
    // Test with modules that have non-sequential properties
    let coverage = CoverageData::builder()
        .add_full_module(ModuleEntry {
            id: 0,
            base: 0x400000,
            end: 0x500000,
            entry: 0x401000,
            path: "/custom_entry".to_string(),
            ..Default::default()
        })
        .add_full_module(ModuleEntry {
            id: 1,
            base: 0x600000, // Gap in addresses
            end: 0x500000,  // Inverted range (end < base)
            entry: 0x650000,
            path: "/inverted".to_string(),
            ..Default::default()
        })
        .build()
        .unwrap();

    // Verify lookups work correctly even with unusual module properties
    assert!(coverage.find_module(0).is_some());
    assert!(coverage.find_module(1).is_some());
    assert_eq!(coverage.find_module(0).unwrap().path, "/custom_entry");
    assert_eq!(coverage.find_module(1).unwrap().path, "/inverted");

    // Address lookup with inverted module should not find anything
    assert!(coverage.find_module_by_address(0x580000).is_none());

    // But normal module should work
    assert!(coverage.find_module_by_address(0x450000).is_some());
    assert_eq!(
        coverage.find_module_by_address(0x450000).unwrap().path,
        "/custom_entry"
    );
}
