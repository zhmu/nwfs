/*-
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * Copyright (c) 2022, 2024 Rink Springer <rink@rink.nu>
 * For conditions of distribution and use, see LICENSE file
 */
use std::io::{Seek, SeekFrom};
use std::fs::File;
use std::env;
use anyhow::Result;

use nwfs::nwfs386::{image, parser, partition, types};

fn get_fat_entry(f: &mut std::fs::File, first_block_offset: u64, entry_num: u32) -> Result<(u32, u32)> {
    let block_offset = first_block_offset + ((entry_num as u64) * 8);
    f.seek(SeekFrom::Start(block_offset))?;
    parser::parse_fat_entry(f)
}

fn dump_fat_chain(f: &mut std::fs::File, first_block_offset: u64, entry_num: u32) -> Result<()> {
    println!("dump_fat_chain: entry {} ->", entry_num);
    let mut current_entry = entry_num;
    while current_entry != 0xffffffff {
        let entry = get_fat_entry(f, first_block_offset, current_entry)?;
        println!("  {}/{} ", entry.0, entry.1);
        current_entry = entry.1;
    }
    Ok(())
}

fn read_dir_block(f: &mut std::fs::File, block_size: u32) -> Result<()> {
    for _ in 0..(block_size / 128) {
        let de = parser::parse_directory_entry(f)?;
        match de {
            parser::DirEntry::Available(av) => {
                println!("<available> {:x?}", av);
            },
            parser::DirEntry::GrantList(gl) => {
                println!("<grant-list>: {:x?}", gl);
            },
            parser::DirEntry::VolumeInformation(vi) => {
                println!("<volume-info>: {:x?}", vi);
            }
            parser::DirEntry::File(fi) => {
                println!("<file>: {:x?}", fi);
            },
            parser::DirEntry::Directory(di) => {
                println!("<directory>: {}", di);
            }
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        panic!("usage: {} file.img", args[0]);
    }

    let path = &args[1];
    let mut f = File::open(path)?;
    let p = image::find_netware_partition(&mut f)?;
    if p.is_none() {
        panic!("cannot find a NetWare partition");
    }
    let lba_start = p.unwrap();
    println!("partition offset {:x}", lba_start * types::SECTOR_SIZE);

    let partition = partition::NWPartition::new(&mut f, lba_start * types::SECTOR_SIZE)?;
    println!("hotfix data: {:?}", partition.hotfix);
    println!("mirror data: {:?}", partition.mirror);
    println!("volume data: {:?}", partition.volumes);

    if partition.volumes.info.is_empty() {
        println!("No NetWare volume found");
        return Ok(())
    }

    let volume_info = partition.volumes.info.first().unwrap();
    let volume_block_size = volume_info.block_size;
    let data_block_offset = partition.first_data_block_offset;

    // vol1
    let first_data_block_offset = partition.first_data_block_offset;
    dump_fat_chain(&mut f, first_data_block_offset, volume_info.rootdir_block_nr)?;

    let mut current_entry = volume_info.rootdir_block_nr;
    while current_entry != 0xffffffff {
        let entry = get_fat_entry(&mut f, first_data_block_offset, current_entry)?;
        if entry.1 == 0xffffffff { break; }
        println!(">> root dir entry {}/{} ", entry.0, entry.1);

        let offset = data_block_offset + (entry.1 as u64 * volume_block_size as u64);
        f.seek(SeekFrom::Start(offset as u64))?;
        read_dir_block(&mut f, volume_block_size)?;
        current_entry = entry.1;
    }

    Ok(())
}
