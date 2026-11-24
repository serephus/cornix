use crate::config::{KEY_COUNT, KEY_LED_COUNT};

pub const LEFT_CHAIN_TO_GLOBAL: [u8; KEY_LED_COUNT] = [
    20, 17, 11, 5, 4, 10, 16, 19, 18, 15, 9, 3, 2, 8, 14, 13, 7, 1, 0, 6, 12,
];

pub const RIGHT_CHAIN_TO_GLOBAL: [u8; KEY_LED_COUNT] = [
    41, 38, 32, 26, 25, 31, 37, 40, 39, 36, 30, 24, 23, 29, 35, 34, 28, 22, 21, 27, 33,
];

pub fn left_leds(brightness: &[u8; KEY_COUNT]) -> [u8; KEY_LED_COUNT] {
    let mut out = [0u8; KEY_LED_COUNT];
    for (chain, &key) in LEFT_CHAIN_TO_GLOBAL.iter().enumerate() {
        out[chain] = brightness[key as usize];
    }
    out
}

pub fn right_leds(brightness: &[u8; KEY_COUNT]) -> [u8; KEY_LED_COUNT] {
    let mut out = [0u8; KEY_LED_COUNT];
    for (chain, &key) in RIGHT_CHAIN_TO_GLOBAL.iter().enumerate() {
        out[chain] = brightness[key as usize];
    }
    out
}

#[macro_export]
macro_rules! left_inputs {
    ($p:expr) => {{
        use embassy_rp::gpio::{Input, Pull};
        [
            Input::new(($p).PIN_22, Pull::Up),
            Input::new(($p).PIN_20, Pull::Up),
            Input::new(($p).PIN_23, Pull::Up),
            Input::new(($p).PIN_26, Pull::Up),
            Input::new(($p).PIN_29, Pull::Up),
            Input::new(($p).PIN_0, Pull::Up),
            Input::new(($p).PIN_19, Pull::Up),
            Input::new(($p).PIN_18, Pull::Up),
            Input::new(($p).PIN_24, Pull::Up),
            Input::new(($p).PIN_27, Pull::Up),
            Input::new(($p).PIN_1, Pull::Up),
            Input::new(($p).PIN_2, Pull::Up),
            Input::new(($p).PIN_17, Pull::Up),
            Input::new(($p).PIN_16, Pull::Up),
            Input::new(($p).PIN_25, Pull::Up),
            Input::new(($p).PIN_28, Pull::Up),
            Input::new(($p).PIN_3, Pull::Up),
            Input::new(($p).PIN_9, Pull::Up),
            Input::new(($p).PIN_14, Pull::Up),
            Input::new(($p).PIN_15, Pull::Up),
            Input::new(($p).PIN_11, Pull::Up),
        ]
    }};
}

#[macro_export]
macro_rules! right_inputs {
    ($p:expr) => {{
        use embassy_rp::gpio::{Input, Pull};
        [
            Input::new(($p).PIN_8, Pull::Up),
            Input::new(($p).PIN_9, Pull::Up),
            Input::new(($p).PIN_3, Pull::Up),
            Input::new(($p).PIN_2, Pull::Up),
            Input::new(($p).PIN_1, Pull::Up),
            Input::new(($p).PIN_27, Pull::Up),
            Input::new(($p).PIN_11, Pull::Up),
            Input::new(($p).PIN_14, Pull::Up),
            Input::new(($p).PIN_4, Pull::Up),
            Input::new(($p).PIN_0, Pull::Up),
            Input::new(($p).PIN_28, Pull::Up),
            Input::new(($p).PIN_26, Pull::Up),
            Input::new(($p).PIN_15, Pull::Up),
            Input::new(($p).PIN_18, Pull::Up),
            Input::new(($p).PIN_5, Pull::Up),
            Input::new(($p).PIN_29, Pull::Up),
            Input::new(($p).PIN_20, Pull::Up),
            Input::new(($p).PIN_22, Pull::Up),
            Input::new(($p).PIN_16, Pull::Up),
            Input::new(($p).PIN_17, Pull::Up),
            Input::new(($p).PIN_19, Pull::Up),
        ]
    }};
}
