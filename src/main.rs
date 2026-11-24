#![no_std]
#![no_main]

use cornix::board;
use cornix::config::{FLUSH_INTERVAL_SECS, KEY_COUNT, KEY_LED_COUNT};
use cornix::freq::Frequency;
use cornix::layout::{HidReport, KeyEvent, LayerState, PressResult};
#[cfg(feature = "v4_0")]
use cornix::split;
#[cfg(not(feature = "v4_0"))]
use cornix::split_half;
use cornix::{leds, matrix, recovery, storage, usb};
use embassy_executor::Spawner;
use embassy_futures::select::{Either, select};
use embassy_rp::gpio::{Input, Pull};
use embassy_rp::watchdog::Watchdog;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Instant, Ticker, Timer};
use panic_halt as _;

static KEY_EVENTS: Channel<CriticalSectionRawMutex, KeyEvent, 16> = Channel::new();
static LOCAL_LEDS: Channel<CriticalSectionRawMutex, [u8; KEY_LED_COUNT], 4> = Channel::new();
static REMOTE_LEDS: Channel<CriticalSectionRawMutex, [u8; KEY_LED_COUNT], 4> = Channel::new();
static HID_REPORTS: Signal<CriticalSectionRawMutex, [u8; 8]> = Signal::new();
static SAVES: Channel<CriticalSectionRawMutex, [u32; KEY_COUNT], 2> = Channel::new();

fn send_leds(freq: &Frequency, is_left: bool) {
    let brightness = freq.brightness();
    let (local, remote) = if is_left {
        (
            board::left_leds(&brightness),
            board::right_leds(&brightness),
        )
    } else {
        (
            board::right_leds(&brightness),
            board::left_leds(&brightness),
        )
    };
    LOCAL_LEDS.try_send(local).ok();
    REMOTE_LEDS.try_send(remote).ok();
}

fn send_report(report: HidReport) {
    let mut bytes = [0u8; 8];
    bytes[0] = report.modifiers;
    bytes[2..8].copy_from_slice(&report.keys);
    HID_REPORTS.signal(bytes);
}

#[embassy_executor::task]
async fn storage_task(mut storage: storage::Storage) -> ! {
    loop {
        let counts = SAVES.receive().await;
        storage.save(&counts);
    }
}

#[embassy_executor::task]
async fn logic_task(initial: [u32; KEY_COUNT], is_left: bool) -> ! {
    let mut freq = Frequency::new(initial);
    let mut layout = LayerState::new();
    let mut ticker = Ticker::every(Duration::from_secs(1));
    let mut last_activity = Instant::now();
    let mut last_save: Option<Instant> = None;

    send_leds(&freq, is_left);

    loop {
        match select(KEY_EVENTS.receive(), ticker.next()).await {
            Either::First(event) => {
                last_activity = Instant::now();
                if event.pressed {
                    freq.press(event.key as usize);
                    match layout.press(event.key as usize) {
                        PressResult::None => {}
                        PressResult::Report => send_report(layout.report()),
                        PressResult::Boot => {
                            recovery::clear_boot_magic_pac();
                            embassy_rp::rom_data::reset_to_usb_boot(0, 0);
                        }
                        PressResult::Reboot => {
                            recovery::clear_boot_magic_pac();
                            cortex_m::peripheral::SCB::sys_reset();
                        }
                        PressResult::Clear => {
                            freq.reset();
                            SAVES.send([0; KEY_COUNT]).await;
                        }
                    }
                    send_leds(&freq, is_left);
                } else if layout.release(event.key as usize) {
                    send_report(layout.report());
                }
            }
            Either::Second(_) => {
                send_leds(&freq, is_left);
                let now = Instant::now();
                let can_save = last_save.is_none_or(|last| {
                    now.duration_since(last) >= Duration::from_secs(FLUSH_INTERVAL_SECS)
                });
                if freq.dirty
                    && now.duration_since(last_activity) >= Duration::from_secs(2)
                    && can_save
                {
                    SAVES.send(freq.counts).await;
                    freq.dirty = false;
                    last_save = Some(now);
                }
            }
        }
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let mut watchdog = Watchdog::new(p.WATCHDOG);
    recovery::check_double_tap(&mut watchdog);
    recovery::arm_double_tap(&mut watchdog);

    let hand = Input::new(p.PIN_21, Pull::Up);
    Timer::after_millis(5).await;
    let is_left = hand.is_high();

    #[cfg(feature = "v4_0")]
    let is_central = is_left;
    #[cfg(not(feature = "v4_0"))]
    let is_central = {
        let vbus = Input::new(p.PIN_13, Pull::Down);
        Timer::after_millis(5).await;
        vbus.is_high()
    };

    let inputs = if is_left {
        cornix::left_inputs!(p)
    } else {
        cornix::right_inputs!(p)
    };
    if recovery::bootmagic_held(&inputs[0]).await {
        recovery::enter_bootloader(&mut watchdog);
    }

    let leds = leds::setup(p.PIO0, p.DMA_CH0, p.PIN_10);

    #[cfg(feature = "v4_0")]
    let uart = split::setup(p.UART0, p.PIN_12, p.PIN_13);
    #[cfg(not(feature = "v4_0"))]
    let half = split_half::setup(p.PIO1, p.PIN_12);

    let key_offset = if is_left { 0 } else { 21 };
    spawner.spawn(matrix::scan_task(inputs, key_offset, &KEY_EVENTS).unwrap());

    if is_central {
        let (storage, counts) = storage::Storage::open(p.FLASH);
        spawner.spawn(usb::usb_task(p.USB, &HID_REPORTS).unwrap());
        spawner.spawn(leds::led_task(leds, &LOCAL_LEDS).unwrap());

        #[cfg(feature = "v4_0")]
        {
            let (split_tx, split_rx) = uart.split();
            spawner.spawn(split::led_tx_task(split_tx, &REMOTE_LEDS).unwrap());
            spawner.spawn(split::key_rx_task(split_rx, &KEY_EVENTS).unwrap());
        }
        #[cfg(not(feature = "v4_0"))]
        spawner.spawn(split_half::central_poll_task(half, &REMOTE_LEDS, &KEY_EVENTS).unwrap());

        spawner.spawn(storage_task(storage).unwrap());
        spawner.spawn(logic_task(counts, is_left).unwrap());
    } else {
        spawner.spawn(leds::led_task(leds, &LOCAL_LEDS).unwrap());

        #[cfg(feature = "v4_0")]
        {
            let (split_tx, split_rx) = uart.split();
            spawner.spawn(split::key_tx_task(split_tx, &KEY_EVENTS).unwrap());
            spawner.spawn(split::led_rx_task(split_rx, &LOCAL_LEDS).unwrap());
        }
        #[cfg(not(feature = "v4_0"))]
        spawner.spawn(split_half::peripheral_poll_task(half, &LOCAL_LEDS, &KEY_EVENTS).unwrap());
    }

    spawner.spawn(recovery::clear_boot_magic(watchdog).unwrap());

    loop {
        Timer::after_secs(3600).await;
    }
}
