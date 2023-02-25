#![windows_subsystem = "windows"]

use std::sync::{Arc, Mutex};

use crossbeam::atomic::AtomicCell;
use eframe::{
    egui::{self, Button, RichText},
    epaint::Color32,
};
use uuid::Uuid;

#[derive(Default)]
struct RsGuiApp {
    url: String,
    messages: Arc<Mutex<Vec<String>>>,
    client_id: Arc<AtomicCell<Uuid>>,
}

impl RsGuiApp {
    fn new(cc: &eframe::CreationContext) -> Self {
        let mut fonts = egui::FontDefinitions::default();

        fonts.font_data.insert(
            "宋体".to_owned(),
            egui::FontData::from_static(include_bytes!("simsun.ttc")),
        );
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "宋体".to_owned());
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .push("宋体".to_owned());

        cc.egui_ctx.set_fonts(fonts);

        Self::default()
    }
}

fn connect_client(app: &RsGuiApp) {
    if app.url.is_empty() {
        if let Ok(mut messages) = app.messages.lock() {
            messages.push(String::from("请输入连接地址！"));
        }
        return;
    }
    if !app.url.starts_with("ws://") && !app.url.starts_with("wss://") {
        if let Ok(mut messages) = app.messages.lock() {
            messages.push(String::from("请输入连接协议：ws:// 或 wss://！"));
        }
        return;
    }
    let url = app.url.clone();
    let messages = Arc::clone(&app.messages);
    let client_id = Arc::clone(&app.client_id);
    let thread_id = Uuid::new_v4();
    client_id.store(thread_id);
    std::thread::spawn(move || {
        if let Ok((mut socket, _)) = tungstenite::connect(&url) {
            if let Ok(mut messages) = messages.lock() {
                messages.push(format!("{} 成功连接到：{}！", thread_id.to_string(), &url));
            }

            loop {
                if client_id.load().ne(&thread_id) {
                    socket.close(None).ok();
                    if let Ok(mut messages) = messages.lock() {
                        messages.push(format!("{} 已断开 {} 连接！", thread_id.to_string(), &url));
                    }
                    break;
                }

                if let Ok(message) = socket.read_message() {
                    if let Ok(mut messages) = messages.lock() {
                        messages.push(format!(
                            "{} 收到消息：{}",
                            thread_id.to_string(),
                            message.to_text().unwrap()
                        ));
                    }
                }
            }

            tracing::info!("{} 已断开 {} 连接！", thread_id.to_string(), &url);
        } else {
            if let Ok(mut messages) = messages.lock() {
                messages.push(format!("{} 无法连接到：{}！", thread_id.to_string(), &url));
            }
        }
    });
}

fn close_client(app: &RsGuiApp) {
    app.client_id.store(Uuid::new_v4());
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
                    connect_client(&self);
                }
                if ui
                    .add(
                        Button::new(RichText::new("清屏").color(Color32::WHITE))
                            .fill(Color32::DARK_BLUE),
                    )
                    .clicked()
                {
                    if let Ok(mut messages) = self.messages.lock() {
                        messages.clear();
                    }
                }
                if ui
                    .add(
                        Button::new(RichText::new("断开").color(Color32::WHITE))
                            .fill(Color32::DARK_RED),
                    )
                    .clicked()
                {
                    close_client(&self);
                }
            });
            ui.horizontal(|ui| {
                ui.add_space(0.);
            });
            egui::ScrollArea::new([false, true])
                .auto_shrink([false; 2])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    if let Ok(messages) = self.messages.lock() {
                        for message in messages.iter() {
                            if message.contains("成功") {
                                ui.label(RichText::new(message).color(Color32::LIGHT_GREEN));
                            } else if message.contains("断开")
                                || message.contains("输入")
                                || message.contains("无法")
                            {
                                ui.label(RichText::new(message).color(Color32::LIGHT_RED));
                            } else {
                                ui.label(message);
                            }
                        }
                    }
                });
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

#[test]
fn server() -> anyhow::Result<()> {
    std::env::set_var("RUST_LOG", "debug");
    tracing_subscriber::fmt::init();

    let server = std::net::TcpListener::bind("127.0.0.1:3456")?;
    for stream in server.incoming() {
        let stream = stream?;
        std::thread::spawn(move || -> anyhow::Result<()> {
            let addr = stream.peer_addr()?;
            let time = std::time::SystemTime::now();
            let mut socket = tungstenite::accept(stream)?;
            tracing::info!("已与 {} 建立连接！", addr);
            loop {
                if let Err(_) = socket.write_message(tungstenite::Message::Text(format!(
                    "连接时间：{}",
                    time.elapsed()?.as_millis()
                ))) {
                    socket.close(None).ok();
                    tracing::warn!("客户端无响应，断开连接！");
                    break;
                }
                std::thread::park_timeout(std::time::Duration::from_millis(10));
            }

            Ok(())
        });
    }

    Ok(())
}
