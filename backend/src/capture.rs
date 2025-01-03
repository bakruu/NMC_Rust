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
use std::collections::HashSet;
use std::time::{Duration, Instant};

struct ConnectionTracker {
    connections: HashSet<(String, String)>,
    last_cleanup: Instant,
}

impl ConnectionTracker {
    fn new() -> Self {
        Self {
            connections: HashSet::new(),
            last_cleanup: Instant::now(),
        }
    }

    fn is_new_connection(&mut self, src: &str, dst: &str) -> bool {
        if self.last_cleanup.elapsed() > Duration::from_secs(60) {
            self.connections.clear();
            self.last_cleanup = Instant::now();
        }
        self.connections.insert((src.to_string(), dst.to_string()))
    }
}

fn should_track_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            // Özel IP'leri filtrele ama DNS sunucularını kabul et
            if ip.is_loopback() || ip.is_broadcast() || ip.is_unspecified() {
                return false;
            }
            // DNS sunucularını kabul et
            if ip.to_string() == "8.8.8.8" || ip.to_string() == "8.8.4.4" {
                return true;
            }
            // Özel IP'leri reddet
            !ip.is_private()
        },
        _ => false
    }
}

pub async fn start_packet_capture(tx: broadcast::Sender<String>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let reader = Arc::new(maxminddb::Reader::open_readfile("assets/GeoLite2-City.mmdb")
        .or_else(|_| maxminddb::Reader::open_readfile("../assets/GeoLite2-City.mmdb"))
        .or_else(|_| maxminddb::Reader::open_readfile("../../assets/GeoLite2-City.mmdb"))
        .map_err(|e| format!("GeoIP veritabanı yüklenemedi: {}", e))?);

    println!("GeoIP veritabanı başarıyla yüklendi");

    let interfaces = datalink::interfaces();
    let interface = interfaces
        .into_iter()
        .find(|iface| iface.is_up() && !iface.is_loopback() && !iface.ips.is_empty())
        .ok_or("Aktif ağ arayüzü bulunamadı")?;

    println!("Seçilen ağ arayüzü: {}", interface.name);
    println!("IP adresleri: {:?}", interface.ips);

    let (_, mut rx) = match datalink::channel(&interface, Default::default()) {
        Ok(datalink::Channel::Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => return Err("Desteklenmeyen kanal türü".into()),
        Err(e) => return Err(e.into()),
    };

    println!("Paket yakalama başladı...");
    let mut tracker = ConnectionTracker::new();

    loop {
        match rx.next() {
            Ok(packet) => {
                if let Some(ip_packet) = Ipv4Packet::new(packet) {
                    let src_ip = IpAddr::V4(ip_packet.get_source());
                    let dst_ip = IpAddr::V4(ip_packet.get_destination());

                    // En az bir IP public olmalı
                    if !should_track_ip(src_ip) && !should_track_ip(dst_ip) {
                        continue;
                    }

                    // Aynı bağlantıyı tekrar gösterme
                    if !tracker.is_new_connection(&src_ip.to_string(), &dst_ip.to_string()) {
                        continue;
                    }

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
                            // Yerel IP için İstanbul koordinatları
                            if src_ip.to_string().starts_with("192.168.") {
                                Some((41.0082, 28.9784))
                            } else {
                                None
                            }
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
                            // Yerel IP için İstanbul koordinatları
                            if dst_ip.to_string().starts_with("192.168.") {
                                Some((41.0082, 28.9784))
                            } else {
                                None
                            }
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

                        println!("Yeni bağlantı: {}:{} -> {}:{}", src_ip, src_port, dst_ip, dst_port);
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