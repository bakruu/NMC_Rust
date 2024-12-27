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
    println!("\nMevcut ağ arayüzleri:");
    for (idx, iface) in interfaces.iter().enumerate() {
        println!("{}. {} ({})", idx + 1, iface.name, if iface.is_up() { "Aktif" } else { "Pasif" });
        println!("   Açıklama: {}", iface.description);
        println!("   MAC: {:?}", iface.mac);
        println!("   IP'ler: {:?}", iface.ips);
        println!("   Flags: up={}, broadcast={}, loopback={}", 
            iface.is_up(), iface.is_broadcast(), iface.is_loopback());
        println!("---");
    }
    
    // Wi-Fi veya Ethernet arayüzünü seç
    let interface = interfaces.iter()
        .find(|iface| {
            let desc = iface.description.to_lowercase();
            iface.is_up() && 
            !iface.is_loopback() &&
            (desc.contains("wi-fi") || 
             desc.contains("wireless") || 
             desc.contains("ethernet") ||
             desc.contains("intel") ||
             desc.contains("realtek"))
        })
        .ok_or("Kullanılabilir ağ arayüzü bulunamadı")?
        .clone();

    println!("\nSeçilen arayüz:");
    println!("İsim: {}", interface.name);
    println!("Açıklama: {}", interface.description);
    println!("MAC: {:?}", interface.mac);
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

    println!("\nAğ arayüzü açılıyor...");
    let (_, mut rx) = match datalink::channel(&interface, config) {
        Ok(datalink::Channel::Ethernet(tx, rx)) => {
            println!("Ağ arayüzü başarıyla açıldı");
            (tx, rx)
        },
        Ok(_) => return Err("Desteklenmeyen kanal türü".into()),
        Err(e) => {
            println!("Ağ arayüzü açılamadı: {}", e);
            return Err(e.into())
        },
    };

    println!("\nPaket yakalama başlatıldı...");
    println!("Ağ trafiği izleniyor...\n");

    // Normal paket yakalamaya başla
    loop {
        match rx.next() {
            Ok(packet) => {
                if let Some(ethernet) = EthernetPacket::new(packet) {
                    if ethernet.get_ethertype().0 == 0x0800 {  // IPv4
                        if let Some(ipv4) = Ipv4Packet::new(ethernet.payload()) {
                            let src_ip = ipv4.get_source().to_string();
                            let dst_ip = ipv4.get_destination().to_string();

                            // Özel IP'leri filtrele
                            if !is_private_ip(&src_ip) || !is_private_ip(&dst_ip) {
                                println!("Dış IP paketi bulundu: {} -> {}", src_ip, dst_ip);

                                let mut source_location = None;
                                let mut dest_location = None;

                                // IP konumlarını bul
                                if let Ok(src_addr) = src_ip.parse::<IpAddr>() {
                                    source_location = get_location(&geoip_dbs, src_addr);
                                }
                                if let Ok(dst_addr) = dst_ip.parse::<IpAddr>() {
                                    dest_location = get_location(&geoip_dbs, dst_addr);
                                }

                                // Paket bilgilerini JSON'a çevir
                                let packet_info = json!({
                                    "size": ethernet.packet().len(),
                                    "source_mac": ethernet.get_source().to_string(),
                                    "dest_mac": ethernet.get_destination().to_string(),
                                    "source_ip": src_ip,
                                    "dest_ip": dst_ip,
                                    "source_location": source_location,
                                    "dest_location": dest_location,
                                    "timestamp": chrono::Local::now().to_rfc3339()
                                });

                                // WebSocket üzerinden gönder
                                println!("Paket gönderiliyor: {}", packet_info);
                                if let Err(e) = tx.send(packet_info.to_string()) {
                                    eprintln!("Paket gönderme hatası: {}", e);
                                } else {
                                    println!("Paket başarıyla gönderildi");
                                }
                            }
                        }
                    }
                }
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
            println!("Konum bulundu: {}, {} ({}, {})", 
                city.country.as_ref().and_then(|c| c.names.as_ref())
                    .and_then(|n| n.get("en")).unwrap_or(&"Unknown"),
                city.city.as_ref().and_then(|c| c.names.as_ref())
                    .and_then(|n| n.get("en")).unwrap_or(&"Unknown"),
                lat, lon);

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
    None
} 