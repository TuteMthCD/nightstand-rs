use std::{convert::TryInto, sync::mpsc::Sender};

use anyhow::{anyhow, Result};
use embedded_svc::{http::Method, io::Write, ws::FrameType};
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

    let params_sender = pixel_sender;

    server.ws_handler("/ws", move |ws| {
        if ws.is_new() {
            info!("WebSocket session {} opened", ws.session());
            ws.send(FrameType::Text(false), b"{\"status\":\"ready\"}")?;
            return Ok(());
        } else if ws.is_closed() {
            info!("WebSocket session {} closed", ws.session());
            return Ok(());
        }

        let (frame_type, raw_len) = match ws.recv(&mut []) {
            Ok(meta) => meta,
            Err(err) => {
                warn!("Failed to read WebSocket frame metadata: {err:?}");
                return Err(err);
            }
        };

        match frame_type {
            FrameType::Ping => {
                ws.send(FrameType::Pong, &[])?;
                return Ok(());
            }
            FrameType::Pong | FrameType::Close | FrameType::SocketClose => {
                return Ok(());
            }
            FrameType::Continue(_) => {
                warn!("Ignoring fragmented frame from session {}", ws.session());
                return Ok(());
            }
            FrameType::Binary(_) => {
                if raw_len > 0 {
                    let mut drain = vec![0u8; raw_len];
                    ws.recv(&mut drain)?;
                }
                warn!(
                    "Binary WebSocket frames not supported (session {})",
                    ws.session()
                );
                ws.send(
                    FrameType::Text(false),
                    b"{\"error\":\"binary_not_supported\"}",
                )?;
                return Ok(());
            }
            FrameType::Text(_) => {}
        }

        if raw_len == 0 {
            return Ok(());
        }

        let payload_len = raw_len.saturating_sub(1);

        if payload_len > MAX_PARAM_LEN {
            let mut discard = vec![0u8; raw_len];
            ws.recv(&mut discard)?;
            warn!("WebSocket payload too large: {} bytes", payload_len);
            ws.send(FrameType::Text(false), b"{\"error\":\"payload_too_large\"}")?;
            ws.send(FrameType::Close, &[])?;
            return Ok(());
        }

        let mut payload = vec![0u8; raw_len];
        ws.recv(&mut payload)?;

        let body = match core::str::from_utf8(&payload[..payload_len]) {
            Ok(body) => body,
            Err(err) => {
                warn!("Received non UTF-8 WebSocket payload: {err:?}");
                ws.send(FrameType::Text(false), b"{\"error\":\"invalid_utf8\"}")?;
                return Ok(());
            }
        };

        info!("Received WebSocket payload len {}", body.len());

        match parse_pixels(body) {
            Ok(pixels) => {
                if let Err(err) = params_sender.send(pixels) {
                    warn!("Pixel queue disconnected: {err:?}");
                    ws.send(
                        FrameType::Text(false),
                        b"{\"error\":\"pixel_queue_unavailable\"}",
                    )?;
                    ws.send(FrameType::Close, &[])?;
                    return Ok(());
                }
                ws.send(FrameType::Text(false), b"{\"status\":\"ok\"}")?;
            }
            Err(err) => {
                warn!("Invalid pixel payload: {err:?}");
                ws.send(FrameType::Text(false), b"{\"error\":\"invalid_payload\"}")?;
            }
        }

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
