use drcov::{CoverageData, ModuleTableVersion};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // create coverage data
    let new_coverage = CoverageData::builder()
        .flavor("my_tool")
        .module_version(ModuleTableVersion::V4)
        .add_module("/bin/program", 0x400000, 0x450000)
        .add_coverage(0, 0x1000, 32)
        .build()?;

    // write to a file
    drcov::to_file(&new_coverage, "/tmp/output.drcov")?;

    // read it back
    let coverage = drcov::from_file("/tmp/output.drcov")?;
    
    println!("Successfully read coverage with {} modules and {} basic blocks", 
             coverage.modules.len(), coverage.basic_blocks.len());
    
    Ok(())
}