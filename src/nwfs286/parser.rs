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

const ATTR_DIRECTORY: u16 = 0xff00;

pub struct VolumeInfo {
    pub name: String,
    pub entry_count: usize,
    pub directory_entries_1_blocks: Vec<u16>,
    pub directory_entries_2_blocks: Vec<u16>,
    pub fat_blocks: Vec<u16>
}

#[derive(Debug)]
pub struct FileItem {
    pub entry_id: u16,
    pub parent_dir: u16,
    pub name: String,
    pub unk14: u16,
    pub attr: u16,
    pub size: u32,
    pub creation_date: NwDate,
    pub last_accessed_date: NwDate,
    pub last_modified_date: NwDate,
    pub last_modified_time: NwTime,
    pub block_nr: u16
}

#[derive(Debug)]
pub struct DirectoryItem {
    pub entry_id: u16,
    pub parent_dir: u16,
    pub name: String,
    pub unk14: u16,
    pub attr: u16,
    pub last_modified_date: NwDate,
    pub last_modified_time: NwTime,
    pub unk22: u16,
    pub unk24: u16,
    pub unk26: u16,
    pub unk28: u16,
    pub unk30: u16,
}

#[derive(Debug)]
pub enum DirEntry {
    File(FileItem),
    Directory(DirectoryItem)
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
        let mut vol_name = [ 0u8; 16 ];

        let marker = cursor.read_u16::<LittleEndian>()?;
        if marker == 0 {
            // NetWare 2.15+ layout
            let magic = cursor.read_u16::<LittleEndian>()?;
            if magic != 0xfade {
                return Err(anyhow!("volume information magic mismatch"));
            }
            let unk4 = cursor.read_u16::<LittleEndian>()?;
            if unk4 != 1 { println!("warning: unexpected value for unk4 ({:x})", unk4); }
            cursor.read_exact(&mut vol_name)?;
        } else {
            // pre NetWare 2.15 layout
            cursor.read_exact(&mut vol_name)?;
            let unk18 = cursor.read_u16::<LittleEndian>()?;
            if unk18 != 4 { println!("warning: unexpected value for unk18 ({:x})", unk18); }
        }
        let name = util::asciiz_to_string(&vol_name);

        let _remap = cursor.read_u16::<LittleEndian>()?; // this value seems to be used for block remapping?
        let entry_count = cursor.read_u8()? as usize;
        let unk23_25 = cursor.read_u8()?;
        if unk23_25 != 3 { println!("warning: unexpected value for unk23_25 ({:x})", unk23_25); }

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

pub fn read_directory_entries(f: &mut File, blocks: &[u16]) -> Result<Vec<DirEntry>> {
    let total_entry_count = (blocks.len() * BLOCK_SIZE as usize) / DIRECTORY_ENTRY_SIZE;
    let mut result = Vec::<DirEntry>::with_capacity(total_entry_count);

    let mut entry_id: u16 = 0;
    for block in blocks {
        f.seek(SeekFrom::Start(block_to_offset(*block)))?;

        let mut record = [ 0u8; DIRECTORY_ENTRY_SIZE ];
        for _ in 0..(BLOCK_SIZE as usize / DIRECTORY_ENTRY_SIZE) {
            f.read_exact(&mut record)?;
            let mut cursor = Cursor::new(&record);
            let parent_dir = cursor.read_u16::<BigEndian>()?;
            let mut name = [ 0u8; 12 ];
            cursor.read_exact(&mut name)?;
            let name = util::asciiz_to_string(&name);
            let unk14 = cursor.read_u16::<LittleEndian>()?;
            let attr = cursor.read_u16::<LittleEndian>()?;
            if (attr & ATTR_DIRECTORY) == ATTR_DIRECTORY {
                let last_modified_date = NwDate::read_from(&mut cursor)?;
                let last_modified_time = NwTime::read_from(&mut cursor)?;
                let unk22 = cursor.read_u16::<LittleEndian>()?;
                let unk24 = cursor.read_u16::<LittleEndian>()?;
                let unk26 = cursor.read_u16::<LittleEndian>()?;
                let unk28 = cursor.read_u16::<LittleEndian>()?;
                let unk30 = cursor.read_u16::<LittleEndian>()?;
                let dir = DirectoryItem{entry_id, parent_dir, name, attr, unk14, last_modified_date, last_modified_time, unk22, unk24, unk26, unk28, unk30 };
                result.push(DirEntry::Directory(dir));
            } else {
                let c = cursor.read_u16::<LittleEndian>()?;
                let d = cursor.read_u16::<LittleEndian>()?;
                let size = ((c as u32) << 16) + d as u32;
                let creation_date = NwDate::read_from(&mut cursor)?;
                let last_accessed_date = NwDate::read_from(&mut cursor)?;
                let last_modified_date = NwDate::read_from(&mut cursor)?;
                let last_modified_time = NwTime::read_from(&mut cursor)?;
                // Note: does not seem to hold for directories!
                let block_nr = cursor.read_u16::<LittleEndian>()?;
                let file = FileItem{entry_id, parent_dir, name, attr, unk14, size, creation_date, last_accessed_date, last_modified_date, last_modified_time, block_nr};
                result.push(DirEntry::File(file));
            }
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
