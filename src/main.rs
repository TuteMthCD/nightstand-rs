mod ws2812;

use anyhow::{anyhow, Result};
use esp_idf_hal::gpio::{Output, OutputPin};
use log::{info, LevelFilter};

use esp_idf_svc::hal::{self, delay::FreeRtos};
use esp_idf_svc::log::{set_target_level, EspLogger};

use crate::ws2812::ws2812_task;

// const SSID: &str = env!("WIFI_SSID");
// const PASSWORD: &str = env!("WIFI_PASS");

fn main() -> Result<()> {
    esp_idf_svc::sys::link_patches();
    EspLogger::initialize_default();
    set_target_level("*", LevelFilter::Info)?;

    let hal::peripherals::Peripherals { rmt, pins, .. } =
        hal::peripherals::Peripherals::take().unwrap();

    let hal::rmt::RMT { channel0, .. } = rmt;

    let hal::gpio::Pins {
        gpio8: blink_pin,
        gpio9: ws_pin,
        ..
    } = pins;

    std::thread::Builder::new()
        .name("ws2812".to_string())
        .stack_size(1024 * 32)
        .spawn(move || ws2812_task(channel0, ws_pin))?;

    std::thread::Builder::new()
        .name("wifi".to_string())
        .stack_size(4096)
        .spawn(wifi_task)?;

    let blink_handle = std::thread::Builder::new()
        .name("blink".to_string())
        .stack_size(4096)
        .spawn(move || {
            let driver = hal::gpio::PinDriver::output(blink_pin)?;

            blink_task(driver)
        })?;

    let blink_result = blink_handle
        .join()
        .map_err(|_| anyhow!("blink thread panicked"))?;

    blink_result?;

    Ok(())
}

#[allow(dead_code)]
fn wifi_task() -> Result<()> {
    info!("init wifi task");

    // let nvs = nvs::EspDefaultNvsPartition::take().unwrap();

    Ok(())
}

fn blink_task<'d, P>(mut pin: hal::gpio::PinDriver<'d, P, Output>) -> Result<()>
where
    P: OutputPin,
{
    info!("init blink task");

    loop {
        pin.toggle()?;
        FreeRtos::delay_ms(1000);
    }
}
