use std::convert::TryInto;

use anyhow::{anyhow, Result};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::{delay::FreeRtos, modem::Modem},
    nvs::EspDefaultNvsPartition,
    wifi::{BlockingWifi, ClientConfiguration, Configuration, EspWifi},
};
use log::info;

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

    loop {
        FreeRtos::delay_ms(1000);
    }
}
