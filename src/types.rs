/*-
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * Copyright (c) 2022 Rink Springer <rink@rink.nu>
 * For conditions of distribution and use, see LICENSE file
 */

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
