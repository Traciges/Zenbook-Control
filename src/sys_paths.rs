// Ayuz - Unofficial Control Center for Asus Laptops
// Copyright (C) 2026 Guido Philipp
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see https://www.gnu.org/licenses/.

pub const SYS_PRODUCT_NAME: &str = "/sys/class/dmi/id/product_name";
pub const SYS_BOARD_NAME: &str = "/sys/class/dmi/id/board_name";
pub const SYS_BIOS_VERSION: &str = "/sys/class/dmi/id/bios_version";
pub const SYS_BIOS_DATE: &str = "/sys/class/dmi/id/bios_date";
pub const SYS_PRODUCT_SERIAL: &str = "/sys/class/dmi/id/product_serial";
pub const SYS_BATTERY0_CAPACITY: &str = "/sys/class/power_supply/BAT0/capacity";
pub const SYS_BATTERY1_CAPACITY: &str = "/sys/class/power_supply/BAT1/capacity";
pub const SYS_LOAD_AVG: &str = "/proc/loadavg";
pub const SYS_MEM_INFO: &str = "/proc/meminfo";
pub const SYS_THERMAL_ZONE0_TEMP: &str = "/sys/class/thermal/thermal_zone0/temp";
pub const SYS_MEM_SLEEP: &str = "/sys/power/mem_sleep";
