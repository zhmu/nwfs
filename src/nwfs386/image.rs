/*-
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * Copyright (c) 2022, 2024 Rink Springer <rink@rink.nu>
 * For conditions of distribution and use, see LICENSE file
 */
use std::io::{Read, Seek, SeekFrom};
use byteorder::{LittleEndian, ReadBytesExt};

use anyhow::{anyhow, Result};
use crate::nwfs386::types;

const SYSTEM_ID_NETWARE: u8 = 0x65;

pub fn find_netware_partition<T: Seek + Read>(f: &mut T) -> Result<Option<u64>> {
    // Seek to MBR and parse the partition table
    f.seek(SeekFrom::Start(446))?;
    for _ in 0..4 {
        // Skip boot_flag, start CHS
        f.seek(SeekFrom::Current(4))?;
        let system_id = f.read_u8()?;
        // Skip end CHS
        f.seek(SeekFrom::Current(3))?;
        let lba_start = f.read_u32::<LittleEndian>()?;
        // Skip lba length
        f.seek(SeekFrom::Current(4))?;
        if system_id != SYSTEM_ID_NETWARE { continue; }
        return Ok(Some(lba_start.into()))
    }
    Ok(None)
}

pub struct Image {
    pub file: std::fs::File,
    pub start_offset: u64,
}

pub struct ImageList  {
    pub images: Vec<Image>
}

impl ImageList {
    pub fn new() -> Self {
        Self{ images: Vec::new() }
    }

    pub fn add_image(&mut self, mut file: std::fs::File) -> Result<()> {
        let p = find_netware_partition(&mut file)?;
        if p.is_none() { return Err(anyhow!("no NetWare 386 partition found")); }
        let start_offset = p.unwrap() * types::SECTOR_SIZE;

        self.images.push(Image{ file, start_offset });
        Ok(())
    }
}

