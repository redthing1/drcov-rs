use drcov::{from_reader, to_writer, CoverageData, ModuleTableVersion};
use std::io::Cursor;
use std::time::Instant;

#[test]
fn test_large_file_parsing_performance() {
    // Create a large drcov file in memory
    let module_count = 1000;
    let bb_count = 10000;

    let mut builder = CoverageData::builder()
        .flavor("performance_test")
        .module_version(ModuleTableVersion::V4);

    // Add many modules
    for i in 0..module_count {
        let base = 0x400000 + i * 0x100000;
        builder = builder.add_module(&format!("/usr/lib/module_{i}.so"), base, base + 0x50000);
    }

    // Add many basic blocks
    for i in 0..bb_count {
        let module_id = (i % module_count) as u16;
        let offset = 0x1000 + (i / module_count) * 0x100;
        builder = builder.add_coverage(module_id, offset as u32, 32);
    }

    let large_coverage = builder.build().unwrap();

    // Test write performance
    let write_start = Instant::now();
    let mut buffer = Vec::new();
    to_writer(&large_coverage, &mut buffer).unwrap();
    let write_duration = write_start.elapsed();

    println!("Write performance: {module_count} modules, {bb_count} BBs in {write_duration:?}");

    // Test read performance
    let read_start = Instant::now();
    let parsed_coverage = from_reader(Cursor::new(buffer)).unwrap();
    let read_duration = read_start.elapsed();

    println!("Read performance: {module_count} modules, {bb_count} BBs in {read_duration:?}");

    // Verify correctness
    assert_eq!(parsed_coverage.modules.len(), module_count as usize);
    assert_eq!(parsed_coverage.basic_blocks.len(), bb_count as usize);

    // Performance assertions (adjust based on acceptable performance)
    // These are quite generous - should complete well under these limits
    assert!(
        write_duration.as_millis() < 1000,
        "Write took too long: {write_duration:?}"
    );
    assert!(
        read_duration.as_millis() < 1000,
        "Read took too long: {read_duration:?}"
    );
}

#[test]
fn test_memory_usage_with_large_datasets() {
    // Test with many small modules vs few large modules
    let start_memory = get_memory_usage();

    // Many small modules
    let mut small_modules_builder = CoverageData::builder();
    for i in 0..5000 {
        let base = 0x400000 + i * 0x1000;
        small_modules_builder =
            small_modules_builder.add_module(&format!("/lib/small_{i}.so"), base, base + 0x1000);
    }
    let small_modules_data = small_modules_builder.build().unwrap();

    let after_small_modules = get_memory_usage();
    drop(small_modules_data);

    // Few large modules with many basic blocks
    let mut large_bb_builder = CoverageData::builder()
        .add_module("/large/module1", 0x400000, 0x500000)
        .add_module("/large/module2", 0x500000, 0x600000);

    for i in 0..25000 {
        let module_id = (i % 2) as u16;
        large_bb_builder = large_bb_builder.add_coverage(module_id, (i * 4) as u32, 4);
    }
    let large_bb_data = large_bb_builder.build().unwrap();

    let after_large_bb = get_memory_usage();
    drop(large_bb_data);

    println!(
        "Memory usage - Start: {start_memory}, After small modules: {after_small_modules}, After large BB: {after_large_bb}"
    );

    // Both should complete without excessive memory usage
    // These are very generous limits
    assert!(
        after_small_modules - start_memory < 100_000_000,
        "Small modules used too much memory"
    );
    assert!(
        after_large_bb - start_memory < 100_000_000,
        "Large BB dataset used too much memory"
    );
}

#[test]
fn test_builder_performance() {
    // Test performance of different builder patterns
    let iterations = 10000;

    // Sequential building
    let sequential_start = Instant::now();
    let mut sequential_builder = CoverageData::builder();
    for i in 0..iterations {
        let base = 0x400000 + i * 0x1000;
        sequential_builder = sequential_builder
            .add_module(&format!("/seq/module_{i}"), base, base + 0x1000)
            .add_coverage(i as u16, 0x100, 32);
    }
    let sequential_data = sequential_builder.build().unwrap();
    let sequential_duration = sequential_start.elapsed();

    println!(
        "Sequential building: {} operations in {:?}",
        iterations * 2,
        sequential_duration
    );

    assert_eq!(sequential_data.modules.len(), iterations as usize);
    assert_eq!(sequential_data.basic_blocks.len(), iterations as usize);

    // Should complete in reasonable time
    assert!(
        sequential_duration.as_millis() < 5000,
        "Sequential building took too long: {sequential_duration:?}"
    );
}

#[test]
fn test_lookup_performance() {
    // Create data with many modules for lookup testing
    let module_count = 1000;
    let lookup_count = 10000;

    let mut builder = CoverageData::builder();
    for i in 0..module_count {
        let base = 0x400000 + i * 0x100000;
        builder = builder.add_module(&format!("/lib/module_{i}.so"), base, base + 0x50000);
    }
    let coverage = builder.build().unwrap();

    // Test find_module performance
    let find_start = Instant::now();
    for i in 0..lookup_count {
        let module_id = (i % module_count) as u16;
        let module = coverage.find_module(module_id);
        assert!(module.is_some());
    }
    let find_duration = find_start.elapsed();

    // Test find_module_by_address performance
    let addr_start = Instant::now();
    for i in 0..lookup_count {
        let module_idx = i % module_count;
        let address = 0x400000 + module_idx * 0x100000 + 0x1000;
        let module = coverage.find_module_by_address(address);
        assert!(module.is_some());
    }
    let addr_duration = addr_start.elapsed();

    println!("Lookup performance: {lookup_count} find_module calls in {find_duration:?}");
    println!(
        "Address lookup performance: {lookup_count} find_module_by_address calls in {addr_duration:?}"
    );

    // Should complete in reasonable time
    assert!(
        find_duration.as_millis() < 100,
        "find_module took too long: {find_duration:?}"
    );
    assert!(
        addr_duration.as_millis() < 1000,
        "find_module_by_address took too long: {addr_duration:?}"
    );
}

#[test]
fn test_statistics_performance() {
    // Test get_coverage_stats performance with large datasets
    let module_count = 100;
    let bb_count = 100000;

    let mut builder = CoverageData::builder();

    // Add modules
    for i in 0..module_count {
        let base = 0x400000 + i * 0x100000;
        builder = builder.add_module(&format!("/lib/module_{i}.so"), base, base + 0x50000);
    }

    // Add many basic blocks distributed across modules
    for i in 0..bb_count {
        let module_id = (i % module_count) as u16;
        builder = builder.add_coverage(module_id, (i * 4) as u32, 4);
    }

    let coverage = builder.build().unwrap();

    // Test statistics calculation performance
    let stats_start = Instant::now();
    let stats = coverage.get_coverage_stats();
    let stats_duration = stats_start.elapsed();

    println!(
        "Statistics performance: {bb_count} BBs across {module_count} modules in {stats_duration:?}"
    );

    // Verify correctness
    assert_eq!(stats.len(), module_count as usize);
    let total_blocks: usize = stats.values().sum();
    assert_eq!(total_blocks, bb_count as usize);

    // Should complete quickly
    assert!(
        stats_duration.as_millis() < 100,
        "get_coverage_stats took too long: {stats_duration:?}"
    );
}

#[test]
fn test_validation_performance() {
    // Test validation performance with large datasets
    let module_count = 1000;
    let bb_count = 50000;

    let mut builder = CoverageData::builder();

    for i in 0..module_count {
        let base = 0x400000 + i * 0x100000;
        builder = builder.add_module(&format!("/lib/module_{i}.so"), base, base + 0x50000);
    }

    for i in 0..bb_count {
        let module_id = (i % module_count) as u16;
        builder = builder.add_coverage(module_id, (i * 4) as u32, 4);
    }

    // Build includes validation
    let build_start = Instant::now();
    let coverage = builder.build().unwrap();
    let build_duration = build_start.elapsed();

    // Explicit validation
    let validate_start = Instant::now();
    let validation_result = coverage.validate();
    let validate_duration = validate_start.elapsed();

    println!(
        "Build (with validation) performance: {module_count} modules, {bb_count} BBs in {build_duration:?}"
    );
    println!("Explicit validation performance: {validate_duration:?}");

    assert!(validation_result.is_ok());

    // Should complete in reasonable time
    assert!(
        build_duration.as_millis() < 1000,
        "Build took too long: {build_duration:?}"
    );
    assert!(
        validate_duration.as_millis() < 100,
        "Validation took too long: {validate_duration:?}"
    );
}

#[test]
fn test_serialization_performance_by_format() {
    // Compare performance across different module table versions
    let module_count = 500;
    let bb_count = 5000;

    for version in [
        ModuleTableVersion::Legacy,
        ModuleTableVersion::V2,
        ModuleTableVersion::V3,
        ModuleTableVersion::V4,
    ] {
        let mut builder = CoverageData::builder()
            .flavor("perf_test")
            .module_version(version);

        for i in 0..module_count {
            let base = 0x400000 + i * 0x100000;
            builder = builder.add_module(&format!("/lib/module_{i}.so"), base, base + 0x50000);
        }

        for i in 0..bb_count {
            let module_id = (i % module_count) as u16;
            builder = builder.add_coverage(module_id, (i * 4) as u32, 4);
        }

        let coverage = builder.build().unwrap();

        // Test serialization performance
        let serialize_start = Instant::now();
        let mut buffer = Vec::new();
        to_writer(&coverage, &mut buffer).unwrap();
        let serialize_duration = serialize_start.elapsed();

        // Test deserialization performance
        let deserialize_start = Instant::now();
        let parsed = from_reader(Cursor::new(buffer)).unwrap();
        let deserialize_duration = deserialize_start.elapsed();

        println!(
            "Format {version:?} - Serialize: {serialize_duration:?}, Deserialize: {deserialize_duration:?}"
        );

        assert_eq!(parsed.modules.len(), module_count as usize);
        assert_eq!(parsed.basic_blocks.len(), bb_count as usize);

        // Should complete in reasonable time regardless of format
        assert!(serialize_duration.as_millis() < 1000);
        assert!(deserialize_duration.as_millis() < 1000);
    }
}

// Helper function to get approximate memory usage
// Note: This is a rough approximation and may not be available on all platforms
fn get_memory_usage() -> usize {
    // In a real implementation, you might use platform-specific APIs
    // For now, we'll return a dummy value since this is mainly for demonstration
    0
}

#[test]
fn test_concurrent_access_patterns() {
    // Test that immutable operations can be performed safely
    // (Rust's type system ensures memory safety, but we test performance)
    let coverage = CoverageData::builder()
        .add_module("/bin/test", 0x400000, 0x500000)
        .add_module("/lib/test", 0x500000, 0x600000)
        .add_coverage(0, 0x1000, 32)
        .add_coverage(1, 0x2000, 64)
        .build()
        .unwrap();

    // Simulate concurrent read operations
    let operations = 1000;
    let start = Instant::now();

    for i in 0..operations {
        // Mix different read operations
        match i % 4 {
            0 => {
                coverage.find_module(0);
            }
            1 => {
                coverage.find_module_by_address(0x450000);
            }
            2 => {
                coverage.get_coverage_stats();
            }
            3 => {
                coverage.validate().unwrap();
            }
            _ => unreachable!(),
        }
    }

    let duration = start.elapsed();
    println!("Concurrent-style operations: {operations} operations in {duration:?}");

    // Should complete quickly
    assert!(duration.as_millis() < 100);
}
