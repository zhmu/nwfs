/*-
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * Copyright (c) 2022, 2024 Rink Springer <rink@rink.nu>
 * For conditions of distribution and use, see LICENSE file
 */
use std::fs::File;
use anyhow::Result;
use clap::Parser;

use nwfs::nwfs386::{image, volume, shell};
use nwfs::shell_cli;

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
    let f = File::open(args.image)?;

    let mut image_list = image::ImageList::new();
    image_list.add_image(f)?;

    let mut vol = volume::LogicalVolume::new(&mut image_list, &args.volume)?;
    let mut shell = shell::Nwfs386Shell::new(&mut vol);
    shell_cli::run(&mut shell)
}
