import React, { useEffect, useState } from 'react';
import { Map } from './Map';

function App() {
  const [packets, setPackets] = useState([]);
  const [connections, setConnections] = useState([]);
  const [ws, setWs] = useState(null);

  useEffect(() => {
    const websocket = new WebSocket('ws://localhost:8080');
    
    websocket.onopen = () => {
      console.log('WebSocket bağlantısı açıldı');
    };

    websocket.onmessage = (event) => {
      const packet = JSON.parse(event.data);
      setPackets(prev => [...prev, packet]);
      
      if (packet.source_ip && packet.dest_ip) {
        setConnections(prev => [...prev, {
          source: packet.source_ip,
          destination: packet.dest_ip,
          timestamp: packet.timestamp
        }]);
      }
    };

    websocket.onerror = (error) => {
      console.error('WebSocket hatası:', error);
    };

    setWs(websocket);

    return () => {
      websocket.close();
    };
  }, []);

  return (
    <div>
      <h1>Ağ Trafiği İzleyici</h1>
      <Map connections={connections} />
      <div className="packets-list">
        {packets.map((packet, index) => (
          <div key={index} className="packet-item">
            <p>Boyut: {packet.size} bytes</p>
            <p>Kaynak MAC: {packet.source_mac}</p>
            <p>Hedef MAC: {packet.dest_mac}</p>
            <p>Tip: {packet.type}</p>
            <p>Zaman: {packet.timestamp}</p>
            <hr />
          </div>
        ))}
      </div>
    </div>
  );
}

export default App; 