# Novell NetWare Filesystem tools

This is an attempt to reverse engineer the Novell NetWare filesystems. It covers both NWFS286 (Novell NetWare 2.x) and NWFS386 (Novell NetWare 3.x and likely also 4.x and later) filesystems.

The code has been tested with disk images from the following NetWare versions:

- NetWare 2.0a
- NetWare 2.0a ELS
- NetWare 2.15 ELS
- NetWare 2.2
- NetWare 3.12

If you have a different NetWare version, please give it a try and let me know the results! Additionally, feel free to reach out to me (rink@rink.nu) if you have any additional information from which this repository would benefit, or have any other information you wish to share (stories, bugs, feature requests and the like)

## transfer

The main tool available in this repository is `transfer`. This can retrieve files from both NWFS286 and NWFS386 disk images, using a commandline interface similar to FTP. `transfer` will automatically detect which NWFS version is used based on the partition type.

Example usage:

```
$ cargo run --bin transfer <path to disk image>
...
SYS:/> dir
<type> Name              Size Last Modified       Last Modifier
 dir   LOGIN                - 16-04-2022 08:34:12 - ? 65535
 dir   SYSTEM               - 12-05-2022 05:36:14 - ? 0
 dir   PUBLIC               - 16-04-2022 08:34:42 - ? 65535
 dir   MAIL                 - 17-04-2022 12:57:58 - ? 0
 dir   DELETED.SAV          - <invalid> - ? 0
 file  VOL$LOG.ERR       3168 12-05-2022 05:36:12 00000001
 file  TTS$LOG.ERR       3687 12-05-2022 05:36:14 00000001
 file  BACKOUT.TTS       8192 12-05-2022 05:36:12 00000001
 dir   ETC                  - 16-04-2022 08:34:12 - ? 65535
SYS:/> cd login
SYS:/login> dir
<type> Name              Size Last Modified       Last Modifier
 dir   NLS                  - 16-04-2022 08:34:00 - ? 65535
 file  LOGIN.EXE       111625 04-05-1993 15:06:26 03000001
 [ ... removed ... ]
SYS:/login> get login.exe
111625 bytes copied
```

Only `cd`, `dir`, `cat` and `get` are supported.

## inspect-nwfs386

This tool allows you to decode and dump all structures on a NWFS386 image. It can be used as follows:

```
$ cargo run --bin inspect-nwfs386 <path to disk image>
```

## TODO / feature request

* NWFS386: Volumes spanning multiple volumes aren't fully implemented
* NWFS386: Long file name support (namespace support)
* NWFS286: Only disk images covering the entire disk are supported for now
* Decode more unknown fields in the various structures

I accept pull requests, so get involved by all means!
