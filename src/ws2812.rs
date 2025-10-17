use anyhow::Result;
use esp_idf_hal::{delay, rmt};
use log::info;

use crate::ws2812::neopixel::Rgb;

pub fn ws2812_task(rmt: rmt::TxRmtDriver) -> Result<()> {
    info!("Init ws2812_task");

    let mut ledstrip = neopixel::Ws2812::new(rmt)?;

    let pixels = vec![
        Rgb::new(255, 255, 255),
        Rgb::new(255, 0, 0),
        Rgb::new(0, 255, 0),
        Rgb::new(0, 0, 255),
        Rgb::new(255, 255, 255),
        Rgb::new(255, 0, 0),
        Rgb::new(0, 255, 0),
        Rgb::new(0, 0, 255),
        Rgb::new(255, 255, 255),
        Rgb::new(255, 0, 0),
        Rgb::new(0, 255, 0),
        Rgb::new(0, 0, 255),
        Rgb::new(255, 255, 255),
        Rgb::new(255, 0, 0),
        Rgb::new(0, 255, 0),
        Rgb::new(0, 0, 255),
    ];

    ledstrip.transmit(&pixels)
}

pub mod neopixel {

    use anyhow::{bail, Result};
    use esp_idf_hal::rmt::{self, TxRmtDriver, VariableLengthSignal};

    struct Timings {
        t0h: rmt::Pulse,
        t0l: rmt::Pulse,
        t1h: rmt::Pulse,
        t1l: rmt::Pulse,
        rst: rmt::Pulse,
    }

    pub struct Ws2812<'d> {
        pixels: Vec<Rgb>,
        tx: TxRmtDriver<'d>,
        timings: Timings,
    }

    impl<'d> Ws2812<'d> {
        pub fn new(tx: TxRmtDriver<'d>) -> Result<Self> {
            use std::time::Duration;

            let pixels = Vec::new();

            let ticks_hz = tx.counter_clock()?;

            let timings = Timings {
                t0h: rmt::Pulse::new_with_duration(
                    ticks_hz,
                    rmt::PinState::High,
                    &Duration::from_nanos(350),
                )?,
                t0l: rmt::Pulse::new_with_duration(
                    ticks_hz,
                    rmt::PinState::Low,
                    &Duration::from_nanos(800),
                )?,
                t1h: rmt::Pulse::new_with_duration(
                    ticks_hz,
                    rmt::PinState::High,
                    &Duration::from_nanos(700),
                )?,
                t1l: rmt::Pulse::new_with_duration(
                    ticks_hz,
                    rmt::PinState::Low,
                    &Duration::from_nanos(600),
                )?,
                rst: rmt::Pulse::new_with_duration(
                    ticks_hz,
                    rmt::PinState::Low,
                    &Duration::from_nanos(50),
                )?,
            };

            Ok(Self {
                pixels,
                tx,
                timings,
            })
        }

        fn encode_signal(
            timings: &Timings,
            pixels: &[Rgb],
        ) -> Result<VariableLengthSignal, anyhow::Error> {
            let mut signal = VariableLengthSignal::with_capacity(pixels.len() * 2 * 24 + 1);

            for pixel in pixels {
                let color = u32::from(pixel);
                for bit_idx in (0..24).rev() {
                    let bit = (color >> bit_idx) & 1 != 0;
                    let (high_pulse, low_pulse) = if bit {
                        (timings.t1h, timings.t1l)
                    } else {
                        (timings.t0h, timings.t0l)
                    };
                    signal.push(&[high_pulse, low_pulse])?;
                }
            }

            signal.push(&[timings.rst])?;

            Ok(signal)
        }

        pub fn transmit(&mut self, pixels: &[Rgb]) -> Result<()> {
            self.pixels.clear();
            self.pixels.extend_from_slice(pixels);

            let signal = Self::encode_signal(&self.timings, &self.pixels)?;

            let res = self.tx.start_blocking(&signal)?;

            Ok(res)
        }
    }

    #[derive(Clone, Copy)]
    pub struct Rgb {
        r: u8,
        g: u8,
        b: u8,
    }

    impl Rgb {
        pub const fn new(r: u8, g: u8, b: u8) -> Self {
            Self { r, g, b }
        }
        /// Converts hue, saturation, value to RGB
        pub fn from_hsv(h: u32, s: u32, v: u32) -> Result<Self, anyhow::Error> {
            if h > 360 || s > 100 || v > 100 {
                bail!("The given HSV values are not in valid range");
            }
            let s = s as f64 / 100.0;
            let v = v as f64 / 100.0;
            let c = s * v;
            let x = c * (1.0 - (((h as f64 / 60.0) % 2.0) - 1.0).abs());
            let m = v - c;
            let (r, g, b) = match h {
                0..=59 => (c, x, 0.0),
                60..=119 => (x, c, 0.0),
                120..=179 => (0.0, c, x),
                180..=239 => (0.0, x, c),
                240..=299 => (x, 0.0, c),
                _ => (c, 0.0, x),
            };
            Ok(Self {
                r: ((r + m) * 255.0) as u8,
                g: ((g + m) * 255.0) as u8,
                b: ((b + m) * 255.0) as u8,
            })
        }
    }

    impl From<&Rgb> for u32 {
        /// Convert RGB to u32 color value
        fn from(rgb: &Rgb) -> Self {
            ((rgb.g as u32) << 16) | ((rgb.r as u32) << 8) | rgb.b as u32
        }
    }
    impl From<Rgb> for u32 {
        fn from(rgb: Rgb) -> Self {
            (&rgb).into()
        }
    }

    #[derive(Clone, Copy)]
    pub struct Frame<const N: usize> {
        len: u16,
        data: [Rgb; N],
    }

    impl<const N: usize> Frame<N> {
        pub fn empty() -> Self {
            Self {
                len: 0,
                data: [Rgb::new(0, 0, 0); N],
            }
        }

        pub fn from_slice(pixels: &[Rgb]) -> Self {
            let mut frame = Self::empty();
            frame.fill_from_slice(pixels);
            frame
        }

        pub fn fill_from_slice(&mut self, pixels: &[Rgb]) {
            let copy_len = pixels.len().min(N);
            self.data[..copy_len].copy_from_slice(&pixels[..copy_len]);
            if copy_len < N {
                self.data[copy_len..].fill(Rgb::new(0, 0, 0));
            }
            self.len = copy_len as u16;
        }

        pub fn as_slice(&self) -> &[Rgb] {
            &self.data[..self.len as usize]
        }
    }
}
