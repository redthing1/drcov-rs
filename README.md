
# drcov-rs

a rust library for parsing and writing DrCov coverage files.

## usage

```rust
use drcov::{CoverageData, ModuleTableVersion};

// read a file
let coverage = drcov::from_file("coverage.drcov")?;

// create coverage data
let new_coverage = CoverageData::builder()
    .flavor("my_tool")
    .module_version(ModuleTableVersion::V4)
    .add_module("/bin/program", 0x400000, 0x450000)
    .add_coverage(0, 0x1000, 32)
    .build()?;

// write to a file
drcov::to_file(&new_coverage, "output.drcov")?;
```

## demo

```sh
cargo run --bin drcov-read --features cli -- file.drcov --detailed
```
