/*-
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * Copyright (c) 2022, 2024 Rink Springer <rink@rink.nu>
 * For conditions of distribution and use, see LICENSE file
 */
use anyhow::{anyhow, Result};
use crate::util;
use crate::nwfs386::types;

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
        match util::find_partition(&mut file)? {
            Some((util::PartitionType::NetWare386, start_lba)) => {
                let start_offset = start_lba * types::SECTOR_SIZE;
                self.images.push(Image{ file, start_offset });
                Ok(())
            }
            _ => Err(anyhow!("no NetWare 386 partition found")),
        }
    }
}

