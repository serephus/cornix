use embassy_rp::Peri;
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::{PIN_12, PIN_13, UART0};
use embassy_rp::uart::{
    BufferedInterruptHandler, BufferedUart, BufferedUartRx, BufferedUartTx, Config,
};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embedded_io_async::{Read, Write};
use static_cell::StaticCell;

use crate::config::{KEY_LED_COUNT, SPLIT_BAUDRATE};
use crate::layout::KeyEvent;

const START0: u8 = 0xAA;
const START1: u8 = 0x55;
const TYPE_KEY: u8 = 1;
const TYPE_LEDS: u8 = 2;

bind_interrupts!(struct UartIrqs {
    UART0_IRQ => BufferedInterruptHandler<UART0>;
});

static TX_BUFFER: StaticCell<[u8; 64]> = StaticCell::new();
static RX_BUFFER: StaticCell<[u8; 64]> = StaticCell::new();

pub fn setup(
    uart: Peri<'static, UART0>,
    tx: Peri<'static, PIN_12>,
    rx: Peri<'static, PIN_13>,
) -> BufferedUart {
    let mut config = Config::default();
    config.baudrate = SPLIT_BAUDRATE;

    BufferedUart::new(
        uart,
        tx,
        rx,
        UartIrqs,
        TX_BUFFER.init([0; 64]),
        RX_BUFFER.init([0; 64]),
        config,
    )
}

enum Frame {
    Key(KeyEvent),
    Leds([u8; KEY_LED_COUNT]),
}

async fn read_byte(rx: &mut BufferedUartRx) -> u8 {
    let mut byte = [0u8; 1];
    loop {
        if let Ok(1) = rx.read(&mut byte).await {
            return byte[0];
        }
    }
}

async fn read_frame(rx: &mut BufferedUartRx) -> Frame {
    loop {
        if read_byte(rx).await != START0 {
            continue;
        }
        if read_byte(rx).await != START1 {
            continue;
        }

        let kind = read_byte(rx).await;
        let len = read_byte(rx).await as usize;
        if len > KEY_LED_COUNT || (kind != TYPE_KEY && kind != TYPE_LEDS) {
            continue;
        }

        let mut payload = [0u8; KEY_LED_COUNT];
        for slot in payload.iter_mut().take(len) {
            *slot = read_byte(rx).await;
        }
        let crc = read_byte(rx).await;

        let mut check = [0u8; 2 + KEY_LED_COUNT];
        check[0] = kind;
        check[1] = len as u8;
        check[2..2 + len].copy_from_slice(&payload[..len]);
        if crc8(&check[..2 + len]) != crc {
            continue;
        }

        match (kind, len) {
            (TYPE_KEY, 2) => {
                return Frame::Key(KeyEvent {
                    key: payload[0],
                    pressed: payload[1] != 0,
                });
            }
            (TYPE_LEDS, KEY_LED_COUNT) => {
                let mut leds = [0u8; KEY_LED_COUNT];
                leds.copy_from_slice(&payload);
                return Frame::Leds(leds);
            }
            _ => continue,
        }
    }
}

fn crc8(data: &[u8]) -> u8 {
    let mut crc = 0u8;
    for &byte in data {
        crc ^= byte;
        for _ in 0..8 {
            if crc & 0x80 != 0 {
                crc = (crc << 1) ^ 0x07;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

async fn write_key(tx: &mut BufferedUartTx, event: KeyEvent) {
    let payload = [event.key, event.pressed as u8];
    let check = [TYPE_KEY, 2, payload[0], payload[1]];
    let crc = crc8(&check);
    let frame = [
        START0, START1, check[0], check[1], payload[0], payload[1], crc,
    ];
    let _ = tx.write_all(&frame).await;
}

async fn write_leds(tx: &mut BufferedUartTx, leds: &[u8; KEY_LED_COUNT]) {
    let mut frame = [0u8; 2 + 2 + KEY_LED_COUNT + 1];
    frame[0] = START0;
    frame[1] = START1;
    frame[2] = TYPE_LEDS;
    frame[3] = KEY_LED_COUNT as u8;
    frame[4..4 + KEY_LED_COUNT].copy_from_slice(leds);

    let mut check = [0u8; 2 + KEY_LED_COUNT];
    check[0] = TYPE_LEDS;
    check[1] = KEY_LED_COUNT as u8;
    check[2..].copy_from_slice(leds);
    let crc = crc8(&check);
    frame[frame.len() - 1] = crc;

    let _ = tx.write_all(&frame).await;
}

#[embassy_executor::task]
pub async fn key_tx_task(
    mut tx: BufferedUartTx,
    events: &'static Channel<CriticalSectionRawMutex, KeyEvent, 16>,
) {
    loop {
        let event = events.receive().await;
        write_key(&mut tx, event).await;
    }
}

#[embassy_executor::task]
pub async fn led_tx_task(
    mut tx: BufferedUartTx,
    leds: &'static Channel<CriticalSectionRawMutex, [u8; KEY_LED_COUNT], 4>,
) {
    loop {
        let frame = leds.receive().await;
        write_leds(&mut tx, &frame).await;
    }
}

#[embassy_executor::task]
pub async fn key_rx_task(
    mut rx: BufferedUartRx,
    events: &'static Channel<CriticalSectionRawMutex, KeyEvent, 16>,
) {
    loop {
        if let Frame::Key(event) = read_frame(&mut rx).await {
            events.send(event).await;
        }
    }
}

#[embassy_executor::task]
pub async fn led_rx_task(
    mut rx: BufferedUartRx,
    leds: &'static Channel<CriticalSectionRawMutex, [u8; KEY_LED_COUNT], 4>,
) {
    loop {
        if let Frame::Leds(frame) = read_frame(&mut rx).await {
            leds.send(frame).await;
        }
    }
}
