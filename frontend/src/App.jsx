import React, { useEffect, useState, useCallback } from 'react';
import { Map } from './Map';
import './App.css';

function App() {
  const [packets, setPackets] = useState([]);
  const [connections, setConnections] = useState([]);
  const [activeConnections, setActiveConnections] = useState(new Set());
  const [wsConnected, setWsConnected] = useState(false);

  const connectWebSocket = useCallback(() => {
    console.log('WebSocket bağlantısı kuruluyor...');
    
    try {
        // WebSocket URL'ini dinamik olarak belirle
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        const wsUrl = `${protocol}//${window.location.hostname}:8080`;
        console.log('WebSocket URL:', wsUrl);
        
        const websocket = new WebSocket(wsUrl);
        
        websocket.onopen = () => {
            console.log('WebSocket bağlantısı açıldı');
            setWsConnected(true);
        };

        websocket.onmessage = (event) => {
            console.log('Ham veri alındı:', event.data);
            try {
                const packet = JSON.parse(event.data);
                console.log('İşlenmiş paket:', packet);
                
                if (packet.type === 'connection_test') {
                    console.log('Bağlantı testi başarılı');
                    return;
                }
                
                setPackets(prev => {
                    if (prev.length > 100) {
                        return [...prev.slice(-99), packet];
                    }
                    return [...prev, packet];
                });
                
                if (packet.source_location && packet.dest_location) {
                    console.log('Konum bilgisi olan paket:', packet);
                    setConnections(prev => [...prev, packet]);
                }
            } catch (error) {
                console.error('Paket işleme hatası:', error);
            }
        };

        websocket.onerror = (error) => {
            console.error('WebSocket hatası:', error);
            setWsConnected(false);
        };

        websocket.onclose = () => {
            console.log('WebSocket bağlantısı kapandı');
            setWsConnected(false);
            // 3 saniye sonra yeniden bağlanmayı dene
            setTimeout(connectWebSocket, 3000);
        };

        return websocket;
    } catch (error) {
        console.error('WebSocket bağlantısı oluşturulamadı:', error);
        setTimeout(connectWebSocket, 3000);
        return null;
    }
  }, []);

  useEffect(() => {
    const ws = connectWebSocket();
    return () => {
      if (ws.readyState === WebSocket.OPEN) {
        ws.close();
      }
    };
  }, [connectWebSocket]);

  return (
    <div className="app-container">
      <Map connections={connections} />
      <div className="packets-list">
        <h1>Ağ Trafiği İzleyici</h1>
        <div className="connection-status">
          <span className={`status-indicator ${wsConnected ? 'connected' : 'disconnected'}`}>
            {wsConnected ? 'Bağlı' : 'Bağlantı Kesik'}
          </span>
        </div>
        <div className="active-connections">
          <h2>Aktif Bağlantılar</h2>
          {connections.map((conn, index) => (
            <div key={index} className="connection-item">
              <p><strong>Kaynak:</strong> {conn.source_ip}</p>
              <p>{conn.source_location?.country}, {conn.source_location?.city}</p>
              <p><strong>Hedef:</strong> {conn.dest_ip}</p>
              <p>{conn.dest_location?.country}, {conn.dest_location?.city}</p>
              <hr />
            </div>
          ))}
        </div>
        <div className="recent-packets">
          <h2>Son Paketler</h2>
          {packets.slice(-10).reverse().map((packet, index) => (
            <div key={index} className="packet-item">
              <p>Boyut: {packet.size} bytes</p>
              {packet.source_ip && (
                <p>Kaynak: {packet.source_ip}
                  {packet.source_location && 
                    <span className="location-info">
                      ({packet.source_location.country}, {packet.source_location.city})
                    </span>
                  }
                </p>
              )}
              {packet.dest_ip && (
                <p>Hedef: {packet.dest_ip}
                  {packet.dest_location && 
                    <span className="location-info">
                      ({packet.dest_location.country}, {packet.dest_location.city})
                    </span>
                  }
                </p>
              )}
              <p>Zaman: {new Date(packet.timestamp).toLocaleTimeString()}</p>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

export default App; 