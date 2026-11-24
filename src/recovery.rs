use embassy_rp::gpio::Input;
use embassy_rp::watchdog::Watchdog;
use embassy_time::{Duration, Timer};

const BOOT_MAGIC: u32 = 0x434E_4958;
const TAP_WINDOW_MS: u64 = 2_000;
const WATCHDOG_MS: u64 = 5_000;

pub fn check_double_tap(watchdog: &mut Watchdog) {
    if watchdog.get_scratch(0) == BOOT_MAGIC {
        enter_bootloader(watchdog);
    }
}

pub fn arm_double_tap(watchdog: &mut Watchdog) {
    watchdog.set_scratch(0, BOOT_MAGIC);
    watchdog.start(Duration::from_millis(WATCHDOG_MS));
}

pub fn enter_bootloader(watchdog: &mut Watchdog) -> ! {
    watchdog.set_scratch(0, 0);
    watchdog.stop();
    embassy_rp::rom_data::reset_to_usb_boot(0, 0);
    loop {
        cortex_m::asm::wfi();
    }
}

pub fn clear_boot_magic_pac() {
    embassy_rp::pac::WATCHDOG.scratch0().write(|w| *w = 0);
}

pub async fn bootmagic_held(pin: &Input<'_>) -> bool {
    Timer::after_millis(20).await;
    if !pin.is_low() {
        return false;
    }
    Timer::after_millis(30).await;
    pin.is_low()
}

#[embassy_executor::task]
pub async fn clear_boot_magic(mut watchdog: Watchdog) {
    Timer::after_millis(TAP_WINDOW_MS).await;
    watchdog.set_scratch(0, 0);
    watchdog.stop();
}
