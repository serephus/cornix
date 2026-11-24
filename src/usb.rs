use embassy_futures::join::join;
use embassy_rp::Peri;
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::USB;
use embassy_rp::usb as rp_usb;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_usb::class::hid;

bind_interrupts!(struct UsbIrqs {
    USBCTRL_IRQ => rp_usb::InterruptHandler<USB>;
});

const KEYBOARD_REPORT_DESCRIPTOR: &[u8] = &[
    0x05, 0x01, 0x09, 0x06, 0xA1, 0x01, 0x05, 0x07, 0x19, 0xE0, 0x29, 0xE7, 0x15, 0x00, 0x25, 0x01,
    0x75, 0x01, 0x95, 0x08, 0x81, 0x02, 0x95, 0x01, 0x75, 0x08, 0x81, 0x01, 0x95, 0x06, 0x75, 0x08,
    0x15, 0x00, 0x25, 0x65, 0x05, 0x07, 0x19, 0x00, 0x29, 0x65, 0x81, 0x00, 0x05, 0x08, 0x19, 0x01,
    0x29, 0x05, 0x95, 0x05, 0x75, 0x01, 0x91, 0x02, 0x95, 0x01, 0x75, 0x03, 0x91, 0x01, 0xC0,
];

#[embassy_executor::task]
pub async fn usb_task(
    usb: Peri<'static, USB>,
    reports: &'static Signal<CriticalSectionRawMutex, [u8; 8]>,
) {
    let mut hid_state = hid::State::new();
    let driver = rp_usb::Driver::new(usb, UsbIrqs);

    let mut config = embassy_usb::Config::new(0x4653, 0x0004);
    config.manufacturer = Some("cornix");
    config.product = Some("Corne v4");
    config.serial_number = Some("1");
    config.max_power = 500;
    config.self_powered = false;

    let mut config_descriptor = [0u8; 256];
    let mut bos_descriptor = [0u8; 256];
    let mut msos_descriptor = [0u8; 256];
    let mut control_buf = [0u8; 64];

    let mut builder = embassy_usb::Builder::new(
        driver,
        config,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut msos_descriptor,
        &mut control_buf,
    );

    let hid_config = hid::Config {
        report_descriptor: KEYBOARD_REPORT_DESCRIPTOR,
        request_handler: None,
        poll_ms: 10,
        max_packet_size: 8,
        hid_subclass: hid::HidSubclass::Boot,
        hid_boot_protocol: hid::HidBootProtocol::Keyboard,
    };

    let mut writer = hid::HidWriter::<_, 8>::new(&mut builder, &mut hid_state, hid_config);
    let mut device = builder.build();

    join(device.run(), async {
        writer.ready().await;
        loop {
            let report = reports.wait().await;
            let _ = writer.write(&report).await;
        }
    })
    .await;
}
