//! Weather forecasts from Open-Meteo (no API key), located by city name or
//! by IP address. Shared by the egui and Tauri editions.

use serde::Serialize;
use serde_json::Value;
use std::time::Duration;

#[derive(Clone, Debug, Default, Serialize)]
pub struct Weather {
    pub place: String,
    pub temp: f32,
    pub code: i32,
    pub is_day: bool,
    pub wind: f32,
    pub humidity: f32,
    pub hi: f32,
    pub lo: f32,
    pub hourly: Vec<(String, f32, i32)>,
    pub daily: Vec<(String, f32, f32, i32)>,
    pub unit: &'static str,
}

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(),
            b' ' => "+".into(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn get_json(url: &str) -> Result<Value, String> {
    ureq::get(url)
        .timeout(Duration::from_secs(15))
        .call()
        .map_err(|e| format!("Weather unavailable: {e}"))?
        .into_string()
        .map_err(|e| format!("Weather unavailable: {e}"))
        .and_then(|t| serde_json::from_str(&t).map_err(|e| format!("Weather unavailable: {e}")))
}

fn locate(city: &str) -> Result<(f64, f64, String), String> {
    if city.is_empty() {
        let v = get_json("http://ip-api.com/json/?fields=status,city,lat,lon")?;
        let lat = v["lat"].as_f64().ok_or("Could not find your location")?;
        let lon = v["lon"].as_f64().ok_or("Could not find your location")?;
        return Ok((lat, lon, v["city"].as_str().unwrap_or("Here").to_string()));
    }
    let v = get_json(&format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={}&count=1&language=en&format=json",
        encode(city)
    ))?;
    let first = v["results"].get(0).ok_or_else(|| format!("No place called “{city}”"))?;
    Ok((
        first["latitude"].as_f64().unwrap_or(0.0),
        first["longitude"].as_f64().unwrap_or(0.0),
        first["name"].as_str().unwrap_or(city).to_string(),
    ))
}

pub fn fetch(city: &str, fahrenheit: bool) -> Result<Weather, String> {
    let (lat, lon, place) = locate(city)?;
    let unit = if fahrenheit { "fahrenheit" } else { "celsius" };
    let v = get_json(&format!(
        "https://api.open-meteo.com/v1/forecast?latitude={lat:.4}&longitude={lon:.4}\
&current=temperature_2m,relative_humidity_2m,weather_code,wind_speed_10m,is_day\
&hourly=temperature_2m,weather_code&daily=weather_code,temperature_2m_max,temperature_2m_min\
&timezone=auto&forecast_days=6&temperature_unit={unit}"
    ))?;
    let cur = &v["current"];
    let f = |x: &Value| x.as_f64().unwrap_or(0.0) as f32;
    let now_time = cur["time"].as_str().unwrap_or("").to_string();
    let times = v["hourly"]["time"].as_array().cloned().unwrap_or_default();
    let temps = v["hourly"]["temperature_2m"].as_array().cloned().unwrap_or_default();
    let codes = v["hourly"]["weather_code"].as_array().cloned().unwrap_or_default();
    let start = times
        .iter()
        .position(|t| t.as_str().is_some_and(|t| t[..13.min(t.len())] >= now_time[..13.min(now_time.len())]))
        .unwrap_or(0);
    let hourly = (start + 1..(start + 7).min(times.len()))
        .filter_map(|i| {
            let t = times.get(i)?.as_str().unwrap_or("");
            Some((t.get(11..13).unwrap_or("").to_string(), f(temps.get(i)?), codes.get(i)?.as_i64().unwrap_or(0) as i32))
        })
        .collect();
    let dt = v["daily"]["time"].as_array().cloned().unwrap_or_default();
    let dmax = v["daily"]["temperature_2m_max"].as_array().cloned().unwrap_or_default();
    let dmin = v["daily"]["temperature_2m_min"].as_array().cloned().unwrap_or_default();
    let dcode = v["daily"]["weather_code"].as_array().cloned().unwrap_or_default();
    let daily = (0..dt.len().min(dmax.len()).min(dmin.len()).min(dcode.len()))
        .map(|i| {
            let label = dt[i]
                .as_str()
                .and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
                .map(|d| d.format("%a").to_string())
                .unwrap_or_default();
            (label, f(&dmax[i]), f(&dmin[i]), dcode[i].as_i64().unwrap_or(0) as i32)
        })
        .collect::<Vec<_>>();
    Ok(Weather {
        place,
        temp: f(&cur["temperature_2m"]),
        code: cur["weather_code"].as_i64().unwrap_or(0) as i32,
        is_day: cur["is_day"].as_i64().unwrap_or(1) == 1,
        wind: f(&cur["wind_speed_10m"]),
        humidity: f(&cur["relative_humidity_2m"]),
        hi: daily.first().map(|d| d.1).unwrap_or(0.0),
        lo: daily.first().map(|d| d.2).unwrap_or(0.0),
        hourly,
        daily,
        unit: if fahrenheit { "°F" } else { "°C" },
    })
}

pub fn describe(code: i32) -> &'static str {
    match code {
        0 => "Clear sky",
        1 => "Mainly clear",
        2 => "Partly cloudy",
        3 => "Overcast",
        45 | 48 => "Fog",
        51 | 53 | 55 => "Drizzle",
        56 | 57 => "Freezing drizzle",
        61 | 63 | 65 => "Rain",
        66 | 67 => "Freezing rain",
        71 | 73 | 75 | 77 => "Snow",
        80..=82 => "Rain showers",
        85 | 86 => "Snow showers",
        95 => "Thunderstorm",
        96 | 99 => "Thunderstorm, hail",
        _ => "—",
    }
}

