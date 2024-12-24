mod capture;
mod websocket;

use tokio;
use crate::websocket::start_websocket_server;
use tokio::sync::broadcast;

#[tokio::main]
async fn main() {
    println!("Ağ trafiği görselleştirici başlatılıyor...");
    
    // Broadcast kanalı oluştur
    let (tx, _) = broadcast::channel(100);
    let tx_clone = tx.clone();

    // WebSocket sunucusunu başlat
    let ws_handle = tokio::spawn(async move {
        start_websocket_server(tx).await;
    });
    
    // Paket yakalamayı başlat
    let capture_handle = tokio::spawn(async move {
        if let Err(e) = capture::start_packet_capture(tx_clone).await {
            eprintln!("Paket yakalama hatası: {}", e);
        }
    });

    // Her iki task'ı da bekle
    tokio::try_join!(ws_handle, capture_handle).unwrap();
} 