/*-
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * Copyright (c) 2022 Rink Springer <rink@rink.nu>
 * For conditions of distribution and use, see LICENSE file
 */

use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Read;
use std::fmt;

pub const SECTOR_SIZE: u64 = 512;

#[derive(Debug)]
pub enum NetWareError {
    IoError(std::io::Error),
    NoPartitionFound,
    VolumeAreaCorrupt,
    VolumeNotFound,
    FATCorrupt(u32),
    BlockOutOfRange(u32),
}

impl From<std::io::Error> for NetWareError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e)
    }
}

#[derive(Default,Debug)]
pub struct Timestamp(u32);

impl Timestamp {
    pub fn read_from<T: Read>(input: &mut T) -> Result<Timestamp, std::io::Error> {
        let v = input.read_u32::<LittleEndian>()?;
        Ok(Timestamp(v))
    }

    pub fn is_valid(&self) -> bool {
        self.0 > 0
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.is_valid() {
            let ts = self.0;
            let date_part = ts >> 16;
            let time_part = ts & 0xffff;

            let hour = time_part >> 11;
            let min = (time_part >> 5) & 63;
            let sec = (time_part & 0x1f) * 2;

            let day = date_part & 0x1f;
            let month = (date_part >> 5) & 0xf;
            let year = (date_part >> 9) + 1980;
            write!(f, "{:02}-{:02}-{:04} {:02}:{:02}:{:02}", day, month, year, hour, min, sec)
        } else {
            write!(f, "<invalid>")
        }
    }
}
