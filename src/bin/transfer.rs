/*-
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * Copyright (c) 2022, 2024 Rink Springer <rink@rink.nu>
 * For conditions of distribution and use, see LICENSE file
 */
use std::fs::File;
use anyhow::{anyhow, Result};
use clap::Parser;

use nwfs::nwfs286::{parser, shell as shell286};
use nwfs::nwfs386::{image, volume, shell as shell386};
use nwfs::{shell_cli, util};

/// Transfer data from NetWare 386 partitions
#[derive(Parser)]
struct Cli {
    /// Image file
    image: String,
    #[clap(long, short, default_value="SYS")]
    /// Volume to access
    volume: String,
}

fn main() -> Result<()> {
    let args = Cli::parse();
    let mut f = File::open(args.image)?;

    let partition = util::find_partition(&mut f)?;
    match partition {
        Some((util::PartitionType::NetWare286, start_lba)) => {
            // Note: for now, this will only work on dedicated installations (partition
            // must cover the entire disk)
            println!("Detected a NWFS286 partition");
            if start_lba != 1 {
                return Err(anyhow!("NetWare 286 partition currently must cover the entire disk - please contribute the image so support can be added"));
            }
            let vi = parser::VolumeInfo::new(&mut f)?;

            let entries = parser::read_directory_entries(&mut f, &vi.directory_entries_1_blocks)?;
            let fat = parser::read_fat_table(&mut f, &vi.fat_blocks)?;

            let mut shell = shell286::Nwfs286Shell::new(vi, entries, fat, f);
            shell_cli::run(&mut shell)
        },
        Some((util::PartitionType::NetWare386, _)) => {
            println!("Detected a NWFS386 partition");
            let mut image_list = image::ImageList::new();
            image_list.add_image(f)?;

            let mut vol = volume::LogicalVolume::new(&mut image_list, &args.volume)?;
            let mut shell = shell386::Nwfs386Shell::new(&mut vol);
            shell_cli::run(&mut shell)
        },
        None => Err(anyhow!("no NetWare partition found")),
    }
}
