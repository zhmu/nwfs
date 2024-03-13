/*-
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * Copyright (c) 2022, 2024 Rink Springer <rink@rink.nu>
 * For conditions of distribution and use, see LICENSE file
 */
use std::io::Write;
use std::io::{self, BufRead};
use std::fs::File;
use anyhow::Result;

pub trait ShellImpl {
    fn get_volume_name(&self) -> String;
    fn get_root_directory_id(&self) -> u32;
    fn dir(&self, current_dir_id: u32);
    fn lookup_directory(&self, pieces: &[String]) -> Option<Vec<u32>>;
    fn retrieve_file_content(&mut self, current_dir_id: u32, fname: &str) -> Result<Vec<u8>>;
    fn handle_command(&mut self, current_dir_id: u32, fields: &Vec<&str>) -> bool;
}

pub fn run(shell: &mut impl ShellImpl) -> Result<()> {
    let mut current_directory_id = vec! [ shell.get_root_directory_id() ];
    let mut current_directory: Vec<String> = vec![ "".to_string() ];

    let stdin = io::stdin();
    let mut input = stdin.lock().lines();
    loop {
        print!("{}:/{}> ", shell.get_volume_name(), current_directory[1..].join("/"));
        io::stdout().flush()?;
        let line = input.next();
        if line.is_none() { break; }
        let line = line.unwrap();
        if line.is_err() { break; }
        let line = line.unwrap();

        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.is_empty() { continue; }
        let command = *fields.first().unwrap();

        let directory_id = *current_directory_id.last().unwrap();
        if shell.handle_command(directory_id, &fields) {
            // Command accepted - nothing to do
        } else if command == "exit" || command == "quit" {
            break;
        } else if command == "cd" || command == "chdir" {
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
                if let Some(directory_ids) = shell.lookup_directory(&new_directory) {
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
        } else if command == "dir" || command == "ls" {
            shell.dir(directory_id);
        } else if command == "get" {
            let fname = fields[1];
            let data = shell.retrieve_file_content(directory_id, fname)?;

            File::create(fname)
                .and_then(|mut f| f.write(&data))
                .unwrap_or_else(|e| { println!("error: {}", e); 0 } );
        } else if command == "cat" || command == "type" {
            let fname = fields[1];
            let data = shell.retrieve_file_content(directory_id, fname)?;

            if let Ok(s) = std::str::from_utf8(&data) {
                println!("{}", s);
            } else {
                println!("{:x?}", data);
            }
        } else {
            println!("unrecognized command");
        }
    }
    Ok(())
}
