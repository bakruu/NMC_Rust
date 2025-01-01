use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_tungstenite::{
    accept_async,
    tungstenite::protocol::Message,
    tungstenite::handshake::server::{Request, Response},
};
use futures::{StreamExt, SinkExt};
use serde_json::json;

pub async fn start_websocket_server(tx: broadcast::Sender<String>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = "127.0.0.1:8080";
    let listener = TcpListener::bind(addr).await?;
    println!("WebSocket sunucusu başlatıldı: {}", addr);

    while let Ok((stream, addr)) = listener.accept().await {
        println!("Yeni bağlantı: {}", addr);
        let tx = tx.clone();
        tokio::spawn(async move {
            match handle_connection(stream, tx).await {
                Ok(_) => println!("Bağlantı normal şekilde kapandı: {}", addr),
                Err(e) => eprintln!("Bağlantı hatası {}: {}", addr, e),
            }
        });
    }

    Ok(())
}

async fn handle_connection(stream: TcpStream, tx: broadcast::Sender<String>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ws_stream = accept_async(stream).await?;
    println!("WebSocket bağlantısı kuruldu");

    let (mut write, mut read) = ws_stream.split();
    let mut rx = tx.subscribe();

    // İlk bağlantıda test verisi gönder
    let test_data = json!([{
        "source": {
            "ip": "192.168.1.1",
            "port": 8080,
            "latitude": 41.0082,
            "longitude": 28.9784
        },
        "destination": {
            "ip": "192.168.1.2",
            "port": 80,
            "latitude": 39.9334,
            "longitude": 32.8597
        }
    }]);

    write.send(Message::Text(test_data.to_string())).await?;
    println!("Test verisi gönderildi");

    // Paket verilerini dinle ve WebSocket üzerinden gönder
    let forward_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            println!("Gönderilen veri: {}", msg);
            if let Err(e) = write.send(Message::Text(msg)).await {
                println!("Mesaj gönderme hatası: {}", e);
                break;
            }
        }
    });

    // WebSocket mesajlarını dinle
    while let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                println!("Gelen mesaj: {}", text);
            }
            Ok(Message::Close(_)) => {
                println!("Bağlantı kapatma isteği alındı");
                break;
            }
            Err(e) => {
                println!("Hata: {}", e);
                break;
            }
            _ => {}
        }
    }

    forward_task.abort();
    println!("WebSocket bağlantısı kapandı");
    Ok(())
} 