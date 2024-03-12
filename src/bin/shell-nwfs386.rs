/*-
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * Copyright (c) 2022, 2024 Rink Springer <rink@rink.nu>
 * For conditions of distribution and use, see LICENSE file
 */
use std::io::{Read, Write};
use std::fs::File;
use std::env;
use std::io::{self, BufRead};
use anyhow::Result;

use nwfs::nwfs386::{parser, image, volume};

pub fn match_parent_dir_id(de: &parser::DirEntry, parent_dir_id: u32) -> bool {
    parent_dir_id == match de {
        parser::DirEntry::Available(v) => { v.parent_dir_id },
        parser::DirEntry::GrantList(v) => { v.parent_dir_id },
        parser::DirEntry::VolumeInformation(v) => { v.parent_dir_id },
        parser::DirEntry::File(v) => { v.parent_dir_id },
        parser::DirEntry::Directory(v) => { v.parent_dir_id },
    }
}

pub fn match_dir_entry_name(de: &parser::DirEntry, name: &str) -> bool {
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

pub fn lookup_directory(directory: &[ parser::DirEntry ], pieces: &[ String ]) -> Option<Vec<u32>> {
    // Remove initial /
    let mut iter = pieces.iter();
    iter.next().unwrap(); // skip /

    let mut directory_ids: Vec<u32> = vec![ 0 ];
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

fn print_direntry_header() {
    println!("<type> Name              Size Last Modified       Last Modifier");
}

fn print_direntry(de: &parser::DirEntry) {
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

fn find_file_entry(vol: &volume::LogicalVolume, current_directory_id: u32, fname: &str) -> Option<(u32, u32)> {
    let items: Vec<_> = vol.directory
        .iter()
        .filter(|de|
            match_parent_dir_id(&de, current_directory_id) &&
            match_dir_entry_name(&de, fname) &&
            !is_deleted_dir_entry(&de)
        ).collect();
    if items.is_empty() {
        println!("File not found");
        return None;
    }
    if items.len() != 1 {
        println!("Multiple items matching");
        return None;
    }
    if let parser::DirEntry::File(fe) = items.first().unwrap() {
        return Some((fe.block_nr, fe.length))
    }
    println!("Not a file");
    None
}

fn copy_file_data(vol: &mut volume::LogicalVolume, f: &mut std::fs::File, block_nr: u32, length: u32) -> Result<usize> {
    let mut current_entry = block_nr;
    let mut bytes_left = length as usize;
    let block_size = vol.volumes.first().unwrap().info.block_size as usize;
    let mut block = vec![ 0u8; block_size ];
    while current_entry != 0xffffffff {
        let chunk_size = if bytes_left > block_size { block_size } else { bytes_left };
        let file = vol.seek_block(current_entry)?;
        file.read(&mut block)?;
        f.write(&block[0..chunk_size])?;

        let entry = vol.read_fat_entry(current_entry)?;
        current_entry = entry.1;
        bytes_left -= chunk_size;
    }
    Ok(length as usize)
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        panic!("usage: {} file.img", args[0]);
    }

    let path = &args[1];
    let f = File::open(path)?;

    let mut image_list = image::ImageList::new();
    image_list.add_image(f)?;

    let mut vol = volume::LogicalVolume::new(&mut image_list, "SYS")?;
    let vol_name = &vol.volumes.first().unwrap().info.name.clone();
    let mut current_directory_id = vec! [ 0 ];
    let mut current_directory: Vec<String> = vec![ "".to_string() ];

    let stdin = io::stdin();
    let mut input = stdin.lock().lines();
    loop {
        print!("{}:/{}> ", vol_name, current_directory[1..].join("/"));
        io::stdout().flush()?;
        let line = input.next();
        if line.is_none() { break; }
        let line = line.unwrap();
        if line.is_err() { break; }
        let line = line.unwrap();

        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.is_empty() { continue; }
        let command = *fields.first().unwrap();

        if command == "dir" || command == "ls" {
            print_direntry_header();
            for de in vol.directory.iter().filter(|de| match_parent_dir_id(&de, *current_directory_id.last().unwrap()) && !is_deleted_dir_entry(&de)) {
                print_direntry(&de);
            }
        } else if command == "cd" {
            if fields.len() != 2 {
                println!("usage: cd directory");
                continue;
            }
            let dest = fields[1];
            if dest != ".." {
                let mut new_directory;
                if dest.starts_with("/") {
                    new_directory = dest.split("/").map(|s| s.to_string()).collect();
                } else {
                    new_directory = current_directory.clone();
                    new_directory.push(dest.to_string());
                }
                if let Some(directory_ids) = lookup_directory(&vol.directory, &new_directory) {
                    current_directory = new_directory;
                    current_directory_id = directory_ids;
                } else {
                    println!("Directory not found");
                }
            } else {
                if current_directory_id.len() > 1 {
                    current_directory.pop();
                    current_directory_id.pop();
                }
            }
        } else if command == "get" {
            if fields.len() != 2 {
                println!("usage: get file");
                continue;
            }
            let fname = fields[1];
            if let Some((block_nr, length)) = find_file_entry(&vol, *current_directory_id.last().unwrap(), fname) {
                if let Ok(mut f) = File::create(fname) {
                    let result = copy_file_data(&mut vol, &mut f, block_nr, length);
                    match result {
                        Ok(size) => {
                            println!("{} bytes copied", size);
                        },
                        Err(e) => {
                            println!("unable to copy file data: {:?}", e);
                        }
                    }
                } else {
                    println!("cannot create {}",  fname);
                }
            }
            continue;
        } else {
            println!("uncreognized command");
        }
    }
    Ok(())
}
