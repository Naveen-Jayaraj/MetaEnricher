# MetaEnricher 🦀

**Ultra-Fast, Secure Rust Backend for [OfflineMusic](https://github.com/Naveen-Jayaraj/OfflineMusic) & [Lyrica](https://github.com/Wilooper/Lyrica)**

MetaEnricher is a high-performance external backend written in Rust using Tokio and Axum. It performs **extreme deep search** to automatically discover missing metadata, correct dirty / wrong tags (such as messy YouTube video names, unformatted artists, missing album details, release years, genres, and extended creative credits), and fetches line-synced (`.lrc`) lyrics using a multi-source aggregator inspired by Wilooper/Lyrica.

---

## ⚡ Core Features

1. **Extreme Deep Search & Web Disambiguation Engine**:
   - **Google & DuckDuckGo Web Disambiguation**: Solves messy real-world metadata where song titles and artists are intermixed, or where the artist string includes movie names and multiple production credits (e.g., `artist: "Pa Va, P Jayachandran, Santhosh Varma, Anand Madhusoodanan"`).
   - Automatically disentangles and maps:
     - **Movie Name** (`Pa Va`) &rarr; sets `album = "Pa Va"` and strips it from artist.
     - **Singer(s)** (`P. Jayachandran`) &rarr; sets canonical `artist`.
     - **Music Director / Composer** (`Anand Madhusoodanan`) &rarr; sets `composer`.
     - **Lyricist** (`Santhosh Varma`) &rarr; sets `composer` / credits.
     - **Producer** (`Siyad Muhammed`) &rarr; sets `producer`.
   - **Swapped / Intermixed Detection**: Reverses swapped titles and artist names (e.g. `title: "P Jayachandran"`, `artist: "podimmesha mulakkana kaalam"`).
   - Sanitizes messy filenames & YouTube titles (e.g. `(Official Video)`, `[4K]`, `[HD]`, `[Clean Audio]`, pipe suffixes).
   - Formats credited artists canonically separated by `" / "` (e.g., `"The Weeknd / Daft Punk"`).
   - Cross-queries **Deezer**, **iTunes / Apple Music**, **MusicBrainz**, and **LRCLIB** concurrently with resolved canonical tokens.
   - Enriches extended creative metadata: **`composer`**, **`producer`**, vocal **`language`**, and **`mood`** classification (e.g. *Chill*, *Energetic*, *Dark*, *Focus*, *Melancholic*, *Uplifting*).
   - Fetches ultra high-resolution artwork (`1200x1200bb` / `cover_xl`).
   - Automatically sets `metachecked = true` and `metadataStatus = "CONFIRMED"`.

2. **Wilooper/Lyrica-Inspired Lyrics Aggregator**:
   - Multi-source fallback: **LRCLIB** + **NetEase Cloud Music**.
   - Retrieves line-synchronized lyrics (`.lrc` with `[mm:ss.xx]` timestamps) and plain text lyrics.
   - Direct raw `.lrc` download endpoint (`GET /lyric/file`) designed specifically for Android `lyricsUri` file caching.

3. **High-Speed & Secure Rust Architecture**:
   - **Axum 0.8** & **Tokio** asynchronous runtime with sub-millisecond in-memory **Moka TTL cache**.
   - **API Key & HTTP Basic Auth**: Locks API endpoints so only your authorized Android app can connect.
   - **Sliding-Window Rate Limiter**: 60 requests/minute per client IP (extracts IP from Cloudflare, Render `X-Forwarded-For`, etc.), returning `429 Too Many Requests` + `Retry-After: 60`.
   - Bounded concurrency semaphore for batch requests to maintain peak throughput without getting rate-limited.
   - 15MB request limits and permissive CORS for native Android integration.

4. **Built-in Dark-Mode Visual Dashboard**:
   - Interactive UI served at `http://localhost:8080/` with live single/batch enrichers, telemetry monitors, and a real-time synchronized LRC karaoke player with built-in API key support.

---

## 📋 Complete TrackEntity Model Output (OfflineMusic App Ready)

Every enriched song produced by MetaEnricher conforms directly to OfflineMusic's `TrackEntity`:

| Field | Sample Value | Status & Verification | Type | Description |
| :--- | :--- | :--- | :--- | :--- |
| `title` | `Starboy` | **CORRECTED** | `String` | Cleaned track title stripped of YouTube tags, resolutions, and video labels. |
| `artist` | `The Weeknd / Daft Punk` | **CORRECTED** | `String` | Credited primary and featured artists canonically formatted with `" / "`. |
| `album` | `Starboy` | **DISCOVERED** | `String?` | Official studio album or movie/soundtrack name discovered via deep search. |
| `year` | `2016` | **DISCOVERED** | `Int?` | Canonical original release year. |
| `genre` | `R&B/Soul` | **DISCOVERED** | `String?` | Canonical musical genre (e.g. Pop, Electronic, Rock, R&B/Soul). |
| `trackNumber` | `1` | **DISCOVERED** | `Int?` | Official index position of the track on the album or CD. |
| `producer` | `—` | **Verified** | `String?` | Credited music producer(s) or recording engineer. |
| `composer` | `—` | **Verified** | `String?` | Credited songwriter(s), music director, or lyricist. |
| `language` | `English` | **DISCOVERED** | `String?` | Detected vocal audio language (English, Malayalam, Hindi, Japanese, etc.). |
| `mood` | `Chill` | **DISCOVERED** | `String?` | Psychoacoustic mood / vibe classification (*Chill*, *Energetic*, *Dark*, *Focus*). |
| `source` | `LOCAL` | **Verified** | `String` | Track origin source (`LOCAL`, `YOUTUBE`, `LUCIDA`). |
| `lyrics_available` | `true` | **Verified** | `Boolean` | Flag indicating whether synchronized or plain lyrics exist for this song. |
| `metachecked` | `true` | **Verified** | `Boolean` | Flag indicating the track has passed deep server verification. |
| `artworkUri` | `https://is1-ssl.mzstatic.com/image/thumb/Music115/.../1200x1200bb.jpg` | **DISCOVERED** | `String?` | Ultra-high-resolution album cover URL (`1200x1200bb` / `cover_xl`). |
| `lyricsUri` | `/lyric/file?id=65ea6459595a2` | **DISCOVERED** | `String?` | Direct relative download URI to the line-synced `.lrc` file. |
| `metadataStatus` | `CONFIRMED` | **Verified** | `String` | Status changed from `PARSED` to `CONFIRMED`. |

---

## 🔒 Security, Authentication & Rate Limiting

To ensure that **only your app** can access your backend, MetaEnricher includes a unified security layer:

### 1. Supported Authentication Methods
* **API Key Header**:
  ```http
  X-API-Key: metaenricher_secret_key
  ```
* **Bearer Token**:
  ```http
  Authorization: Bearer metaenricher_secret_key
  ```
* **HTTP Basic Auth**:
  ```http
  Authorization: Basic YWRtaW46bWV0YWVucmljaGVyX3NlY3JldA==
  ```
* **Query Parameter** (Convenient for `.lrc` file downloads):
  ```http
  GET /lyric/file?title=Starboy&artist=The%20Weeknd&key=metaenricher_secret_key
  ```

### 2. Rate Limiting
* Default: **60 requests per minute** per client IP.
* IP extraction resolves across reverse proxies (`cf-connecting-ip`, `x-forwarded-for`, `x-real-ip`).
* When exceeded: Returns HTTP `429 Too Many Requests` with a `Retry-After: 60` response header.

### 3. Environment Variables
You can customize credentials easily via environment variables in Render or Docker:

| Variable | Default Value | Description |
| :--- | :--- | :--- |
| `API_KEY` | `metaenricher_secret_key` | Secret key required in `X-API-Key` or `Authorization: Bearer` |
| `AUTH_USERNAME` | `admin` | Username for HTTP Basic Auth |
| `AUTH_PASSWORD` | `metaenricher_secret` | Password for HTTP Basic Auth |
| `RATE_LIMIT_PER_MINUTE` | `60` | Maximum allowed requests per IP per minute |
| `PORT` | `8080` | Port for the HTTP server |

*Note: `/health` and the visual dashboard shell at `/` are public so health checks and browser previews work out-of-the-box.*

---

## 🚀 Endpoints Specification

### 1. Single Song Metadata (`song`)
* **Endpoint:** `PUT /song` (also accepts `POST /song`, `PUT /api/enrich-metadata`)
* **Headers:** `X-API-Key: metaenricher_secret_key`, `Content-Type: application/json`
* **Request Body:**
```json
{
  "title": "Starboy (Official Music Video) [4K] ft. Daft Punk",
  "artist": "The Weeknd",
  "source": "LOCAL",
  "durationMs": 230000
}
```
* **Response (HTTP 200):**
```json
{
  "success": true,
  "song": {
    "id": "65ea6459595a2",
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
    "lyricsUri": "/lyric/file?id=65ea6459595a2",
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
* **Endpoint:** `PUT /songs` (also accepts `POST /songs`, `PUT /api/enrich-metadata/batch`)
* **Headers:** `X-API-Key: metaenricher_secret_key`, `Content-Type: application/json`
* **Request Body:** JSON array `[TrackEntity, ...]` or `{"tracks": [TrackEntity, ...]}`
* **Response (HTTP 200):**
```json
{
  "success": true,
  "total": 2,
  "enriched": 2,
  "tracks": [ /* Array of SingleSongResponse objects */ ]
}
```

---

### 3. Single Song Lyrics (`lyric`)
* **Endpoint:** `PUT /lyric` (also accepts `POST /lyric`, `GET /lyric?title=...&artist=...`)
* **Headers:** `X-API-Key: metaenricher_secret_key`, `Content-Type: application/json`
* **Request Body:**
```json
{
  "title": "Starboy",
  "artist": "The Weeknd"
}
```
* **Response (HTTP 200):**
```json
{
  "id": "65ea6459595a2",
  "title": "Starboy",
  "artist": "The Weeknd",
  "lyricsAvailable": true,
  "hasSynced": true,
  "syncedLyrics": "[00:15.71] I'm tryna put you in the worst mood, ah\n[00:18.36] P1 cleaner than your church shoes, ah\n...",
  "plainLyrics": "I'm tryna put you in the worst mood, ah\nP1 cleaner than your church shoes, ah\n...",
  "source": "LRCLIB",
  "linesCount": 79,
  "lyricsUri": "/lyric/file?id=65ea6459595a2"
}
```

---

### 4. Direct `.lrc` File Download (`/lyric/file`)
* **Endpoint:** `GET /lyric/file?title=Starboy&artist=The%20Weeknd&key=metaenricher_secret_key`
* **Response:** Raw text with `Content-Type: text/plain; charset=utf-8` and `Content-Disposition: attachment; filename="Starboy.lrc"`. Can be downloaded directly and saved to disk.

---

### 5. Batch Song Lyrics (`lyrics`)
* **Endpoint:** `PUT /lyrics` (also accepts `POST /lyrics`, `PUT /api/enrich-lyrics/batch`)
* **Headers:** `X-API-Key: metaenricher_secret_key`, `Content-Type: application/json`
* **Request Body:**
```json
[
  { "title": "Levitating", "artist": "Dua Lipa" },
  { "title": "Starboy", "artist": "The Weeknd" }
]
```
* **Response (HTTP 200):**
```json
{
  "success": true,
  "total": 2,
  "found": 2,
  "results": [ /* Array of LyricResponse objects */ ]
}
```

---

### 6. Health & Telemetry
* **Endpoint:** `GET /health` (Public, no auth required)
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

### 1. Models & Retrofit Interface

```kotlin
// MetaEnricherModels.kt
package com.example.offlinemusic.data.remote

import com.example.offlinemusic.data.model.Song
import okhttp3.ResponseBody
import retrofit2.http.*

data class SingleSongResponse(
    val success: Boolean,
    val song: Song, // Maps to your app's Song / TrackEntity
    val audit: EnrichmentAudit?,
    val lyricsSample: String?
)

data class BatchSongResponse(
    val success: Boolean,
    val total: Int,
    val enriched: Int,
    val tracks: List<SingleSongResponse>
)

data class EnrichmentAudit(
    val matched: Boolean,
    val confidenceScore: Float,
    val fieldsUpdated: List<String>,
    val fieldsCorrected: List<String>,
    val sourcesConsulted: List<String>,
    val rawTitleCleaned: Boolean,
    val artistsReformatted: Boolean,
    val lyricsFound: Boolean,
    val detectedLanguage: String?,
    val detectedMood: String?
)

data class LyricQuery(
    val title: String,
    val artist: String,
    val album: String? = null,
    val durationMs: Long? = null
)

data class LyricResponse(
    val id: String,
    val title: String,
    val artist: String,
    val lyricsAvailable: Boolean,
    val hasSynced: Boolean,
    val syncedLyrics: String?,
    val plainLyrics: String?,
    val source: String,
    val linesCount: Int,
    val lyricsUri: String?
)

interface MetaEnricherApi {
    @PUT("song")
    suspend fun enrichSong(@Body track: Song): SingleSongResponse

    @PUT("songs")
    suspend fun enrichSongsBatch(@Body tracks: List<Song>): BatchSongResponse

    @PUT("lyric")
    suspend fun getLyrics(@Body query: LyricQuery): LyricResponse

    @PUT("lyrics")
    suspend fun getLyricsBatch(@Body queries: List<LyricQuery>): List<LyricResponse>

    @Streaming
    @GET("lyric/file")
    suspend fun downloadLrcFile(
        @Query("title") title: String,
        @Query("artist") artist: String,
        @Query("key") apiKey: String = "metaenricher_secret_key"
    ): ResponseBody
}
```

### 2. Authenticated Retrofit Client

```kotlin
// ApiClient.kt
package com.example.offlinemusic.data.remote

import okhttp3.OkHttpClient
import retrofit2.Retrofit
import retrofit2.converter.gson.GsonConverterFactory
import java.util.concurrent.TimeUnit

object ApiClient {
    // Replace with your Render URL (or http://10.0.2.2:8080/ for Android Emulator)
    private const val BASE_URL = "https://your-meta-enricher.onrender.com/"
    private const val API_KEY = "metaenricher_secret_key"

    private val okHttpClient = OkHttpClient.Builder()
        .addInterceptor { chain ->
            val request = chain.request().newBuilder()
                .header("X-API-Key", API_KEY)
                .build()
            chain.proceed(request)
        }
        .connectTimeout(45, TimeUnit.SECONDS)
        .readTimeout(45, TimeUnit.SECONDS)
        .build()

    val api: MetaEnricherApi by lazy {
        Retrofit.Builder()
            .baseUrl(BASE_URL)
            .client(okHttpClient)
            .addConverterFactory(GsonConverterFactory.create())
            .build()
            .create(MetaEnricherApi::class.java)
    }
}
```

### 3. Usage in Track Repository / Worker

```kotlin
suspend fun enrichTrack(track: Song) = withContext(Dispatchers.IO) {
    try {
        val response = ApiClient.api.enrichSong(track)
        if (response.success) {
            // Save enriched track to Room Database
            songDao.updateSong(response.song)
        }
    } catch (e: Exception) {
        e.printStackTrace()
    }
}
```

---

## 🛠️ Building & Running Locally

### 1. Development mode:
```bash
cargo run
```

### 2. Optimized release build:
```bash
cargo build --release
./target/release/meta_enricher.exe
```

The server binds to `0.0.0.0:8080` (or the port defined by `PORT`).
Open `http://localhost:8080` to access the interactive web dashboard!

---

## ☁️ 100% Free Cloud Deployment (Render.com)

1. Push your repository to GitHub:
   ```powershell
   git push origin main
   ```
2. Go to **[dashboard.render.com](https://dashboard.render.com)** and sign in with GitHub (No credit card required).
3. Click **New +** &rarr; **Blueprint** and select your `MetaEnricher` repository (or **New +** &rarr; **Web Service** with runtime `Docker` and plan `Free`).
4. Click **Apply**.
5. Once deployed, Render provides your permanent HTTPS URL:
   ```text
   https://meta-enricher-xxxx.onrender.com
   ```
   Paste this into your Android app's `ApiClient.kt` and enjoy fast, secure, automated metadata enrichment!
