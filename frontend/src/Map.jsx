import React, { useEffect, useRef } from 'react';
import L from 'leaflet';
import 'leaflet/dist/leaflet.css';

// Leaflet marker ikonları için gerekli
import icon from 'leaflet/dist/images/marker-icon.png';
import iconShadow from 'leaflet/dist/images/marker-shadow.png';

let DefaultIcon = L.icon({
  iconUrl: icon,
  shadowUrl: iconShadow,
  iconSize: [25, 41],
  iconAnchor: [12, 41]
});

L.Marker.prototype.options.icon = DefaultIcon;

export function Map({ connections }) {
  const mapRef = useRef(null);
  const mapInstanceRef = useRef(null);
  const linesRef = useRef([]);

  useEffect(() => {
    if (!mapInstanceRef.current) {
      mapInstanceRef.current = L.map(mapRef.current).setView([0, 0], 2);
      L.tileLayer('https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', {
        attribution: '© OpenStreetMap contributors'
      }).addTo(mapInstanceRef.current);
    }
  }, []);

  useEffect(() => {
    // Önceki çizgileri temizle
    linesRef.current.forEach(line => line.remove());
    linesRef.current = [];

    connections.forEach(conn => {
      if (conn.source_location && conn.dest_location) {
        // Kaynak ve hedef için marker'lar
        const sourceMarker = L.marker([conn.source_location.latitude, conn.source_location.longitude])
          .bindPopup(`
            <div>
              <strong>Kaynak:</strong><br/>
              IP: ${conn.source_ip}<br/>
              Ülke: ${conn.source_location.country}<br/>
              Şehir: ${conn.source_location.city}
            </div>
          `)
          .addTo(mapInstanceRef.current);

        const destMarker = L.marker([conn.dest_location.latitude, conn.dest_location.longitude])
          .bindPopup(`
            <div>
              <strong>Hedef:</strong><br/>
              IP: ${conn.dest_ip}<br/>
              Ülke: ${conn.dest_location.country}<br/>
              Şehir: ${conn.dest_location.city}
            </div>
          `)
          .addTo(mapInstanceRef.current);

        // Bağlantı çizgisi
        const line = L.polyline(
          [
            [conn.source_location.latitude, conn.source_location.longitude],
            [conn.dest_location.latitude, conn.dest_location.longitude]
          ],
          { color: 'red', weight: 2, opacity: 0.5 }
        ).addTo(mapInstanceRef.current);

        linesRef.current.push(sourceMarker, destMarker, line);
      }
    });
  }, [connections]);

  return (
    <div 
      ref={mapRef} 
      style={{ 
        height: '100vh', 
        width: '100%',
        position: 'fixed',
        top: 0,
        left: 0,
        zIndex: 1 
      }} 
    />
  );
} 