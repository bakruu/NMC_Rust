use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{
    accept_async,
    tungstenite::{Message, Error as WsError, handshake::server::{Request, Response}},
};
use futures::{StreamExt, SinkExt};
use tokio::sync::broadcast;
use std::net::SocketAddr;
use http::{HeaderValue, header};

async fn handle_websocket_upgrade(request: Request) -> Result<Response, WsError> {
    let mut response = Response::new(None);
    
    // CORS başlıklarını ekle
    let headers = response.headers_mut();
    headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, 
        HeaderValue::from_static("*"));
    headers.insert(header::ACCESS_CONTROL_ALLOW_METHODS, 
        HeaderValue::from_static("GET, POST, OPTIONS"));
    headers.insert(header::ACCESS_CONTROL_ALLOW_HEADERS, 
        HeaderValue::from_static("*"));
    
    Ok(response)
}

pub async fn start_websocket_server(tx: broadcast::Sender<String>) {
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));  // Tüm IP'lerden bağlantı kabul et
    println!("WebSocket sunucusu {} adresinde başlatılıyor...", addr);
    
    let try_socket = TcpListener::bind(&addr).await;
    let listener = match try_socket {
        Ok(l) => {
            println!("WebSocket sunucusu başarıyla başlatıldı");
            l
        },
        Err(e) => {
            eprintln!("WebSocket sunucusu başlatılamadı: {}", e);
            eprintln!("Port 8080 zaten kullanımda olabilir");
            return;
        }
    };

    println!("WebSocket bağlantıları bekleniyor...");

    while let Ok((stream, peer)) = listener.accept().await {
        println!("Yeni bağlantı isteği: {}", peer);
        let tx = tx.subscribe();
        
        tokio::spawn(async move {
            match handle_connection(stream, peer, tx).await {
                Ok(_) => println!("Bağlantı kapandı: {}", peer),
                Err(e) => eprintln!("Bağlantı hatası {}: {}", peer, e),
            }
        });
    }
}

async fn handle_connection(
    stream: TcpStream,
    peer: SocketAddr,
    mut rx: broadcast::Receiver<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let ws_stream = accept_async(stream).await?;
    println!("WebSocket el sıkışması tamamlandı: {}", peer);

    let (mut write, mut read) = ws_stream.split();

    // Test mesajı gönder
    let test_msg = serde_json::json!({
        "type": "connection_test",
        "message": "WebSocket bağlantısı başarılı",
        "timestamp": chrono::Local::now().to_rfc3339()
    }).to_string();

    write.send(Message::Text(test_msg)).await?;
    println!("Test mesajı gönderildi: {}", peer);

    // Gelen mesajları dinle
    let read_future = async move {
        while let Some(msg) = read.next().await {
            match msg {
                Ok(_) => println!("İstemciden mesaj alındı: {}", peer),
                Err(e) => {
                    eprintln!("WebSocket okuma hatası {}: {}", peer, e);
                    break;
                }
            }
        }
    };

    // Broadcast kanalından gelen mesajları gönder
    let write_future = async move {
        while let Ok(msg) = rx.recv().await {
            println!("Mesaj gönderiliyor -> {}: {}", peer, msg);
            if let Err(e) = write.send(Message::Text(msg)).await {
                eprintln!("WebSocket gönderme hatası {}: {}", peer, e);
                break;
            }
        }
    };

    // Her iki task'ı da çalıştır
    tokio::select! {
        _ = read_future => println!("Okuma task'ı sonlandı: {}", peer),
        _ = write_future => println!("Yazma task'ı sonlandı: {}", peer),
    }

    Ok(())
} 