use serde::Serialize;
use serialport::SerialPort;
use std::io::Write;
use std::sync::Mutex;
use std::time::Duration;

const BAUD_RATE: u32 = 38_400;
const COMMAND_BAUD_RATE: u32 = 115_200;
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

struct CommandPort(Mutex<Option<(String, Box<dyn SerialPort>)>>);

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
fn fetch_logs(port_name: String) -> Result<Vec<CableLog>, String> {
  let mut port = serialport::new(&port_name, BAUD_RATE)
    .timeout(Duration::from_millis(250))
    .open()
    .map_err(|error| format!("Unable to open {port_name}: {error}"))?;

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
fn send_button_command(
  state: tauri::State<'_, CommandPort>,
  port_name: String,
  command_byte: u8,
) -> Result<(), String> {
  let mut command_port = state
    .0
    .lock()
    .map_err(|_| "Command port state is unavailable".to_string())?;

  if command_port
    .as_ref()
    .map(|(active_port, _)| active_port != &port_name)
    .unwrap_or(true)
  {
    let port = serialport::new(&port_name, COMMAND_BAUD_RATE)
      .open()
      .map_err(|error| format!("Unable to open command port {port_name}: {error}"))?;
    *command_port = Some((port_name.clone(), port));
  }

  let (_, port) = command_port
    .as_mut()
    .ok_or_else(|| "Command port is not open".to_string())?;

  match port.write_all(&[0xA0, command_byte, 0xA1]) {
    Ok(()) => Ok(()),
    Err(error) => {
      *command_port = None;
      Err(format!("Unable to write button command to {port_name}: {error}"))
    }
  }
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
    .manage(CommandPort(Mutex::new(None)))
    .invoke_handler(tauri::generate_handler![list_serial_ports, fetch_logs, send_button_command])
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while building tauri application");
}
