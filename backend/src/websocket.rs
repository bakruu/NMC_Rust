use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use futures::StreamExt;
use futures::SinkExt;
use tokio::sync::broadcast;

pub async fn start_websocket_server(tx: broadcast::Sender<String>) {
    let addr = "127.0.0.1:8080";
    let listener = TcpListener::bind(addr).await.expect("WebSocket sunucusu başlatılamadı");
    println!("WebSocket sunucusu {} adresinde başlatıldı", addr);

    while let Ok((stream, _)) = listener.accept().await {
        let tx = tx.subscribe();
        tokio::spawn(handle_connection(stream, tx));
    }
}

async fn handle_connection(stream: TcpStream, mut rx: broadcast::Receiver<String>) {
    let ws_stream = accept_async(stream).await.expect("WebSocket bağlantısı kurulamadı");
    println!("Yeni WebSocket bağlantısı kabul edildi");

    let (mut write, _) = ws_stream.split();

    while let Ok(msg) = rx.recv().await {
        if let Err(e) = write.send(Message::Text(msg)).await {
            eprintln!("WebSocket gönderme hatası: {}", e);
            break;
        }
    }
} 