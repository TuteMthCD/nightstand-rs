use std::time::Duration;

use anyhow::Result;
use esp_idf_hal::delay::FreeRtos;
use log::info;

use esp_idf_hal::{
    gpio::OutputPin,
    peripheral::Peripheral,
    rmt::{self, FixedLengthSignal, RmtChannel},
};

pub fn ws2812_task<C, CH, P, PIN>(channel: CH, pin: PIN) -> Result<()>
where
    C: RmtChannel,
    CH: Peripheral<P = C>,
    P: OutputPin,
    PIN: Peripheral<P = P>,
{
    info!("Init ws2812_task");
    let conf = rmt::TxRmtConfig::new().clock_divider(1);

    let mut tx = rmt::TxRmtDriver::new(channel, pin, &conf)?;

    let ticks_hz = tx.counter_clock()?;
    info!("RMT counter clock: {} Hz", ticks_hz.0);

    let (t0h, t0l, t1h, t1l) = (
        rmt::Pulse::new_with_duration(ticks_hz, rmt::PinState::High, &Duration::from_nanos(350))?,
        rmt::Pulse::new_with_duration(ticks_hz, rmt::PinState::Low, &Duration::from_nanos(800))?,
        rmt::Pulse::new_with_duration(ticks_hz, rmt::PinState::High, &Duration::from_nanos(700))?,
        rmt::Pulse::new_with_duration(ticks_hz, rmt::PinState::Low, &Duration::from_nanos(600))?,
    );

    let mut color = 0x00_00_00;

    loop {
        color += 0xFF;
        if let Ok(signal) = encode_color(color, t0h, t0l, t1h, t1l) {
            tx.start_blocking(&signal)?;
            info!("frame sent {:x}", color);
        }
        FreeRtos::delay_ms(1000);
    }
}

fn encode_color(
    code: u32,
    t0h: rmt::Pulse,
    t0l: rmt::Pulse,
    t1h: rmt::Pulse,
    t1l: rmt::Pulse,
) -> Result<FixedLengthSignal<24>> {
    let mut signal = FixedLengthSignal::<24>::new();

    for i in (0..24).rev() {
        let p = 1_u32 << i;
        let bit_set = p & code != 0;
        let (high_pulse, low_pulse) = if bit_set { (t1h, t1l) } else { (t0h, t0l) };
        signal.set(23 - i as usize, &(high_pulse, low_pulse))?;
    }

    Ok(signal)
}
