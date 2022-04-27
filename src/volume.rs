/*-
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * Copyright (c) 2022 Rink Springer <rink@rink.nu>
 * For conditions of distribution and use, see LICENSE file
 */

use std::io::{Seek, SeekFrom};
use byteorder::{LittleEndian, ReadBytesExt};

use crate::{image, partition, parser};
use crate::types::*;

pub struct VolumeInImage {
    pub file: std::fs::File,
    pub first_data_block_offset: u64,
    pub info: parser::VolumeInfo,
}

impl VolumeInImage {
    fn calculate_block_range(&self) -> (u32, u32) {
        let v = &self.info;
        let first_block = v.first_segment_block;
        let sectors_per_block = v.block_size / SECTOR_SIZE as u32;
        let last_block = first_block + v.num_sectors * sectors_per_block;
        (first_block, last_block)
    }
}

pub struct LogicalVolume  {
    pub volumes: Vec<VolumeInImage>,
    pub directory: Vec<parser::DirEntry>,
}

impl LogicalVolume {
    pub fn new(image_list: &mut image::ImageList, name: &str) -> Result<LogicalVolume, NetWareError> {
        let mut volumes = Vec::new();
        for partition in image_list.images.iter_mut() {
            let nwp = partition::NWPartition::new(&mut partition.file, partition.start_offset)?;
            for info in nwp.volumes.info {
                if info.name == name {
                    let file = partition.file.try_clone()?;
                    volumes.push(VolumeInImage{ file, first_data_block_offset: nwp.first_data_block_offset, info });
                    break;
                }
            }
        }
        if volumes.is_empty() {
            return Err(NetWareError::VolumeNotFound);
        }
        let mut logical_volume = LogicalVolume{ volumes, directory: Vec::new() };
        logical_volume.read_directory()?;
        Ok(logical_volume)
    }

    fn read_directory(&mut self) -> Result<(), NetWareError> {
        let volume = self.volumes.first().unwrap();
        let block_size = volume.info.block_size;
        let mut items = Vec::new();
        let mut current_entry = volume.info.rootdir_block_nr;
        while current_entry != 0xffffffff {
            let file = self.seek_block(current_entry)?;
            for _ in 0..(block_size / 128) {
                let dir_entry = parser::parse_directory_entry(file)?;
                items.push(dir_entry);
            }

            let entry = self.read_fat_entry(current_entry)?;
            current_entry = entry.1;
        }
        self.directory = items;
        Ok(())
    }

    pub fn seek_block(&mut self, block: u32) -> Result<&mut std::fs::File, NetWareError> {
        for vol in self.volumes.iter_mut() {
            let (first_block, last_block) = vol.calculate_block_range();
            if block >= first_block && block < last_block {
                let index = (block - first_block) as u64;
                let block_size = vol.info.block_size as u64;
                let offset = vol.first_data_block_offset + index * block_size;
                let file = &mut vol.file;
                file.seek(SeekFrom::Start(offset))?;
                return Ok(file)
            }
        }
        Err(NetWareError::BlockOutOfRange(block))
    }

    pub fn read_fat_entry(&mut self, entry: u32) -> Result<(u32, u32), NetWareError> {
        for vol in self.volumes.iter_mut() {
            let (first_block, last_block) = vol.calculate_block_range();
            if entry >= first_block && entry < last_block {
                let offset = vol.first_data_block_offset + (entry - first_block) as u64 * 8;
                let file = &mut vol.file;
                file.seek(SeekFrom::Start(offset))?;
                let a = file.read_u32::<LittleEndian>()?;
                let b = file.read_u32::<LittleEndian>()?;
                return Ok((a, b));
            }
        }
        Err(NetWareError::FATCorrupt(entry))
    }
}

