use std::error::Error;
use std::net::IpAddr;
use maxminddb::geoip2;
use pnet::datalink;
use pnet::packet::ethernet::EthernetPacket;
use pnet::packet::ipv4::Ipv4Packet;
use pnet::packet::Packet;
use serde_json::json;
use tokio::sync::broadcast;

// GeoIP veritabanlarını yükle
struct GeoDatabases {
    city: maxminddb::Reader<Vec<u8>>,
    country: maxminddb::Reader<Vec<u8>>,
}

fn load_geoip_dbs() -> Result<GeoDatabases, Box<dyn Error>> {
    let city_reader = maxminddb::Reader::open_readfile("assets/GeoLite2-City.mmdb")
        .map_err(|e| format!("GeoIP City veritabanı yüklenemedi: {}", e))?;
    
    let country_reader = maxminddb::Reader::open_readfile("assets/GeoLite2-Country.mmdb")
        .map_err(|e| format!("GeoIP Country veritabanı yüklenemedi: {}", e))?;
    
    Ok(GeoDatabases {
        city: city_reader,
        country: country_reader,
    })
}

pub async fn start_packet_capture(tx: broadcast::Sender<String>) -> Result<(), Box<dyn Error>> {
    println!("GeoIP veritabanları yükleniyor...");
    let geoip_dbs = load_geoip_dbs()?;
    println!("GeoIP veritabanları başarıyla yüklendi");

    // Tüm arayüzleri listele
    let interfaces = datalink::interfaces();
    
    // Arayüzleri göster
    println!("Mevcut ağ arayüzleri:");
    for iface in &interfaces {
        println!("- {} ({})", iface.name, if iface.is_up() { "Aktif" } else { "Pasif" });
        println!("  Açıklama: {}", iface.description);
        if let Some(mac) = iface.mac {
            println!("  MAC: {}", mac);
        }
        if !iface.ips.is_empty() {
            println!("  IPs: {:?}", iface.ips);
        }
    }
    
    // Aktif ağ arayüzünü seç
    let interface = interfaces.iter()
        .find(|iface| {
            println!("Arayüz kontrol ediliyor: {}", iface.name);
            println!("  Açıklama: {}", iface.description);
            println!("  Aktif: {}", iface.is_up());
            println!("  IP'ler: {:?}", iface.ips);
            
            !iface.is_loopback() && 
            iface.is_up() &&  // Aktif olmalı
            !iface.ips.is_empty() &&  // IP adresi olmalı
            iface.name.starts_with("\\Device\\NPF_")
        })
        .ok_or("Kullanılabilir ağ arayüzü bulunamadı")?
        .clone();

    println!("\nSeçilen arayüz: {}", interface.name);
    println!("IP'ler: {:?}", interface.ips);

    // Arayüzü aç - promiscuous modu aktif et
    let config = pnet::datalink::Config {
        read_timeout: None,
        write_timeout: None,
        read_buffer_size: 65536,
        write_buffer_size: 65536,
        channel_type: pnet::datalink::ChannelType::Layer2,
        bpf_fd_attempts: 1000,
        linux_fanout: None,
        promiscuous: true,
    };

    let (_, mut rx) = match datalink::channel(&interface, config) {
        Ok(datalink::Channel::Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => return Err("Desteklenmeyen kanal türü".into()),
        Err(e) => return Err(e.into()),
    };

    println!("\nPaket yakalama başlatıldı...");

    // Test paketi gönder
    let test_packet = json!({
        "size": 100,
        "source_ip": "192.168.1.1",
        "dest_ip": "8.8.8.8",
        "source_location": {
            "latitude": 41.0082,
            "longitude": 28.9784,
            "country": "Turkey",
            "city": "Istanbul"
        },
        "dest_location": {
            "latitude": 37.4223,
            "longitude": -122.0847,
            "country": "United States",
            "city": "Mountain View"
        },
        "timestamp": chrono::Local::now().to_rfc3339()
    });

    println!("Test paketi gönderiliyor...");
    if let Err(e) = tx.send(test_packet.to_string()) {
        eprintln!("Test paketi gönderilemedi: {}", e);
    } else {
        println!("Test paketi gönderildi");
    }

    // Normal paket yakalamaya devam et
    loop {
        match rx.next() {
            Ok(packet) => {
                println!("Paket yakalandı: {} bytes", packet.len());
                // ... geri kalan kodlar aynı ...
            },
            Err(e) => {
                eprintln!("Paket yakalama hatası: {}", e);
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }
    }
}

// Özel IP aralıklarını kontrol et
fn is_private_ip(ip: &str) -> bool {
    if let Ok(addr) = ip.parse::<std::net::Ipv4Addr>() {
        if addr.is_loopback() || addr.is_link_local() {
            println!("Loopback/Link-local IP bulundu: {}", ip);
            return true;
        }
        if addr.is_private() {
            println!("Özel IP bulundu: {}", ip);
            return true;
        }
        if ip.starts_with("224.") || ip.starts_with("255.") {
            println!("Multicast/Broadcast IP bulundu: {}", ip);
            return true;
        }
        println!("Genel IP bulundu: {}", ip);
        false
    } else {
        println!("Geçersiz IP adresi: {}", ip);
        true
    }
}

// IP konumunu bul
fn get_location(dbs: &GeoDatabases, addr: IpAddr) -> Option<serde_json::Value> {
    if let Ok(city) = dbs.city.lookup::<geoip2::City>(addr) {
        if let (Some(lat), Some(lon)) = (city.location.as_ref().and_then(|l| l.latitude), 
                                       city.location.as_ref().and_then(|l| l.longitude)) {
            return Some(json!({
                "latitude": lat,
                "longitude": lon,
                "country": city.country.as_ref().and_then(|c| c.names.as_ref())
                            .and_then(|n| n.get("en")).unwrap_or(&"Unknown"),
                "city": city.city.as_ref().and_then(|c| c.names.as_ref())
                         .and_then(|n| n.get("en")).unwrap_or(&"Unknown")
            }));
        }
    }
    
    // City bulunamazsa country'yi dene
    if let Ok(country) = dbs.country.lookup::<geoip2::Country>(addr) {
        return Some(json!({
            "country": country.country.as_ref().and_then(|c| c.names.as_ref())
                      .and_then(|n| n.get("en")).unwrap_or(&"Unknown"),
            "city": "Unknown"
        }));
    }
    
    None
} 