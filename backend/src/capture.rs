use std::error::Error;
use std::net::IpAddr;
use maxminddb::geoip2;
use pnet::datalink;
use pnet::packet::ethernet::EthernetPacket;
use pnet::packet::ipv4::Ipv4Packet;
use pnet::packet::Packet;
use serde_json::json;
use tokio::sync::broadcast;

// GeoIP veritabanını yükle
fn load_geoip_db() -> Result<maxminddb::Reader<Vec<u8>>, Box<dyn Error>> {
    let reader = maxminddb::Reader::open_readfile("GeoLite2-City.mmdb")?;
    Ok(reader)
}

pub async fn start_packet_capture(tx: broadcast::Sender<String>) -> Result<(), Box<dyn Error>> {
    // GeoIP veritabanını yükle
    let geoip_reader = load_geoip_db()?;
    
    // Tüm arayüzleri listele
    let interfaces = datalink::interfaces();
    
    // Arayüzleri göster
    println!("Mevcut ağ arayüzleri:");
    for iface in &interfaces {
        println!("- {} ({})", iface.name, if iface.is_up() { "Aktif" } else { "Pasif" });
        if let Some(mac) = iface.mac {
            println!("  MAC: {}", mac);
        }
        if !iface.ips.is_empty() {
            println!("  IPs: {:?}", iface.ips);
        }
    }
    
    // Çalışan bir arayüz bul
    let interface = interfaces.iter()
        .find(|iface| {
            // Windows'ta NPF ile başlayan ve loopback olmayan arayüzleri seç
            iface.name.starts_with("\\Device\\NPF_") && 
            !iface.name.contains("loopback")
        })
        .or_else(|| interfaces.iter().find(|iface| !iface.is_loopback()))
        .ok_or("Kullanılabilir ağ arayüzü bulunamadı")?
        .clone();

    println!("\nSeçilen ağ arayüzü: {}", interface.name);
    println!("Arayüz detayları:");
    println!("  MAC: {:?}", interface.mac);
    println!("  IP Adresleri: {:?}", interface.ips);

    // Arayüzü aç
    let (_, mut rx) = match datalink::channel(&interface, Default::default()) {
        Ok(datalink::Channel::Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => return Err("Desteklenmeyen kanal türü".into()),
        Err(e) => return Err(e.into()),
    };

    println!("\nPaket yakalama başlatıldı...");

    // Paketleri yakala
    loop {
        match rx.next() {
            Ok(packet) => {
                if let Some(ethernet) = EthernetPacket::new(packet) {
                    let mut source_ip = None;
                    let mut dest_ip = None;
                    let mut source_location = None;
                    let mut dest_location = None;

                    if ethernet.get_ethertype().0 == 0x0800 {
                        if let Some(ipv4) = Ipv4Packet::new(ethernet.payload()) {
                            let src_ip = ipv4.get_source().to_string();
                            let dst_ip = ipv4.get_destination().to_string();

                            // IP adreslerinin coğrafi konumlarını bul
                            if let Ok(src_addr) = src_ip.parse::<IpAddr>() {
                                if let Ok(city) = geoip_reader.lookup::<geoip2::City>(src_addr) {
                                    if let (Some(lat), Some(lon)) = (city.location.as_ref().and_then(|l| l.latitude), 
                                                                    city.location.as_ref().and_then(|l| l.longitude)) {
                                        source_location = Some(json!({
                                            "latitude": lat,
                                            "longitude": lon,
                                            "country": city.country.as_ref().and_then(|c| c.names.as_ref())
                                                          .and_then(|n| n.get("en")).unwrap_or(&"Unknown"),
                                            "city": city.city.as_ref().and_then(|c| c.names.as_ref())
                                                       .and_then(|n| n.get("en")).unwrap_or(&"Unknown")
                                        }));
                                    }
                                }
                            }

                            if let Ok(dst_addr) = dst_ip.parse::<IpAddr>() {
                                if let Ok(city) = geoip_reader.lookup::<geoip2::City>(dst_addr) {
                                    if let (Some(lat), Some(lon)) = (city.location.as_ref().and_then(|l| l.latitude), 
                                                                    city.location.as_ref().and_then(|l| l.longitude)) {
                                        dest_location = Some(json!({
                                            "latitude": lat,
                                            "longitude": lon,
                                            "country": city.country.as_ref().and_then(|c| c.names.as_ref())
                                                          .and_then(|n| n.get("en")).unwrap_or(&"Unknown"),
                                            "city": city.city.as_ref().and_then(|c| c.names.as_ref())
                                                       .and_then(|n| n.get("en")).unwrap_or(&"Unknown")
                                        }));
                                    }
                                }
                            }

                            source_ip = Some(src_ip);
                            dest_ip = Some(dst_ip);
                        }
                    }

                    let packet_info = json!({
                        "size": ethernet.packet().len(),
                        "source_mac": ethernet.get_source().to_string(),
                        "dest_mac": ethernet.get_destination().to_string(),
                        "type": format!("{:?}", ethernet.get_ethertype()),
                        "source_ip": source_ip,
                        "dest_ip": dest_ip,
                        "source_location": source_location,
                        "dest_location": dest_location,
                        "timestamp": chrono::Local::now().to_rfc3339()
                    });

                    // WebSocket üzerinden gönder
                    if let Err(e) = tx.send(packet_info.to_string()) {
                        eprintln!("WebSocket gönderme hatası: {}", e);
                    }

                    // Konsola da yazdır
                    println!("Paket yakalandı: {} bytes", ethernet.packet().len());
                    println!("Kaynak MAC: {}", ethernet.get_source());
                    println!("Hedef MAC: {}", ethernet.get_destination());
                    println!("Tip: {:?}", ethernet.get_ethertype());
                    if let Some(sip) = &source_ip {
                        println!("Kaynak IP: {}", sip);
                    }
                    if let Some(dip) = &dest_ip {
                        println!("Hedef IP: {}", dip);
                    }
                    println!("---");
                }
            },
            Err(e) => {
                eprintln!("Paket yakalama hatası: {}", e);
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }
    }
} 