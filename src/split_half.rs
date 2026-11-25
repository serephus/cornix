use embassy_futures::select::{Either, select};
use embassy_rp::Peri;
use embassy_rp::bind_interrupts;
use embassy_rp::clocks::clk_sys_freq;
use embassy_rp::gpio::Level;
use embassy_rp::pac::PADS_BANK0;
use embassy_rp::peripherals::{PIN_12, PIO1};
use embassy_rp::pio::{Config, Direction, FifoJoin, Pin, Pio, ShiftDirection};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Duration, Timer};
use fixed::traits::ToFixed;

use crate::config::{HALF_DUPLEX_BAUD, HALF_DUPLEX_TURNAROUND_US, KEY_LED_COUNT};
use crate::layout::KeyEvent;

const START0: u8 = 0xAA;
const START1: u8 = 0x55;
const TYPE_POLL: u8 = 1;
const TYPE_RESP: u8 = 2;
const MAX_PAYLOAD: usize = 32;
const POLL_FLAG_LEDS: u8 = 1;

bind_interrupts!(struct Pio1Irqs {
    PIO1_IRQ_0 => embassy_rp::pio::InterruptHandler<PIO1>;
});

pub struct HalfDuplex {
    pio: Pio<'static, PIO1>,
    tx_pin: Pin<'static, PIO1>,
}

struct Frame {
    len: usize,
    data: [u8; MAX_PAYLOAD],
}

pub fn setup(pio: Peri<'static, PIO1>, pin: Peri<'static, PIN_12>) -> HalfDuplex {
    let mut pio = Pio::new(pio, Pio1Irqs);
    let tx_pin = pio.common.make_pio_pin(pin);

    let tx_prg = pio::pio_asm!(
        r#"
            .side_set 1 opt

                pull       side 1 [7]
                set x, 7   side 0 [7]
            bitloop:
                out pins, 1
                jmp x-- bitloop [6]
        "#
    );
    let tx_prog = pio.common.load_program(&tx_prg.program);

    let rx_prg = pio::pio_asm!(
        r#"
            start:
                wait 0 pin 0
                set x, 7    [10]
            rx_bitloop:
                in pins, 1
                jmp x-- rx_bitloop [6]
                jmp pin good_rx_stop

                irq 4 rel
                wait 1 pin 0
                jmp start

            good_rx_stop:
                in null 24
                push
        "#
    );
    let rx_prog = pio.common.load_program(&rx_prg.program);

    let mut cfg = Config::default();
    cfg.set_out_pins(&[&tx_pin]);
    cfg.use_program(&tx_prog, &[&tx_pin]);
    cfg.shift_out.auto_fill = false;
    cfg.shift_out.direction = ShiftDirection::Right;
    cfg.fifo_join = FifoJoin::TxOnly;
    cfg.clock_divider = (clk_sys_freq() / (8 * HALF_DUPLEX_BAUD)).to_fixed();
    pio.sm0.set_config(&cfg);
    pio.sm0.set_enable(false);
    pio.sm0.set_pin_dirs(Direction::In, &[&tx_pin]);
    pio.sm0.set_pins(Level::High, &[&tx_pin]);

    let mut cfg = Config::default();
    cfg.use_program(&rx_prog, &[]);
    cfg.set_in_pins(&[&tx_pin]);
    cfg.set_jmp_pin(&tx_pin);
    cfg.shift_in.auto_fill = false;
    cfg.shift_in.direction = ShiftDirection::Right;
    cfg.shift_in.threshold = 32;
    cfg.fifo_join = FifoJoin::RxOnly;
    cfg.clock_divider = (clk_sys_freq() / (8 * HALF_DUPLEX_BAUD)).to_fixed();
    pio.sm1.set_config(&cfg);
    pio.sm1.set_pin_dirs(Direction::In, &[&tx_pin]);
    pio.sm1.clear_fifos();
    pio.sm1.restart();
    pio.sm1.set_enable(true);

    PADS_BANK0.gpio(12).modify(|w| {
        w.set_pue(true);
        w.set_pde(false);
        w.set_ie(true);
        w.set_od(false);
    });

    HalfDuplex { pio, tx_pin }
}

impl HalfDuplex {
    async fn send(&mut self, bytes: &[u8]) {
        self.pio.sm1.set_enable(false);
        self.pio.sm0.set_pin_dirs(Direction::Out, &[&self.tx_pin]);
        self.pio.sm0.set_pins(Level::High, &[&self.tx_pin]);
        self.pio.sm0.set_enable(true);

        for &byte in bytes {
            self.pio.sm0.tx().wait_push(byte as u32).await;
        }
        while !self.pio.sm0.tx().empty() {
            Timer::after_micros(20).await;
        }
        Timer::after_micros(11 * 1_000_000 / HALF_DUPLEX_BAUD as u64 + 100).await;

        self.pio.sm0.set_enable(false);
        self.pio.sm0.set_pin_dirs(Direction::In, &[&self.tx_pin]);
        self.pio.sm1.clear_fifos();
        self.pio.sm1.restart();
        self.pio.sm1.set_enable(true);
    }

    async fn recv_byte(&mut self, timeout: Duration) -> Option<u8> {
        let rx = self.pio.sm1.rx();
        match select(rx.wait_pull(), Timer::after(timeout)).await {
            Either::First(value) => Some(value as u8),
            Either::Second(_) => None,
        }
    }

    async fn recv_frame(&mut self, kind: u8, timeout: Duration) -> Option<Frame> {
        loop {
            if self.recv_byte(timeout).await? != START0 {
                continue;
            }
            if self.recv_byte(timeout).await? != START1 {
                continue;
            }

            let frame_kind = self.recv_byte(timeout).await?;
            let len = self.recv_byte(timeout).await? as usize;
            if len > MAX_PAYLOAD {
                continue;
            }

            let mut data = [0u8; MAX_PAYLOAD];
            for byte in data.iter_mut().take(len) {
                *byte = self.recv_byte(timeout).await?;
            }
            let crc = self.recv_byte(timeout).await?;

            let mut check = [0u8; 2 + MAX_PAYLOAD];
            check[0] = frame_kind;
            check[1] = len as u8;
            check[2..2 + len].copy_from_slice(&data[..len]);
            if crc8(&check[..2 + len]) != crc {
                continue;
            }

            if frame_kind == kind {
                return Some(Frame { len, data });
            }
        }
    }

    async fn send_frame(&mut self, kind: u8, payload: &[u8]) {
        let mut frame = [0u8; 4 + MAX_PAYLOAD + 1];
        frame[0] = START0;
        frame[1] = START1;
        frame[2] = kind;
        frame[3] = payload.len() as u8;
        frame[4..4 + payload.len()].copy_from_slice(payload);
        frame[4 + payload.len()] = crc8(&frame[2..4 + payload.len()]);
        self.send(&frame[..4 + payload.len() + 1]).await;
    }
}

#[embassy_executor::task]
pub async fn central_poll_task(
    mut hd: HalfDuplex,
    leds: &'static Channel<CriticalSectionRawMutex, [u8; KEY_LED_COUNT], 4>,
    events: &'static Channel<CriticalSectionRawMutex, KeyEvent, 16>,
) -> ! {
    let mut last_leds = [0u8; KEY_LED_COUNT];
    let mut led_gen = 0u8;
    let mut acked_led = u8::MAX;
    let mut ack = 0u16;

    loop {
        let mut changed = false;
        while let Ok(next) = leds.try_receive() {
            if next != last_leds {
                last_leds = next;
                changed = true;
            }
        }
        if changed {
            led_gen = led_gen.wrapping_add(1);
        }

        let mut payload = [0u8; 4 + KEY_LED_COUNT];
        payload[0..2].copy_from_slice(&ack.to_le_bytes());
        payload[2] = led_gen;
        let include_leds = led_gen != acked_led;
        payload[3] = if include_leds { POLL_FLAG_LEDS } else { 0 };
        let payload_len = if include_leds {
            payload[4..].copy_from_slice(&last_leds);
            4 + KEY_LED_COUNT
        } else {
            4
        };

        hd.send_frame(TYPE_POLL, &payload[..payload_len]).await;

        if let Some(frame) = hd.recv_frame(TYPE_RESP, Duration::from_millis(30)).await
            && frame.len >= 4
        {
            let seq = u16::from_le_bytes([frame.data[0], frame.data[1]]);
            let count = frame.data[2] as usize;
            acked_led = frame.data[3];
            if seq != ack && frame.len >= 4 + count * 2 {
                for index in 0..count {
                    let key = frame.data[4 + index * 2];
                    let pressed = frame.data[5 + index * 2] != 0;
                    events.send(KeyEvent { key, pressed }).await;
                }
                ack = seq;
            }
        }

        Timer::after_micros(HALF_DUPLEX_TURNAROUND_US).await;
    }
}

#[embassy_executor::task]
pub async fn peripheral_poll_task(
    mut hd: HalfDuplex,
    leds: &'static Channel<CriticalSectionRawMutex, [u8; KEY_LED_COUNT], 4>,
    events: &'static Channel<CriticalSectionRawMutex, KeyEvent, 16>,
) -> ! {
    let mut in_flight = [0u8; 4 + 16];
    let mut in_flight_len = 0usize;
    let mut in_flight_seq = 0u16;
    let mut waiting_ack = false;
    let mut last_led_gen = u8::MAX;

    loop {
        let Some(frame) = hd.recv_frame(TYPE_POLL, Duration::from_millis(50)).await else {
            continue;
        };

        if frame.len >= 4 {
            let ack = u16::from_le_bytes([frame.data[0], frame.data[1]]);
            let led_gen = frame.data[2];
            let flags = frame.data[3];
            if flags & POLL_FLAG_LEDS != 0 && frame.len >= 4 + KEY_LED_COUNT {
                let mut frame_leds = [0u8; KEY_LED_COUNT];
                frame_leds.copy_from_slice(&frame.data[4..4 + KEY_LED_COUNT]);
                last_led_gen = led_gen;
                leds.try_send(frame_leds).ok();
            }

            if waiting_ack && ack == in_flight_seq {
                waiting_ack = false;
                in_flight_len = 0;
            }
        }

        Timer::after_micros(HALF_DUPLEX_TURNAROUND_US).await;

        if waiting_ack {
            in_flight[3] = last_led_gen;
            hd.send_frame(TYPE_RESP, &in_flight[..in_flight_len]).await;
        } else {
            let mut count = 0usize;
            in_flight[0..2].copy_from_slice(&in_flight_seq.wrapping_add(1).to_le_bytes());
            while count < 8 {
                if let Ok(event) = events.try_receive() {
                    in_flight[4 + count * 2] = event.key;
                    in_flight[5 + count * 2] = event.pressed as u8;
                    count += 1;
                } else {
                    break;
                }
            }
            in_flight[2] = count as u8;
            in_flight[3] = last_led_gen;
            in_flight_seq = in_flight_seq.wrapping_add(1);
            in_flight_len = 4 + count * 2;
            waiting_ack = true;
            hd.send_frame(TYPE_RESP, &in_flight[..in_flight_len]).await;
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
