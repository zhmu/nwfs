# NWFS286

Last update: 16-March-2023. If you have any additional information or corrections, please create a pull request on GitHub or reach out to rink@rink.nu!

This document describes the internal layout of the filesysted used by Novell NetWare 2.x, also known as NetWare 286. According to [Wikipedia](https://en.wikipedia.org/wiki/NetWare_File_System), it is a 'based on a heavily modified version of FAT'. NWFS was definitely inspired by FAT and even refers to the linked list of block numbers as _FAT_.

NWFS286 partitions use partition ID 0x64. All filesystem blocks have a fixed size of 4096 bytes - this cannot be changed and is hardcoded in the loader code.

Most values are stored as _little endian_; if the value is stored as _big endian_ a _b_ is prefixed to the type. For example, `u16` is a 16-bit unsigned little endian value, whereas `bu32` is a 32-bit unsigned big endian value. Optionally, a _*n_ suffix is used to indicate there is an array of size _n_.

## Layout

_Note_ This information is only accurate if the entire disk is dedicated to a NetWare installation: there is only a NetWare 286 partition which starts from LBA number 1 and covers the entire disk. A custom boot loader is used to run the server executable, `SYSTEM:NET$OS.EXE`. It is possible to customize the partition layout, but this hasn't been analyzed yet. For now, data block _n_ can be located at sector _(n + 4) * 8_ (there are 8 sectors per block, since 4096 / 512 = 8)


|Sector |Description                                                  |
|-------|-------------------------------------------------------------|
|0      |Boot sector containing the partition table                   |
|1      |First partition sector, loads the loader and executes it     |
|2 .. 14|Loader: locates and executes `SYSTEM:NET$OS.EXE`            |
|15     |Control sector                                               |
|16     |Volume information                                           |

Within the loader, the directory ID of `SYSTEM` is hardcoded. It is set during installation.

## Control sector

This is read by the loader, but it doesn't seem to actually use any of the values.

## Volume information

There are two versions of the volume information in use. Versions prior to 2.15 use the following layout:

|Offset |Type     |Description                 |
|-------|---------|------------------------------------------------------------------------------|
|0      |u16      |Unknown (always 1?)                                                           |
|2      |u8\*16   |Volume name                                                                   |
|4      |u16      |Unknown (always 4?)                                                           |

For NetWare 2.15 and up, it is as follows:

|Offset |Type     |Description                 |
|-------|---------|------------------------------------------------------------------------------|
|0      |u16      |Must be zero                                                                  |
|2      |u16      |Magic value, must be `0xfade`                                                 |
|4      |u16      |Unknown (always 1?)                                                           |
|6      |u8\*16   |Volume name                                                                   |

Regardless of the NetWare version, the remainder of the structure is as follows (offsets between parentheses apply to NetWare 2.15 and up):

|Offset |Type     |Description                 |
|-------|---------|------------------------------------------------------------------------------|
|20 (22)|u16      |Undecided, appears to be related to block remapping                           |
|22 (24)|u8       |Entry count                                                                   |
|23 (25)|u8       |Unknown (always 3?)                                                           |
|24 (26)|(varies) |Directory entries #1 blocks                                                   |
|...    |(varies) |Directory entries #2 blocks                                                   |
|...    |(varies) |FAT blocks                                                                    |

## Directory entries

All files and directories on the volume are stored in a single directory listing. The volume's information _entry count_ describe the _blocks_ used to store the directory entries (they are not linked using the FAT).

Every directory entry is 32 bytes. All directory entries share the following header:

|Offset|Type    |Description                 |
|------|--------|----------------------------|
|0     |u16     |Parent directory ID         |
|2     |u8\*12  |Name                        |
|14    |u16     |Unknown (0)                 |
|16    |u16     |Entry attributes            |

The _attributes_ can be used to determine whether this is a _file_ or _directory_ entry: directories always have bits 8..15 set (`0xff00`).

The _parent directory ID_ refers to the directory in which this entry resides. It is constructed as follows:

|Bits |Description                 |
|-----|----------------------------|
|4..15|Block index of the entry    |
|0..3 |Entry index within block    |

For example if the parent directory is stored in block `0x12` index `0xd`, the ID will be `0x12d`.


### File entries

|Offset|Type    |Description                 |
|------|--------|----------------------------|
|18    |u32     |File size, in bytes         |
|22    |u16     |Creation date               |
|24    |u16     |Last accessed date          |
|26    |u16     |Last modified date          |
|28    |u16     |Last modified time          |
|30    |u16     |First data block            |

### Directory entries

|Offset|Type    |Description                                           |
|------|--------|------------------------------------------------------|
|18    |u16     |Last modified date                                    |
|20    |u16     |Last modified time                                    |
|22    |u16     |Unknown                                               |
|24    |u16     |Unknown                                               |
|26    |u16     |Unknown                                               |
|28    |u16     |Always `0xd1d1`                                       |
|30    |u16     |If non-zero, seems to be used for trustee information |

## FAT entries

Every FAT entry has the following layout:

|Offset|Type    |Description                 |
|------|--------|----------------------------|
|0     |u16     |Index                       |
|2     |u16     |Block number                |

The first block has index 0 and refers to the second block. The second block has index 1 and refers to the third block, and so forth. The last block number contains the value `0xffff`.
