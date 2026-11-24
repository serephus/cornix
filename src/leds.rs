use embassy_rp::Peri;
use embassy_rp::bind_interrupts;
use embassy_rp::dma;
use embassy_rp::peripherals::{DMA_CH0, PIN_10, PIO0};
use embassy_rp::pio::{InterruptHandler as PioInterruptHandler, Pio};
use embassy_rp::pio_programs::ws2812::{Grb, PioWs2812, PioWs2812Program};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use smart_leds::RGB8;

use crate::config::{KEY_LED_COUNT, LED_COLOR, LED_COUNT};

bind_interrupts!(struct PioIrqs {
    PIO0_IRQ_0 => PioInterruptHandler<PIO0>;
});

bind_interrupts!(struct DmaIrqs {
    DMA_IRQ_0 => dma::InterruptHandler<DMA_CH0>;
});

pub type Leds = PioWs2812<'static, PIO0, 0, LED_COUNT, Grb>;

pub fn setup(
    pio: Peri<'static, PIO0>,
    dma: Peri<'static, DMA_CH0>,
    pin: Peri<'static, PIN_10>,
) -> Leds {
    let mut pio = Pio::new(pio, PioIrqs);
    let program = PioWs2812Program::new(&mut pio.common);
    let sm0 = pio.sm0;
    PioWs2812::<PIO0, 0, LED_COUNT, Grb>::new(&mut pio.common, sm0, dma, DmaIrqs, pin, &program)
}

#[embassy_executor::task]
pub async fn led_task(
    mut leds: Leds,
    frames: &'static Channel<CriticalSectionRawMutex, [u8; KEY_LED_COUNT], 4>,
) {
    loop {
        let frame = frames.receive().await;
        let mut colors = [RGB8 { r: 0, g: 0, b: 0 }; LED_COUNT];
        for (index, &value) in frame.iter().enumerate() {
            let scale = |channel: u8| ((channel as u16 * value as u16) / 255) as u8;
            colors[index] = RGB8 {
                r: scale(LED_COLOR.0),
                g: scale(LED_COLOR.1),
                b: scale(LED_COLOR.2),
            };
        }
        leds.write(&colors).await;
    }
}
