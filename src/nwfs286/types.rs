/*-
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * Copyright (c) 2024 Rink Springer <rink@rink.nu>
 * For conditions of distribution and use, see LICENSE file
 */
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Read;
use std::fmt;

pub const SECTOR_SIZE: u64 = 512;
pub const BLOCK_SIZE: u64 = 4096;

#[derive(Default,Debug)]
pub struct NwDate(u16);

impl NwDate {
    pub fn read_from<T: Read>(input: &mut T) -> Result<NwDate, std::io::Error> {
        let v = input.read_u16::<LittleEndian>()?;
        Ok(NwDate(v))
    }

    pub fn is_valid(&self) -> bool {
        self.0 > 0
    }
}

impl fmt::Display for NwDate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.is_valid() {
            let ts = self.0;
            // 0x9821 = 1001100000100001 = 24-12-1996
            let day = (ts >> 8) & 31;
            let month = (ts >> 13) + ((ts & 1) << 3);
            let year = ((ts & 0xff) >> 1) + 1980;
            write!(f, "{:02}-{:02}-{:04}", day, month, year)
        } else {
            write!(f, "<invalid>")
        }
    }
}

#[derive(Default,Debug)]
pub struct NwTime(u16);

impl NwTime {
    pub fn read_from<T: Read>(input: &mut T) -> Result<NwTime, std::io::Error> {
        let v = input.read_u16::<LittleEndian>()?;
        Ok(NwTime(v))
    }

    pub fn is_valid(&self) -> bool {
        self.0 > 0
    }
}

impl fmt::Display for NwTime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.is_valid() {
            let ts = self.0;
            // 0x4179 =  10 00001 01111 001 = 15:10
            // 0xe574 = 111 00101 01110 100 = 14:39
            // 0x4f39 =  10 01111 00111 001 =  7:10
            // 0x7251    11 10010 01010 001 = 10:11
            // 0x6b3a =  11 01011 00111 010 =  7:19
            // 0xfd56 = 111 11101 01010 110 = 10:55
            // 0x0e50        1110 01010 000 = 10:00
            // 0x2a50     1 01010 01010 000 = 10:01
            // 0x4250    10 00010 01010 000 = 10:02
            //          aaa  sec  hour  bbb
            // min = bbb aaa
            let hour = (ts >> 3) & 31;
            let min = (ts >> 13) + ((ts & 7) << 3);
            let sec = ((ts >> 8) & 31) * 2;
            write!(f, "{:02}:{:02}:{:02}", hour, min, sec)
        } else {
            write!(f, "<invalid>")
        }
    }
}

