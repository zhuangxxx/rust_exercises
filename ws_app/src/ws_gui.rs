#![windows_subsystem = "windows"]

use std::sync::{
    mpsc::{channel, Sender},
    Arc,
};

use crossbeam::atomic::AtomicCell;
use eframe::{
    egui::{self, Button, RichText},
    epaint::{mutex::Mutex, Color32},
};

#[derive(Clone)]
struct WsClient {
    sender: ws::Sender,
    messenger: Sender<Option<String>>,
}

impl ws::Handler for WsClient {
    fn on_open(&mut self, shake: ws::Handshake) -> ws::Result<()> {
        if let Some(addr) = shake.remote_addr()? {
            let log = format!("与服务器 {} 建立连接。", addr);
            tracing::info!(log);
            self.messenger.send(Some(log)).ok();
        }

        Ok(())
    }

    fn on_message(&mut self, msg: ws::Message) -> ws::Result<()> {
        self.messenger
            .send(Some(format!("收到服务器消息：{}", msg.as_text()?)))
            .ok();

        Ok(())
    }

    fn on_close(&mut self, _: ws::CloseCode, _: &str) {
        tracing::warn!("与服务器断开连接。");
        self.sender.close(ws::CloseCode::Normal).ok();
    }
}

#[derive(Default)]
struct RsGuiApp {
    url: String,
    // TODO: 待优化 无锁读写
    messages: Arc<Mutex<Vec<String>>>,
    client: Arc<AtomicCell<Option<WsClient>>>,
}

impl RsGuiApp {
    fn new(cc: &eframe::CreationContext) -> Self {
        let mut fonts = egui::FontDefinitions::default();

        if let Ok(font_data) = std::fs::read("C:\\Windows\\Fonts\\msyh.ttc") {
            fonts
                .font_data
                .insert("Font".to_owned(), egui::FontData::from_owned(font_data));
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "Font".to_owned());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push("Font".to_owned());

            cc.egui_ctx.set_fonts(fonts);
        }

        Self::default()
    }

    fn connect_client(&self) {
        if self.url.is_empty() {
            self.messages.lock().push(String::from("请输入连接地址！"));
            return;
        }
        if !self.url.starts_with("ws://") && !self.url.starts_with("wss://") {
            self.messages
                .lock()
                .push(String::from("请输入连接协议：ws:// 或 wss://！"));
            return;
        }

        let messages = Arc::clone(&self.messages);
        let client = Arc::clone(&self.client);
        if let Some(client) = client.take() {
            let log = String::from("建立新连接，与旧服务器断开连接。");
            tracing::warn!(log);
            self.messages.lock().push(log);
            client.sender.close(ws::CloseCode::Normal).ok();
            client.messenger.send(None).ok();
        }

        let url = self.url.clone();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            ws::connect(url, move |sender| {
                let ws_client = WsClient {
                    sender,
                    messenger: tx.clone(),
                };
                client.store(Some(ws_client.clone()));
                ws_client
            })
            .ok();
        });

        std::thread::spawn(move || loop {
            if let Ok(msg) = rx.recv() {
                if let Some(msg) = msg {
                    messages.lock().push(msg);
                } else {
                    break;
                }
            }
        });
    }

    fn close_client(&self) {
        if let Some(client) = self.client.take() {
            let log = String::from("与服务器断开连接。");
            tracing::warn!(log);
            self.messages.lock().push(log);
            client.sender.close(ws::CloseCode::Normal).ok();
            client.messenger.send(None).ok();
        }
    }
}

impl eframe::App for RsGuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                let label_id = ui.label("地址: ").id;
                ui.text_edit_singleline(&mut self.url).labelled_by(label_id);
                if ui
                    .add(
                        Button::new(RichText::new("连接").color(Color32::WHITE))
                            .fill(Color32::DARK_GREEN),
                    )
                    .clicked()
                {
                    self.connect_client();
                }
                if ui
                    .add(
                        Button::new(RichText::new("清屏").color(Color32::WHITE))
                            .fill(Color32::DARK_BLUE),
                    )
                    .clicked()
                {
                    self.messages.lock().clear();
                }
                if ui
                    .add(
                        Button::new(RichText::new("断开").color(Color32::WHITE))
                            .fill(Color32::DARK_RED),
                    )
                    .clicked()
                {
                    self.close_client();
                }
            });

            ui.horizontal(|ui| {
                ui.add_space(0.);
            });

            let messages = self.messages.lock();
            egui::ScrollArea::new([false, true])
                .auto_shrink([false; 2])
                .stick_to_bottom(true)
                .show_rows(
                    ui,
                    ui.text_style_height(&egui::TextStyle::Body),
                    messages.len(),
                    |ui, range| {
                        for i in range {
                            if messages[i].contains("成功") {
                                ui.label(
                                    RichText::new(messages[i].as_str()).color(Color32::LIGHT_GREEN),
                                );
                            } else if messages[i].contains("断开") || messages[i].contains("输入")
                            {
                                ui.label(
                                    RichText::new(messages[i].as_str()).color(Color32::LIGHT_RED),
                                );
                            } else {
                                ui.label(messages[i].as_str());
                            }
                        }

                        std::mem::drop(messages);
                    },
                );
        });
    }
}

fn main() {
    std::env::set_var("RUST_LOG", "debug");
    tracing_subscriber::fmt::init();

    eframe::run_native(
        "Rust Gui Application",
        eframe::NativeOptions {
            initial_window_size: Some(egui::vec2(480., 720.)),
            ..Default::default()
        },
        Box::new(|cc| Box::new(RsGuiApp::new(cc))),
    )
    .ok();
}
