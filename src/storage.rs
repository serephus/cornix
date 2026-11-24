use embassy_rp::Peri;
use embassy_rp::flash::{Blocking, Flash};
use embassy_rp::peripherals::FLASH;

use crate::config::{
    FLASH_SIZE, KEY_COUNT, STORAGE_BASE, STORAGE_MAGIC, STORAGE_RECORD_SIZE, STORAGE_SECTOR_SIZE,
    STORAGE_SECTORS, STORAGE_SLOT_SIZE, STORAGE_SLOTS_PER_SECTOR,
};

type FlashDev = Flash<'static, FLASH, Blocking, FLASH_SIZE>;

pub struct Storage {
    flash: FlashDev,
    sector: u8,
    slot: u8,
    seq: u32,
    need_new_sector: bool,
}

impl Storage {
    pub fn open(flash: Peri<'static, FLASH>) -> (Self, [u32; KEY_COUNT]) {
        let mut flash = FlashDev::new_blocking(flash);
        let mut best: Option<(u32, u8, u8, [u32; KEY_COUNT])> = None;
        let mut record = [0u8; STORAGE_RECORD_SIZE];

        for sector in 0..STORAGE_SECTORS {
            for slot in 0..STORAGE_SLOTS_PER_SECTOR {
                let offset = STORAGE_BASE + sector * STORAGE_SECTOR_SIZE + slot * STORAGE_SLOT_SIZE;
                if flash.blocking_read(offset, &mut record).is_err() {
                    continue;
                }
                if let Some((seq, counts)) = parse_record(&record)
                    && best
                        .as_ref()
                        .is_none_or(|(best_seq, _, _, _)| seq > *best_seq)
                {
                    best = Some((seq, sector as u8, slot as u8, counts));
                }
            }
        }

        if let Some((seq, sector, slot, counts)) = best {
            let next_slot = slot + 1;
            let mut need_new_sector = next_slot >= STORAGE_SLOTS_PER_SECTOR as u8;

            if !need_new_sector {
                let offset = STORAGE_BASE
                    + sector as u32 * STORAGE_SECTOR_SIZE
                    + next_slot as u32 * STORAGE_SLOT_SIZE;
                if flash.blocking_read(offset, &mut record).is_ok()
                    && record.iter().any(|&byte| byte != 0xFF)
                {
                    need_new_sector = true;
                }
            }

            (
                Self {
                    flash,
                    sector,
                    slot: next_slot,
                    seq,
                    need_new_sector,
                },
                counts,
            )
        } else {
            (
                Self {
                    flash,
                    sector: STORAGE_SECTORS as u8 - 1,
                    slot: STORAGE_SLOTS_PER_SECTOR as u8,
                    seq: 0,
                    need_new_sector: true,
                },
                [0; KEY_COUNT],
            )
        }
    }

    pub fn save(&mut self, counts: &[u32; KEY_COUNT]) {
        let seq = self.seq.wrapping_add(1);

        if self.need_new_sector || self.slot as u32 >= STORAGE_SLOTS_PER_SECTOR {
            let next = ((self.sector as u32 + 1) % STORAGE_SECTORS) as u8;
            let base = STORAGE_BASE + next as u32 * STORAGE_SECTOR_SIZE;
            let _ = self.flash.blocking_erase(base, base + STORAGE_SECTOR_SIZE);
            self.sector = next;
            self.slot = 0;
            self.need_new_sector = false;
        }

        let offset = STORAGE_BASE
            + self.sector as u32 * STORAGE_SECTOR_SIZE
            + self.slot as u32 * STORAGE_SLOT_SIZE;
        let record = build_record(seq, counts);

        if self.flash.blocking_write(offset, &record).is_ok() {
            self.seq = seq;
            self.slot += 1;
            if self.slot as u32 >= STORAGE_SLOTS_PER_SECTOR {
                self.need_new_sector = true;
            }
        } else {
            self.need_new_sector = true;
            self.slot = STORAGE_SLOTS_PER_SECTOR as u8;
        }
    }
}

fn build_record(seq: u32, counts: &[u32; KEY_COUNT]) -> [u8; STORAGE_RECORD_SIZE] {
    let mut record = [0xFFu8; STORAGE_RECORD_SIZE];
    record[0..4].copy_from_slice(&STORAGE_MAGIC.to_le_bytes());
    record[4..8].copy_from_slice(&seq.to_le_bytes());
    for (index, &count) in counts.iter().enumerate() {
        let at = 8 + index * 4;
        record[at..at + 4].copy_from_slice(&count.to_le_bytes());
    }
    let crc = crc32(&record[..STORAGE_RECORD_SIZE - 4]);
    record[STORAGE_RECORD_SIZE - 4..].copy_from_slice(&crc.to_le_bytes());
    record
}

fn parse_record(record: &[u8; STORAGE_RECORD_SIZE]) -> Option<(u32, [u32; KEY_COUNT])> {
    let magic = u32::from_le_bytes(record[0..4].try_into().ok()?);
    if magic != STORAGE_MAGIC {
        return None;
    }
    let seq = u32::from_le_bytes(record[4..8].try_into().ok()?);
    let crc = u32::from_le_bytes(record[STORAGE_RECORD_SIZE - 4..].try_into().ok()?);
    if crc != crc32(&record[..STORAGE_RECORD_SIZE - 4]) {
        return None;
    }

    let mut counts = [0u32; KEY_COUNT];
    for (index, count) in counts.iter_mut().enumerate() {
        let at = 8 + index * 4;
        *count = u32::from_le_bytes(record[at..at + 4].try_into().ok()?);
    }
    Some((seq, counts))
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}
