use serde::{Deserialize, Serialize};
use serialport::{ClearBuffer, SerialPort};
use std::{fs, io::Write};
use std::time::Duration;
use tauri::Manager;

const LOG_SIZE: usize = 249;
const LOG_END: u8 = 0x5A;

#[derive(Serialize)]
pub struct SerialPortInfo {
  name: String,
  description: Option<String>,
}

#[derive(Serialize)]
pub struct CableLog {
  pin_count: u8,
  connections: Vec<[u8; 2]>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CableDefinition {
  id: u64,
  name: String,
  description: String,
  pin_count: u8,
  color: String,
  expected_connections: Vec<[u8; 2]>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
  baud_rate: u32,
  manual_port: String,
  auto_connect: bool,
  test_pull_mode: String,
}

fn app_data_file(app: &tauri::AppHandle, file_name: &str) -> Result<std::path::PathBuf, String> {
  let app_data_dir = app
    .path()
    .app_data_dir()
    .map_err(|error| format!("Unable to determine the application data directory: {error}"))?;
  fs::create_dir_all(&app_data_dir)
    .map_err(|error| format!("Unable to create the application data directory: {error}"))?;
  Ok(app_data_dir.join(file_name))
}

fn cable_definitions_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
  app_data_file(app, "cable-definitions.json")
}

#[tauri::command]
fn load_cable_definitions(app: tauri::AppHandle) -> Result<Vec<CableDefinition>, String> {
  let path = cable_definitions_path(&app)?;
  if !path.exists() {
    return Ok(Vec::new());
  }

  let contents = fs::read_to_string(&path)
    .map_err(|error| format!("Unable to read cable definitions: {error}"))?;
  serde_json::from_str(&contents)
    .map_err(|error| format!("Unable to parse cable definitions: {error}"))
}

#[tauri::command]
fn save_cable_definitions(
  app: tauri::AppHandle,
  definitions: Vec<CableDefinition>,
) -> Result<(), String> {
  let path = cable_definitions_path(&app)?;
  let contents = serde_json::to_string_pretty(&definitions)
    .map_err(|error| format!("Unable to serialize cable definitions: {error}"))?;
  fs::write(&path, contents).map_err(|error| format!("Unable to save cable definitions: {error}"))
}

#[tauri::command]
fn load_settings(app: tauri::AppHandle) -> Result<Option<AppSettings>, String> {
  let path = app_data_file(&app, "settings.json")?;
  if !path.exists() {
    return Ok(None);
  }

  let contents =
    fs::read_to_string(&path).map_err(|error| format!("Unable to read settings: {error}"))?;
  serde_json::from_str(&contents).map_err(|error| format!("Unable to parse settings: {error}"))
}

#[tauri::command]
fn save_settings(app: tauri::AppHandle, settings: AppSettings) -> Result<(), String> {
  let path = app_data_file(&app, "settings.json")?;
  let contents = serde_json::to_string_pretty(&settings)
    .map_err(|error| format!("Unable to serialize settings: {error}"))?;
  fs::write(&path, contents).map_err(|error| format!("Unable to save settings: {error}"))
}

#[tauri::command]
fn save_results_file(path: String, contents: String) -> Result<(), String> {
  fs::write(&path, contents).map_err(|error| format!("Unable to save results: {error}"))
}

#[tauri::command]
fn load_results_file(path: String) -> Result<String, String> {
  fs::read_to_string(&path).map_err(|error| format!("Unable to open results: {error}"))
}

#[tauri::command]
fn list_serial_ports() -> Result<Vec<SerialPortInfo>, String> {
  serialport::available_ports()
    .map(|ports| {
      ports
        .into_iter()
        .map(|port| SerialPortInfo {
          name: port.port_name,
          description: match port.port_type {
            serialport::SerialPortType::UsbPort(info) => info.product,
            _ => None,
          },
        })
        .collect()
    })
    .map_err(|error| format!("Unable to list serial ports: {error}"))
}

#[tauri::command]
fn fetch_logs(port_name: String, baud_rate: u32) -> Result<Vec<CableLog>, String> {
  let mut port = serialport::new(&port_name, baud_rate)
    .timeout(Duration::from_millis(250))
    .open()
    .map_err(|error| format!("Unable to open {port_name}: {error}"))?;

  port
    .write_all(&[0x2A, 0x00, 0x00, 0x3A])
    .and_then(|_| port.flush())
    .map_err(|error| format!("Unable to reset log mode: {error}"))?;

  std::thread::sleep(Duration::from_secs(2));
  port
    .clear(ClearBuffer::Input)
    .map_err(|error| format!("Unable to clear the reset response: {error}"))?;

  port
    .write_all(&[0x2A, 0x00, 0x3A])
    .and_then(|_| port.flush())
    .map_err(|error| format!("Unable to request logs: {error}"))?;

  let mut count = [0u8; 1];
  read_exact_with_timeout(&mut *port, &mut count)?;
  let total_logs = count[0] as usize;
  let mut logs = Vec::with_capacity(total_logs);

  for index in 0..total_logs {
    let mut raw_log = [0u8; LOG_SIZE];
    read_exact_with_timeout(&mut *port, &mut raw_log)?;

    if raw_log[LOG_SIZE - 1] != LOG_END {
      return Err(format!("Log {} has invalid end byte", index + 1));
    }

    let mut connections = Vec::new();
    for a_pin in 0..40 {
      for byte_offset in 0..6 {
        let byte = raw_log[1 + (a_pin * 6) + byte_offset];
        for bit in 0..8 {
          if byte & (1 << bit) != 0 {
            let b_pin = byte_offset * 8 + bit;
            if b_pin < 40 {
              connections.push([(a_pin + 1) as u8, (b_pin + 1) as u8]);
            }
          }
        }
      }
    }

    logs.push(CableLog {
      pin_count: raw_log[0],
      connections,
    });
  }

  Ok(logs)
}

#[tauri::command]
fn send_device_command(
  port_name: String,
  command_byte: u8,
  baud_rate: u32,
) -> Result<(), String> {
  let mut port = serialport::new(&port_name, baud_rate)
    .timeout(Duration::from_millis(250))
    .open()
    .map_err(|error| format!("Unable to open {port_name}: {error}"))?;

  port
    .write_all(&[0x0A, command_byte, 0x1A])
    .and_then(|_| port.flush())
    .map_err(|error| format!("Unable to send command: {error}"))
}

fn read_exact_with_timeout(port: &mut dyn SerialPort, buffer: &mut [u8]) -> Result<(), String> {
  let mut offset = 0;
  while offset < buffer.len() {
    match port.read(&mut buffer[offset..]) {
      Ok(0) => return Err("Serial port closed while reading logs".to_string()),
      Ok(bytes_read) => offset += bytes_read,
      Err(error) if error.kind() == std::io::ErrorKind::TimedOut => {
        return Err("Timed out waiting for the cable tester response".to_string());
      }
      Err(error) => return Err(format!("Unable to read logs: {error}")),
    }
  }
  Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![
      list_serial_ports,
      fetch_logs,
      send_device_command,
      load_cable_definitions,
      save_cable_definitions,
      load_settings,
      save_settings,
      save_results_file,
      load_results_file
    ])
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      app.handle().plugin(tauri_plugin_dialog::init())?;
      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while building tauri application");
}
