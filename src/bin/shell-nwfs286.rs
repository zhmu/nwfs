use anyhow::{anyhow, Result};

use clap::Parser;

use std::io::{Read, Seek, SeekFrom};
use std::fs::File;
use std::io;
use std::io::{BufRead, Write};

use nwfs::nwfs286::{parser, types};
use nwfs::util;

/// Transfer data from NetWare 286 partitions
#[derive(Parser)]
struct Cli {
    /// Input file
    in_file: String,
}

fn find_file_entry<'a>(entries: &'a [parser::DirectoryEntry], current_directory_id: u16, fname: &str) -> Option<&'a parser::DirectoryEntry> {
    let items: Vec<_> = entries
        .iter()
        .filter(|de|
            de.parent_dir == current_directory_id &&
            de.fname == fname
        ).collect();
    if items.is_empty() {
        println!("File not found");
        return None;
    }
    if items.len() != 1 {
        println!("Multiple items matching");
        return None;
    }
    Some(items.first().unwrap())
}

fn copy_file_data(de: &parser::DirectoryEntry, fat: &[parser::FatEntry], in_f: &mut File, out_f: &mut File) -> Result<u32> {
    let mut blk = de.block_nr;
    let mut bytes_left = de.size as u64;
    while bytes_left > 0 {
        let chunk_size = std::cmp::min(types::BLOCK_SIZE, bytes_left);

        in_f.seek(SeekFrom::Start(parser::block_to_offset(blk)))?;
        let mut data = [ 0u8; types::BLOCK_SIZE as usize ];
        in_f.read_exact(&mut data)?;
        out_f.write(&data[0..chunk_size as usize])?;

        blk = fat[blk as usize].block;
        bytes_left -= chunk_size;
    }
    Ok(de.size)
}

fn print_direntry_header() {
    println!("<type> ID Name            Attr Size    Last Modified");
}

fn print_direntry(de: &parser::DirectoryEntry) {
    println!("???   {:3x} {:14} {:04x} {:8} {} {}",
      de.entry_id, de.fname, de.attr, de.size, de.last_modified_date, de.last_modified_time);
}

fn main() -> Result<()> {
    let args = Cli::parse();
    let mut f = File::open(args.in_file)?;

    // Note: for now, this will only work on dedicated installations (partition
    // must cover the entire disk)
    let p = util::find_partition(&mut f, util::PartitionType::NetWare286)?;
    if p.is_none() { return Err(anyhow!("no NetWare 286 partition found")); }
    if p.unwrap() != 1 { return Err(anyhow!("NetWare 286 partition doesn't cover the entire disk")); }
    let vi = parser::VolumeInfo::new(&mut f)?;

    let entries = parser::read_directory_entries(&mut f, &vi.directory_entries_1_blocks)?;
    let fat = parser::read_fat_table(&mut f, &vi.fat_blocks)?;

    let vol_name = vi.name;
    let mut current_directory_id = vec! [ 1 ];
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
            for de in entries.iter().filter(|e| e.parent_dir == *current_directory_id.last().unwrap()) {
                print_direntry(&de);
            }
        } else if command == "cd" {
            if fields.len() != 2 {
                println!("usage: cd directory");
                continue;
            }
            let dest = fields[1];
            if dest != ".." {
                if let Some(new_id) = find_file_entry(&entries, *current_directory_id.last().unwrap(), dest) {
                    current_directory.push(dest.to_string());
                    current_directory_id.push(new_id.entry_id);
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
            if let Some(de) = entries.iter().filter(|e| e.parent_dir == *current_directory_id.last().unwrap() && e.fname == fname).next() {
                if let Ok(mut out_f) = File::create(fname) {
                    match copy_file_data(de, &fat, &mut f, &mut out_f) {
                        Ok(size) => {
                            println!("{} bytes copied", size);
                        },
                        Err(e) => {
                            println!("unable to copy file data: {:?}", e);
                        }
                    }
                } else {
                    println!("couldn't create {}", fname);
                }
            } else {
                println!("file not found");
            }
        }
    }
    Ok(())
}
