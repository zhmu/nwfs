/*-
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * Copyright (c) 2024 Rink Springer <rink@rink.nu>
 * For conditions of distribution and use, see LICENSE file
 */
use anyhow::{anyhow, Result};

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

use crate::nwfs286::{parser, types};
use crate::shell_cli;

const NWFS286_ROOT_ID: u32 = 1;

fn match_parent_dir_id(de: &parser::DirEntry, parent_dir_id: u32) -> bool {
    parent_dir_id == match de {
        parser::DirEntry::File(v) => { v.parent_dir as u32 },
        parser::DirEntry::Directory(d) => { d.parent_dir as u32 },
    }
}

fn match_dir_entry_name(de: &parser::DirEntry, name: &str) -> bool {
    match de {
        parser::DirEntry::File(f) => f.name.eq_ignore_ascii_case(name),
        parser::DirEntry::Directory(d) => d.name.eq_ignore_ascii_case(name)
    }
}

fn find_directory_entry<'a>(entries: &'a [parser::DirEntry], current_directory_id: u32, fname: &str) -> Result<&'a parser::DirEntry> {
    let items: Vec<_> = entries
        .iter()
        .filter(|de|
            match_parent_dir_id(de, current_directory_id) &&
            match_dir_entry_name(de, fname)
        ).collect();
    if items.is_empty() {
        return Err(anyhow!("File not found"));
    }
    if items.len() != 1 {
        return Err(anyhow!("Multiple items matching"));
    }
    Ok(items.first().unwrap())
}

pub struct Nwfs286Shell {
    vol: parser::VolumeInfo,
    entries: Vec<parser::DirEntry>,
    fat: Vec<parser::FatEntry>,
    file: File
}

impl Nwfs286Shell {
    pub fn new(vol: parser::VolumeInfo, entries: Vec<parser::DirEntry>, fat: Vec<parser::FatEntry>, file: File) -> Self {
        Nwfs286Shell{ vol, entries, fat, file }
    }
}

impl shell_cli::ShellImpl for Nwfs286Shell {
    fn get_volume_name(&self) -> String {
        self.vol.name.clone()
    }

    fn get_root_directory_id(&self) -> u32 {
        NWFS286_ROOT_ID
    }

    fn dir(&self, current_dir_id: u32) {
        println!("<type> ID Name            Attr Size    Last Modified");
        for de in self.entries.iter().filter(|e| match_parent_dir_id(e, current_dir_id)) {
            match de {
                parser::DirEntry::File(f) => {
                    println!("<file> {:3x} {:14} {:04x} {:8} {} {}",
                      f.entry_id, f.name, f.attr, f.size, f.last_modified_date, f.last_modified_time);
                },
                parser::DirEntry::Directory(d) => {
                    println!("<dir>  {:3x} {:14} {:04x} {} {} {:x} {:x} {:x} {:x} {:x}",
                      d.entry_id, d.name, d.attr,
                      d.last_modified_date, d.last_modified_time,
                      d.unk22, d.unk24, d.unk26, d.unk28, d.unk30);
                }
            }
        }
    }

    fn lookup_directory(&self, pieces: &[String]) -> Option<Vec<u32>> {
        // Remove initial /
        let mut iter = pieces.iter();
        iter.next().unwrap(); // skip /

        let mut directory_ids: Vec<u32> = vec![ NWFS286_ROOT_ID ];
        while let Some(piece) = iter.next() {
            if piece.is_empty() { continue; }
            match find_directory_entry(&self.entries, *directory_ids.last().unwrap(), piece).ok()? {
                parser::DirEntry::Directory(d) => {
                    directory_ids.push(d.entry_id as u32);
                },
                _ => { return None; }
            }
        }
        Some(directory_ids)
    }

    fn retrieve_file_content(&mut self, current_dir_id: u32, fname: &str) -> Result<Vec<u8>> {
        match find_directory_entry(&self.entries, current_dir_id, fname)? {
            parser::DirEntry::File(f) => {
                let mut bytes_left = f.size as usize;
                let mut data = vec![ 0u8; bytes_left ];

                let mut blk = f.block_nr;
                let mut current_offset: usize = 0;
                while bytes_left > 0 {
                    let chunk_size = std::cmp::min(types::BLOCK_SIZE as usize, bytes_left);

                    self.file.seek(SeekFrom::Start(parser::block_to_offset(blk)))?;
                    self.file.read_exact(&mut data[current_offset..current_offset + chunk_size])?;

                    blk = self.fat[blk as usize].block;
                    current_offset += chunk_size;
                    bytes_left -= chunk_size;
                }
                Ok(data)
            },
            _ => Err(anyhow!("not a file"))
        }
    }

    fn handle_command(&mut self, _current_dir_id: u32, _fields: &Vec<&str>) -> bool {
        false
    }
}
