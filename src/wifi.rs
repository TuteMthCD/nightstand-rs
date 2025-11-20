use std::{convert::TryInto, sync::mpsc::Sender};

use anyhow::{anyhow, Result};
use embedded_svc::{
    http::{Headers, Method},
    io::{Read, Write},
};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::{delay::FreeRtos, modem::Modem},
    http::server::{Configuration as HttpServerConfig, EspHttpServer},
    nvs::EspDefaultNvsPartition,
    wifi::{BlockingWifi, ClientConfiguration, Configuration, EspWifi},
};
use log::{info, warn};
use serde::Deserialize;

use crate::ws2812::neopixel::Rgb;

const MAX_PARAM_LEN: usize = 512;
const HTTP_STACK_SIZE: usize = 8192;

pub fn connect_wifi(
    modem: Modem,
    ssid: &'static str,
    password: &'static str,
    pixel_sender: Sender<Vec<Rgb>>,
) -> Result<()> {
    let sysloop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    let mut wifi = BlockingWifi::wrap(EspWifi::new(modem, sysloop.clone(), Some(nvs))?, sysloop)?;

    let client_config = Configuration::Client(ClientConfiguration {
        ssid: ssid
            .try_into()
            .map_err(|_| anyhow!("SSID is too long for the Wi-Fi driver"))?,
        password: password
            .try_into()
            .map_err(|_| anyhow!("Password is too long for the Wi-Fi driver"))?,
        ..Default::default()
    });

    wifi.set_configuration(&client_config)?;
    wifi.start()?;
    wifi.connect()?;
    wifi.wait_netif_up()?;

    info!("Wi-Fi connected to SSID: {ssid}");

    let mut server = create_http_server()?;
    register_http_handlers(&mut server, pixel_sender)?;

    info!("HTTP control server ready on port 80");

    loop {
        FreeRtos::delay_ms(1000);
    }
}

fn create_http_server() -> Result<EspHttpServer<'static>> {
    let config = HttpServerConfig {
        stack_size: HTTP_STACK_SIZE,
        ..Default::default()
    };

    Ok(EspHttpServer::new(&config)?)
}

fn register_http_handlers(
    server: &mut EspHttpServer<'_>,
    pixel_sender: Sender<Vec<Rgb>>,
) -> Result<()> {
    server.fn_handler::<anyhow::Error, _>("/", Method::Get, |req| {
        req.into_ok_response()?.write_all(b"Nightstand online")?;
        Ok(())
    })?;

    let params_sender = pixel_sender.clone();

    server.fn_handler::<anyhow::Error, _>("/params", Method::Post, move |mut req| {
        let mut payload = Vec::new();

        if let Some(len) = req.content_len().map(|len| len as usize) {
            if len > MAX_PARAM_LEN {
                req.into_status_response(413)? //Content Too Large
                    .write_all(b"Payload too large")?;
                return Ok(());
            }
            payload.resize(len, 0);
            req.read_exact(&mut payload)?;
        } else {
            let mut chunk = [0u8; 128];
            loop {
                let read = req.read(&mut chunk)?;
                if read == 0 {
                    break;
                }
                if payload.len() + read > MAX_PARAM_LEN {
                    req.into_status_response(413)?
                        .write_all(b"Payload too large")?;
                    return Ok(());
                }
                payload.extend_from_slice(&chunk[..read]);
            }
        }

        match core::str::from_utf8(&payload) {
            Ok(body) => {
                info!("Received params payload len {}", body.len());
                match parse_pixels(body) {
                    Ok(pixels) => {
                        if let Err(err) = params_sender.send(pixels) {
                            warn!("Pixel queue disconnected: {err:?}");
                            req.into_status_response(500)?
                                .write_all(b"Pixel queue unavailable")?;
                            return Ok(());
                        }
                    }
                    Err(err) => {
                        warn!("Invalid pixel payload: {err:?}");
                        req.into_status_response(400)?
                            .write_all(b"Invalid pixel payload")?;
                        return Ok(());
                    }
                }
            }
            Err(err) => {
                warn!("Received non UTF-8 payload: {err:?}");
                req.into_status_response(400)?
                    .write_all(b"Payload must be UTF-8")?;
                return Ok(());
            }
        }

        req.into_ok_response()?.write_all(b"{\"status\":\"ok\"}")?;

        Ok(())
    })?;

    Ok(())
}

#[derive(Deserialize)]
struct PixelInput {
    r: u8,
    g: u8,
    b: u8,
}

impl From<PixelInput> for Rgb {
    fn from(value: PixelInput) -> Self {
        Rgb::new(value.r, value.g, value.b)
    }
}

fn parse_pixels(body: &str) -> Result<Vec<Rgb>> {
    let parsed: Vec<PixelInput> = serde_json::from_str(body)?;
    Ok(parsed.into_iter().map(Into::into).collect())
}
