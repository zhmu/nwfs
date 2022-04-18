/*-
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * Copyright (c) 2022 Rink Springer <rink@rink.nu>
 * For conditions of distribution and use, see LICENSE file
 */
use packed_struct::prelude::*;
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::fs::File;
use std::env;
use byteorder::{LittleEndian, BigEndian, ReadBytesExt};

const SYSTEM_ID_NETWARE: u8 = 0x65;
const SECTOR_SIZE: usize = 512;
const HOTFIX_OFFSET: usize = 0x4000;

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

const RIGHT_READ: u16 = 0x1;
const RIGHT_WRITE: u16 = 0x2;
const RIGHT_CREATE: u16 = 0x8;
const RIGHT_ERASE: u16 = 0x10;
const RIGHT_ACCESS_CONTROL: u16 = 0x20;
const RIGHT_FILESCAN: u16 = 0x40;
const RIGHT_MODIFY: u16 = 0x80;
const RIGHT_SUPERVISOR: u16 = 0x100;

#[derive(PackedStruct)]
#[derive(Debug)]
#[packed_struct(endian="lsb")]
pub struct PartitionEntry {
    pub boot_flag: u8,
    pub start_head: u8,
    pub start_sect_hi_cyl: u8, // bits 0-5: sector, 6-7: upper cylinder
    pub start_lo_cyl: u8,      // bits 0-5: sector, 6-7: upper cylinder
    pub system_id: u8,
    pub end_head: u8,
    pub end_sect_lo_cyl: u8,   // bits 0-5: sector, 6-7: upper cylinder
    pub end_lo_cyl: u8,        // bits 0-5: sector, 6-7: upper cylinder
    pub lba_start: u32,
    pub lba_size: u32,
}

fn find_netware_partition(mbr: &[u8]) -> Option<PartitionEntry> {
    for partition in 0..4 {
        let offset = 446 + partition * 16;
        let p = PartitionEntry::unpack_from_slice(&mbr[offset..offset + 16]).unwrap();
        if p.system_id != SYSTEM_ID_NETWARE { continue; }
        return Some(p)
    }
    return None
}

fn terminate_string(s: &[u8]) -> String {
    let mut s = String::from_utf8_lossy(&s).to_string();
    if let Some(n) = s.find(char::from(0)) {
        s.truncate(n);
    }
    s
}

fn make_string(s: &[u8], length: u8) -> String {
    let mut s = String::from_utf8_lossy(&s).to_string();
    s.truncate(length.into());
    s
}

fn format_timestamp(ts: u32) -> String {
    if ts == 0 {
        return "<0>".to_string();
    }
    let date_part = ts >> 16;
    let time_part = ts & 0xffff;

    let hour = time_part >> 11;
    let min = (time_part >> 5) & 63;
    let sec = (time_part & 0x1f) * 2;

    let day = date_part & 0x1f;
    let month = (date_part >> 5) & 0xf;
    let year = (date_part >> 9) + 1980;
    format!("{:02}-{:02}-{:04} {:02}:{:02}:{:02}", day, month, year, hour, min, sec)
}

fn format_attr(attr: u32) -> String {
    let mut s = String::new();
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
    s
}

fn format_rights(rights: u16) -> String {
    let mut s = String::new();
    if (rights & RIGHT_SUPERVISOR) != 0 { s += "S" } else { s += " " };
    if (rights & RIGHT_READ) != 0 { s += "R" } else { s += " " };
    if (rights & RIGHT_WRITE) != 0 { s += "W" } else { s += " " };
    if (rights & RIGHT_CREATE) != 0 { s += "C" } else { s += " " };
    if (rights & RIGHT_ERASE) != 0 { s += "E" } else { s += " " };
    if (rights & RIGHT_MODIFY) != 0 { s += "M" } else { s += " " };
    if (rights & RIGHT_FILESCAN) != 0 { s += "F" } else { s += " " };
    if (rights & RIGHT_ACCESS_CONTROL) != 0 { s += "A" } else { s += " " };
    s
}

fn decode_acl<T: Read + ReadBytesExt>(rdr: &mut T, num: usize) -> Result<String, std::io::Error> {
    let mut s = String::new();
    let mut trustees = vec![ 0u32; num ];
    for n in 0..num {
        trustees[n] = rdr.read_u32::<BigEndian>()?;
    }
    let mut rights = vec![ 0u16; num ];
    for n in 0..num {
        rights[n] = rdr.read_u16::<LittleEndian>()?;
    }
    for n in 0..num {
        let trustee = trustees[n];
        if trustee == 0 { continue; }
        s += format!("object {:08x} rights [{}]", trustee, format_rights(rights[n])).as_str();
    }
    Ok(s)
}

fn get_fat_entry(f: &mut std::fs::File, first_block_offset: usize, entry_num: u32) -> Result<(u32, u32), std::io::Error> {
    let block_offset = first_block_offset as u64 + ((entry_num as u64) * 8);
    f.seek(SeekFrom::Start(block_offset))?;
    let mut data = [ 0u8; 8 ];
    f.read(&mut data)?;
    let mut rdr = Cursor::new(&data);
    let a = rdr.read_u32::<LittleEndian>()?;
    let b = rdr.read_u32::<LittleEndian>()?;
    Ok((a, b))
}

fn dump_fat_chain(f: &mut std::fs::File, first_block_offset: usize, entry_num: u32) -> Result<(), std::io::Error> {
    println!("dump_fat_chain: entry {} ->", entry_num);
    let mut current_entry = entry_num;
    while current_entry != 0xffffffff {
        let entry = get_fat_entry(f, first_block_offset, current_entry)?;
        println!("  {}/{} ", entry.0, entry.1);
        current_entry = entry.1;
    }
    Ok(())
}

fn dump_dir_block(block: &[u8]) -> Result<(), std::io::Error> {
    let mut rdr = Cursor::new(&block);
    for _ in 0..(block.len() / 128) {
        let offs = rdr.position();
        let parent_dir_id = rdr.read_u32::<LittleEndian>()?;
        if parent_dir_id == 0xfffffffe /* grant list */ {
            let v2 = rdr.read_u32::<LittleEndian>()?;
            let v3 = rdr.read_u32::<LittleEndian>()?;
            let v4 = rdr.read_u32::<LittleEndian>()?;
            let v5 = rdr.read_u32::<LittleEndian>()?;
            let v6 = rdr.read_u32::<LittleEndian>()?;
            println!("<more trustees>: v2 {:x} v3 {:x} v4 {:x} v5 {:x} v6 {:x}", v2, v3, v4, v5, v6);
            let s = decode_acl(&mut rdr, 16)?;
            println!("  acl: {}", s);
            let v7 = rdr.read_u32::<LittleEndian>()?;
            let v8 = rdr.read_u32::<LittleEndian>()?;
            println!("  v7 {:x} v8 {:x}", v7, v8);
        } else if parent_dir_id == 0xfffffffd /* volume information */ {
            let v2 = rdr.read_u32::<LittleEndian>()?;
            let v3 = rdr.read_u32::<LittleEndian>()?;
            let v4 = rdr.read_u32::<LittleEndian>()?;
            let v5 = rdr.read_u32::<LittleEndian>()?;
            let v6 = rdr.read_u32::<LittleEndian>()?;
            let create_time = rdr.read_u32::<LittleEndian>()?;
            let owner_id = rdr.read_u32::<LittleEndian>()?;
            println!("<volume info>: v2 {:x} v3 {:x} v4 {:x} v5 {:x} v6 {:x} create_time {} owner_id {:x}", v2, v3, v4, v5, v6, format_timestamp(create_time), owner_id);

            let v7 = rdr.read_u32::<LittleEndian>()?;
            let v8 = rdr.read_u32::<LittleEndian>()?;
            let modify_time = rdr.read_u32::<LittleEndian>()?;
            let v10 = rdr.read_u32::<LittleEndian>()?;
            println!("  v7 {:x} v8 {:x} modify_time {} v10 {:x}", v7, v8, format_timestamp(modify_time), v10);

            let s = decode_acl(&mut rdr, 8)?;
            println!("  acl: {}", s);

            let unk1 = rdr.read_u32::<LittleEndian>()?;
            let unk2 = rdr.read_u32::<LittleEndian>()?;
            let unk3 = rdr.read_u32::<LittleEndian>()?;
            let unk4 = rdr.read_u32::<LittleEndian>()?;
            let unk5 = rdr.read_u32::<LittleEndian>()?;
            let unk6 = rdr.read_u32::<LittleEndian>()?;
            let unk7 = rdr.read_u32::<LittleEndian>()?;
            let unk8 = rdr.read_u32::<LittleEndian>()?;
            println!("  unk1 {:x} unk2 {:x} unk3 {:x} unk4 {:x} unk5 {:x} unk6 {:x} unk7 {:x} unk8 {:x}", unk1, unk2, unk3, unk4, unk5, unk6, unk7, unk8);
        } else if parent_dir_id == 0xffffffff {
            let mut num_nonzero = 0;
            let mut s = String::new();
            for _ in 0..31 {
                let v = rdr.read_u32::<LittleEndian>()?;
                if v != 0 { num_nonzero += 1; }
                s += format!(" {:x}", v).as_str();
            }
            if num_nonzero > 0 {
                println!("<unused>;{}", s);
            }
        } else {
            let attr = rdr.read_u32::<LittleEndian>()?;
            if (attr & ATTR_DIRECTORY) != 0 {
                // Directory
                let v3 = rdr.read_u16::<LittleEndian>()?;
                let v4 = rdr.read_u8()?;
                let name_len = rdr.read_u8()?;
                let mut fname = [ 0u8; 12 ];
                rdr.read(&mut fname)?;
                let fname = make_string(&fname, name_len);
                let create_time = rdr.read_u32::<LittleEndian>()?;
                let owner_id = rdr.read_u32::<BigEndian>()?;

                println!("{}: <dir@{:x}> parent_dir_id {:x} attr [{}] ({:x}) v3 {:x} v4 {:x} create_time {} owner_id {:x}", fname, offs, parent_dir_id, format_attr(attr), attr, v3, v4, format_timestamp(create_time), owner_id);
                // v3 is unknown at this point
                let v1 = rdr.read_u32::<LittleEndian>()?;
                let v2 = rdr.read_u32::<LittleEndian>()?;
                let modify_time = rdr.read_u32::<LittleEndian>()?;
                let v4 = rdr.read_u32::<LittleEndian>()?;
                println!("  v1 {:x} v2 {:x} modify_time {} v4 {:x}", v1, v2, format_timestamp(modify_time), v4);

                let s = decode_acl(&mut rdr, 8)?;
                println!("  acl: {}", s);

                let v1 = rdr.read_u16::<LittleEndian>()?;
                let v2 = rdr.read_u16::<LittleEndian>()?;
                let inherited_rights_mask = rdr.read_u16::<LittleEndian>()?;
                let subdir_index = rdr.read_u32::<LittleEndian>()?;
                println!("  v1 {} v2 {} inherited_rights_mask {:x} subdir_index {}", v1, v2, inherited_rights_mask, subdir_index);

                let unk1 = rdr.read_u16::<LittleEndian>()?;
                let unk2 = rdr.read_u16::<LittleEndian>()?;
                let unk3 = rdr.read_u16::<LittleEndian>()?;
                let unk4 = rdr.read_u16::<LittleEndian>()?;
                let unk5 = rdr.read_u16::<LittleEndian>()?;
                let unk6 = rdr.read_u16::<LittleEndian>()?;
                let unk7 = rdr.read_u16::<LittleEndian>()?;
                println!("  unk1 {:x} unk2 {:x} unk3 {:x} unk4 {:x} unk5 {:x} unk6 {:x} unk7 {:x}",
                    unk1, unk2, unk3, unk4, unk5, unk6, unk7);
                let directory_id = rdr.read_u32::<LittleEndian>()?;
                println!("  directory_id {:x}", directory_id);
                let unk8 = rdr.read_u16::<LittleEndian>()?;
                let unk9 = rdr.read_u16::<LittleEndian>()?;
                println!("  unk8 {:x} unk9 {:x}", unk8, unk9);
            } else {
                // File
                let v3 = rdr.read_u16::<LittleEndian>()?; // unknown
                let v4 = rdr.read_u8()?;
                let name_len = rdr.read_u8()?;
                let mut fname = [ 0u8; 12 ];
                rdr.read(&mut fname)?;
                let fname = make_string(&fname, name_len);
                let create_time = rdr.read_u32::<LittleEndian>()?;

                let owner_id = rdr.read_u32::<BigEndian>()?;

                println!("{}: <file@{:x}> parent_dir_id {:x} attr [{}] ({:x}) v3 {:x} v4 {:x} create_time {} owner_id {:x}", fname, offs, parent_dir_id, format_attr(attr), attr, v3, v4, format_timestamp(create_time), owner_id);
                let u2 = rdr.read_u32::<LittleEndian>()?;
                let u3 = rdr.read_u32::<LittleEndian>()?;
                let modify_time = rdr.read_u32::<LittleEndian>()?;
                let modifier_id = rdr.read_u32::<BigEndian>()?;
                let length = rdr.read_u32::<LittleEndian>()?;
                let block_nr = rdr.read_u32::<LittleEndian>()?;
                let u4 = rdr.read_u32::<LittleEndian>()?;

                println!("  u2 {:x} u3 {:x} modify_time {} modifier_id {:x} length {} block_nr {} u4 {:x}", u2, u3, format_timestamp(modify_time), modifier_id, length, block_nr, u4);

                let s = decode_acl(&mut rdr, 6)?;
                println!("  acl: {}", s);
                let unk1 = rdr.read_u32::<LittleEndian>()?;
                let unk2 = rdr.read_u32::<LittleEndian>()?;
                println!("  unk1 {:x} unk2 {:x}", unk1, unk2);
                let delete_time = rdr.read_u32::<LittleEndian>()?;
                let delete_id = rdr.read_u32::<BigEndian>()?;
                println!("  delete_time {} delete_id {:x}", format_timestamp(delete_time), delete_id);
                let v1 = rdr.read_u32::<LittleEndian>()?;
                let v2 = rdr.read_u32::<LittleEndian>()?;
                let file_entry = rdr.read_u32::<LittleEndian>()?;
                let v4 = rdr.read_u32::<LittleEndian>()?;
                println!("  v1 {} v2 {} file_entry {} v4 {}", v1, v2, file_entry, v4);
            }
        }
        if rdr.position() != offs + 0x80 {
            panic!("consumed {} bytes", rdr.position() - offs);
        }
    }
    Ok(())
}

fn main() -> Result<(), std::io::Error> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        panic!("usage: {} file.img", args[0]);
    }

    let path = &args[1];
    let mut f = File::open(path)?;
    let mut mbr = [ 0u8; SECTOR_SIZE ];
    f.read(&mut mbr)?;

    let p = find_netware_partition(&mbr);
    if p.is_none() {
        panic!("cannot find a NetWare partition");
    }
    let p = p.unwrap();
    println!("partition offset {:x}", p.lba_start as usize * SECTOR_SIZE);
    let hotfix_offset = (p.lba_start as usize) * SECTOR_SIZE + HOTFIX_OFFSET;
    println!("hotfix area offset: {:x}", hotfix_offset);

    let mut block = [ 0u8; SECTOR_SIZE ];
    f.seek(SeekFrom::Start(hotfix_offset as u64))?;
    f.read(&mut block)?;

    println!(">> Dumping hotfix data");
    let mut rdr = Cursor::new(&block);
    let mut id = [ 0u8; 8 ];
    rdr.read(&mut id )?;
    let id = terminate_string(&id);
    let v_id = rdr.read_u32::<LittleEndian>()?;
    let v2 = rdr.read_u16::<LittleEndian>()?;
    let v3 = rdr.read_u16::<LittleEndian>()?;
    let v4 = rdr.read_u16::<LittleEndian>()?;
    let v5 = rdr.read_u16::<LittleEndian>()?;
    let data_area_sectors = rdr.read_u32::<LittleEndian>()?;
    let redir_area_sectors = rdr.read_u32::<LittleEndian>()?;
    println!("id {} v_id {:x} v2 {:x} v3 {:x} v4 {:x} v5 {:x}", id, v_id, v2, v3, v4, v5);
    println!("  data_area_sectors {} redir_area_sectors {}", data_area_sectors, redir_area_sectors);
    let unk1 = rdr.read_u32::<LittleEndian>()?;
    let unk2 = rdr.read_u32::<LittleEndian>()?;
    let unk3 = rdr.read_u32::<LittleEndian>()?;
    let unk4 = rdr.read_u32::<LittleEndian>()?;
    let unk5 = rdr.read_u32::<LittleEndian>()?;
    let unk6 = rdr.read_u32::<LittleEndian>()?;
    let unk7 = rdr.read_u32::<LittleEndian>()?;
    let unk8 = rdr.read_u32::<LittleEndian>()?;
    println!("  unk1 {:x} unk2 {:x} unk3 {:x} unk4 {:x} unk5 {:x} unk6 {:x} unk7 {:x} unk8 {:x}",
        unk1, unk2, unk3, unk4, unk5, unk6, unk7, unk8);

    let mirror_offset = hotfix_offset + SECTOR_SIZE;
    f.seek(SeekFrom::Start(mirror_offset as u64))?;
    f.read(&mut block)?;

    println!(">> Dumping mirror data");
    let mut rdr = Cursor::new(&block);
    let mut id = [ 0u8; 8 ];
    rdr.read(&mut id )?;
    let id = terminate_string(&id);
    println!("  id {}", id);
    let create_time = rdr.read_u32::<LittleEndian>()?;
    println!("  create_time {}", format_timestamp(create_time));
    // unk1 seems to be a flags fields, 90000 usually or b0000 if we have a mirror
    let unk1 = rdr.read_u32::<LittleEndian>()?;
    let unk2 = rdr.read_u32::<LittleEndian>()?;
    let unk3 = rdr.read_u32::<LittleEndian>()?;
    let unk4 = rdr.read_u32::<LittleEndian>()?;
    println!("  unk1 {:x} unk2 {:x} unk3 {:x} unk4 {:x}",
        unk1, unk2, unk3, unk4);
    let unk5 = rdr.read_u32::<LittleEndian>()?;
    // There can likely be more here, if more mirrors are defined
    let hotfix_v_id1 = rdr.read_u32::<LittleEndian>()?;
    let hotfix_v_id2 = rdr.read_u32::<LittleEndian>()?;
    println!("  unk5 {:x} hotfix_v_id1 {:x} hotfix_v_id2 {:x}", unk5, hotfix_v_id1, hotfix_v_id2);

    println!(">> dumping ??? data");
    let wtf_offset = hotfix_offset + 4 * 16384; // skip hotfix/mirror data
    println!("wtf_offset {:x}", wtf_offset);

    // 32KB of stuff, repeats 4 times. No idea what it is
/*
    let mut block= [ 0u8; 32768 ];
    f.seek(SeekFrom::Start(wtf_offset as u64))?;
    f.read(&mut block)?;
    let mut rdr = Cursor::new(&block);
    for n in 0..(32768 / 4) {
        let v = rdr.read_u32::<LittleEndian>()?;
        println!("{}: {:x}", n, v);
    }
*/

    let volume_offset = hotfix_offset + (redir_area_sectors as usize * SECTOR_SIZE);
    println!("volume data offset {:x}", volume_offset);

    let mut block = [ 0u8; SECTOR_SIZE ];
    f.seek(SeekFrom::Start(volume_offset as u64))?;
    f.read(&mut block)?;

    println!(">> VOLUME DATA");
    let mut rdr = Cursor::new(&block);
    // 16 bytes = "NetWare Volumes\0"
    rdr.seek(SeekFrom::Current(16))?;
    let num_volumes = rdr.read_u32::<LittleEndian>()?;
    rdr.seek(SeekFrom::Current(12))?; // ???
    println!("number of volumes: {}", num_volumes);
    let mut volume_block_size: Option<usize> = None;
    let mut volume_root_block: Option<u32> = None;
    for _ in 0..num_volumes {
        // 60 bytes per volume
        let name_len = rdr.read_u8()?;
        let mut vol_name = [ 0u8; 19 ];
        rdr.read(&mut vol_name)?;
        let vol_name = make_string(&vol_name, name_len);

        // v1 seems to be some kind of ID...
        let v1 = rdr.read_u16::<LittleEndian>()?;
        let segment_num = rdr.read_u16::<LittleEndian>()?;
        // first_sector is always 160 for the first segment and then gets
        // added more
        let first_sector = rdr.read_u32::<LittleEndian>()?;
        let num_sectors = rdr.read_u32::<LittleEndian>()?;
        let total_blocks = rdr.read_u32::<LittleEndian>()?;
        let first_segment_block = rdr.read_u32::<LittleEndian>()?;
        let v6 = rdr.read_u32::<LittleEndian>()?;
        let block_size = rdr.read_u32::<LittleEndian>()?;
        let rootdir_block_nr = rdr.read_u32::<LittleEndian>()?;
        let rootdir_copy_block_nr = rdr.read_u32::<LittleEndian>()?;
        let v10 = rdr.read_u32::<LittleEndian>()?;

        let block_size = (256 / block_size) * 1024;

        println!("  volume {}", vol_name);
        println!("  v1 {:x} ({}) segment_num {} first_sector {} num_sectors {} total_blocks {} first_segment_block {} v6 {:x} ({}) block_size {}", v1, v1, segment_num, first_sector, num_sectors, total_blocks, first_segment_block, v6, v6, block_size);
        println!("  rootdir_block_nr {} rootdir_copy_block_nr {} v10 {:x} ({})", rootdir_block_nr, rootdir_copy_block_nr, v10, v10);
        volume_root_block = Some(rootdir_block_nr);
        volume_block_size = Some(block_size as usize);
    }
    if volume_root_block.is_none() {
        panic!("No NetWare volumes found in parition");
    }
    let volume_root_block = volume_root_block.unwrap();
    let volume_block_size = volume_block_size.unwrap();

    let first_block_offset = volume_offset + 4 * 16384; // 4 entries of 16KB each
    let second_block_offset = first_block_offset + 256 * 1024;
    println!("first block offset {:x}", first_block_offset);
    println!("second block offset {:x}", second_block_offset);

    // vol1
    let rootdir_offset = first_block_offset + (volume_root_block as usize * volume_block_size);
    f.seek(SeekFrom::Start(rootdir_offset as u64))?;

    let mut block = vec![ 0u8; volume_block_size ];
    f.read(&mut block)?;

    println!("dir offset {:x}", rootdir_offset);
    println!(">> DIR LISTING");
    dump_dir_block(&block)?;

    dump_fat_chain(&mut f, first_block_offset, volume_root_block)?;

    let mut current_entry = volume_root_block;
    while current_entry != 0xffffffff {
        let entry = get_fat_entry(&mut f, first_block_offset, current_entry)?;
        if entry.1 == 0xffffffff { break; }
        println!(">> root dir entry {}/{} ", entry.0, entry.1);

        let mut block = vec![ 0u8; volume_block_size ];
        let offset = first_block_offset + (entry.1 as usize * volume_block_size);
        f.seek(SeekFrom::Start(offset as u64))?;
        f.read(&mut block)?;
        dump_dir_block(&block)?;
        current_entry = entry.1;
    }

    dump_fat_chain(&mut f, first_block_offset, 16)?;

/*
    let mut temp_f = File::create("/tmp/out.bin")?;
    let mut current_entry = 16;
    while current_entry != 0xffffffff {
        let mut f_d;
        let mut block_delta;
        if current_entry < 8004 {
            f_d = File::open("nw312-data.img")?;
            block_delta = 0;
        } else {
            f_d = File::open("nw312-data-2.img")?;
            block_delta = 8004;
        }

        let data_offset = first_block_offset + ((current_entry - block_delta) as usize* volume_block_size);
        f_d.seek(SeekFrom::Start(data_offset as u64))?;
        let mut block = vec![ 0u8; volume_block_size ];
        f_d.read(&mut block)?;

        println!("data_offset {:x}", data_offset);
        temp_f.write(&block)?;

        let next_entry = get_fat_entry(&mut f, first_block_offset, current_entry)?;
        if next_entry.1 == 0xffffffff { break; }
        current_entry = next_entry.1
    }
    temp_f.set_len(99160)?;
*/
    Ok(())
}
