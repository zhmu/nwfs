# Novell NetWare Filesystem tools

This is an attempt to reverse engineer the Novell NetWare filesystems. It covers both the NWFS286 (Novell NetWare 2.x) and NWFS386 (Novell NetWare 3.x and likely also 4.x and later) filesystems.

Please reach out to me (rink@rink.nu) if you have any additional information from which these tools would benefit, or have any other information you wish to share (stories, bugs, feature requests and the like)

## NWFS386

There are two tools included in this repository, `inspect-nwfs386` and `shell-nwfs386`. Both tools expect a disk image file as argument. The tools will parse the partition table and expect to find a single partion (NetWare supports only a single partition per disk)

## shell-nwfs386

Provides an interactive shell to browse content in a NWFS386 partition. For example:

```
$ cargo run --bin shell-nwfs386 <path to disk image>
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
SYS:/login> dir
<type> Name              Size Last Modified       Last Modifier
 dir   NLS                  - 16-04-2022 08:34:00 - ? 65535
 file  LOGIN.EXE       111625 04-05-1993 15:06:26 03000001
 [ ... removed ... ]
SYS:/login> get login.exe
111625 bytes copied
```

Only `cd`, `dir` and `get` are supported.

## inspect-nwfs386

This tool allows you to decode and dump all structures. It can be used as follows:

```
$ cargo run --bin inspect-nwfs386 <path to disk image>
```

## NWFS286

Only a preliminary `shell-nwfs286` is available - this tool expects a disk image file. Usage is similar to `shell-nwfs386`.

## TODO / feature request

* NWFS386: Volumes spanning multiple volumes aren't fully implemented
* NWFS386: Long file name support (namespace support)
* NWFS286: Only disk images covering the entire disk are supported for now
* Decode more unknown fields in the various structures

I accept pull requests, so get involved by all means!
