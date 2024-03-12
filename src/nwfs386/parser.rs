/*-
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * Copyright (c) 2022, 2024 Rink Springer <rink@rink.nu>
 * For conditions of distribution and use, see LICENSE file
 */
use byteorder::{LittleEndian, BigEndian, ReadBytesExt};

use std::io::{Read, Seek, SeekFrom};
use std::fmt;

use anyhow::{anyhow, Result};

use crate::nwfs386::types::{Attributes, Rights, Timestamp};
use crate::util;

const DIRID_VOLUME_INFO: u32 = 0xfffffffd;
const DIRID_GRANT_LIST: u32 = 0xfffffffe;
const DIRID_AVAILABLE: u32 = 0xffffffff;

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

fn parse_unknown_u32<T: Read>(f: &mut T, unk: &mut [u32]) -> Result<()> {
    for n in 0..unk.len() {
        unk[n] = f.read_u32::<LittleEndian>()?;
    }
    Ok(())
}

fn parse_unknown_u16<T: Read>(f: &mut T, unk: &mut [u16]) -> Result<()> {
    for n in 0..unk.len() {
        unk[n] = f.read_u16::<LittleEndian>()?;
    }
    Ok(())
}

fn parse_unknown_u8<T: Read>(f: &mut T, unk: &mut [u8]) -> Result<()> {
    for n in 0..unk.len() {
        unk[n] = f.read_u8()?;
    }
    Ok(())
}

fn parse_trustees<T: Read>(f: &mut T, trustees: &mut [Trustee]) -> Result<()> {
    for n in 0..trustees.len() {
        trustees[n].object_id = f.read_u32::<BigEndian>()?;
        trustees[n].rights = Rights::read_from(f)?;
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
    pub fn new<T: Seek + Read>(f: &mut T, offset: u64) -> Result<Hotfix> {
        f.seek(SeekFrom::Start(offset))?;

        let mut hotfix = Hotfix{ ..Default::default() };
        let mut id = [ 0u8; 8 ];
        f.read(&mut id )?;
        hotfix.id = util::asciiz_to_string(&id);
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
    pub create_time: Timestamp,
    // unk1[0] seems to be a flags fields, 90000 usually or b0000 if we have a mirror
    pub unk1: [ u32; 5 ],
    pub hotfix_v_id1: u32,
    pub hotfix_v_id2: u32,
}

impl Mirror {
    pub fn new<T: Seek + Read>(f: &mut T, offset: u64) -> Result<Mirror> {
        f.seek(SeekFrom::Start(offset))?;

        let mut mirror = Mirror{ ..Default::default() };
        let mut id = [ 0u8; 8 ];
        f.read(&mut id )?;
        mirror.id = util::asciiz_to_string(&id);
        mirror.create_time = Timestamp::read_from(f)?;
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
    pub fn new<T: Read>(f: &mut T) -> Result<VolumeInfo> {
        let mut volume = VolumeInfo{ ..Default::default() };
        let name_len = f.read_u8()?;
        let mut vol_name = [ 0u8; 19 ];
        f.read(&mut vol_name)?;
        volume.name = util::ascii_with_length_to_string(&vol_name, name_len);
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
    pub fn new<T: Seek + Read>(f: &mut T, offset: u64) -> Result<Volumes> {
        f.seek(SeekFrom::Start(offset))?;
        let mut magic = [ 0u8; 16 ];
        f.read(&mut magic )?;
        let magic = util::asciiz_to_string(&magic);
        if magic != "NetWare Volumes" { return Err(anyhow!("volume area is corrupt (magic mismatches)")); }

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
    pub rights: Rights,
}

#[derive(Default,Debug)]
pub struct GrantList {
    pub parent_dir_id: u32,
    pub unk1: [ u32; 5 ],
    pub trustees: [ Trustee; 16 ],
    pub unk2: [ u32; 2 ],
}

impl fmt::Display for GrantList {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "parent_dir_id {} unk1 {:?} trustees {:?} unk2 {:?}",
            self.parent_dir_id,
            self.unk1,
            self.trustees,
            self.unk2)
    }
}

#[derive(Default,Debug)]
pub struct VolumeInformation {
    pub parent_dir_id: u32,
    pub unk1: [ u32; 5 ],
    pub create_time: Timestamp,
    pub owner_id: u32,
    pub unk2: [ u32; 2 ],
    pub modify_time: Timestamp,
    pub unk3: [ u32; 1 ],
    pub trustees: [ Trustee; 8 ],
    pub unk4: [ u32; 8 ]
}

impl fmt::Display for VolumeInformation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "parent_dir_id {} unk1 {:?} create_time {} owner_id {} unk2 {:?} modify_time {} unk3 {:?} trustees {:?} unk4 {:?}",
            self.parent_dir_id,
            self.unk1,
            self.create_time,
            self.owner_id,
            self.unk2,
            self.modify_time,
            self.unk3,
            self.trustees,
            self.unk4)
    }
}

#[derive(Default,Debug)]
pub struct FileItem {
    pub parent_dir_id: u32,
    pub attr: Attributes,
    pub unk1: [ u8; 3 ],
    pub name: String,
    pub create_time: Timestamp,
    pub owner_id: u32,
    pub unk2: [ u32; 2 ],
    pub modify_time: Timestamp,
    pub modifier_id: u32,
    pub length: u32,
    pub block_nr: u32,
    pub unk3: [ u32; 1 ],
    pub trustees: [ Trustee; 6 ],
    pub unk4: [ u32; 2 ],
    pub delete_time: Timestamp,
    pub delete_id: u32,
    pub unk5: [ u32; 2 ],
    pub file_entry: u32,
    pub unk6: [ u32; 1 ],
}

impl fmt::Display for FileItem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "parent_dir_id {} attr {} unk1 {:?} name {} create_time {} owner_id {} unk2 {:?} modify_time {} modifier_id {} length {} block_nr {} unk3 {:?} trustees {:?} unk4 {:?} delete_time {} delete_id {} unk5 {:?} file_entry {} unk6 {:?}",
            self.parent_dir_id,
            self.attr,
            self.unk1,
            self.name,
            self.create_time,
            self.owner_id,
            self.unk2,
            self.modify_time,
            self.modifier_id,
            self.length,
            self.block_nr,
            self.unk3,
            self.trustees,
            self.unk4,
            self.delete_time,
            self.delete_id,
            self.unk5,
            self.file_entry,
            self.unk6)
    }
}

#[derive(Default,Debug)]
pub struct DirectoryItem {
    pub parent_dir_id: u32,
    pub attr: Attributes,
    pub unk1: [ u8; 3 ],
    pub name: String,
    pub create_time: Timestamp,
    pub owner_id: u32,
    pub unk2: [ u32; 2 ],
    pub modify_time: Timestamp,
    pub unk3: [ u32; 1 ],
    pub trustees: [ Trustee; 8 ],
    pub unk4: [ u16; 2 ],
    pub inherited_rights_mask: u16,
    pub subdir_index: u32,
    pub unk5: [ u16; 7 ],
    pub directory_id: u32,
    pub unk6: [ u16; 2 ],
}

impl fmt::Display for DirectoryItem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "parent_dir_id {} attr {} unk1 {:?} name {} create_time {} owner_id {} unk2 {:?} modify_time {} unk3 {:?} trustees {:?} unk4 {:?} inherited_rights_mask {} subdir_index {} unk5 {:?} directory_id {} unk6 {:?}",
            self.parent_dir_id,
            self.attr,
            self.unk1,
            self.name,
            self.create_time,
            self.owner_id,
            self.unk2,
            self.modify_time,
            self.unk3,
            self.trustees,
            self.unk4,
            self.inherited_rights_mask,
            self.subdir_index,
            self.unk5,
            self.directory_id,
            self.unk6)
    }
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

pub fn parse_directory_entry<T: Seek + Read>(f: &mut T) -> Result<DirEntry> {
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
            v.create_time = Timestamp::read_from(f)?;
            v.owner_id = f.read_u32::<LittleEndian>()?;
            parse_unknown_u32(f, &mut v.unk2)?;
            v.modify_time = Timestamp::read_from(f)?;
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
            //let attr = f.read_u32::<LittleEndian>()?;
            let attr = Attributes::read_from(f)?;
            if attr.is_directory() {
                // Directory
                let mut de = DirectoryItem{ parent_dir_id, attr, ..Default::default() };
                parse_unknown_u8(f, &mut de.unk1)?;
                let name_len = f.read_u8()?;
                let mut fname = [ 0u8; 12 ];
                f.read(&mut fname)?;
                de.name = util::ascii_with_length_to_string(&fname, name_len);
                de.create_time = Timestamp::read_from(f)?;
                de.owner_id = f.read_u32::<BigEndian>()?;
                parse_unknown_u32(f, &mut de.unk2)?;
                de.modify_time = Timestamp::read_from(f)?;
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
                fe.name = util::ascii_with_length_to_string(&fname, name_len);
                fe.create_time = Timestamp::read_from(f)?;
                fe.owner_id = f.read_u32::<BigEndian>()?;
                parse_unknown_u32(f, &mut fe.unk2)?;
                fe.modify_time = Timestamp::read_from(f)?;
                fe.modifier_id = f.read_u32::<BigEndian>()?;
                fe.length = f.read_u32::<LittleEndian>()?;
                fe.block_nr = f.read_u32::<LittleEndian>()?;
                parse_unknown_u32(f, &mut fe.unk3)?;
                parse_trustees(f, &mut fe.trustees)?;
                parse_unknown_u32(f, &mut fe.unk4)?;
                fe.delete_time = Timestamp::read_from(f)?;
                fe.delete_id = f.read_u32::<BigEndian>()?;
                parse_unknown_u32(f, &mut fe.unk5)?;
                fe.file_entry = f.read_u32::<LittleEndian>()?;
                parse_unknown_u32(f, &mut fe.unk6)?;
                return Ok(DirEntry::File(fe));
            }
        }
    }
}

pub fn parse_fat_entry<T: Read>(f: &mut T) -> Result<(u32, u32)> {
    let a = f.read_u32::<LittleEndian>()?;
    let b = f.read_u32::<LittleEndian>()?;
    Ok((a, b))
}

