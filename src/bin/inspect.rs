/*-
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * Copyright (c) 2022, 2024 Rink Springer <rink@rink.nu>
 * For conditions of distribution and use, see LICENSE file
 */
use std::env;
use std::fs::File;
use anyhow::{anyhow, Result};

use nwfs::nwfs286::inspect as inspect286;
use nwfs::nwfs386::inspect as inspect386;
use nwfs::util;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        panic!("usage: {} file.img", args[0]);
    }

    let path = &args[1];
    let mut f = File::open(path)?;
    match util::find_partition(&mut f)? {
        Some((util::PartitionType::NetWare286, lba_start)) => {
            inspect286::inspect(&mut f, lba_start)
        },
        Some((util::PartitionType::NetWare386, lba_start)) => {
            inspect386::inspect(&mut f, lba_start)
        },
        None => { Err(anyhow!("no NetWare partition found")) },
    }
}
