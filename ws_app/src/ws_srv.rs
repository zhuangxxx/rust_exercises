use std::sync::mpsc::{channel, Sender};

struct WebsocketServer {
    sender: ws::Sender,
    closer: Option<Sender<()>>,
}

impl ws::Handler for WebsocketServer {
    fn on_open(&mut self, shake: ws::Handshake) -> ws::Result<()> {
        if let Some(addr) = shake.remote_addr()? {
            tracing::info!("与客户端 {} 建立连接。", addr);
            let (tx, rx) = channel();
            self.closer = Some(tx);
            let sender = self.sender.clone();
            std::thread::spawn(move || -> ws::Result<()> {
                let time = std::time::SystemTime::now();
                loop {
                    if let Ok(_) = rx.try_recv() {
                        tracing::warn!("与客户端 {} 断开连接。", addr);
                        break sender.close(ws::CloseCode::Normal);
                    }
                    std::thread::park_timeout(std::time::Duration::from_millis(25));
                    tracing::debug!("发送消息。");
                    if let Err(_) = sender.send(format!(
                        "连接时长：{} ms",
                        time.elapsed().unwrap().as_millis()
                    )) {
                        tracing::warn!("与客户端 {} 断开连接。", addr);
                        break sender.close(ws::CloseCode::Normal);
                    }
                }
            });
        }

        Ok(())
    }

    fn on_message(&mut self, msg: ws::Message) -> ws::Result<()> {
        self.sender.send(format!("收到消息：{}", msg.as_text()?))
    }

    fn on_close(&mut self, code: ws::CloseCode, reason: &str) {
        tracing::warn!("连接关闭，原因：({:?}){}。", code, reason);
        if let Some(closer) = &self.closer {
            closer.send(()).ok();
        }
    }
}

fn main() -> ws::Result<()> {
    std::env::set_var("RUST_LOG", "debug");
    tracing_subscriber::fmt::init();

    ws::listen("127.0.0.1:6543", |sender| WebsocketServer {
        sender,
        closer: None,
    })
}
