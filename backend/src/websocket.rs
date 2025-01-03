use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_tungstenite::{
    accept_async,
    tungstenite::protocol::Message,
};
use futures::{StreamExt, SinkExt};
use serde_json::json;

pub async fn start_websocket_server(tx: broadcast::Sender<String>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = "127.0.0.1:8080";
    let listener = TcpListener::bind(addr).await?;
    println!("WebSocket sunucusu başlatıldı: {}", addr);

    while let Ok((stream, addr)) = listener.accept().await {
        println!("Yeni WebSocket bağlantısı: {}", addr);
        let tx = tx.clone();
        
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, tx).await {
                eprintln!("Bağlantı hatası {}: {}", addr, e);
            }
        });
    }

    Ok(())
}

async fn handle_connection(stream: TcpStream, tx: broadcast::Sender<String>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ws_stream = accept_async(stream).await?;
    println!("WebSocket el sıkışması tamamlandı");

    let (mut write, mut read) = ws_stream.split();
    let mut rx = tx.subscribe();

    // Başlangıç test mesajı
    let test_data = json!([{
        "source": {
            "ip": "8.8.8.8",  // Google DNS
            "port": 53,
            "latitude": 37.751,
            "longitude": -97.822
        },
        "destination": {
            "ip": "8.8.4.4",  // Google DNS
            "port": 53,
            "latitude": 37.751,
            "longitude": -97.822
        }
    }]);

    write.send(Message::Text(test_data.to_string())).await?;
    println!("Test verisi gönderildi");

    // İki task oluştur: biri okuma, diğeri yazma için
    let (tx1, mut rx1) = tokio::sync::mpsc::channel(32);
    
    // Okuma task'ı
    let read_task = tokio::spawn(async move {
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    println!("İstemciden mesaj alındı: {}", text);
                }
                Ok(Message::Close(_)) => {
                    println!("Bağlantı kapatma isteği alındı");
                    break;
                }
                Err(e) => {
                    println!("Okuma hatası: {}", e);
                    break;
                }
                _ => {}
            }
        }
    });

    // Yazma task'ı
    let write_task = tokio::spawn(async move {
        while let Some(msg) = rx1.recv().await {
            if let Err(e) = write.send(Message::Text(msg)).await {
                println!("Yazma hatası: {}", e);
                break;
            }
        }
    });

    // Broadcast kanalından gelen mesajları işle
    while let Ok(msg) = rx.recv().await {
        println!("Broadcast'ten mesaj alındı: {}", msg);
        tx1.send(msg).await?;
    }

    // Task'ları temizle
    read_task.abort();
    write_task.abort();

    println!("WebSocket bağlantısı kapandı");
    Ok(())
} 