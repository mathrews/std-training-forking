use anyhow::Result;
use core::str;
use embedded_svc::{http::Method, io::Write};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::{
        i2c::{I2cConfig, I2cDriver},
        prelude::*,
    },
    http::server::{Configuration, EspHttpServer},
};
use shtcx::{self, shtc3, PowerMode};
use std::{
    sync::{Arc, Mutex},
    thread::sleep,
    time::Duration,
};
use wifi::wifi;

#[toml_cfg::toml_config]
pub struct Config {
    #[default("")]
    wifi_ssid: &'static str,
    #[default("")]
    wifi_psk: &'static str,
}

fn main() -> Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take().unwrap();
    let sysloop = EspSystemEventLoop::take()?;

    // The constant `CONFIG` is auto-generated by `toml_config`.
    let app_config = CONFIG;

    // Connect to the Wi-Fi network
    let _wifi = wifi(
        app_config.wifi_ssid,
        app_config.wifi_psk,
        peripherals.modem,
        sysloop,
    )?;

    // Initialize temperature sensor
    let sda = peripherals.pins.gpio10;
    let scl = peripherals.pins.gpio8;
    let i2c = peripherals.i2c0;
    let config = I2cConfig::new().baudrate(100.kHz().into());
    let i2c = I2cDriver::new(i2c, sda, scl, &config)?;
    let temp_sensor_main = Arc::new(Mutex::new(shtc3(i2c)));
    #[allow(unused_mut)]
    let mut temp_sensor = temp_sensor_main.clone();
    temp_sensor
        .lock()
        .unwrap()
        .start_measurement(PowerMode::NormalMode)
        .unwrap();

    // Set the HTTP server
    let mut server = EspHttpServer::new(&Configuration::default())?;
    // http://<sta ip>/ handler
    server.fn_handler("/", Method::Get, |request| {
        let html = index_html();
        let mut response = request.into_ok_response()?;
        response.write_all(html.as_bytes())?;
        Ok(())
    })?;
    
    server.fn_handler("/temperature", Method::Get, |request| {
        let val_temp = temp_sensor.lock().unwrap().get_measurement_result().unwrap().temperature.as_degrees_celsius();
        let temperature = temperature(val_temp);
        let mut response = request.into_ok_response()?;
        response.write_all(temperature.as_bytes())?;
        Ok(())
    })?;

    server.fn_handler("*", Method::Get, |request| {
        let message = String::from("This URL does not exist");
        let mut response = request.into_ok_response()?;
        response.write_all(message.as_bytes())?;
        Ok(())
    })?;

    // This is not true until you actually create one
    println!("Server awaiting connection");

    // Prevent program from exiting
    loop {
        sleep(Duration::from_millis(1000));
    }
}

fn templated(content: impl AsRef<str>) -> String {
    format!(
        r#"
<!DOCTYPE html>
<html>
    <head>
        <meta charset="utf-8">
        <title>esp-rs web server</title>
    </head>
    <body>
        {}
    </body>
</html>
"#,
        content.as_ref()
    )
}

fn index_html() -> String {
    templated("Hello from mcu!")
}

fn temperature(val: f32) -> String {
    templated(format!("chip temperature: {:.2}°C", val))
}
