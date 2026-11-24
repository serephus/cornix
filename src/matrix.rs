use embassy_rp::gpio::Input;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Duration, Ticker};

use crate::config::{DEBOUNCE_SAMPLES, KEY_LED_COUNT, SCAN_INTERVAL_MS};
use crate::layout::KeyEvent;

#[embassy_executor::task]
pub async fn scan_task(
    pins: [Input<'static>; KEY_LED_COUNT],
    key_offset: u8,
    events: &'static Channel<CriticalSectionRawMutex, KeyEvent, 16>,
) {
    let mut stable = [false; KEY_LED_COUNT];
    let mut counter = [0u8; KEY_LED_COUNT];
    let mut ticker = Ticker::every(Duration::from_millis(SCAN_INTERVAL_MS));

    loop {
        ticker.next().await;

        for (index, pin) in pins.iter().enumerate() {
            let raw = pin.is_low();
            if raw == stable[index] {
                counter[index] = 0;
                continue;
            }

            counter[index] = counter[index].saturating_add(1);
            if counter[index] >= DEBOUNCE_SAMPLES {
                stable[index] = raw;
                counter[index] = 0;
                events
                    .send(KeyEvent {
                        key: key_offset + index as u8,
                        pressed: raw,
                    })
                    .await;
            }
        }
    }
}
