/*-
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * Copyright (c) 2022 Rink Springer <rink@rink.nu>
 * For conditions of distribution and use, see LICENSE file
 */

pub fn format_timestamp(ts: u32) -> String {
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

