/*-
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * Copyright (c) 2022, 2024 Rink Springer <rink@rink.nu>
 * For conditions of distribution and use, see LICENSE file
 */
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Read;
use std::fmt;

pub const SECTOR_SIZE: u64 = 512;

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

#[derive(Default,Debug)]
pub struct Attributes(u32);

impl Attributes {
    pub fn read_from<T: Read>(input: &mut T) -> Result<Attributes, std::io::Error> {
        let v = input.read_u32::<LittleEndian>()?;
        Ok(Attributes(v))
    }

    pub fn is_directory(&self) -> bool {
        (self.0 & ATTR_DIRECTORY) != 0
    }
}

const ATTR_COPY_INHIBIT: u32 = 0x80000;
const ATTR_DELETE_INHIBIT: u32 = 0x40000;
const ATTR_RENAME_INHIBIT: u32 = 0x20000;
const ATTR_PURGE: u32 = 0x10000;
const ATTR_TRANSACTIONAL: u32 = 0x1000;
const ATTR_SHAREABLE: u32 = 0x80;
const ATTR_ARCHIVE: u32 = 0x20;
const ATTR_DIRECTORY: u32 = 0x10;
const ATTR_SYSTEM: u32 = 0x4;
const ATTR_HIDDEN: u32 = 0x2;
const ATTR_READONLY: u32 = 0x1;

impl fmt::Display for Attributes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut s = String::new();
        let attr = self.0;
        if (attr & ATTR_READONLY) != 0 { s += "Ro" } else { s += "Rw" };
        if (attr & ATTR_SHAREABLE) != 0 { s += "S" } else { s += "-" };
        if (attr & ATTR_ARCHIVE) != 0 { s += "A" } else { s += "-" };
        s += "-";
        if (attr & ATTR_HIDDEN) != 0 { s += "H" } else { s += "-" };
        if (attr & ATTR_SYSTEM) != 0 { s += "Sy" } else { s += "--" };
        if (attr & ATTR_TRANSACTIONAL) != 0 { s += "T" } else { s += "-" };
        s += "-";
        if (attr & ATTR_PURGE) != 0 { s += "P" } else { s += "-" };
        s += "--"; // read audit (Ra)
        s += "--"; // write audit (Wa)
        if (attr & ATTR_COPY_INHIBIT) != 0 { s += "Ci" } else { s += "--" };
        if (attr & ATTR_COPY_INHIBIT) != 0 { s += "Ci" } else { s += "--" };
        if (attr & ATTR_DELETE_INHIBIT) != 0 { s += "Di" } else { s += "--" };
        if (attr & ATTR_RENAME_INHIBIT) != 0 { s += "Ri" } else { s += "--" };
        write!(f, "{}", s)
    }
}

#[derive(Default,Debug)]
pub struct Rights(u16);

impl Rights {
    pub fn read_from<T: Read>(input: &mut T) -> Result<Rights, std::io::Error> {
        let v = input.read_u16::<LittleEndian>()?;
        Ok(Rights(v))
    }
}

const RIGHT_READ: u16 = 0x1;
const RIGHT_WRITE: u16 = 0x2;
const RIGHT_CREATE: u16 = 0x8;
const RIGHT_ERASE: u16 = 0x10;
const RIGHT_ACCESS_CONTROL: u16 = 0x20;
const RIGHT_FILESCAN: u16 = 0x40;
const RIGHT_MODIFY: u16 = 0x80;
const RIGHT_SUPERVISOR: u16 = 0x100;

impl fmt::Display for Rights {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut s = String::new();
        let rights = self.0;
        if (rights & RIGHT_SUPERVISOR) != 0 { s += "S" } else { s += " " };
        if (rights & RIGHT_READ) != 0 { s += "R" } else { s += " " };
        if (rights & RIGHT_WRITE) != 0 { s += "W" } else { s += " " };
        if (rights & RIGHT_CREATE) != 0 { s += "C" } else { s += " " };
        if (rights & RIGHT_ERASE) != 0 { s += "E" } else { s += " " };
        if (rights & RIGHT_MODIFY) != 0 { s += "M" } else { s += " " };
        if (rights & RIGHT_FILESCAN) != 0 { s += "F" } else { s += " " };
        if (rights & RIGHT_ACCESS_CONTROL) != 0 { s += "A" } else { s += " " };
        write!(f, "{}", s)
    }
}
