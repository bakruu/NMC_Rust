use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{
    accept_async,
    tungstenite::{
        Message,
        handshake::server::{Request, Response},
        error::Error as WsError,
    },
};
use futures::{StreamExt, SinkExt};
use tokio::sync::broadcast;

pub async fn start_websocket_server(tx: broadcast::Sender<String>) {
    let addr = "0.0.0.0:8080";  // Tüm arayüzlerden bağlantı kabul et
    println!("WebSocket sunucusu {} adresinde başlatılıyor...", addr);
    
    let listener = match TcpListener::bind(addr).await {
        Ok(l) => {
            println!("WebSocket sunucusu başlatıldı");
            l
        },
        Err(e) => {
            eprintln!("WebSocket sunucusu başlatılamadı: {}", e);
            return;
        }
    };

    println!("WebSocket sunucusu bağlantıları dinliyor...");
    while let Ok((stream, addr)) = listener.accept().await {
        println!("Yeni bağlantı: {}", addr);
        let tx = tx.subscribe();
        tokio::spawn(handle_connection(stream, tx));
    }
}

async fn handle_connection(stream: TcpStream, mut rx: broadcast::Receiver<String>) {
    let ws_stream = match accept_async(stream).await {
        Ok(ws) => {
            println!("WebSocket bağlantısı başarıyla kuruldu");
            ws
        },
        Err(e) => {
            eprintln!("WebSocket bağlantısı kurulamadı: {}", e);
            return;
        }
    };

    let (mut write, mut read) = ws_stream.split();

    // Test mesajı gönder
    let test_msg = serde_json::json!({
        "type": "connection_test",
        "message": "WebSocket bağlantısı başarılı"
    }).to_string();

    if let Err(e) = write.send(Message::Text(test_msg)).await {
        eprintln!("Test mesajı gönderilemedi: {}", e);
        return;
    }

    // Gelen mesajları dinle
    let read_task = tokio::spawn(async move {
        while let Some(msg) = read.next().await {
            match msg {
                Ok(_) => println!("İstemciden mesaj alındı"),
                Err(e) => {
                    eprintln!("WebSocket okuma hatası: {}", e);
                    break;
                }
            }
        }
    });

    // Broadcast kanalından gelen mesajları gönder
    let write_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            println!("Mesaj gönderiliyor: {}", msg);
            if let Err(e) = write.send(Message::Text(msg)).await {
                eprintln!("WebSocket gönderme hatası: {}", e);
                break;
            }
        }
    });

    // Her iki task'ı da bekle
    tokio::select! {
        _ = read_task => println!("Okuma task'ı sonlandı"),
        _ = write_task => println!("Yazma task'ı sonlandı"),
    }
} 