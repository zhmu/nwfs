/*-
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * Copyright (c) 2022, 2024 Rink Springer <rink@rink.nu>
 * For conditions of distribution and use, see LICENSE file
 */
use anyhow::{anyhow, Result};

use std::io::Read;

use crate::nwfs386::{parser, volume};
use crate::shell_cli;

const NWFS386_ROOT_ID: u32 = 0;

fn match_parent_dir_id(de: &parser::DirEntry, parent_dir_id: u32) -> bool {
    parent_dir_id == match de {
        parser::DirEntry::Available(v) => { v.parent_dir_id },
        parser::DirEntry::GrantList(v) => { v.parent_dir_id },
        parser::DirEntry::VolumeInformation(v) => { v.parent_dir_id },
        parser::DirEntry::File(v) => { v.parent_dir_id },
        parser::DirEntry::Directory(v) => { v.parent_dir_id },
    }
}

fn match_dir_entry_name(de: &parser::DirEntry, name: &str) -> bool {
    match de {
        parser::DirEntry::File(v) => { v.name.eq_ignore_ascii_case(name) },
        parser::DirEntry::Directory(v) => { v.name.eq_ignore_ascii_case(name) },
        _ => { false },
    }
}

pub fn is_deleted_dir_entry(de: &parser::DirEntry) -> bool {
    match de {
        parser::DirEntry::File(v) => { v.delete_time.is_valid()  },
        _ => { false },
    }
}

pub struct Nwfs386Shell<'a> {
    vol: &'a mut volume::LogicalVolume
}

impl<'a> Nwfs386Shell<'a> {
    pub fn new(vol: &'a mut volume::LogicalVolume) -> Self {
        Nwfs386Shell{ vol }
    }
}

impl shell_cli::ShellImpl for Nwfs386Shell<'_> {
    fn get_volume_name(&self) -> String {
        self.vol.volumes.first().unwrap().info.name.clone()
    }

    fn get_root_directory_id(&self) -> u32 {
        NWFS386_ROOT_ID
    }

    fn dir(&self, current_dir_id: u32) {
        println!("<type> Name              Size Last Modified       Last Modifier");
        for de in self.vol.directory.iter().filter(|de| match_parent_dir_id(&de, current_dir_id) && !is_deleted_dir_entry(&de)) {
            match de {
                parser::DirEntry::File(f) => {
                    println!(" file  {:14} {:7} {} {:08x}", f.name, f.length, f.modify_time, f.modifier_id);
                    //println!("  created at {} by {:x}", f.create_time, f.owner_id);
                },
                parser::DirEntry::Directory(d) => {
                    println!(" dir   {:14}       - {:19} - ? {}", d.name, d.modify_time, d.inherited_rights_mask);
                },
                parser::DirEntry::Available(_) => {},
                parser::DirEntry::VolumeInformation(_) => {},
                parser::DirEntry::GrantList(_) => {},
            }
        }
    }

    fn lookup_directory(&self, pieces: &[String]) -> Option<Vec<u32>> {
        // Remove initial /
        let mut iter = pieces.iter();
        iter.next().unwrap(); // skip /

        let directory = &self.vol.directory;
        let mut directory_ids: Vec<u32> = vec![ NWFS386_ROOT_ID ];
        while let Some(piece) = iter.next() {
            if piece.is_empty() { continue; }
            let matches: Vec<_> = directory.iter().filter(|de|
                match_parent_dir_id(&de, *directory_ids.last().unwrap()) &&
                match_dir_entry_name(&de, piece) &&
                !is_deleted_dir_entry(&de)
            ).collect();
            if matches.len() != 1 { return None }
            let item = matches.first().unwrap();
            match item {
                parser::DirEntry::Directory(v) => {
                    directory_ids.push(v.directory_id);
                },
                _ => {
                    // Not a directory
                    return None
                }
            }
        }
        Some(directory_ids)
    }

    fn retrieve_file_content(&mut self, current_dir_id: u32, fname: &str) -> Result<Vec<u8>> {
        let block_size = self.vol.volumes.first().unwrap().info.block_size as usize;
        let (block_nr, length) = find_file_entry(&self.vol, current_dir_id, fname)?;

        let mut current_entry = block_nr;
        let mut bytes_left = length as usize;
        let mut data = vec![ 0u8; length as usize ];
        let mut current_offset: usize = 0;
        while current_entry != 0xffffffff {
            let chunk_size = if bytes_left > block_size { block_size } else { bytes_left };
            let file = self.vol.seek_block(current_entry)?;
            file.read_exact(&mut data[current_offset..current_offset + chunk_size])?;

            let entry = self.vol.read_fat_entry(current_entry)?;
            current_entry = entry.1;
            current_offset += chunk_size;
            bytes_left -= chunk_size;
        }
        Ok(data)
    }

    fn handle_command(&mut self, _current_dir_id: u32, _fields: &Vec<&str>) -> bool {
        false
    }
}

fn find_file_entry(vol: &volume::LogicalVolume, current_directory_id: u32, fname: &str) -> Result<(u32, u32)> {
    let items: Vec<_> = vol.directory
        .iter()
        .filter(|de|
            match_parent_dir_id(&de, current_directory_id) &&
            match_dir_entry_name(&de, fname) &&
            !is_deleted_dir_entry(&de)
        ).collect();
    if items.is_empty() {
        return Err(anyhow!("file not found"));
    }
    if items.len() != 1 {
        return Err(anyhow!("multiple items matching"));
    }
    if let parser::DirEntry::File(fe) = items.first().unwrap() {
        return Ok((fe.block_nr, fe.length))
    }
    return Err(anyhow!("not a file"));
}

