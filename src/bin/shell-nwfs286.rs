use anyhow::{anyhow, Result};

use clap::Parser;

use std::fs::File;

use nwfs::nwfs286::parser;
use nwfs::util;
use nwfs::shell_cli;
use nwfs::nwfs286::shell;

/// Transfer data from NetWare 286 partitions
#[derive(Parser)]
struct Cli {
    /// Input file
    in_file: String,
}

fn main() -> Result<()> {
    let args = Cli::parse();
    let mut f = File::open(args.in_file)?;

    // Note: for now, this will only work on dedicated installations (partition
    // must cover the entire disk)
    let p = util::find_partition(&mut f, util::PartitionType::NetWare286)?;
    if p.is_none() { return Err(anyhow!("no NetWare 286 partition found")); }
    if p.unwrap() != 1 { return Err(anyhow!("NetWare 286 partition doesn't cover the entire disk")); }
    let vi = parser::VolumeInfo::new(&mut f)?;

    let entries = parser::read_directory_entries(&mut f, &vi.directory_entries_1_blocks)?;
    let fat = parser::read_fat_table(&mut f, &vi.fat_blocks)?;

    let mut shell = shell::Nwfs286Shell::new(vi, entries, fat, f);
    shell_cli::run(&mut shell)
}
