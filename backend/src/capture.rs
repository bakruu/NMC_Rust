use pnet::datalink::{self, NetworkInterface};
use pnet::packet::ip::IpNextHeaderProtocols;
use pnet::packet::ipv4::Ipv4Packet;
use pnet::packet::tcp::TcpPacket;
use pnet::packet::udp::UdpPacket;
use pnet::packet::Packet;
use tokio::sync::broadcast;
use serde_json::json;
use std::net::IpAddr;
use maxminddb::geoip2;
use std::sync::Arc;

fn is_interesting_connection(src_ip: IpAddr, dst_ip: IpAddr) -> bool {
    match (src_ip, dst_ip) {
        (IpAddr::V4(src), IpAddr::V4(dst)) => {
            // Yerel ağ IP'lerini filtrele
            if src.is_private() || dst.is_private() {
                return false;
            }

            // Loopback ve özel IP'leri filtrele
            if src.is_loopback() || dst.is_loopback() ||
               src.is_unspecified() || dst.is_unspecified() ||
               src.is_broadcast() || dst.is_broadcast() {
                return false;
            }

            // Çok kullanılan bazı portları filtrele (isteğe bağlı)
            // if src_port == 53 || dst_port == 53 {  // DNS
            //     return false;
            // }

            true
        },
        _ => false  // IPv6'yı şimdilik yok say
    }
}

pub async fn start_packet_capture(tx: broadcast::Sender<String>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // GeoIP veritabanı yolunu düzelt
    let reader = Arc::new(maxminddb::Reader::open_readfile("assets/GeoLite2-City.mmdb")
        .or_else(|_| maxminddb::Reader::open_readfile("../assets/GeoLite2-City.mmdb"))
        .or_else(|_| maxminddb::Reader::open_readfile("../../assets/GeoLite2-City.mmdb"))
        .map_err(|e| format!("GeoIP veritabanı yüklenemedi: {}", e))?);

    println!("GeoIP veritabanı başarıyla yüklendi");

    // Ağ arayüzlerini al
    let interfaces = datalink::interfaces();
    
    // İlk aktif arayüzü bul
    let interface = interfaces
        .into_iter()
        .find(|iface| iface.is_up() && !iface.is_loopback() && !iface.ips.is_empty())
        .ok_or("Aktif ağ arayüzü bulunamadı")?;

    println!("Seçilen arayüz: {}", interface.name);

    // Paket yakalayıcıyı oluştur
    let (_, mut rx) = match datalink::channel(&interface, Default::default()) {
        Ok(datalink::Channel::Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => return Err("Desteklenmeyen kanal türü".into()),
        Err(e) => return Err(e.into()),
    };

    println!("Paket yakalama başladı...");

    // Paketleri yakala
    loop {
        match rx.next() {
            Ok(packet) => {
                if let Some(ip_packet) = Ipv4Packet::new(packet) {
                    let src_ip = IpAddr::V4(ip_packet.get_source());
                    let dst_ip = IpAddr::V4(ip_packet.get_destination());

                    // Sadece ilginç bağlantıları işle
                    if !is_interesting_connection(src_ip, dst_ip) {
                        continue;
                    }

                    // Port bilgilerini al
                    let (src_port, dst_port) = match ip_packet.get_next_level_protocol() {
                        IpNextHeaderProtocols::Tcp => {
                            if let Some(tcp) = TcpPacket::new(ip_packet.payload()) {
                                (tcp.get_source(), tcp.get_destination())
                            } else {
                                continue;
                            }
                        },
                        IpNextHeaderProtocols::Udp => {
                            if let Some(udp) = UdpPacket::new(ip_packet.payload()) {
                                (udp.get_source(), udp.get_destination())
                            } else {
                                continue;
                            }
                        },
                        _ => continue,
                    };

                    // GeoIP sorguları
                    let src_location = match reader.lookup::<geoip2::City>(src_ip) {
                        Ok(city) => {
                            city.location.as_ref()
                                .and_then(|loc| {
                                    Some((
                                        loc.latitude.unwrap_or_default(),
                                        loc.longitude.unwrap_or_default()
                                    ))
                                })
                        },
                        Err(e) => {
                            println!("GeoIP hatası (kaynak): {} için {}", src_ip, e);
                            None
                        }
                    };

                    let dst_location = match reader.lookup::<geoip2::City>(dst_ip) {
                        Ok(city) => {
                            city.location.as_ref()
                                .and_then(|loc| {
                                    Some((
                                        loc.latitude.unwrap_or_default(),
                                        loc.longitude.unwrap_or_default()
                                    ))
                                })
                        },
                        Err(e) => {
                            println!("GeoIP hatası (hedef): {} için {}", dst_ip, e);
                            None
                        }
                    };

                    if let (Some((src_lat, src_lon)), Some((dst_lat, dst_lon))) = (src_location, dst_location) {
                        let connection = json!([{
                            "source": {
                                "ip": src_ip.to_string(),
                                "port": src_port,
                                "latitude": src_lat,
                                "longitude": src_lon
                            },
                            "destination": {
                                "ip": dst_ip.to_string(),
                                "port": dst_port,
                                "latitude": dst_lat,
                                "longitude": dst_lon
                            }
                        }]);

                        println!("Bağlantı: {}:{} -> {}:{}", src_ip, src_port, dst_ip, dst_port);
                        println!("Konumlar: ({}, {}) -> ({}, {})", src_lat, src_lon, dst_lat, dst_lon);

                        if let Err(e) = tx.send(connection.to_string()) {
                            eprintln!("Veri gönderme hatası: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Paket yakalama hatası: {}", e);
            }
        }
    }
} 