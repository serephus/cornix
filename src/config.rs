pub const KEY_COUNT: usize = 42;
pub const KEY_LED_COUNT: usize = 21;
pub const LED_COUNT: usize = 23;
pub const LAYERS: usize = 4;

pub const B_MAX: u8 = 40;
pub const LED_COLOR: (u8, u8, u8) = (0x00, 0xFF, 0x30);
pub const FLUSH_INTERVAL_SECS: u64 = 60;
pub const SCAN_INTERVAL_MS: u64 = 1;
pub const DEBOUNCE_SAMPLES: u8 = 5;
pub const SPLIT_BAUDRATE: u32 = 1_000_000;
pub const HALF_DUPLEX_BAUD: u32 = 38_400;
pub const HALF_DUPLEX_TURNAROUND_US: u64 = 1_000;

pub const FLASH_SIZE: usize = 2 * 1024 * 1024;
pub const STORAGE_SECTOR_SIZE: u32 = 4096;
pub const STORAGE_SECTORS: u32 = 4;
pub const STORAGE_SLOT_SIZE: u32 = 256;
pub const STORAGE_SLOTS_PER_SECTOR: u32 = STORAGE_SECTOR_SIZE / STORAGE_SLOT_SIZE;
pub const STORAGE_BASE: u32 = FLASH_SIZE as u32 - STORAGE_SECTORS * STORAGE_SECTOR_SIZE;
pub const STORAGE_RECORD_SIZE: usize = 4 + 4 + KEY_COUNT * 4 + 4;
pub const STORAGE_MAGIC: u32 = 0x5849_4e43;
