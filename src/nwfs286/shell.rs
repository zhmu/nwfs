use anyhow::{anyhow, Result};

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

use crate::nwfs286::{parser, types};
use crate::shell_cli;

const NWFS286_ROOT_ID: u32 = 1;

fn find_directory_entry<'a>(entries: &'a [parser::DirectoryEntry], current_directory_id: u32, fname: &str) -> Result<&'a parser::DirectoryEntry> {
    let items: Vec<_> = entries
        .iter()
        .filter(|de|
            de.parent_dir as u32 == current_directory_id &&
            de.fname.eq_ignore_ascii_case(fname)
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
    entries: Vec<parser::DirectoryEntry>,
    fat: Vec<parser::FatEntry>,
    file: File
}

impl Nwfs286Shell {
    pub fn new(vol: parser::VolumeInfo, entries: Vec<parser::DirectoryEntry>, fat: Vec<parser::FatEntry>, file: File) -> Self {
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
        for de in self.entries.iter().filter(|e| e.parent_dir as u32 == current_dir_id) {
            println!("???   {:3x} {:14} {:04x} {:8} {} {}",
              de.entry_id, de.fname, de.attr, de.size, de.last_modified_date, de.last_modified_time);
        }
    }

    fn lookup_directory(&self, pieces: &[String]) -> Option<Vec<u32>> {
        // Remove initial /
        let mut iter = pieces.iter();
        iter.next().unwrap(); // skip /

        let mut directory_ids: Vec<u32> = vec![ NWFS286_ROOT_ID ];
        while let Some(piece) = iter.next() {
            if piece.is_empty() { continue; }
            let item = find_directory_entry(&self.entries, *directory_ids.last().unwrap(), piece).ok()?;
            directory_ids.push(item.entry_id as u32);
        }
        Some(directory_ids)
    }

    fn retrieve_file_content(&mut self, current_dir_id: u32, fname: &str) -> Result<Vec<u8>> {
        let de = find_directory_entry(&self.entries, current_dir_id, fname)?;

        let mut bytes_left = de.size as usize;
        let mut data = vec![ 0u8; bytes_left ];

        let mut blk = de.block_nr;
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
    }

    fn handle_command(&mut self, _current_dir_id: u32, _fields: &Vec<&str>) -> bool {
        false
    }
}
