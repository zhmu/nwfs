/*-
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * Copyright (c) 2022, 2024 Rink Springer <rink@rink.nu>
 * For conditions of distribution and use, see LICENSE file
 */
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{Read, Seek, SeekFrom};
use anyhow::Result;

const PARTID_NETWARE_286: u8 = 0x64;
const PARTID_NETWARE_386: u8 = 0x65;

pub enum PartitionType {
    NetWare286,
    NetWare386,
}

pub fn find_partition<T: Seek + Read>(f: &mut T) -> Result<Option<(PartitionType, u64)>> {
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
        match system_id {
            PARTID_NETWARE_286 => { return Ok(Some((PartitionType::NetWare286, lba_start.into()))); },
            PARTID_NETWARE_386 => { return Ok(Some((PartitionType::NetWare386, lba_start.into()))); },
            _ => { }
        }
    }
    Ok(None)
}

pub fn asciiz_to_string(s: &[u8]) -> String {
    let mut s = String::from_utf8_lossy(s).to_string();
    if let Some(n) = s.find(char::from(0)) {
        s.truncate(n);
    }
    s
}

pub fn ascii_with_length_to_string(s: &[u8], length: u8) -> String {
    let mut s = String::from_utf8_lossy(s).to_string();
    s.truncate(length.into());
    s
}

