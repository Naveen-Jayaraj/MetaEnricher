# MetaEnricher 🦀

**Ultra-Fast, Secure Rust Backend for [OfflineMusic](https://github.com/Naveen-Jayaraj/OfflineMusic) & [Lyrica](https://github.com/Wilooper/Lyrica)**

MetaEnricher is a high-performance external backend written in Rust using Tokio and Axum. It performs **extreme deep search** to automatically discover missing metadata, correct dirty / wrong tags (such as messy YouTube video names, unformatted artists, missing album details, release years, genres, and extended creative credits), and fetches line-synced (`.lrc`) lyrics using a multi-source aggregator inspired by Wilooper/Lyrica.

---

## ⚡ Core Features

1. **Extreme Deep Search & Web Disambiguation Engine**:
   - **Google & DuckDuckGo Web Disambiguation**: Solves messy real-world metadata where song titles and artists are intermixed, or where the artist string includes movie names and multiple production credits (e.g., `artist: "Pa Va, P Jayachandran, Santhosh Varma, Anand Madhusoodanan"`).
   - Automatically disentangles and maps:
     - **Movie Name** (`Pa Va`) -> sets `album = "Pa Va"` and strips it from artist.
     - **Singer(s)** (`P. Jayachandran`) -> sets canonical `artist`.
     - **Music Director / Composer** (`Anand Madhusoodanan`) -> sets `composer`.
     - **Lyricist** (`Santhosh Varma`) -> sets `composer` / credits.
     - **Producer** (`Siyad Muhammed`) -> sets `producer`.
   - **Swapped / Intermixed Detection**: Reverses swapped titles and artist names (e.g. `title: "P Jayachandran"`, `artist: "podimmesha mulakkana kaalam"`).
   - Sanitizes messy filenames & YouTube titles (e.g. `(Official Video)`, `[4K]`, `[HD]`, `[Clean Audio]`, pipe suffixes).
   - Formats credited artists canonically separated by `" / "` (e.g., `"The Weeknd / Daft Punk"`).
   - Cross-queries **Deezer**, **iTunes / Apple Music**, **MusicBrainz**, and **LRCLIB** concurrently with resolved canonical tokens.
   - Enriches extended creative metadata: **`composer`**, **`producer`**, vocal **`language`**, and **`mood`** classification (e.g. *Chill*, *Energetic*, *Dark*, *Focus*, *Melancholic*, *Uplifting*).
   - Fetches ultra high-resolution artwork (`1200x1200bb` / `cover_xl`).
   - Automatically sets `metachecked = true` and `metadataStatus = "CONFIRMED"`.

2. **Wilooper/Lyrica-Inspired Lyrics Aggregator**:
   - Multi-source fallback: **LRCLIB** + **NetEase Cloud Music**.
   - Retrieves line-synchronized lyrics (`.lrc` with `[mm:ss.xx]` timestamps).
   - Direct raw `.lrc` download endpoint (`GET /lyric/file`) designed specifically for Android `lyricsUri` file caching.

3. **High-Speed & Secure Rust Architecture**:
   - **Axum 0.8** & **Tokio** asynchronous runtime with sub-millisecond in-memory **Moka TTL cache**.
   - Bounded concurrency semaphore for batch requests to prevent upstream rate-limiting while maintaining peak throughput.
   - 15MB request limits and permissive CORS for native Android integration.

4. **Built-in Dark-Mode Visual Dashboard**:
   - Interactive UI served at `http://localhost:8080/` with live single/batch enrichers, telemetry monitors, and a real-time synchronized LRC karaoke player.

---

## 🚀 Endpoints Specification

### 1. Single Song Metadata (`song`)
* **Endpoint:** `PUT /song` (or `POST /song`, `PUT /api/enrich-metadata`)
* **Request Body:** JSON representing a track (`TrackEntity` / `Song`):
```json
{
  "title": "Starboy (Official Music Video) [4K] ft. Daft Punk",
  "artist": "The Weeknd",
  "source": "LOCAL",
  "durationMs": 230000
}
```
* **Response:**
```json
{
  "success": true,
  "song": {
    "id": "65e9e895bc3e7",
    "title": "Starboy",
    "artist": "The Weeknd / Daft Punk",
    "album": "Starboy",
    "year": 2016,
    "genre": "R&B/Soul",
    "trackNumber": 1,
    "producer": null,
    "composer": null,
    "language": "English",
    "mood": "Chill",
    "source": "LOCAL",
    "lyricsAvailable": true,
    "metachecked": true,
    "durationMs": 230000,
    "artworkUri": "https://is1-ssl.mzstatic.com/image/thumb/Music115/v4/b5/92/bb/b592bb72-52e3-e756-9b26-9f56d08f47ab/16UMGIM67864.rgb.jpg/1200x1200bb.jpg",
    "lyricsUri": "/lyric/file?id=65e9e895bc3e7",
    "format": "mp3",
    "metadataStatus": "CONFIRMED"
  },
  "audit": {
    "matched": true,
    "confidenceScore": 0.88,
    "fieldsUpdated": ["album", "year", "genre", "trackNumber", "language", "mood", "artworkUri", "lyricsUri"],
    "fieldsCorrected": ["title", "artist"],
    "sourcesConsulted": ["iTunes", "MusicBrainz", "LRCLIB"],
    "rawTitleCleaned": true,
    "artistsReformatted": true,
    "artworkEnhanced": true,
    "lyricsFound": true,
    "detectedLanguage": "English",
    "detectedMood": "Chill"
  },
  "lyricsSample": "[00:15.71] I'm tryna put you in the worst mood, ah\n[00:18.36] P1 cleaner than your church shoes, ah..."
}
```

---

### 2. Batch Songs Metadata (`songs`)
* **Endpoint:** `PUT /songs` (or `POST /songs`, `PUT /api/enrich-metadata/batch`)
* **Request Body:** JSON array `[TrackEntity, ...]` or `{"tracks": [TrackEntity, ...]}`
* **Response:**
```json
{
  "success": true,
  "total": 2,
  "enriched": 2,
  "tracks": [ /* Array of SingleSongResponse */ ]
}
```

---

### 3. Single Song Lyrics (`lyric`)
* **Endpoint:** `PUT /lyric` (or `POST /lyric`, `GET /lyric?title=...&artist=...`)
* **Request Body:**
```json
{
  "title": "Starboy",
  "artist": "The Weeknd"
}
```
* **Response:**
```json
{
  "id": "65e9e895bc3e7",
  "title": "Starboy",
  "artist": "The Weeknd",
  "lyricsAvailable": true,
  "hasSynced": true,
  "syncedLyrics": "[00:15.71] I'm tryna put you in the worst mood, ah\n[00:18.36] P1 cleaner than your church shoes, ah\n...",
  "plainLyrics": "I'm tryna put you in the worst mood, ah\nP1 cleaner than your church shoes, ah\n...",
  "source": "LRCLIB",
  "linesCount": 79,
  "lyricsUri": "/lyric/file?id=65e9e895bc3e7"
}
```

---

### 4. Direct `.lrc` File Download (`/lyric/file`)
* **Endpoint:** `GET /lyric/file?title=Starboy&artist=The%20Weeknd`
* **Response:** Raw text with `Content-Type: text/plain; charset=utf-8` and `Content-Disposition: attachment; filename="Starboy.lrc"`. Can be downloaded directly and written to disk by OfflineMusic.

---

### 5. Batch Song Lyrics (`lyrics`)
* **Endpoint:** `PUT /lyrics` (or `POST /lyrics`, `PUT /api/enrich-lyrics/batch`)
* **Request Body:**
```json
[
  { "title": "Levitating", "artist": "Dua Lipa" },
  { "title": "Starboy", "artist": "The Weeknd" }
]
```
* **Response:**
```json
{
  "success": true,
  "total": 2,
  "found": 2,
  "results": [ /* Array of LyricResponse */ ]
}
```

---

### 6. Health & Telemetry
* **Endpoint:** `GET /health`
* **Response:**
```json
{
  "status": "ok",
  "server": "MetaEnricher-Rust",
  "version": "0.1.0",
  "uptimeSeconds": 145,
  "totalSongRequests": 24,
  "totalLyricRequests": 18,
  "metadataCacheSize": 12,
  "lyricsCacheSize": 9
}
```

---

## 📱 OfflineMusic Android Kotlin Integration

Add Retrofit definition in your Android app:

```kotlin
// MetaEnricherApi.kt
import retrofit2.http.Body
import retrofit2.http.GET
import retrofit2.http.PUT
import retrofit2.http.Query
import com.example.offlinemusic.data.database.entities.TrackEntity

interface MetaEnricherApi {
    @PUT("song")
    suspend fun enrichSong(@Body track: TrackEntity): SingleSongResponse

    @PUT("songs")
    suspend fun enrichSongsBatch(@Body tracks: List<TrackEntity>): BatchSongResponse

    @PUT("lyric")
    suspend fun getLyrics(@Body query: LyricQuery): LyricResponse

    @PUT("lyrics")
    suspend fun getLyricsBatch(@Body queries: List<LyricQuery>): BatchLyricResponse

    @GET("lyric/file")
    suspend fun downloadLrcFile(
        @Query("title") title: String,
        @Query("artist") artist: String
    ): okhttp3.ResponseBody
}

// In your Track Repository or WorkManager:
suspend fun syncAndEnrichTrack(track: TrackEntity) {
    val response = metaEnricherApi.enrichSong(track)
    if (response.success) {
        // Enriched track has metachecked = true, metadataStatus = "CONFIRMED", 
        // canonical " / " artists, high-res artwork, and lyrics status!
        trackDao.updateTrack(response.song)
    }
}
```

---

## 🛠️ Building & Running Locally

### Development mode:
```bash
cargo run
```

### Optimized release build:
```bash
cargo build --release
./target/release/meta_enricher.exe
```

The server binds to `0.0.0.0:8080` (or the port defined by `PORT` environment variable).
Open `http://localhost:8080` to access the interactive web dashboard!
