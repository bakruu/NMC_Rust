use tokio;
mod capture;
mod websocket;

#[tokio::main]
async fn main() {
    println!("Uygulama başlatılıyor...");
    
    // Broadcast kanalı oluştur
    let (tx, _) = tokio::sync::broadcast::channel(100);
    let tx_ws = tx.clone();

    // WebSocket sunucusunu başlat
    let websocket_task = tokio::spawn(async move {
        println!("WebSocket sunucusu başlatılıyor...");
        websocket::start_websocket_server(tx_ws).await;
    });

    // Paket yakalamayı başlat
    let capture_task = tokio::spawn(async move {
        println!("Paket yakalama başlatılıyor...");
        if let Err(e) = capture::start_packet_capture(tx).await {
            eprintln!("Paket yakalama hatası: {}", e);
        }
    });

    // Her iki task'ı da bekle
    tokio::select! {
        _ = websocket_task => println!("WebSocket sunucusu durdu"),
        _ = capture_task => println!("Paket yakalama durdu"),
    }
} 