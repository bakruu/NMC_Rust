import React, { useState, useEffect } from 'react';
import Map from './Map';

function App() {
    const [connections, setConnections] = useState([]);
    const [error, setError] = useState(null);
    const [wsInstance, setWsInstance] = useState(null);

    useEffect(() => {
        let ws = null;
        let reconnectTimeout = null;

        const connectWebSocket = () => {
            try {
                ws = new WebSocket('ws://localhost:8080');
                setWsInstance(ws);

                ws.onopen = () => {
                    console.log('WebSocket Bağlantısı Kuruldu');
                    setError(null);
                    // Test mesajı gönder
                    ws.send(JSON.stringify({ type: 'test' }));
                };

                ws.onmessage = (event) => {
                    try {
                        const data = JSON.parse(event.data);
                        console.log('Gelen veri:', data);
                        setConnections(data);
                    } catch (err) {
                        console.error('Veri işleme hatası:', err);
                    }
                };

                ws.onerror = (error) => {
                    console.error('WebSocket hatası:', error);
                    setError('WebSocket bağlantı hatası');
                };

                ws.onclose = () => {
                    console.log('WebSocket bağlantısı kapandı');
                    // 5 saniye sonra yeniden bağlanmayı dene
                    reconnectTimeout = setTimeout(connectWebSocket, 5000);
                };
            } catch (error) {
                console.error('WebSocket bağlantı hatası:', error);
                setError('WebSocket bağlantı hatası');
                reconnectTimeout = setTimeout(connectWebSocket, 5000);
            }
        };

        connectWebSocket();

        return () => {
            if (reconnectTimeout) {
                clearTimeout(reconnectTimeout);
            }
            if (ws) {
                ws.close();
            }
        };
    }, []);

    if (error) {
        return (
            <div>
                <div>Hata: {error}</div>
                <div>Yeniden bağlanmaya çalışılıyor...</div>
            </div>
        );
    }

    return (
        <div style={{ height: '100vh', width: '100%' }}>
            <Map connections={connections} />
        </div>
    );
}

export default App; 