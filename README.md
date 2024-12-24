## Network Traffic Visualizer

Bu uygulama ağ trafiğini gerçek zamanlı olarak görselleştirir. IP adreslerinin coğrafi konumlarını harita üzerinde gösterir ve bağlantıları takip eder.

### Özellikler
- Gerçek zamanlı ağ trafiği izleme
- IP adreslerinin coğrafi konumlarını haritada gösterme
- Kaynak ve hedef arasındaki bağlantıları görselleştirme
- Paket detaylarını anlık olarak listeleme

### Kurulum ve Çalıştırma

1. Uygulamayı indirin:
   ```bash
   git clone https://github.com/kullanici/proje.git
   cd proje
   ```

2. Backend'i başlatın:
   ```bash
   cd backend
   cargo run
   ```

3. Frontend'i başlatın:
   ```bash
   cd frontend
   npm install
   npm run dev
   ```

4. Tarayıcınızda http://localhost:5173 adresini açın

### Notlar
- Bu uygulama GeoLite2 veritabanlarını kullanmaktadır
- © MaxMind, Inc. https://www.maxmind.com 