/*-
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * Copyright (c) 2022, 2024 Rink Springer <rink@rink.nu>
 * For conditions of distribution and use, see LICENSE file
 */
use std::fs::File;
use anyhow::{anyhow, Result};

use crate::nwfs286::parser;

pub fn inspect(f: &mut File, lba_start: u64) -> Result<()> {
    println!("found NetWare 286 partition starting at lba {}", lba_start);
    if lba_start != 1 { return Err(anyhow!("partitions that do not start at block 1 are not yet supported")) }

    let vi = parser::VolumeInfo::new(f)?;
    println!("volume '{}', entry_count {}", vi.name, vi.entry_count);
    println!("directory entries 1 blocks: {:?}", vi.directory_entries_1_blocks);
    println!("directory entries 2 blocks: {:?}", vi.directory_entries_2_blocks);
    println!("fat blocks: {:?}", vi.fat_blocks);

    let entries = parser::read_directory_entries(f, &vi.directory_entries_1_blocks)?;
    for (n, entry) in entries.iter().enumerate() {
        println!("entry {}: {:?}", n, entry);
    }

    let fat = parser::read_fat_table(f, &vi.fat_blocks)?;
    for (n, entry) in fat.iter().enumerate() {
        println!("fat entry {}: index {} block {}", n, entry.index, entry.block);
    }
    Ok(())
}
