use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::alert::Alert;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::form::{Field, Form};
use gpui_kit::component::group_box::{GroupBox, GroupBoxVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::scroll::Scrollbar;
use gpui_kit::component::select::{Select, SelectState};
use gpui_kit::component::sidebar::{
    Sidebar, SidebarCollapsible, SidebarHeader, SidebarMenu, SidebarMenuItem,
};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::Selectable as _;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, IndexPath, Sizable as _, Theme, ThemeMode, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::e220::{self, E220Config};
use crate::i18n::{self, Copy, Lang};
use crate::serial::{self, PortInfo, SerialCmd, SerialEvent};
use crate::update;

const LOG_LIMIT: usize = 300;

const BAUD_RATES: &[&str] = &[
    "1200", "2400", "4800", "9600", "19200", "38400", "57600", "115200",
];
const PARITY: &[&str] = &["8N1", "8O1", "8E1"];
const AIR_RATES: &[&str] = &["0.3", "1.2", "2.4", "4.8", "9.6", "19.2"];
const PACKETS: &[&str] = &["200", "128", "64", "32"];
const POWERS: &[&str] = &["22", "17", "13", "10"];
const METHODS: &[&str] = &["transparent", "fixed"];
const WOR_CYCLES: &[&str] = &[
    "500", "1000", "1500", "2000", "2500", "3000", "3500", "4000",
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    Disconnected,
    Connecting,
    Connected,
}

#[derive(Clone, Copy)]
enum LogKind {
    Tx,
    Rx,
}

struct LogLine {
    kind: LogKind,
    at: String,
    body: String,
}

#[derive(Clone)]
enum Notice {
    Info(NoticeCode),
    Error(NoticeCode),
}

#[derive(Clone)]
enum NoticeCode {
    NotConnected,
    Sent,
    Written,
    ReadOk,
    Timeout,
    InvalidReply,
    OddHex,
    EmptyPayload,
    Reconnecting,
    UpdateReady(String),
    Detail(String),
}

struct PendingRead {
    id: u64,
    buf: Vec<u8>,
}

struct Session {
    path: String,
    cmd_tx: async_channel::Sender<SerialCmd>,
}

type Choices = SelectState<Vec<SharedString>>;

pub struct E220App {
    lang: Lang,
    tab: usize,
    status: Status,
    want_link: bool,
    auto_reconnect: bool,
    link_gen: u64,
    last_path: Option<String>,
    last_baud: u32,
    session: Option<Session>,
    ports: Vec<PortInfo>,
    notice: Option<Notice>,
    busy: bool,
    read_seq: u64,
    pending_read: Option<PendingRead>,
    pending_config: Option<E220Config>,
    normal_hex: bool,
    normal_crlf: bool,
    send_hex: bool,
    send_crlf: bool,
    ambient_noise: bool,
    rssi_byte: bool,
    lbt: bool,
    normal_logs: Vec<LogLine>,
    receive_logs: Vec<LogLine>,
    send_logs: Vec<LogLine>,
    normal_scroll: ScrollHandle,
    receive_scroll: ScrollHandle,
    send_scroll: ScrollHandle,
    settings_scroll: ScrollHandle,
    port_select: Entity<Choices>,
    baud_select: Entity<Choices>,
    uart_baud: Entity<Choices>,
    parity: Entity<Choices>,
    air_rate: Entity<Choices>,
    packet: Entity<Choices>,
    power: Entity<Choices>,
    method: Entity<Choices>,
    wor_cycle: Entity<Choices>,
    normal_input: Entity<InputState>,
    send_input: Entity<InputState>,
    address_input: Entity<InputState>,
    channel_input: Entity<InputState>,
    crypto_input: Entity<InputState>,
    event_tx: async_channel::Sender<SerialEvent>,
}

impl E220App {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (event_tx, event_rx) = async_channel::unbounded();
        let ports = serial::list_ports();
        let port_names = ports
            .iter()
            .map(|port| SharedString::from(port.name.clone()))
            .collect::<Vec<_>>();
        let port_index = (!port_names.is_empty()).then(IndexPath::default);

        let app = Self {
            lang: Lang::Ja,
            tab: 0,
            status: Status::Disconnected,
            want_link: false,
            auto_reconnect: true,
            link_gen: 0,
            last_path: None,
            last_baud: 9600,
            session: None,
            ports,
            notice: None,
            busy: false,
            read_seq: 0,
            pending_read: None,
            pending_config: None,
            normal_hex: false,
            normal_crlf: true,
            send_hex: false,
            send_crlf: true,
            ambient_noise: false,
            rssi_byte: false,
            lbt: false,
            normal_logs: Vec::new(),
            receive_logs: Vec::new(),
            send_logs: Vec::new(),
            normal_scroll: ScrollHandle::default(),
            receive_scroll: ScrollHandle::default(),
            send_scroll: ScrollHandle::default(),
            settings_scroll: ScrollHandle::default(),
            port_select: choices(port_names, port_index, window, cx),
            baud_select: choices(labels(BAUD_RATES), Some(IndexPath::default().row(3)), window, cx),
            uart_baud: choices(labels(BAUD_RATES), Some(IndexPath::default().row(3)), window, cx),
            parity: choices(labels(PARITY), Some(IndexPath::default()), window, cx),
            air_rate: choices(labels(AIR_RATES), Some(IndexPath::default().row(2)), window, cx),
            packet: choices(labels(PACKETS), Some(IndexPath::default()), window, cx),
            power: choices(labels(POWERS), Some(IndexPath::default()), window, cx),
            method: choices(labels(METHODS), Some(IndexPath::default()), window, cx),
            wor_cycle: choices(labels(WOR_CYCLES), Some(IndexPath::default().row(3)), window, cx),
            normal_input: line_input("", i18n::copy(Lang::Ja).placeholder_text, window, cx),
            send_input: line_input("", i18n::copy(Lang::Ja).placeholder_text, window, cx),
            address_input: line_input("0x0000", "0x0000", window, cx),
            channel_input: line_input("15", "0-83", window, cx),
            crypto_input: line_input("0x0000", "0x0000", window, cx),
            event_tx,
        };

        cx.spawn(async move |this, cx| {
            while let Ok(event) = event_rx.recv().await {
                if this
                    .update(cx, |this, cx| this.on_serial_event(event, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();

        let (update_tx, update_rx) = async_channel::bounded(1);
        std::thread::spawn(move || {
            if let Some(version) = update::download_latest() {
                update_tx.try_send(version).ok();
            }
        });
        cx.spawn(async move |this, cx| {
            if let Ok(version) = update_rx.recv().await {
                this.update(cx, |this, cx| {
                    this.notice = Some(Notice::Info(NoticeCode::UpdateReady(version)));
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();

        app
    }

    fn copy(&self) -> Copy {
        i18n::copy(self.lang)
    }

    fn set_lang(&mut self, lang: Lang, window: &mut Window, cx: &mut Context<Self>) {
        self.lang = lang;
        window.set_window_title(self.copy().title);
        self.sync_placeholders(window, cx);
        cx.notify();
    }

    fn sync_placeholders(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let copy = self.copy();
        let normal = if self.normal_hex {
            copy.placeholder_hex
        } else {
            copy.placeholder_text
        };
        let send = if self.send_hex {
            copy.placeholder_hex
        } else {
            copy.placeholder_text
        };
        self.normal_input
            .update(cx, |state, cx| state.set_placeholder(normal, window, cx));
        self.send_input
            .update(cx, |state, cx| state.set_placeholder(send, window, cx));
    }

    fn on_serial_event(&mut self, event: SerialEvent, cx: &mut Context<Self>) {
        match event {
            SerialEvent::Connected { gen } if gen == self.link_gen => {
                self.status = Status::Connected;
                self.notice = None;
            }
            SerialEvent::Data { gen, bytes } if gen == self.link_gen => {
                self.ingest(bytes, cx);
            }
            SerialEvent::Disconnected { gen, message } if gen == self.link_gen => {
                self.session = None;
                self.pending_read = None;
                self.busy = false;
                if self.want_link && self.auto_reconnect {
                    self.status = Status::Connecting;
                    self.notice = Some(Notice::Error(NoticeCode::Reconnecting));
                    self.schedule_reconnect(cx);
                } else {
                    self.want_link = false;
                    self.status = Status::Disconnected;
                    self.notice = Some(Notice::Error(NoticeCode::Detail(message)));
                }
            }
            _ => {}
        }
        cx.notify();
    }

    fn ingest(&mut self, bytes: Vec<u8>, cx: &mut Context<Self>) {
        if self.pending_read.is_some() {
            let done = {
                let pending = self.pending_read.as_mut().unwrap();
                pending.buf.extend_from_slice(&bytes);
                if pending.buf.len() >= 12 {
                    Some(pending.buf.clone())
                } else {
                    None
                }
            };
            if let Some(frame) = done {
                self.pending_read = None;
                self.busy = false;
                match e220::parse_read_response(&frame) {
                    Ok(config) => {
                        self.pending_config = Some(config);
                        self.notice = Some(Notice::Info(NoticeCode::ReadOk));
                        self.apply_pending_config(cx);
                    }
                    Err(_) => self.notice = Some(Notice::Error(NoticeCode::InvalidReply)),
                }
            }
            return;
        }

        let line = LogLine {
            kind: LogKind::Rx,
            at: timestamp(),
            body: format_bytes(&bytes),
        };
        if self.tab == 1 {
            push_log(&mut self.receive_logs, line);
            self.receive_scroll.scroll_to_bottom();
        } else {
            push_log(&mut self.normal_logs, line);
            self.normal_scroll.scroll_to_bottom();
        }
    }

    fn apply_pending_config(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            this.update_in(cx, |this, window, cx| {
                let Some(config) = this.pending_config.take() else {
                    return;
                };
                this.ambient_noise = config.ambient_noise;
                this.rssi_byte = config.rssi_byte;
                this.lbt = config.lbt;
                set_line(&this.address_input, &e220::format_address(config.address), window, cx);
                set_line(&this.channel_input, &config.channel.to_string(), window, cx);
                set_line(&this.crypto_input, &e220::format_address(config.crypto), window, cx);
                select_value(&this.uart_baud, &config.uart_baud, window, cx);
                select_value(&this.parity, &config.uart_parity, window, cx);
                select_value(&this.air_rate, &config.air_data_rate, window, cx);
                select_value(&this.packet, &config.sub_packet, window, cx);
                select_value(&this.power, &config.tx_power, window, cx);
                select_value(
                    &this.method,
                    if config.fixed { "fixed" } else { "transparent" },
                    window,
                    cx,
                );
                select_value(&this.wor_cycle, &config.wor_cycle, window, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn connect(&mut self, cx: &mut Context<Self>) {
        let Some(path) = selected(&self.port_select, cx) else {
            self.notice = Some(Notice::Error(NoticeCode::NotConnected));
            cx.notify();
            return;
        };
        let baud = selected(&self.baud_select, cx)
            .and_then(|value| value.parse().ok())
            .unwrap_or(9600);
        self.want_link = true;
        self.notice = None;
        self.open_link(path, baud, cx);
        cx.notify();
    }

    fn open_link(&mut self, path: String, baud: u32, _cx: &mut Context<Self>) {
        if let Some(session) = self.session.take() {
            let _ = session.cmd_tx.try_send(SerialCmd::Close);
        }
        self.link_gen = self.link_gen.wrapping_add(1);
        let gen = self.link_gen;
        let (cmd_tx, cmd_rx) = async_channel::unbounded();
        serial::spawn_session(gen, path.clone(), baud, cmd_rx, self.event_tx.clone());
        self.last_path = Some(path.clone());
        self.last_baud = baud;
        self.session = Some(Session { path, cmd_tx });
        self.status = Status::Connecting;
    }

    fn disconnect(&mut self, cx: &mut Context<Self>) {
        self.want_link = false;
        self.pending_read = None;
        self.busy = false;
        if let Some(session) = self.session.take() {
            let _ = session.cmd_tx.try_send(SerialCmd::Close);
        }
        self.status = Status::Disconnected;
        cx.notify();
    }

    fn schedule_reconnect(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.last_path.clone() else {
            return;
        };
        let baud = self.last_baud;
        let gen = self.link_gen;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(1)).await;
            this.update(cx, |this, cx| {
                if this.want_link && this.session.is_none() && this.link_gen == gen {
                    this.open_link(path, baud, cx);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn refresh_ports(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current = selected(&self.port_select, cx);
        self.ports = serial::list_ports();
        let items = self
            .ports
            .iter()
            .map(|port| SharedString::from(port.name.clone()))
            .collect::<Vec<_>>();
        self.port_select.update(cx, |state, cx| {
            state.set_items(items, window, cx);
            if let Some(current) = current.as_ref() {
                state.set_selected_value(&SharedString::from(current.clone()), window, cx);
            }
        });
        cx.notify();
    }

    fn send_normal(&mut self, cx: &mut Context<Self>) {
        let input = self.normal_input.clone();
        let hex = self.normal_hex;
        let crlf = self.normal_crlf;
        self.send_from(&input, hex, crlf, true, cx);
    }

    fn send_wor(&mut self, cx: &mut Context<Self>) {
        let input = self.send_input.clone();
        let hex = self.send_hex;
        let crlf = self.send_crlf;
        self.send_from(&input, hex, crlf, false, cx);
    }

    fn send_from(
        &mut self,
        input: &Entity<InputState>,
        hex: bool,
        crlf: bool,
        normal_log: bool,
        cx: &mut Context<Self>,
    ) {
        let text = input.read(cx).value().to_string();
        if self.session.is_none() {
            self.notice = Some(Notice::Error(NoticeCode::NotConnected));
            cx.notify();
            return;
        }
        let payload = match e220::encode_payload(text.trim(), hex, crlf) {
            Ok(payload) => payload,
            Err("odd hex") => {
                self.notice = Some(Notice::Error(NoticeCode::OddHex));
                cx.notify();
                return;
            }
            Err(_) => {
                self.notice = Some(Notice::Error(NoticeCode::EmptyPayload));
                cx.notify();
                return;
            }
        };
        if !self.write_bytes(&payload) {
            self.notice = Some(Notice::Error(NoticeCode::NotConnected));
            cx.notify();
            return;
        }
        let line = LogLine {
            kind: LogKind::Tx,
            at: timestamp(),
            body: format_bytes(&payload),
        };
        if normal_log {
            push_log(&mut self.normal_logs, line);
            self.normal_scroll.scroll_to_bottom();
        } else {
            push_log(&mut self.send_logs, line);
            self.send_scroll.scroll_to_bottom();
        }
        self.notice = Some(Notice::Info(NoticeCode::Sent));
        cx.notify();
    }

    fn read_settings(&mut self, cx: &mut Context<Self>) {
        if self.session.is_none() {
            self.notice = Some(Notice::Error(NoticeCode::NotConnected));
            cx.notify();
            return;
        }
        self.read_seq = self.read_seq.wrapping_add(1);
        let id = self.read_seq;
        self.pending_read = Some(PendingRead {
            id,
            buf: Vec::new(),
        });
        self.busy = true;
        self.notice = None;
        if !self.write_bytes(&e220::read_command()) {
            self.pending_read = None;
            self.busy = false;
            self.notice = Some(Notice::Error(NoticeCode::NotConnected));
            cx.notify();
            return;
        }
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(3)).await;
            this.update(cx, |this, cx| {
                if this.pending_read.as_ref().is_some_and(|pending| pending.id == id) {
                    this.pending_read = None;
                    this.busy = false;
                    this.notice = Some(Notice::Error(NoticeCode::Timeout));
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn write_settings(&mut self, cx: &mut Context<Self>) {
        if self.session.is_none() {
            self.notice = Some(Notice::Error(NoticeCode::NotConnected));
            cx.notify();
            return;
        }
        let config = match self.collect_config(cx) {
            Ok(config) => config,
            Err(code) => {
                self.notice = Some(Notice::Error(code));
                cx.notify();
                return;
            }
        };
        let frame = match e220::encode_write(&config) {
            Ok(frame) => frame,
            Err(_) => {
                self.notice = Some(Notice::Error(NoticeCode::Detail("channel 0-83".into())));
                cx.notify();
                return;
            }
        };
        self.busy = true;
        let wrote = self.write_bytes(&frame);
        self.busy = false;
        self.notice = Some(if wrote {
            Notice::Info(NoticeCode::Written)
        } else {
            Notice::Error(NoticeCode::NotConnected)
        });
        cx.notify();
    }

    fn collect_config(&self, cx: &App) -> Result<E220Config, NoticeCode> {
        let address = e220::parse_address(&self.address_input.read(cx).value().to_string())
            .map_err(|_| NoticeCode::Detail("address".into()))?;
        let channel = e220::parse_channel(&self.channel_input.read(cx).value().to_string())
            .map_err(|_| NoticeCode::Detail("channel".into()))?;
        let crypto = e220::parse_address(&self.crypto_input.read(cx).value().to_string())
            .map_err(|_| NoticeCode::Detail("crypto".into()))?;
        Ok(E220Config {
            address,
            uart_baud: selected(&self.uart_baud, cx).unwrap_or_else(|| "9600".into()),
            uart_parity: selected(&self.parity, cx).unwrap_or_else(|| "8N1".into()),
            air_data_rate: selected(&self.air_rate, cx).unwrap_or_else(|| "2.4".into()),
            sub_packet: selected(&self.packet, cx).unwrap_or_else(|| "200".into()),
            ambient_noise: self.ambient_noise,
            tx_power: selected(&self.power, cx).unwrap_or_else(|| "22".into()),
            channel,
            rssi_byte: self.rssi_byte,
            fixed: selected(&self.method, cx).as_deref() == Some("fixed"),
            lbt: self.lbt,
            wor_cycle: selected(&self.wor_cycle, cx).unwrap_or_else(|| "2000".into()),
            crypto,
        })
    }

    fn write_bytes(&self, bytes: &[u8]) -> bool {
        self.session
            .as_ref()
            .and_then(|session| session.cmd_tx.try_send(SerialCmd::Write(bytes.to_vec())).ok())
            .is_some()
    }

    fn notice_text(&self, notice: &Notice) -> String {
        let copy = self.copy();
        let code = match notice {
            Notice::Info(code) | Notice::Error(code) => code,
        };
        match code {
            NoticeCode::NotConnected => copy.not_connected.into(),
            NoticeCode::Sent => copy.sent.into(),
            NoticeCode::Written => copy.written.into(),
            NoticeCode::ReadOk => copy.read_ok.into(),
            NoticeCode::Timeout => copy.timeout.into(),
            NoticeCode::InvalidReply => copy.invalid_reply.into(),
            NoticeCode::OddHex => copy.odd_hex.into(),
            NoticeCode::EmptyPayload => copy.empty_payload.into(),
            NoticeCode::Reconnecting => copy.reconnecting.into(),
            NoticeCode::UpdateReady(version) => copy.update_ready.replace("{version}", version),
            NoticeCode::Detail(text) => text.clone(),
        }
    }
}

impl Render for E220App {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let copy = self.copy();
        let this = cx.entity().downgrade();
        let notice = self.notice.clone().map(|notice| {
            let text = self.notice_text(&notice);
            let dismiss = this.clone();
            match notice {
                Notice::Info(_) => Alert::info("notice", text).on_close(move |_, _, cx| {
                    dismiss
                        .update(cx, |this, cx| {
                            this.notice = None;
                            cx.notify();
                        })
                        .ok();
                }),
                Notice::Error(_) => Alert::error("notice", text).on_close(move |_, _, cx| {
                    dismiss
                        .update(cx, |this, cx| {
                            this.notice = None;
                            cx.notify();
                        })
                        .ok();
                }),
            }
        });

        h_flex()
            .size_full()
            .items_start()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.sidebar(&copy, cx))
            .child(
                v_flex()
                    .flex_1()
                    .h_full()
                    .min_w_0()
                    .min_h_0()
                    .justify_start()
                    .when_some(notice, |this, notice| {
                        this.child(div().px_4().pt_3().child(notice))
                    })
                    .child(
                        v_flex()
                            .flex_1()
                            .min_h_0()
                            .w_full()
                            .justify_start()
                            .overflow_hidden()
                            .p_4()
                            .child(match self.tab {
                                0 => self.normal_page(&copy, cx).into_any_element(),
                                1 => self.wor_page(&copy, cx).into_any_element(),
                                _ => self.settings_page(&copy, cx).into_any_element(),
                            }),
                    ),
            )
    }
}

impl E220App {
    fn sidebar(&self, copy: &Copy, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity().downgrade();
        let theme = this.clone();
        let to_dark = copy.theme_to_dark;
        let to_light = copy.theme_to_light;
        let dark = cx.theme().is_dark();

        Sidebar::new("nav")
            .collapsible(SidebarCollapsible::None)
            .w(px(232.))
            .header(
                SidebarHeader::new().child(
                    v_flex()
                        .gap(px(2.))
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(copy.title.to_string()),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(copy.subtitle.to_string()),
                        ),
                ),
            )
            .child(
                SidebarMenu::new()
                    .child(nav_item(
                        copy.tab_normal,
                        IconName::Terminal,
                        self.tab == 0,
                        this.clone(),
                        0,
                    ))
                    .child(nav_item(
                        copy.nav_wor,
                        IconName::Radio,
                        self.tab == 1,
                        this.clone(),
                        1,
                    ))
                    .child(nav_item(
                        copy.tab_settings,
                        IconName::Settings,
                        self.tab == 2,
                        this.clone(),
                        2,
                    )),
            )
            .footer(
                v_flex()
                    .w(px(200.))
                    .gap_3()
                    .child(self.connection_panel(&copy, cx))
                    .child(
                        h_flex()
                            .w_full()
                            .justify_between()
                            .gap_2()
                            .child(
                                Button::new("theme")
                                    .icon(if dark { IconName::Sun } else { IconName::Moon })
                                    .ghost()
                                    .selected(dark)
                                    .tooltip(if dark { to_light } else { to_dark })
                                    .accessibility_label(if dark { to_light } else { to_dark })
                                    .on_click(move |_, _, cx| {
                                        let mode = if cx.theme().is_dark() {
                                            ThemeMode::Light
                                        } else {
                                            ThemeMode::Dark
                                        };
                                        Theme::change(mode, None, cx);
                                        theme.update(cx, |_, cx| cx.notify()).ok();
                                    }),
                            )
                            .child({
                                let japanese = self.lang == Lang::Ja;
                                let pick_ja = this.clone();
                                let pick_en = this.clone();
                                Button::new("lang")
                                    .icon(IconName::Globe)
                                    .ghost()
                                    .tooltip(copy.lang_button)
                                    .accessibility_label(copy.lang_button)
                                    .dropdown_menu_with_anchor(
                                        Anchor::BottomRight,
                                        move |menu, _, _| {
                                            let pick_ja = pick_ja.clone();
                                            let pick_en = pick_en.clone();
                                            menu.item(
                                                PopupMenuItem::new("日本語")
                                                    .checked(japanese)
                                                    .on_click(move |_, window, cx| {
                                                        pick_ja
                                                            .update(cx, |this, cx| {
                                                                this.set_lang(Lang::Ja, window, cx);
                                                            })
                                                            .ok();
                                                    }),
                                            )
                                            .item(
                                                PopupMenuItem::new("English")
                                                    .checked(!japanese)
                                                    .on_click(move |_, window, cx| {
                                                        pick_en
                                                            .update(cx, |this, cx| {
                                                                this.set_lang(Lang::En, window, cx);
                                                            })
                                                            .ok();
                                                    }),
                                            )
                                        },
                                    )
                            }),
                    ),
            )
    }

    fn connection_panel(&self, copy: &Copy, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity().downgrade();
        let connected = self.status == Status::Connected;
        let port = self.session.as_ref().and_then(|session| {
            self.ports.iter().find(|port| port.name == session.path)
        });
        let (dot, status) = match self.status {
            Status::Connected => (
                cx.theme().success,
                Tag::success().child(copy.connected),
            ),
            Status::Connecting => (
                cx.theme().warning,
                Tag::warning().child(copy.connecting),
            ),
            Status::Disconnected => (
                cx.theme().muted_foreground,
                Tag::secondary().child(copy.disconnected),
            ),
        };
        let mut panel = v_flex().w(px(200.)).gap_2().child(
            h_flex()
                .items_center()
                .gap_2()
                .child(div().size(px(8.)).rounded(px(99.)).bg(dot))
                .child(status),
        );
        if connected {
            let vid = serial::format_id(port.and_then(|port| port.vid));
            let pid = serial::format_id(port.and_then(|port| port.pid));
            let name = self
                .session
                .as_ref()
                .map(|session| session.path.clone())
                .unwrap_or_default();
            let disconnect = this.clone();
            panel = panel
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("{name}  VID {vid}  PID {pid}")),
                )
                .child(
                    Button::new("disconnect")
                        .label(copy.disconnect)
                        .danger()
                        .w_full()
                        .on_click(move |_, _, cx| {
                            disconnect.update(cx, |this, cx| this.disconnect(cx)).ok();
                        }),
                );
        } else {
            let refresh = this.clone();
            let connect = this.clone();
            let reconnect = this.clone();
            panel = panel
                .child(
                    Select::new(&self.port_select)
                        .w_full()
                        .accessibility_label(copy.port)
                        .placeholder(copy.no_ports),
                )
                .child(
                    Select::new(&self.baud_select)
                        .w_full()
                        .accessibility_label(copy.baud),
                )
                .child(
                    Switch::new("auto-reconnect")
                        .checked(self.auto_reconnect)
                        .label(copy.auto_reconnect)
                        .on_change(move |checked, _, cx| {
                            reconnect
                                .update(cx, |this, cx| {
                                    this.auto_reconnect = *checked;
                                    cx.notify();
                                })
                                .ok();
                        }),
                )
                .child(
                    h_flex()
                        .w_full()
                        .gap_2()
                        .child(
                            Button::new("refresh").label(copy.refresh).on_click(
                                move |_, window, cx| {
                                    refresh
                                        .update(cx, |this, cx| this.refresh_ports(window, cx))
                                        .ok();
                                },
                            ),
                        )
                        .child(
                            Button::new("connect")
                                .label(copy.connect)
                                .primary()
                                .disabled(self.ports.is_empty())
                                .on_click(move |_, _, cx| {
                                    connect.update(cx, |this, cx| this.connect(cx)).ok();
                                }),
                        ),
                );
        }
        panel
    }

    fn normal_page(&self, copy: &Copy, cx: &mut Context<Self>) -> impl IntoElement {
        let clear = cx.entity().downgrade();
        v_flex()
            .flex_1()
            .min_h_0()
            .w_full()
            .justify_start()
            .gap_3()
            .child(mode_line(None, copy.pins_normal, copy.normal_desc, cx))
            .child(log_view(
                "normal-log",
                copy,
                &self.normal_logs,
                &self.normal_scroll,
                move |_, _, cx| {
                    clear
                        .update(cx, |this, cx| {
                            this.normal_logs.clear();
                            cx.notify();
                        })
                        .ok();
                },
                cx,
            ))
            .child(self.send_dock(
                "normal",
                &self.normal_input,
                self.normal_hex,
                self.normal_crlf,
                true,
                copy,
                cx,
            ))
    }

    fn wor_page(&self, copy: &Copy, cx: &mut Context<Self>) -> impl IntoElement {
        let clear_rx = cx.entity().downgrade();
        let clear_tx = cx.entity().downgrade();
        v_flex()
            .flex_1()
            .min_h_0()
            .w_full()
            .justify_start()
            .gap_3()
            .child(
                h_flex()
                    .w_full()
                    .items_start()
                    .gap_4()
                    .child(
                        div().flex_1().min_w_0().child(mode_line(
                            Some(copy.receive_title),
                            copy.pins_rx,
                            copy.receive_note,
                            cx,
                        )),
                    )
                    .child(
                        div().flex_1().min_w_0().child(mode_line(
                            Some(copy.send_title),
                            copy.pins_tx,
                            copy.send_note,
                            cx,
                        )),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .gap_4()
                    .child(log_view(
                        "receive-log",
                        copy,
                        &self.receive_logs,
                        &self.receive_scroll,
                        move |_, _, cx| {
                            clear_rx
                                .update(cx, |this, cx| {
                                    this.receive_logs.clear();
                                    cx.notify();
                                })
                                .ok();
                        },
                        cx,
                    ))
                    .child(log_view(
                        "send-log",
                        copy,
                        &self.send_logs,
                        &self.send_scroll,
                        move |_, _, cx| {
                            clear_tx
                                .update(cx, |this, cx| {
                                    this.send_logs.clear();
                                    cx.notify();
                                })
                                .ok();
                        },
                        cx,
                    )),
            )
            .child(
                h_flex().w_full().items_start().gap_4().child(div().flex_1().min_w_0()).child(
                    v_flex().flex_1().min_w_0().child(self.send_dock(
                        "wor",
                        &self.send_input,
                        self.send_hex,
                        self.send_crlf,
                        false,
                        copy,
                        cx,
                    )),
                ),
            )
    }

    fn send_dock(
        &self,
        id: &'static str,
        input: &Entity<InputState>,
        hex: bool,
        crlf: bool,
        normal_log: bool,
        copy: &Copy,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let hex_target = cx.entity().downgrade();
        let crlf_target = cx.entity().downgrade();
        let send = cx.entity().downgrade();
        v_flex()
            .gap_2()
            .p_3()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().secondary)
            .child(
                h_flex()
                    .gap_4()
                    .items_center()
                    .child(
                        Switch::new(SharedString::from(format!("{id}-hex")))
                            .checked(hex)
                            .label(copy.hex)
                            .on_change(move |checked, window, cx| {
                                hex_target
                                    .update(cx, |this, cx| {
                                        if normal_log {
                                            this.normal_hex = *checked;
                                        } else {
                                            this.send_hex = *checked;
                                        }
                                        this.sync_placeholders(window, cx);
                                        cx.notify();
                                    })
                                    .ok();
                            }),
                    )
                    .child(
                        Switch::new(SharedString::from(format!("{id}-crlf")))
                            .checked(crlf)
                            .label(copy.crlf)
                            .on_change(move |checked, _, cx| {
                                crlf_target
                                    .update(cx, |this, cx| {
                                        if normal_log {
                                            this.normal_crlf = *checked;
                                        } else {
                                            this.send_crlf = *checked;
                                        }
                                        cx.notify();
                                    })
                                    .ok();
                            }),
                    ),
            )
            .child(
                h_flex().gap_2().items_center().child(
                    div().flex_1().child(Input::new(input)),
                )
                .child(
                    Button::new(SharedString::from(format!("{id}-send")))
                        .label(copy.send)
                        .primary()
                        .on_click(move |_, _, cx| {
                            send.update(cx, |this, cx| {
                                if normal_log {
                                    this.send_normal(cx);
                                } else {
                                    this.send_wor(cx);
                                }
                            })
                            .ok();
                        }),
                ),
            )
    }

    fn settings_page(&self, copy: &Copy, cx: &mut Context<Self>) -> impl IntoElement {
        let read = cx.entity().downgrade();
        let write = cx.entity().downgrade();
        let ambient = cx.entity().downgrade();
        let rssi = cx.entity().downgrade();
        let lbt = cx.entity().downgrade();

        v_flex()
            .flex_1()
            .min_h_0()
            .w_full()
            .justify_start()
            .gap_3()
            .child(
                h_flex()
                    .items_start()
                    .justify_between()
                    .gap_4()
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_2()
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(copy.settings_note_title.to_string()),
                                    )
                                    .child(Tag::warning().child(copy.pins_cfg)),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(copy.settings_note.to_string()),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("read-settings")
                                    .label(copy.read)
                                    .disabled(self.busy)
                                    .on_click(move |_, _, cx| {
                                        read.update(cx, |this, cx| this.read_settings(cx)).ok();
                                    }),
                            )
                            .child(
                                Button::new("write-settings")
                                    .label(copy.write)
                                    .primary()
                                    .disabled(self.busy)
                                    .on_click(move |_, _, cx| {
                                        write.update(cx, |this, cx| this.write_settings(cx)).ok();
                                    }),
                            ),
                    ),
            )
            .child(
                div()
                    .id("settings-scroll")
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.settings_scroll)
                    .child(
                        h_flex()
                            .w_full()
                            .items_start()
                            .gap_4()
                            .pb_4()
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_4()
                                    .child(group(
                                        "basic",
                                        copy.basic,
                                        Form::new()
                                            .child(setting_field(
                                                copy.address,
                                                copy.address_hint,
                                                Input::new(&self.address_input),
                                            ))
                                            .child(setting_field(
                                                copy.channel,
                                                copy.channel_hint,
                                                Input::new(&self.channel_input),
                                            ))
                                            .child(setting_field(
                                                copy.crypto,
                                                copy.address_hint,
                                                Input::new(&self.crypto_input),
                                            )),
                                    ))
                                    .child(group(
                                        "packet",
                                        copy.packet_power,
                                        Form::new()
                                            .child(setting_field(
                                                copy.packet,
                                                "",
                                                Select::new(&self.packet).w_full(),
                                            ))
                                            .child(setting_field(
                                                copy.ambient,
                                                "",
                                                Switch::new("ambient")
                                                    .checked(self.ambient_noise)
                                                    .on_change(move |checked, _, cx| {
                                                        ambient
                                                            .update(cx, |this, cx| {
                                                                this.ambient_noise = *checked;
                                                                cx.notify();
                                                            })
                                                            .ok();
                                                    }),
                                            ))
                                            .child(setting_field(
                                                copy.power,
                                                "",
                                                Select::new(&self.power).w_full(),
                                            )),
                                    )),
                            )
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_4()
                                    .child(group(
                                        "speed",
                                        copy.speed,
                                        Form::new()
                                            .child(setting_field(
                                                copy.uart_baud,
                                                "",
                                                Select::new(&self.uart_baud).w_full(),
                                            ))
                                            .child(setting_field(
                                                copy.parity,
                                                "",
                                                Select::new(&self.parity).w_full(),
                                            ))
                                            .child(setting_field(
                                                copy.air,
                                                "",
                                                Select::new(&self.air_rate).w_full(),
                                            )),
                                    ))
                                    .child(group(
                                        "advanced",
                                        copy.advanced,
                                        Form::new()
                                            .child(setting_field(
                                                copy.rssi,
                                                "",
                                                Switch::new("rssi-byte").checked(self.rssi_byte).on_change(
                                                    move |checked, _, cx| {
                                                        rssi.update(cx, |this, cx| {
                                                            this.rssi_byte = *checked;
                                                            cx.notify();
                                                        })
                                                        .ok();
                                                    },
                                                ),
                                            ))
                                            .child(setting_field(
                                                copy.method,
                                                "",
                                                Select::new(&self.method).w_full(),
                                            ))
                                            .child(setting_field(
                                                copy.lbt,
                                                "",
                                                Switch::new("lbt").checked(self.lbt).on_change(
                                                    move |checked, _, cx| {
                                                        lbt.update(cx, |this, cx| {
                                                            this.lbt = *checked;
                                                            cx.notify();
                                                        })
                                                        .ok();
                                                    },
                                                ),
                                            ))
                                            .child(setting_field(
                                                copy.wor,
                                                "",
                                                Select::new(&self.wor_cycle).w_full(),
                                            )),
                                    )),
                            ),
                    ),
            )
    }
}

fn choices(
    items: Vec<SharedString>,
    selected: Option<IndexPath>,
    window: &mut Window,
    cx: &mut Context<E220App>,
) -> Entity<Choices> {
    cx.new(|cx| SelectState::new(items, selected, window, cx))
}

fn labels(items: &[&str]) -> Vec<SharedString> {
    items.iter().copied().map(SharedString::from).collect()
}

fn line_input(
    value: &str,
    placeholder: &str,
    window: &mut Window,
    cx: &mut Context<E220App>,
) -> Entity<InputState> {
    cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder(placeholder)
            .default_value(value)
    })
}

fn set_line(input: &Entity<InputState>, value: &str, window: &mut Window, cx: &mut App) {
    input.update(cx, |state, cx| {
        state.set_value(value, window, cx);
    });
}

fn select_value(select: &Entity<Choices>, value: &str, window: &mut Window, cx: &mut App) {
    select.update(cx, |state, cx| {
        state.set_selected_value(&SharedString::from(value), window, cx);
    });
}

fn selected(select: &Entity<Choices>, cx: &App) -> Option<String> {
    select
        .read(cx)
        .selected_value()
        .map(|value| value.to_string())
}

fn nav_item(
    label: &str,
    icon: IconName,
    active: bool,
    this: WeakEntity<E220App>,
    index: usize,
) -> SidebarMenuItem {
    SidebarMenuItem::new(label.to_string())
        .icon(icon)
        .active(active)
        .on_click(move |_, _, cx| {
            this.update(cx, |this, cx| {
                this.tab = index;
                cx.notify();
            })
            .ok();
        })
}

fn mode_line(title: Option<&str>, pin: &str, note: &str, cx: &App) -> impl IntoElement {
    v_flex().gap_1().child(
        h_flex()
            .items_center()
            .gap_2()
            .when_some(title, |this, title| {
                this.child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title.to_string()),
                )
            })
            .child(Tag::secondary().child(pin.to_string())),
    )
    .child(
        div()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child(note.to_string()),
    )
}

fn group(id: &'static str, title: &str, form: Form) -> impl IntoElement {
    GroupBox::new()
        .id(id)
        .fill()
        .w_full()
        .title(title.to_string())
        .child(form)
}

fn setting_field(label: &str, hint: &str, control: impl IntoElement) -> Field {
    let field = Field::new().label(label.to_string()).child(control);
    if hint.is_empty() {
        field
    } else {
        field.description(hint.to_string())
    }
}

fn log_view(
    id: &'static str,
    copy: &Copy,
    logs: &[LogLine],
    scroll: &ScrollHandle,
    on_clear: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    v_flex()
        .flex_1()
        .h_full()
        .min_h_0()
        .w_full()
        .gap_2()
        .justify_start()
        .child(
            h_flex()
                .w_full()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(copy.log),
                )
                .child(
                    Button::new(SharedString::from(format!("{id}-clear")))
                        .label(copy.clear)
                        .small()
                        .ghost()
                        .on_click(on_clear),
                ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .flex_1()
                .h_full()
                .min_h_0()
                .w_full()
                .gap_2()
                .child(
                    div()
                        .id(id)
                        .flex_1()
                        .h_full()
                        .min_h_0()
                        .w_full()
                        .p_3()
                        .overflow_y_scroll()
                        .track_scroll(scroll)
                        .bg(cx.theme().muted)
                        .border_1()
                        .border_color(cx.theme().border)
                        .rounded(cx.theme().radius)
                        .font_family("Consolas")
                        .text_sm()
                        .child(if logs.is_empty() {
                            div()
                                .text_color(cx.theme().muted_foreground)
                                .child(copy.no_log)
                                .into_any_element()
                        } else {
                            v_flex()
                                .w_full()
                                .justify_start()
                                .gap_2()
                                .children(logs.iter().map(|line| log_line(line, cx)))
                                .into_any_element()
                        }),
                )
                .child(Scrollbar::vertical(scroll)),
        )
}

fn log_line(line: &LogLine, cx: &App) -> impl IntoElement {
    let (label, color) = match line.kind {
        LogKind::Tx => ("TX", cx.theme().info),
        LogKind::Rx => ("RX", cx.theme().success),
    };
    v_flex()
        .gap_1()
        .pb_2()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(
            h_flex()
                .gap_2()
                .child(div().text_color(color).child(label))
                .child(
                    div()
                        .text_color(cx.theme().muted_foreground)
                        .child(line.at.clone()),
                ),
        )
        .child(div().child(line.body.clone()))
}

fn format_bytes(bytes: &[u8]) -> String {
    format!(
        "HEX  {}\nTXT  {}",
        e220::bytes_to_hex(bytes),
        e220::bytes_to_text(bytes)
    )
}

fn push_log(logs: &mut Vec<LogLine>, line: LogLine) {
    logs.push(line);
    if logs.len() > LOG_LIMIT {
        let extra = logs.len() - LOG_LIMIT;
        logs.drain(0..extra);
    }
}

fn timestamp() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}
