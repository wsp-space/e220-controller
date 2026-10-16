use std::io::{ErrorKind, Read, Write};
use std::time::Duration;

use serialport::{SerialPortType, UsbPortInfo};

#[derive(Clone, Debug)]
pub struct PortInfo {
    pub name: String,
    pub vid: Option<u16>,
    pub pid: Option<u16>,
}

pub enum SerialCmd {
    Write(Vec<u8>),
    Close,
}

pub enum SerialEvent {
    Connected { gen: u64 },
    Data { gen: u64, bytes: Vec<u8> },
    Disconnected { gen: u64, message: String },
}

pub fn list_ports() -> Vec<PortInfo> {
    let mut ports = serialport::available_ports()
        .unwrap_or_default()
        .into_iter()
        .map(|port| {
            let (vid, pid) = match port.port_type {
                SerialPortType::UsbPort(UsbPortInfo { vid, pid, .. }) => (Some(vid), Some(pid)),
                _ => (None, None),
            };
            PortInfo {
                name: port.port_name,
                vid,
                pid,
            }
        })
        .collect::<Vec<_>>();
    ports.sort_by(|left, right| left.name.cmp(&right.name));
    ports
}

pub fn spawn_session(
    gen: u64,
    path: String,
    baud: u32,
    cmd_rx: async_channel::Receiver<SerialCmd>,
    event_tx: async_channel::Sender<SerialEvent>,
) {
    std::thread::spawn(move || {
        let mut port = match serialport::new(&path, baud)
            .timeout(Duration::from_millis(80))
            .open()
        {
            Ok(port) => port,
            Err(error) => {
                let _ = event_tx.send_blocking(SerialEvent::Disconnected {
                    gen,
                    message: error.to_string(),
                });
                return;
            }
        };

        let _ = event_tx.send_blocking(SerialEvent::Connected { gen });
        let mut buffer = [0_u8; 512];

        loop {
            match cmd_rx.try_recv() {
                Ok(SerialCmd::Write(bytes)) => {
                    if let Err(error) = port.write_all(&bytes).and_then(|_| port.flush()) {
                        let _ = event_tx.send_blocking(SerialEvent::Disconnected {
                            gen,
                            message: error.to_string(),
                        });
                        break;
                    }
                }
                Ok(SerialCmd::Close) | Err(async_channel::TryRecvError::Closed) => break,
                Err(async_channel::TryRecvError::Empty) => {}
            }

            match Read::read(&mut *port, &mut buffer) {
                Ok(0) => {}
                Ok(count) => {
                    let _ = event_tx.send_blocking(SerialEvent::Data {
                        gen,
                        bytes: buffer[..count].to_vec(),
                    });
                }
                Err(error) if error.kind() == ErrorKind::TimedOut => {}
                Err(error) => {
                    let _ = event_tx.send_blocking(SerialEvent::Disconnected {
                        gen,
                        message: error.to_string(),
                    });
                    break;
                }
            }
        }
    });
}

pub fn format_id(value: Option<u16>) -> String {
    match value {
        Some(value) => format!("0x{value:04X}"),
        None => "N/A".into(),
    }
}
