use std::convert::TryInto;

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

const MAX_PARAM_LEN: usize = 512;
const HTTP_STACK_SIZE: usize = 8192;

pub fn connect_wifi(modem: Modem, ssid: &'static str, password: &'static str) -> Result<()> {
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
    register_http_handlers(&mut server)?;

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

fn register_http_handlers(server: &mut EspHttpServer<'_>) -> Result<()> {
    server.fn_handler::<anyhow::Error, _>("/", Method::Get, |req| {
        req.into_ok_response()?.write_all(b"Nightstand online")?;
        Ok(())
    })?;

    server.fn_handler::<anyhow::Error, _>("/params", Method::Post, |mut req| {
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
            Ok(body) => info!("Received params payload: {body}"),
            Err(err) => warn!("Received non UTF-8 payload: {err:?}"),
        }

        req.into_ok_response()?.write_all(b"{\"status\":\"ok\"}")?;

        Ok(())
    })?;

    Ok(())
}
