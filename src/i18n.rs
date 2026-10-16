use std::sync::OnceLock;

use serde::Deserialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Ja,
    En,
}

macro_rules! locale_fields {
    ($($field:ident),* $(,)?) => {
        #[derive(Clone, Copy, Debug)]
        pub struct Copy {
            $(pub $field: &'static str,)*
        }

        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawCopy {
            $($field: String,)*
        }

        impl RawCopy {
            fn leak(self) -> Copy {
                Copy {
                    $($field: Box::leak(self.$field.into_boxed_str()),)*
                }
            }
        }
    };
}

locale_fields!(
    title,
    subtitle,
    nav_wor,
    pins_normal,
    pins_rx,
    pins_tx,
    pins_cfg,
    lang_button,
    connected,
    connecting,
    disconnected,
    port,
    baud,
    refresh,
    connect,
    disconnect,
    no_ports,
    tab_normal,
    tab_settings,
    log,
    clear,
    send,
    hex,
    crlf,
    placeholder_text,
    placeholder_hex,
    no_log,
    normal_desc,
    receive_title,
    receive_note,
    send_title,
    send_note,
    settings_note_title,
    settings_note,
    read,
    write,
    basic,
    address,
    address_hint,
    channel,
    channel_hint,
    crypto,
    speed,
    uart_baud,
    parity,
    air,
    packet_power,
    packet,
    ambient,
    power,
    advanced,
    rssi,
    method,
    lbt,
    wor,
    not_connected,
    sent,
    written,
    read_ok,
    timeout,
    invalid_reply,
    odd_hex,
    empty_payload,
    reconnecting,
    update_ready,
    auto_reconnect,
    theme_to_dark,
    theme_to_light,
);

pub fn copy(lang: Lang) -> Copy {
    *cached(lang)
}

fn cached(lang: Lang) -> &'static Copy {
    static JA: OnceLock<Copy> = OnceLock::new();
    static EN: OnceLock<Copy> = OnceLock::new();
    match lang {
        Lang::Ja => JA.get_or_init(|| load("locales/ja.toml", include_str!("../locales/ja.toml"))),
        Lang::En => EN.get_or_init(|| load("locales/en.toml", include_str!("../locales/en.toml"))),
    }
}

fn load(name: &str, text: &str) -> Copy {
    toml::from_str::<RawCopy>(text)
        .unwrap_or_else(|err| panic!("{name} is invalid: {err}"))
        .leak()
}

#[cfg(test)]
mod tests {
    use super::{Lang, copy};

    #[test]
    fn locales_parse() {
        let ja = copy(Lang::Ja);
        let en = copy(Lang::En);
        assert_eq!(ja.title, "E220 Controller");
        assert_eq!(en.tab_normal, "Console");
        assert_ne!(ja.connected, en.connected);
    }
}
