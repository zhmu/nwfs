# Introduction

Last update: 16-March-2023. If you have any additional information or corrections, please create a pull request on GitHub or reach out to rink@rink.nu!

NWFS386 is a lot more than just a filesystem: in addition to remapping bad sectors, the partition can be mirrored (similar to RAID-1). Furthermore, it is possible to divide the partition into multiple volumes - and, a volume can span multiple partitions.

The block size is configurable, and can range from 1KB to 64KB, using powers of 2.

- HOTFIX area is fixed at sector 32
- MIRROR area is fixed at sector 33
- VOLUME area follows the HOTFIX/MIRROR area
- First data block immediatly follows the VOLUME AREA

Most values are stored as _little endian_; if the value is stored as _big endian_ a _b_ is prefixed to the type. For example, `u16` is a 16-bit unsigned little endian value, whereas `bu32` is a 32-bit unsigned big endian value. Optionally, a _*n_ suffix is used to indicate there is an array of size _n_.

## Hotfix/mirror area

This area is always located at sector 32 and is 16KB in length. There are 4 copies and each copy contains the following structure:

|Offset|Type     |Description                       |
|------|---------|----------------------------------|
|0     |u8\*8    |Identifier - `HOTFIX00`           |
|8     |u32      |ID code                           |
|12    |u16\*4   |?                                 |
|20    |u32      |Number of data sectors            |
|24    |u32      |Number of redirection data sectors|
|28    |u8\*8    |?                                 |

## Mirror area

The sector directly after the hotfix header contains the mirror information:

|Offset|Type     |Description                       |
|------|---------|----------------------------------|
|0     |u8\*8    |Identifier - `MIRROR00`           |
|8     |u32      |Creation timestamp                |
|12    |u32\*5   |?                                 |
|32    |u32      |Hotfix area #1                    |
|36    |u32      |Hotfix area #2                    |

## Volume area

This area is located exactly _number of redirection data sectors_ after the first hotfix sector. The volume area is always 64KB in length, regardless of the number of volumes in use.

|Offset|Type     |Description                       |
|------|---------|----------------------------------|
|0     |u8\*16   |Identifier - `NetWare Volumes\0`  |
|16    |u32      |Number of volume entries          |
|20    |u32\*3   |?                                 |

This header is followed by _number of volume entries_ tims the following structure

|Offset|Type     |Description                                 |
|------|---------|--------------------------------------------|
|0     |u8       |Volume name length                          |
|1     |u8\*19   |Volume name                                 |
|20    |u16      |?                                           |
|22    |u16      |Volume segment number                       |
|24    |u32      |First sector (always 160?)                  |
|28    |u32      |Total number of blocks in this volume       |
|32    |u32      |First data block in this segment            |
|36    |u32      |?                                           |
|40    |u32      |`block_value` - used to calculate block size|
|44    |u32      |Block number containing the directory       |
|48    |u32      |Block number containing the directory backup|
|52    |u32      |?                                           |

The actual block size can be calculated as `(256 / block_value) * 1024`. This means the block size can vary between 1KB and 256KB, but I haven't checked if values not offered by the installer will work.

If the volume spans multiple partitions, all partitions will have the volume listed in their volume area. The _first data block_ will vary: the first segment will contain the value of 0, whereas the others will not. Thus, all blocks within the segment are relative to _first data block_, and given a block number you must determine which partition must be accessed.

# Data area

The data area directly follows the volume area. All block numbers refer to this area: block number 0 is the first block in the data area, which is occupied by the FAT.

# FAT

Like the MS-DOS FAT, given some block number, the FAT is used to look up the next block. The FAT is stored directly within the data area.

# Directory entries

All directory entries are 128 bytes in size. All fields share the following common header:

|Offset|Type       |Description                                            |
|------|-----------|-------------------------------------------------------|
|0     |u32        |Directory ID where this entry resides (parent ID)      |
|4     |u32        |Attributes                                             |
|8     |u8\*3      |?                                                      |
|11    |u8         |Entry name length                                      |
|12    |u8\*12     |8.3 file name                                          |
|24    |u32        |Entry creation timestamp                               |
|28    |bu32       |Object ID of the entry owner                           |
|32    |u32\*2     |?                                                      |
|40    |u32        |Last modification timestamp                            |

There are several special _parent ID_ values to identify unique entries. For all other entries, the _attributes_ field can be used to determine whether the entry is a file or directory, the latter has bit 4 set.

### File

|Offset|Type       |Description                                            |
|------|-----------|-------------------------------------------------------|
|44    |bu32       |Object ID of file's last modifier                      |
|48    |u32        |File length, in bytes                                  |
|52    |u32        |First block number                                     |
|56    |u32        |?                                                      |
|60    |Trustee\*6 |Trustees                                                |
|96    |u32\*2     |?                                                       |
|104   |u32        |If non-zero, file deletion timestamp                    |
|108   |bu32       |Object ID of file's deleter                             |
|112   |u32\*2     |?                                                       |
|120   |u32        |file_entry (unknown)                                    |
|124   |u32        |?                                                       |

## Directory

|Offset|Type       |Description                                            |
|------|-----------|-------------------------------------------------------|
|44    |u32        |?                                                      |
|48    |Trustee\*8 |Trustees                                               |
|96    |u16\*2     |?                                                      |
|100   |u16        |Mask for inherited rights                              |
|102   |u32        |subdir_index (unknown)                                 |
|106   |u16\*7     |?                                                      |
|120   |u32        |Directory ID                                           |
|124   |u16\*2     |?                                                      |

## Available entry

|Offset|Type       |Description                                            |
|------|-----------|-------------------------------------------------------|
|0     |u32        |0xffffffff to mark this entry as unused                |
|4     |u8\*124    |Typically 0                                            |

## Grant list

|Offset|Type       |Description                                            |
|------|-----------|-------------------------------------------------------|
|0     |u32        |0xfffffffe to mark this entry as grant list            |
|4     |u32\*5     |?                                                      |
|24    |Trustee\*16|Trustees                                               |
|120   |u32\*2     |?                                                      |

## Volume information

|Offset|Type       |Description                                            |
|------|-----------|-------------------------------------------------------|
|0     |u32        |0xfffffffd to mark this entry as grant list            |
|4     |u32\*5     |?                                                      |
|24    |u32        |Volume creation timestamp                              |
|28    |bu32       |Object ID of volume owner                              |
|32    |u32\*2     |?                                                      |
|40    |u32        |Last modification timestamp                            |
|44    |u32        |?                                                      |
|48    |Trustee\*8 |Trustees                                               |
|96    |u32\*8     |?                                                      |

### Trustee

Each trustee uses a 6-byte structure:

|Offset|Type       |Description                                            |
|------|-----------|-------------------------------------------------------|
|0     |bu32       |Object ID where the trustee applies to                 |
|2     |u16        |Bitmask containing trustee rights                      |

The rights mask contains the following bits

|Bit|NetWare right|Description                                             |
|---|-------------|--------------------------------------------------------|
|0  |R            |Read access                                             |
|1  |W            |Write access                                            |
|2  |             |Seems to be used internally?                            |
|3  |C            |Allowed to create subentries                            |
|4  |E            |Erase subentries                                        |
|5  |A            |Access control                                          |
|6  |F            |File scan                                               |
|7  |M            |Modify attributes                                       |
|8  |S            |Supervisory (overrides all others)                      |

### Attribute bits

|Bit|FILER flag|Description                                                     |
|---|------------------|--------------------------------------------------------|
|0  |Ro (Rw if clear)  |Read-only                                               |
|1  |H                 |Hidden                                                  |
|2  |Sy                |System                                                  |
|4  |                  |Directory                                               |
|5  |A                 |Archive                                                 |
|7  |S                 |Sharable                                                |
|12 |T                 |Transactional                                           |
|16 |P                 |Purge                                                   |
|17 |Ri                |Rename Inhibit                                          |
|18 |Di                |Delete Inhibit                                          |
|19 |Ci                |Copy Inhibit                                            |

### Timestamps

A timestamp is a 32-bit value, which is to be interpreted as two 16-bit values: the high part is the date and the low part is the time.

|Piece|Bits  |Description                                                     |
|-----|------|----------------------------------------------------------------|
|Time |11..15|Hour                                                            |
|     |5..10 |Minute                                                          |
|     |0..4  |Seconds divided by two                                          |
|Date |9..15 |Year minus 1980                                                 |
|     |5..8  |Month                                                           |
|     |0..4  |Day                                                             |
