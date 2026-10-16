/// E220 register frame helpers.
///
/// Read command is `C1 00 09` (9 bytes from address 0). The reply is
/// `C1 00 09` plus those 9 bytes. The first 8 data bytes are ADDH through CRYPT_L.
/// Write command is `C0 00 08` plus those 8 bytes.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct E220Config {
    pub address: u16,
    pub uart_baud: String,
    pub uart_parity: String,
    pub air_data_rate: String,
    pub sub_packet: String,
    pub ambient_noise: bool,
    pub tx_power: String,
    pub channel: u8,
    pub rssi_byte: bool,
    pub fixed: bool,
    pub lbt: bool,
    pub wor_cycle: String,
    pub crypto: u16,
}

impl Default for E220Config {
    fn default() -> Self {
        Self {
            address: 0,
            uart_baud: "9600".into(),
            uart_parity: "8N1".into(),
            air_data_rate: "2.4".into(),
            sub_packet: "200".into(),
            ambient_noise: false,
            tx_power: "22".into(),
            channel: 15,
            rssi_byte: false,
            fixed: false,
            lbt: false,
            wor_cycle: "2000".into(),
            crypto: 0,
        }
    }
}

const BAUD: [&str; 8] = [
    "1200", "2400", "4800", "9600", "19200", "38400", "57600", "115200",
];
const PARITY: [&str; 3] = ["8N1", "8O1", "8E1"];
const AIR: [&str; 6] = ["0.3", "1.2", "2.4", "4.8", "9.6", "19.2"];
const PACKET: [&str; 4] = ["200", "128", "64", "32"];
const POWER: [&str; 4] = ["22", "17", "13", "10"];
const WOR: [&str; 8] = [
    "500", "1000", "1500", "2000", "2500", "3000", "3500", "4000",
];

pub fn read_command() -> [u8; 3] {
    [0xC1, 0x00, 0x09]
}

pub fn parse_read_response(data: &[u8]) -> Result<E220Config, &'static str> {
    if data.len() < 12 || data[0] != 0xC1 || data[1] != 0x00 || data[2] != 0x09 {
        return Err("invalid reply");
    }
    decode_registers(&data[3..11])
}

pub fn encode_write(config: &E220Config) -> Result<Vec<u8>, &'static str> {
    if config.channel > 83 {
        return Err("channel");
    }
    let regs = encode_registers(config)?;
    let mut frame = vec![0xC0, 0x00, 0x08];
    frame.extend_from_slice(&regs);
    Ok(frame)
}

fn decode_registers(regs: &[u8]) -> Result<E220Config, &'static str> {
    if regs.len() < 8 {
        return Err("short");
    }
    let reg0 = regs[2];
    let reg1 = regs[3];
    let reg3 = regs[5];
    let baud_bits = ((reg0 >> 5) & 0x07) as usize;
    let parity_bits = ((reg0 >> 3) & 0x03) as usize;
    let air_bits = (reg0 & 0x07) as usize;
    let packet_bits = ((reg1 >> 6) & 0x03) as usize;
    let power_bits = (reg1 & 0x03) as usize;
    let wor_bits = (reg3 & 0x07) as usize;

    Ok(E220Config {
        address: u16::from_be_bytes([regs[0], regs[1]]),
        uart_baud: BAUD.get(baud_bits).unwrap_or(&"9600").to_string(),
        uart_parity: match parity_bits {
            1 => "8O1",
            2 => "8E1",
            _ => "8N1",
        }
        .to_string(),
        air_data_rate: AIR.get(air_bits.min(5)).unwrap_or(&"2.4").to_string(),
        sub_packet: PACKET.get(packet_bits).unwrap_or(&"200").to_string(),
        ambient_noise: ((reg1 >> 5) & 0x01) == 1,
        tx_power: POWER.get(power_bits).unwrap_or(&"22").to_string(),
        channel: regs[4],
        rssi_byte: ((reg3 >> 7) & 0x01) == 1,
        fixed: ((reg3 >> 6) & 0x01) == 1,
        lbt: ((reg3 >> 4) & 0x01) == 1,
        wor_cycle: WOR.get(wor_bits).unwrap_or(&"2000").to_string(),
        crypto: u16::from_be_bytes([regs[6], regs[7]]),
    })
}

fn encode_registers(config: &E220Config) -> Result<[u8; 8], &'static str> {
    let baud = index_of(&BAUD, &config.uart_baud).unwrap_or(3);
    let parity = index_of(&PARITY, &config.uart_parity).unwrap_or(0);
    let air = index_of(&AIR, &config.air_data_rate).unwrap_or(2);
    let packet = index_of(&PACKET, &config.sub_packet).unwrap_or(0);
    let power = index_of(&POWER, &config.tx_power).unwrap_or(0);
    let wor = index_of(&WOR, &config.wor_cycle).unwrap_or(3);

    let reg0 = ((baud as u8) << 5) | ((parity as u8) << 3) | (air as u8);
    let reg1 = ((packet as u8) << 6) | ((u8::from(config.ambient_noise)) << 5) | (power as u8);
    let reg3 = (u8::from(config.rssi_byte) << 7)
        | (u8::from(config.fixed) << 6)
        | (u8::from(config.lbt) << 4)
        | (wor as u8);
    let address = config.address.to_be_bytes();
    let crypto = config.crypto.to_be_bytes();

    Ok([
        address[0],
        address[1],
        reg0,
        reg1,
        config.channel,
        reg3,
        crypto[0],
        crypto[1],
    ])
}

fn index_of(options: &[&str], value: &str) -> Option<usize> {
    options.iter().position(|item| *item == value)
}

pub fn encode_payload(text: &str, hex: bool, crlf: bool) -> Result<Vec<u8>, &'static str> {
    let mut payload = if hex {
        hex_to_bytes(text)?
    } else {
        text.as_bytes().to_vec()
    };
    if payload.is_empty() && !crlf {
        return Err("empty");
    }
    if crlf {
        payload.extend_from_slice(&[0x0D, 0x0A]);
    }
    if payload.is_empty() {
        return Err("empty");
    }
    Ok(payload)
}

pub fn hex_to_bytes(text: &str) -> Result<Vec<u8>, &'static str> {
    let clean: String = text.chars().filter(|ch| ch.is_ascii_hexdigit()).collect();
    if clean.is_empty() {
        return Err("empty");
    }
    if clean.len() % 2 != 0 {
        return Err("odd hex");
    }
    (0..clean.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&clean[index..index + 2], 16).map_err(|_| "hex"))
        .collect()
}

pub fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn bytes_to_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .chars()
        .map(|ch| {
            if ch == '\n' || ch == '\r' || ch == '\t' {
                ch
            } else if ch.is_control() {
                '.'
            } else {
                ch
            }
        })
        .collect()
}

pub fn format_address(address: u16) -> String {
    format!("0x{address:04X}")
}

pub fn parse_address(text: &str) -> Result<u16, &'static str> {
    let trimmed = text.trim();
    let hex = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);
    u16::from_str_radix(hex, 16).map_err(|_| "address")
}

pub fn parse_channel(text: &str) -> Result<u8, &'static str> {
    let channel: u16 = text.trim().parse().map_err(|_| "channel")?;
    if channel > 83 {
        return Err("channel");
    }
    Ok(channel as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_frame_matches_register_map() {
        let config = E220Config {
            address: 0x1234,
            uart_baud: "9600".into(),
            uart_parity: "8N1".into(),
            air_data_rate: "2.4".into(),
            sub_packet: "200".into(),
            ambient_noise: false,
            tx_power: "22".into(),
            channel: 15,
            rssi_byte: false,
            fixed: false,
            lbt: false,
            wor_cycle: "2000".into(),
            crypto: 0xABCD,
        };
        assert_eq!(
            encode_write(&config).unwrap(),
            vec![0xC0, 0x00, 0x08, 0x12, 0x34, 0x62, 0x00, 0x0F, 0x03, 0xAB, 0xCD]
        );
    }

    #[test]
    fn read_response_round_trips() {
        let frame = [
            0xC1, 0x00, 0x09, 0x12, 0x34, 0x62, 0x00, 0x0F, 0x03, 0xAB, 0xCD, 0x00,
        ];
        let config = parse_read_response(&frame).unwrap();
        assert_eq!(config.address, 0x1234);
        assert_eq!(config.uart_baud, "9600");
        assert_eq!(config.air_data_rate, "2.4");
        assert_eq!(config.channel, 15);
        assert_eq!(config.crypto, 0xABCD);
        assert_eq!(encode_write(&config).unwrap()[3..], frame[3..11]);
    }

    #[test]
    fn hex_payload_rejects_odd_length_and_appends_crlf() {
        assert!(encode_payload("ABC", true, false).is_err());
        assert_eq!(
            encode_payload("0A 1B", true, true).unwrap(),
            vec![0x0A, 0x1B, 0x0D, 0x0A]
        );
    }
}
