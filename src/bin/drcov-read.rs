use clap::Parser;
use std::path::PathBuf;
use std::process;

#[derive(Parser, Debug)]
#[command(author, version, about = "A tool to read and analyze DrCov files.", long_about = None)]
struct Args {
    /// Path to the .drcov file
    #[arg(required = true)]
    file: PathBuf,

    /// Print detailed list of all basic blocks
    #[arg(short, long)]
    detailed: bool,

    /// Filter and show details for a specific module (by name substring)
    #[arg(short, long)]
    module: Option<String>,
}

fn main() {
    let args = Args::parse();

    let coverage_data = match drcov::from_file(&args.file) {
        Ok(data) => data,
        Err(e) => {
            eprintln!(
                "Error: Failed to parse DrCov file '{}': {}",
                args.file.display(),
                e
            );
            process::exit(1);
        }
    };

    println!("=== DrCov File Analysis ===");
    println!("File: {}", args.file.display());
    println!("Version: {}", coverage_data.header.version);
    println!("Flavor: {}", coverage_data.header.flavor);
    println!(
        "Module Table Version: {}",
        coverage_data.module_version as u32
    );
    println!();

    println!("=== Summary ===");
    println!("Total Modules: {}", coverage_data.modules.len());
    println!("Total Basic Blocks: {}", coverage_data.basic_blocks.len());

    let total_coverage_bytes: u64 = coverage_data
        .basic_blocks
        .iter()
        .map(|bb| bb.size as u64)
        .sum();
    println!("Total Coverage: {total_coverage_bytes} bytes");
    println!();

    println!("=== Module Coverage ===");
    println!(
        "{:<4} {:<8} {:<12} {:<20} Name",
        "ID", "Blocks", "Size", "Base Address"
    );
    println!("{}", "-".repeat(80));

    let stats = coverage_data.get_coverage_stats();
    for module in &coverage_data.modules {
        let block_count = stats.get(&(module.id as u16)).copied().unwrap_or(0);
        let module_bytes: u64 = coverage_data
            .basic_blocks
            .iter()
            .filter(|bb| bb.module_id as u32 == module.id)
            .map(|bb| bb.size as u64)
            .sum();

        println!(
            "{:<4} {:<8} {:<12} 0x{:016x} {}",
            module.id,
            block_count,
            format!("{} bytes", module_bytes),
            module.base,
            module.path
        );
    }
    println!();

    if args.detailed {
        println!("=== Detailed Basic Blocks ===");
        println!(
            "{:<8} {:<14} {:<8} {:<18} Module Name",
            "Module", "Offset", "Size", "Absolute Addr"
        );
        println!("{}", "-".repeat(80));
        for bb in &coverage_data.basic_blocks {
            if let Some(module) = coverage_data.find_module(bb.module_id) {
                let abs_addr = bb.absolute_address(module);
                println!(
                    "{:<8} 0x{:<11x} {:<8} 0x{:<15x} {}",
                    bb.module_id, bb.start, bb.size, abs_addr, module.path
                );
            }
        }
        println!();
    }

    if let Some(module_filter) = &args.module {
        println!("=== Module-Specific Analysis: {module_filter} ===");
        let mut found = false;
        for module in coverage_data
            .modules
            .iter()
            .filter(|m| m.path.contains(module_filter))
        {
            found = true;
            println!("Module ID: {}", module.id);
            println!("Name: {}", module.path);
            println!("Base: 0x{:x}", module.base);
            println!("End: 0x{:x}", module.end);
            println!("Size: {} bytes", module.size());

            let block_count = stats.get(&(module.id as u16)).copied().unwrap_or(0);
            let module_bytes: u64 = coverage_data
                .basic_blocks
                .iter()
                .filter(|bb| bb.module_id as u32 == module.id)
                .map(|bb| bb.size as u64)
                .sum();

            println!("Covered Blocks: {block_count}");
            println!("Covered Bytes: {module_bytes}");
            println!();
        }
        if !found {
            println!("No modules found matching: {module_filter}");
        }
    }
}
