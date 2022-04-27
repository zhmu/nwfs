/*-
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * Copyright (c) 2022 Rink Springer <rink@rink.nu>
 * For conditions of distribution and use, see LICENSE file
 */
use std::io::{Read, Seek, SeekFrom, Write};
use std::fs::File;
use std::env;
use std::io::{self, BufRead};
use byteorder::{LittleEndian, BigEndian, ReadBytesExt};

const SYSTEM_ID_NETWARE: u8 = 0x65;

const DIRID_VOLUME_INFO: u32 = 0xfffffffd;
const DIRID_GRANT_LIST: u32 = 0xfffffffe;
const DIRID_AVAILABLE: u32 = 0xffffffff;

const SECTOR_SIZE: u64 = 512;
const HOTFIX_OFFSET: u64 = 0x4000;
const VOLUME_SIZE: u64 = 4 * 16384;

pub const ATTR_COPY_INHIBIT: u32 = 0x80000;
pub const ATTR_DELETE_INHIBIT: u32 = 0x40000;
pub const ATTR_RENAME_INHIBIT: u32 = 0x20000;
pub const ATTR_PURGE: u32 = 0x10000;
pub const ATTR_TRANSACTIONAL: u32 = 0x1000;
pub const ATTR_SHAREABLE: u32 = 0x80;
pub const ATTR_ARCHIVE: u32 = 0x20;
pub const ATTR_DIRECTORY: u32 = 0x10;
pub const ATTR_SYSTEM: u32 = 0x4;
pub const ATTR_HIDDEN: u32 = 0x2;
pub const ATTR_READONLY: u32 = 0x1;

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

fn find_netware_partition<T: Seek + Read>(f: &mut T) -> Result<Option<u64>, std::io::Error> {
    // Seek to MBR and parse the partition table
    f.seek(SeekFrom::Start(446))?;
    for _ in 0..4 {
        // Skip boot_flag, start CHS
        f.seek(SeekFrom::Current(4))?;
        let system_id = f.read_u8()?;
        // Skip end CHS
        f.seek(SeekFrom::Current(3))?;
        let lba_start = f.read_u32::<LittleEndian>()?;
        // Skip lba length
        f.seek(SeekFrom::Current(4))?;
        if system_id != SYSTEM_ID_NETWARE { continue; }
        return Ok(Some(lba_start.into()))
    }
    Ok(None)
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

fn parse_unknown_u32<T: Read>(f: &mut T, unk: &mut [u32]) -> Result<(), std::io::Error> {
    for n in 0..unk.len() {
        unk[n] = f.read_u32::<LittleEndian>()?;
    }
    Ok(())
}

fn parse_unknown_u16<T: Read>(f: &mut T, unk: &mut [u16]) -> Result<(), std::io::Error> {
    for n in 0..unk.len() {
        unk[n] = f.read_u16::<LittleEndian>()?;
    }
    Ok(())
}

fn parse_unknown_u8<T: Read>(f: &mut T, unk: &mut [u8]) -> Result<(), std::io::Error> {
    for n in 0..unk.len() {
        unk[n] = f.read_u8()?;
    }
    Ok(())
}

fn parse_trustees<T: Read>(f: &mut T, trustees: &mut [Trustee]) -> Result<(), std::io::Error> {
    for n in 0..trustees.len() {
        trustees[n].object_id = f.read_u32::<BigEndian>()?;
        trustees[n].rights = f.read_u16::<LittleEndian>()?;
    }
    Ok(())
}

#[derive(Default,Debug)]
pub struct Hotfix {
    pub id: String,
    pub v_id: u32,
    pub unk1: [ u16; 4 ],
    pub data_area_sectors: u32,
    pub redir_area_sectors: u32,
    pub unk2: [ u32; 8 ],
}

impl Hotfix {
    pub fn new<T: Seek + Read>(f: &mut T, offset: u64) -> Result<Hotfix, std::io::Error> {
        f.seek(SeekFrom::Start(offset))?;

        let mut hotfix = Hotfix{ ..Default::default() };
        let mut id = [ 0u8; 8 ];
        f.read(&mut id )?;
        hotfix.id = terminate_string(&id);
        hotfix.v_id = f.read_u32::<LittleEndian>()?;
        parse_unknown_u16(f, &mut hotfix.unk1)?;
        hotfix.data_area_sectors = f.read_u32::<LittleEndian>()?;
        hotfix.redir_area_sectors = f.read_u32::<LittleEndian>()?;
        parse_unknown_u32(f, &mut hotfix.unk2)?;
        Ok(hotfix)
    }
}

#[derive(Default,Debug)]
pub struct Mirror {
    pub id: String,
    pub create_time: u32,
    // unk1[0] seems to be a flags fields, 90000 usually or b0000 if we have a mirror
    pub unk1: [ u32; 5 ],
    pub hotfix_v_id1: u32,
    pub hotfix_v_id2: u32,
}

impl Mirror {
    pub fn new<T: Seek + Read>(f: &mut T, offset: u64) -> Result<Mirror, std::io::Error> {
        f.seek(SeekFrom::Start(offset))?;

        let mut mirror = Mirror{ ..Default::default() };
        let mut id = [ 0u8; 8 ];
        f.read(&mut id )?;
        mirror.id = terminate_string(&id);
        mirror.create_time = f.read_u32::<LittleEndian>()?;
        for n in 0..5 {
            mirror.unk1[n] = f.read_u32::<LittleEndian>()?;
        }
        // There can likely be more here, if more mirrors are defined
        mirror.hotfix_v_id1 = f.read_u32::<LittleEndian>()?;
        mirror.hotfix_v_id2 = f.read_u32::<LittleEndian>()?;
        Ok(mirror)
    }
}

#[derive(Default,Debug)]
pub struct VolumeInfo {
    pub name: String,
    // unk1 seems to be some kind of ID...
    pub unk1: u16,
    pub segment_num: u16,
    // first_sector is always 160 for the first segment and then gets
    // added more
    pub first_sector: u32,
    pub num_sectors: u32,
    pub total_blocks: u32,
    pub first_segment_block: u32,
    pub unk2: u32,
    pub block_size: u32,
    pub rootdir_block_nr: u32,
    pub rootdir_copy_block_nr: u32,
    pub unk3: u32,
}

impl VolumeInfo {
    pub fn new<T: Read>(f: &mut T) -> Result<VolumeInfo, std::io::Error> {
        let mut volume = VolumeInfo{ ..Default::default() };
        let name_len = f.read_u8()?;
        let mut vol_name = [ 0u8; 19 ];
        f.read(&mut vol_name)?;
        volume.name = make_string(&vol_name, name_len);
        volume.unk1 = f.read_u16::<LittleEndian>()?;
        volume.segment_num = f.read_u16::<LittleEndian>()?;
        volume.first_sector = f.read_u32::<LittleEndian>()?;
        volume.num_sectors = f.read_u32::<LittleEndian>()?;
        volume.total_blocks = f.read_u32::<LittleEndian>()?;
        volume.first_segment_block = f.read_u32::<LittleEndian>()?;
        volume.unk2 = f.read_u32::<LittleEndian>()?;
        let block_value = f.read_u32::<LittleEndian>()?;
        volume.rootdir_block_nr = f.read_u32::<LittleEndian>()?;
        volume.rootdir_copy_block_nr = f.read_u32::<LittleEndian>()?;
        volume.unk3 = f.read_u32::<LittleEndian>()?;
        volume.block_size = (256 / block_value) * 1024;
        Ok(volume)
    }
}

#[derive(Default,Debug)]
pub struct Volumes {
    pub unk1: [ u32; 3 ],
    pub info: Vec<VolumeInfo>,
}

impl Volumes {
    pub fn new<T: Seek + Read>(f: &mut T, offset: u64) -> Result<Volumes, NetWareError> {
        f.seek(SeekFrom::Start(offset))?;
        let mut magic = [ 0u8; 16 ];
        f.read(&mut magic )?;
        let magic = terminate_string(&magic);
        if magic != "NetWare Volumes" { return Err(NetWareError::VolumeAreaCorrupt); }

        let mut volumes = Volumes{ ..Default::default() };
        let num_volumes = f.read_u32::<LittleEndian>()?;
        for n in 0..3 {
            let v = f.read_u32::<LittleEndian>()?;
            volumes.unk1[n] = v;
        }
        for _ in 0..num_volumes {
            let volume = VolumeInfo::new(f)?;
            volumes.info.push(volume);
        }
        Ok(volumes)
    }
}

#[derive(Default,Debug)]
pub struct Trustee {
    pub object_id: u32,
    pub rights: u16
}

#[derive(Default,Debug)]
pub struct GrantList {
    pub parent_dir_id: u32,
    pub unk1: [ u32; 5 ],
    pub trustees: [ Trustee; 16 ],
    pub unk2: [ u32; 2 ],
}

#[derive(Default,Debug)]
pub struct VolumeInformation {
    pub parent_dir_id: u32,
    pub unk1: [ u32; 5 ],
    pub create_time: u32,
    pub owner_id: u32,
    pub unk2: [ u32; 2 ],
    pub modify_time: u32,
    pub unk3: [ u32; 1 ],
    pub trustees: [ Trustee; 8 ],
    pub unk4: [ u32; 8 ]
}

#[derive(Default,Debug)]
pub struct FileItem {
    pub parent_dir_id: u32,
    pub attr: u32,
    pub unk1: [ u8; 3 ],
    pub name: String,
    pub create_time: u32,
    pub owner_id: u32,
    pub unk2: [ u32; 2 ],
    pub modify_time: u32,
    pub modifier_id: u32,
    pub length: u32,
    pub block_nr: u32,
    pub unk3: [ u32; 1 ],
    pub trustees: [ Trustee; 6 ],
    pub unk4: [ u32; 2 ],
    pub delete_time: u32,
    pub delete_id: u32,
    pub unk5: [ u32; 2 ],
    pub file_entry: u32,
    pub unk6: [ u32; 1 ],
}

#[derive(Default,Debug)]
pub struct DirectoryItem {
    pub parent_dir_id: u32,
    pub attr: u32,
    pub unk1: [ u8; 3 ],
    pub name: String,
    pub create_time: u32,
    pub owner_id: u32,
    pub unk2: [ u32; 2 ],
    pub modify_time: u32,
    pub unk3: [ u32; 1 ],
    pub trustees: [ Trustee; 8 ],
    pub unk4: [ u16; 2 ],
    pub inherited_rights_mask: u16,
    pub subdir_index: u32,
    pub unk5: [ u16; 7 ],
    pub directory_id: u32,
    pub unk6: [ u16; 2 ],
}

#[derive(Default,Debug)]
pub struct Available {
    pub parent_dir_id: u32,
}

#[derive(Debug)]
pub enum DirEntry {
    Available(Available),
    GrantList(GrantList),
    VolumeInformation(VolumeInformation),
    File(FileItem),
    Directory(DirectoryItem),
}

fn parse_directory_entry<T: Seek + Read>(f: &mut T) -> Result<DirEntry, std::io::Error> {
    let parent_dir_id = f.read_u32::<LittleEndian>()?;
    return match parent_dir_id {
        DIRID_GRANT_LIST => {
            let mut g = GrantList{ parent_dir_id, ..Default::default() };
            parse_unknown_u32(f, &mut g.unk1)?;
            parse_trustees(f, &mut g.trustees)?;
            parse_unknown_u32(f, &mut g.unk2)?;
            Ok(DirEntry::GrantList(g))
        },
        DIRID_VOLUME_INFO => {
            let mut v = VolumeInformation{ parent_dir_id, ..Default::default() };
            parse_unknown_u32(f, &mut v.unk1)?;
            v.create_time = f.read_u32::<LittleEndian>()?;
            v.owner_id = f.read_u32::<LittleEndian>()?;
            parse_unknown_u32(f, &mut v.unk2)?;
            v.modify_time = f.read_u32::<LittleEndian>()?;
            parse_unknown_u32(f, &mut v.unk3)?;
            parse_trustees(f, &mut v.trustees)?;
            parse_unknown_u32(f, &mut v.unk4)?;
            Ok(DirEntry::VolumeInformation(v))
        },
        DIRID_AVAILABLE => {
            f.seek(SeekFrom::Current(124))?;
            let available = Available{ parent_dir_id };
            Ok(DirEntry::Available(available))
        },
        _ => {
            let attr = f.read_u32::<LittleEndian>()?;
            if (attr & ATTR_DIRECTORY) != 0 {
                // Directory
                let mut de = DirectoryItem{ parent_dir_id, attr, ..Default::default() };
                parse_unknown_u8(f, &mut de.unk1)?;
                let name_len = f.read_u8()?;
                let mut fname = [ 0u8; 12 ];
                f.read(&mut fname)?;
                de.name = make_string(&fname, name_len);
                de.create_time = f.read_u32::<LittleEndian>()?;
                de.owner_id = f.read_u32::<BigEndian>()?;
                parse_unknown_u32(f, &mut de.unk2)?;
                de.modify_time = f.read_u32::<LittleEndian>()?;
                parse_unknown_u32(f, &mut de.unk3)?;
                parse_trustees(f, &mut de.trustees)?;
                parse_unknown_u16(f, &mut de.unk4)?;
                de.inherited_rights_mask = f.read_u16::<LittleEndian>()?;
                de.subdir_index = f.read_u32::<LittleEndian>()?;
                parse_unknown_u16(f, &mut de.unk5)?;
                de.directory_id = f.read_u32::<LittleEndian>()?;
                parse_unknown_u16(f, &mut de.unk6)?;
                return Ok(DirEntry::Directory(de));
            } else {
                // File
                let mut fe = FileItem{ parent_dir_id, attr, ..Default::default() };
                parse_unknown_u8(f, &mut fe.unk1)?;
                let name_len = f.read_u8()?;
                let mut fname = [ 0u8; 12 ];
                f.read(&mut fname)?;
                fe.name = make_string(&fname, name_len);
                fe.create_time = f.read_u32::<LittleEndian>()?;
                fe.owner_id = f.read_u32::<BigEndian>()?;
                parse_unknown_u32(f, &mut fe.unk2)?;
                fe.modify_time = f.read_u32::<LittleEndian>()?;
                fe.modifier_id = f.read_u32::<BigEndian>()?;
                fe.length = f.read_u32::<LittleEndian>()?;
                fe.block_nr = f.read_u32::<LittleEndian>()?;
                parse_unknown_u32(f, &mut fe.unk3)?;
                parse_trustees(f, &mut fe.trustees)?;
                parse_unknown_u32(f, &mut fe.unk4)?;
                fe.delete_time = f.read_u32::<LittleEndian>()?;
                fe.delete_id = f.read_u32::<BigEndian>()?;
                parse_unknown_u32(f, &mut fe.unk5)?;
                fe.file_entry = f.read_u32::<LittleEndian>()?;
                parse_unknown_u32(f, &mut fe.unk6)?;
                return Ok(DirEntry::File(fe));
            }
        }
    }
}

pub struct NWPartition {
    pub hotfix: Hotfix,
    pub mirror: Mirror,
    pub volumes: Volumes,
    pub first_data_block_offset: u64,
}

impl NWPartition {
    pub fn new(file: &mut std::fs::File, start_offset: u64) -> Result<NWPartition, NetWareError> {
        let hotfix_offset = start_offset + HOTFIX_OFFSET;
        let hotfix = Hotfix::new(file, hotfix_offset)?;
        let mirror_offset = hotfix_offset + SECTOR_SIZE;
        let mirror = Mirror::new(file, mirror_offset)?;
        let volume_offset = hotfix_offset + (hotfix.redir_area_sectors as u64 * SECTOR_SIZE);
        let volumes = Volumes::new(file, volume_offset)?;
        let first_data_block_offset = volume_offset + VOLUME_SIZE;
        Ok(NWPartition{ hotfix, mirror, volumes, first_data_block_offset })
    }
}

struct VolumeOnPartition {
    file: std::fs::File,
    first_data_block_offset: u64,
    info: VolumeInfo,
}

impl VolumeOnPartition {
    fn calculate_block_range(&self) -> (u32, u32) {
        let v = &self.info;
        let first_block = v.first_segment_block;
        let sectors_per_block = v.block_size / SECTOR_SIZE as u32;
        let last_block = first_block + v.num_sectors * sectors_per_block;
        (first_block, last_block)
    }
}

struct LogicalVolume  {
    volumes: Vec<VolumeOnPartition>,
    pub directory: Vec<DirEntry>,
}

impl LogicalVolume {
    pub fn new(pm: &mut PartitionManager, name: &str) -> Result<LogicalVolume, NetWareError> {
        let mut volumes = Vec::new();
        for partition in pm.partitions.iter_mut() {
            let nwp = NWPartition::new(&mut partition.file, partition.start_offset)?;
            for info in nwp.volumes.info {
                if info.name == name {
                    let file = partition.file.try_clone()?;
                    volumes.push(VolumeOnPartition{ file, first_data_block_offset: nwp.first_data_block_offset, info });
                    break;
                }
            }
        }
        if volumes.is_empty() {
            return Err(NetWareError::VolumeNotFound);
        }
        let mut logical_volume = LogicalVolume{ volumes, directory: Vec::new() };
        logical_volume.read_directory()?;
        Ok(logical_volume)
    }

    fn read_directory(&mut self) -> Result<(), NetWareError> {
        let volume = self.volumes.first().unwrap();
        let block_size = volume.info.block_size;
        let mut items = Vec::new();
        let mut current_entry = volume.info.rootdir_block_nr;
        while current_entry != 0xffffffff {
            let file = self.seek_block(current_entry)?;
            for _ in 0..(block_size / 128) {
                let dir_entry = parse_directory_entry(file)?;
                items.push(dir_entry);
            }

            let entry = self.read_fat_entry(current_entry)?;
            current_entry = entry.1;
        }
        self.directory = items;
        Ok(())
    }

    pub fn seek_block(&mut self, block: u32) -> Result<&mut std::fs::File, NetWareError> {
        for vol in self.volumes.iter_mut() {
            let (first_block, last_block) = vol.calculate_block_range();
            if block >= first_block && block < last_block {
                let index = (block - first_block) as u64;
                let block_size = vol.info.block_size as u64;
                let offset = vol.first_data_block_offset + index * block_size;
                let file = &mut vol.file;
                file.seek(SeekFrom::Start(offset))?;
                return Ok(file)
            }
        }
        Err(NetWareError::BlockOutOfRange(block))
    }

    pub fn read_fat_entry(&mut self, entry: u32) -> Result<(u32, u32), NetWareError> {
        for vol in self.volumes.iter_mut() {
            let (first_block, last_block) = vol.calculate_block_range();
            if entry >= first_block && entry < last_block {
                let offset = vol.first_data_block_offset + (entry - first_block) as u64 * 8;
                let file = &mut vol.file;
                file.seek(SeekFrom::Start(offset))?;
                let a = file.read_u32::<LittleEndian>()?;
                let b = file.read_u32::<LittleEndian>()?;
                return Ok((a, b));
            }
        }
        Err(NetWareError::FATCorrupt(entry))
    }
}

struct Partition {
    pub file: std::fs::File,
    pub start_offset: u64,
}

struct PartitionManager {
    pub partitions: Vec<Partition>
}

impl PartitionManager {
    pub fn new() -> Self {
        Self{ partitions: Vec::new() }
    }

    pub fn add_image(&mut self, mut file: std::fs::File) -> Result<(), NetWareError> {
        let p = find_netware_partition(&mut file)?;
        if p.is_none() { return Err(NetWareError::NoPartitionFound) };
        let start_offset = p.unwrap() * SECTOR_SIZE;

        self.partitions.push(Partition{ file, start_offset });
        Ok(())
    }
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

pub fn match_parent_dir_id(de: &DirEntry, parent_dir_id: u32) -> bool {
    parent_dir_id == match de {
        DirEntry::Available(v) => { v.parent_dir_id },
        DirEntry::GrantList(v) => { v.parent_dir_id },
        DirEntry::VolumeInformation(v) => { v.parent_dir_id },
        DirEntry::File(v) => { v.parent_dir_id },
        DirEntry::Directory(v) => { v.parent_dir_id },
    }
}

pub fn match_dir_entry_name(de: &DirEntry, name: &str) -> bool {
    match de {
        DirEntry::File(v) => { v.name.eq_ignore_ascii_case(name) },
        DirEntry::Directory(v) => { v.name.eq_ignore_ascii_case(name) },
        _ => { false },
    }
}

pub fn is_deleted_dir_entry(de: &DirEntry) -> bool {
    match de {
        DirEntry::File(v) => { v.delete_time > 0  },
        _ => { false },
    }
}

pub fn lookup_directory(directory: &[ DirEntry ], pieces: &[ String ]) -> Option<Vec<u32>> {
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
            DirEntry::Directory(v) => {
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

fn print_direntry(de: &DirEntry) {
    match de {
        DirEntry::File(f) => {
            println!(" file  {:14} {:7} {} {:08x}", f.name, f.length, format_timestamp(f.modify_time), f.modifier_id);
            //println!("  created at {} by {:x}", format_timestamp(f.create_time), f.owner_id);
        },
        DirEntry::Directory(d) => {
            println!(" dir   {:14}       - {:19} - ? {}", d.name, format_timestamp(d.modify_time), d.inherited_rights_mask);
        },
        DirEntry::Available(_) => {},
        DirEntry::VolumeInformation(_) => {},
        DirEntry::GrantList(_) => {},
    }
}

fn find_file_entry(vol: &LogicalVolume, current_directory_id: u32, fname: &str) -> Option<(u32, u32)> {
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
    if let DirEntry::File(fe) = items.first().unwrap() {
        return Some((fe.block_nr, fe.length))
    }
    println!("Not a file");
    None
}

fn copy_file_data(vol: &mut LogicalVolume, f: &mut std::fs::File, block_nr: u32, length: u32) -> Result<usize, NetWareError> {
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

fn main() -> Result<(), NetWareError> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        panic!("usage: {} file.img", args[0]);
    }

    let path = &args[1];
    let f = File::open(path)?;

    let mut pm = PartitionManager::new();
    pm.add_image(f)?;

    let mut vol = LogicalVolume::new(&mut pm, "SYS")?;
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
