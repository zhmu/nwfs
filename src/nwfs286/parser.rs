/*-
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * Copyright (c) 2024 Rink Springer <rink@rink.nu>
 * For conditions of distribution and use, see LICENSE file
 */
use byteorder::{LittleEndian, BigEndian, ReadBytesExt};
use std::io::{Read, Seek, SeekFrom, Cursor};
use std::fs::File;

use anyhow::{anyhow, Result};

use crate::nwfs286::types::{SECTOR_SIZE, BLOCK_SIZE, NwDate, NwTime};
use crate::util;

const _SECTOR_CONTROL: u64 = 0xf; // does not appear to be used?
const SECTOR_VOLUME_INFO: u64 = 0x10;

const DIRECTORY_ENTRY_SIZE: usize = 32;
const FAT_ENTRY_SIZE: usize = 4;

pub struct VolumeInfo {
    pub name: String,
    pub entry_count: usize,
    pub directory_entries_1_blocks: Vec<u16>,
    pub directory_entries_2_blocks: Vec<u16>,
    pub fat_blocks: Vec<u16>
}

#[derive(Debug)]
pub struct DirectoryEntry {
    pub entry_id: u16,
    pub parent_dir: u16,
    pub fname: String,
    pub attr: u16,
    pub size: u32,
    pub creation_date: NwDate,
    pub last_accessed_date: NwDate,
    pub last_modified_date: NwDate,
    pub last_modified_time: NwTime,
    pub block_nr: u16
}

#[derive(Debug)]
pub struct FatEntry {
    pub index: u16,
    pub block: u16
}

impl VolumeInfo {
    pub fn new(f: &mut File) -> Result<VolumeInfo> {
        //
        // Volume info:
        //
        // word     if zero, the next byte must be the magic value
        // word     magic value (0xfade)
        // byte[16] volume name
        // byte     number of entries (#)
        // word[#]  block numbers of directory
        // word[#]  block numbers of directory copy
        // word[#]  block numbers of fat
        //
        f.seek(SeekFrom::Start(SECTOR_VOLUME_INFO * SECTOR_SIZE))?;
        let mut sector = vec![ 0u8; SECTOR_SIZE as usize ];
        f.read_exact(&mut sector)?;

        let mut cursor = Cursor::new(&sector);
        let id = cursor.read_u16::<LittleEndian>()?;
        let mut vol_name = [ 0u8; 14 ];
        let offset = if id == 0 {
            let magic = cursor.read_u16::<LittleEndian>()?;
            if magic != 0xfade {
                return Err(anyhow!("volume information magic mismatch"));
            }
            let _ = cursor.read_u16::<LittleEndian>()?; // 1 ?
            22
        } else {
            20
        };
        cursor.read_exact(&mut vol_name)?;
        let name = util::asciiz_to_string(&vol_name);

        cursor.seek(SeekFrom::Start(offset))?;
        let _remap = cursor.read_u16::<LittleEndian>()?; // this value seems to be used for block remapping?
        let entry_count = cursor.read_u8()? as usize;
        let _ = cursor.read_u8()?; // unknown byte (3) following block size

        let mut directory_entries_1_blocks = Vec::<u16>::with_capacity(entry_count); // 2de
        for _ in 0..entry_count {
            let block = cursor.read_u16::<LittleEndian>()?;
            directory_entries_1_blocks.push(block);
        }
        let mut directory_entries_2_blocks = Vec::<u16>::with_capacity(entry_count); // 2e2
        for _ in 0..entry_count {
            let block = cursor.read_u16::<LittleEndian>()?;
            directory_entries_2_blocks.push(block);
        }
        let mut fat_blocks = Vec::<u16>::with_capacity(entry_count); // 2e0
        for _ in 0..entry_count {
            let block = cursor.read_u16::<LittleEndian>()?;
            fat_blocks.push(block);
        }

        // Note: this is related to bad sector remapping, not currently in use...
        let mut local_e = _remap >> 7;
        if (_remap & 0x7f) != 0 {
            local_e += 1;
        }
        let mut data_2f0 = local_e >> 3;
        if (local_e & 7) != 0 {
            data_2f0 += 1;
        }
        let _fat_blocks_offset = data_2f0 << 1; // 2e4 - used in fat_lookup()
        Ok(VolumeInfo{ name, entry_count, directory_entries_1_blocks, directory_entries_2_blocks, fat_blocks })
    }
}

pub fn read_directory_entries(f: &mut File, blocks: &[u16]) -> Result<Vec<DirectoryEntry>> {
    let total_entry_count = (blocks.len() * BLOCK_SIZE as usize) / DIRECTORY_ENTRY_SIZE;
    let mut result = Vec::<DirectoryEntry>::with_capacity(total_entry_count);

    let mut entry_id: u16 = 0;
    for block in blocks {
        f.seek(SeekFrom::Start(block_to_offset(*block)))?;

        let mut record = [ 0u8; DIRECTORY_ENTRY_SIZE ];
        for _ in 0..(BLOCK_SIZE as usize / DIRECTORY_ENTRY_SIZE) {
            f.read_exact(&mut record)?;
            let mut cursor = Cursor::new(&record);
            let parent_dir = cursor.read_u16::<BigEndian>()?;
            let mut fname = [ 0u8; 14 ];
            cursor.read_exact(&mut fname)?;
            let attr = cursor.read_u16::<LittleEndian>()?;
            let c = cursor.read_u16::<BigEndian>()?;
            let d = cursor.read_u16::<BigEndian>()?;
            let size = ((c as u32) << 16) + d as u32;
            let creation_date = NwDate::read_from(&mut cursor)?;
            let last_accessed_date = NwDate::read_from(&mut cursor)?;
            let last_modified_date = NwDate::read_from(&mut cursor)?;
            let last_modified_time = NwTime::read_from(&mut cursor)?;
            // Note: does not seem to hold for directories!
            let block_nr = cursor.read_u16::<LittleEndian>()?;
            let fname = util::asciiz_to_string(&fname);
            result.push(DirectoryEntry{ entry_id, parent_dir, fname, attr, size, creation_date, last_accessed_date, last_modified_date, last_modified_time, block_nr });
            entry_id += 1;
        }

    }
    Ok(result)
}

pub fn read_fat_table(f: &mut File, blocks: &[u16]) -> Result<Vec<FatEntry>> {
    let total_entry_count = (blocks.len() * BLOCK_SIZE as usize) / FAT_ENTRY_SIZE;
    let mut result = Vec::<FatEntry>::with_capacity(total_entry_count);
    for block in blocks {
        f.seek(SeekFrom::Start(block_to_offset(*block)))?;

        let mut record = [ 0u8; FAT_ENTRY_SIZE ];
        for _ in 0..(BLOCK_SIZE as usize / FAT_ENTRY_SIZE) {
            f.read_exact(&mut record)?;
            let mut cursor = Cursor::new(&record);
            let index = cursor.read_u16::<LittleEndian>()?;
            let block = cursor.read_u16::<LittleEndian>()?;
            result.push(FatEntry{ index, block });
        }
    }
    Ok(result)
}

pub fn block_to_offset(block: u16) -> u64 {
    // TODO This is obviously not correct as it doesn't consider the
    // partition offset
    ((block as u64) + 4) * BLOCK_SIZE
}
