use std::path::Path;

#[test]
fn test_parse_simple_drcov_file() {
    let test_file = Path::new("testdata/simple.drcov");

    // Skip test if file doesn't exist (for CI environments)
    if !test_file.exists() {
        return;
    }

    let coverage = drcov::from_file(test_file).unwrap();

    assert_eq!(coverage.header.version, 2);
    assert_eq!(coverage.header.flavor, "test");
    assert_eq!(coverage.modules.len(), 2);
    assert_eq!(coverage.basic_blocks.len(), 3);

    // Check first module
    assert_eq!(coverage.modules[0].id, 0);
    assert_eq!(coverage.modules[0].base, 0x400000);
    assert_eq!(coverage.modules[0].end, 0x450000);
    assert_eq!(coverage.modules[0].path, "/bin/test_program");

    // Check basic blocks
    assert_eq!(coverage.basic_blocks[0].start, 0x1000);
    assert_eq!(coverage.basic_blocks[0].size, 32);
    assert_eq!(coverage.basic_blocks[0].module_id, 0);
}

#[test]
fn test_parse_v4_format_file() {
    let test_file = Path::new("testdata/v4_format.drcov");

    // Skip test if file doesn't exist (for CI environments)
    if !test_file.exists() {
        return;
    }

    let coverage = drcov::from_file(test_file).unwrap();

    assert_eq!(coverage.header.flavor, "modern_tool");
    assert_eq!(coverage.module_version, drcov::ModuleTableVersion::V4);
    assert_eq!(coverage.modules.len(), 1);
    assert_eq!(coverage.basic_blocks.len(), 2);

    // Check module with extended fields
    let module = &coverage.modules[0];
    assert_eq!(module.containing_id, Some(-1));
    assert_eq!(module.checksum, Some(0x12345678));
    assert_eq!(module.timestamp, Some(0x87654321));
    assert_eq!(module.offset, Some(0));
}
