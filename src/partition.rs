/*-
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * Copyright (c) 2022 Rink Springer <rink@rink.nu>
 * For conditions of distribution and use, see LICENSE file
 */
use crate::types::*;
use crate::parser;

const HOTFIX_OFFSET: u64 = 0x4000;
const VOLUME_SIZE: u64 = 4 * 16384;

pub struct NWPartition {
    pub hotfix: parser::Hotfix,
    pub mirror: parser::Mirror,
    pub volumes: parser::Volumes,
    pub first_data_block_offset: u64,
}

impl NWPartition {
    pub fn new(file: &mut std::fs::File, start_offset: u64) -> Result<NWPartition, NetWareError> {
        let hotfix_offset = start_offset + HOTFIX_OFFSET;
        let hotfix = parser::Hotfix::new(file, hotfix_offset)?;
        let mirror_offset = hotfix_offset + SECTOR_SIZE;
        let mirror = parser::Mirror::new(file, mirror_offset)?;
        let volume_offset = hotfix_offset + (hotfix.redir_area_sectors as u64 * SECTOR_SIZE);
        let volumes = parser::Volumes::new(file, volume_offset)?;
        let first_data_block_offset = volume_offset + VOLUME_SIZE;
        Ok(NWPartition{ hotfix, mirror, volumes, first_data_block_offset })
    }
}

