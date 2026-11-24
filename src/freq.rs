use crate::config::{B_MAX, KEY_COUNT};

pub struct Frequency {
    pub counts: [u32; KEY_COUNT],
    pub total: u64,
    pub dirty: bool,
}

impl Frequency {
    pub fn new(counts: [u32; KEY_COUNT]) -> Self {
        let total = counts.iter().map(|&count| count as u64).sum();
        Self {
            counts,
            total,
            dirty: false,
        }
    }

    pub fn press(&mut self, key: usize) {
        if key >= KEY_COUNT {
            return;
        }
        if self.counts[key] != u32::MAX {
            self.counts[key] += 1;
            self.total += 1;
            self.dirty = true;
        }
    }

    pub fn reset(&mut self) {
        self.counts = [0; KEY_COUNT];
        self.total = 0;
        self.dirty = true;
    }

    pub fn brightness(&self) -> [u8; KEY_COUNT] {
        let mut out = [0u8; KEY_COUNT];
        if self.total == 0 {
            return out;
        }

        let denominator = self.total * 2;
        for (index, &count) in self.counts.iter().enumerate() {
            let scaled = B_MAX as u64 * KEY_COUNT as u64 * count as u64 / denominator;
            out[index] = scaled.min(B_MAX as u64) as u8;
        }
        out
    }
}
