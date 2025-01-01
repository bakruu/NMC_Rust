import React, { Component } from 'react';
import { MapContainer, TileLayer, Marker, Popup, Polyline } from 'react-leaflet';
import 'leaflet/dist/leaflet.css';

class Map extends Component {
    constructor(props) {
        super(props);
        this.state = {
            connections: [],
            error: null
        };
    }

    render() {
        const connections = Array.isArray(this.props.connections) ? this.props.connections : [];

        return (
            <MapContainer center={[0, 0]} zoom={2} style={{ height: '100vh', width: '100%' }}>
                <TileLayer
                    url="https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png"
                    attribution='&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors'
                />
                {connections.map((connection, index) => {
                    const sourcePosition = [connection.source.latitude, connection.source.longitude];
                    const destPosition = [connection.destination.latitude, connection.destination.longitude];

                    return (
                        <React.Fragment key={index}>
                            <Marker position={sourcePosition}>
                                <Popup>
                                    Source IP: {connection.source.ip}<br />
                                    Port: {connection.source.port}
                                </Popup>
                            </Marker>
                            <Marker position={destPosition}>
                                <Popup>
                                    Destination IP: {connection.destination.ip}<br />
                                    Port: {connection.destination.port}
                                </Popup>
                            </Marker>
                            <Polyline 
                                positions={[sourcePosition, destPosition]}
                                color="red"
                                weight={1}
                                opacity={0.5}
                            />
                        </React.Fragment>
                    );
                })}
            </MapContainer>
        );
    }
}

export default Map; 