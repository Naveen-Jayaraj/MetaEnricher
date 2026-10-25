use axum::response::Html;

pub async fn dashboard_handler() -> Html<&'static str> {
    Html(DASHBOARD_HTML)
}

const DASHBOARD_HTML: &str = r###"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>MetaEnricher &bull; High-Performance Audio Metadata &amp; Lyrics Engine</title>
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
  <link href="https://fonts.googleapis.com/css2?family=Outfit:wght@300;400;500;600;700;800&family=JetBrains+Mono:wght@400;500;700&display=swap" rel="stylesheet">
  <style>
    :root {
      --bg: #090c10;
      --bg-card: rgba(18, 23, 33, 0.75);
      --bg-card-hover: rgba(26, 33, 48, 0.9);
      --border: rgba(255, 255, 255, 0.08);
      --border-accent: rgba(99, 102, 241, 0.4);
      --primary: #6366f1;
      --primary-glow: rgba(99, 102, 241, 0.35);
      --secondary: #ec4899;
      --accent: #06b6d4;
      --success: #10b981;
      --warning: #f59e0b;
      --text: #f3f4f6;
      --text-muted: #9ca3af;
      --text-dim: #6b7280;
      --radius-sm: 8px;
      --radius-md: 14px;
      --radius-lg: 20px;
      --font: 'Outfit', -apple-system, BlinkMacSystemFont, sans-serif;
      --mono: 'JetBrains Mono', monospace;
    }

    * { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      background-color: var(--bg);
      background-image: 
        radial-gradient(at 0% 0%, rgba(99, 102, 241, 0.15) 0px, transparent 50%),
        radial-gradient(at 100% 0%, rgba(236, 72, 153, 0.12) 0px, transparent 50%),
        radial-gradient(at 50% 100%, rgba(6, 182, 212, 0.12) 0px, transparent 50%);
      background-attachment: fixed;
      color: var(--text);
      font-family: var(--font);
      min-height: 100vh;
      overflow-x: hidden;
      line-height: 1.5;
    }

    /* Container & Layout */
    .container {
      max-width: 1320px;
      margin: 0 auto;
      padding: 24px 20px 60px;
    }

    header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      padding-bottom: 24px;
      border-bottom: 1px solid var(--border);
      margin-bottom: 30px;
      flex-wrap: wrap;
      gap: 16px;
    }

    .brand {
      display: flex;
      align-items: center;
      gap: 14px;
    }

    .brand-icon {
      width: 44px;
      height: 44px;
      background: linear-gradient(135deg, var(--primary), var(--secondary));
      border-radius: var(--radius-sm);
      display: flex;
      align-items: center;
      justify-content: center;
      box-shadow: 0 0 20px var(--primary-glow);
    }

    .brand-icon svg { width: 26px; height: 26px; fill: white; }

    .brand-title {
      font-size: 1.5rem;
      font-weight: 800;
      letter-spacing: -0.02em;
      background: linear-gradient(135deg, #ffffff 40%, #c7d2fe);
      -webkit-background-clip: text;
      -webkit-text-fill-color: transparent;
    }

    .brand-tag {
      font-size: 0.72rem;
      font-weight: 600;
      background: rgba(99, 102, 241, 0.2);
      color: #a5b4fc;
      padding: 3px 8px;
      border-radius: 999px;
      border: 1px solid rgba(99, 102, 241, 0.3);
      text-transform: uppercase;
      letter-spacing: 0.05em;
    }

    .status-badge {
      display: inline-flex;
      align-items: center;
      gap: 8px;
      padding: 6px 14px;
      background: rgba(16, 185, 129, 0.12);
      border: 1px solid rgba(16, 185, 129, 0.3);
      border-radius: 999px;
      font-size: 0.82rem;
      color: #34d399;
      font-weight: 600;
    }

    .status-dot {
      width: 8px;
      height: 8px;
      background: #10b981;
      border-radius: 50%;
      box-shadow: 0 0 10px #10b981;
      animation: pulse 2s infinite;
    }

    @keyframes pulse {
      0%, 100% { opacity: 1; transform: scale(1); }
      50% { opacity: 0.4; transform: scale(0.85); }
    }

    /* Stats Grid */
    .stats-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(210px, 1fr));
      gap: 16px;
      margin-bottom: 30px;
    }

    .stat-card {
      background: var(--bg-card);
      backdrop-filter: blur(16px);
      border: 1px solid var(--border);
      border-radius: var(--radius-md);
      padding: 18px 20px;
      transition: all 0.25s ease;
    }
    .stat-card:hover {
      border-color: var(--border-accent);
      transform: translateY(-2px);
    }
    .stat-label {
      font-size: 0.78rem;
      font-weight: 600;
      color: var(--text-muted);
      text-transform: uppercase;
      letter-spacing: 0.06em;
      margin-bottom: 6px;
    }
    .stat-value {
      font-size: 1.6rem;
      font-weight: 700;
      color: #fff;
    }
    .stat-sub {
      font-size: 0.75rem;
      color: var(--text-dim);
      margin-top: 4px;
    }

    /* Tabs */
    .tabs-nav {
      display: flex;
      gap: 10px;
      border-bottom: 1px solid var(--border);
      padding-bottom: 12px;
      margin-bottom: 24px;
      overflow-x: auto;
    }
    .tab-btn {
      background: transparent;
      border: none;
      color: var(--text-muted);
      font-family: var(--font);
      font-size: 0.95rem;
      font-weight: 600;
      padding: 10px 18px;
      border-radius: var(--radius-sm);
      cursor: pointer;
      display: flex;
      align-items: center;
      gap: 8px;
      transition: all 0.2s;
    }
    .tab-btn:hover {
      color: #fff;
      background: rgba(255, 255, 255, 0.04);
    }
    .tab-btn.active {
      color: #fff;
      background: rgba(99, 102, 241, 0.2);
      border: 1px solid rgba(99, 102, 241, 0.4);
    }

    .tab-content { display: none; }
    .tab-content.active { display: block; }

    /* Panels & Cards */
    .card {
      background: var(--bg-card);
      backdrop-filter: blur(16px);
      border: 1px solid var(--border);
      border-radius: var(--radius-lg);
      padding: 26px;
      margin-bottom: 24px;
    }
    .card-title {
      font-size: 1.15rem;
      font-weight: 700;
      margin-bottom: 6px;
      display: flex;
      align-items: center;
      gap: 10px;
    }
    .card-desc {
      font-size: 0.85rem;
      color: var(--text-muted);
      margin-bottom: 20px;
    }

    /* Form Inputs */
    .form-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
      gap: 16px;
      margin-bottom: 20px;
    }
    .form-group {
      display: flex;
      flex-direction: column;
      gap: 6px;
    }
    label {
      font-size: 0.78rem;
      font-weight: 600;
      color: var(--text-muted);
      text-transform: uppercase;
      letter-spacing: 0.04em;
    }
    input, textarea, select {
      background: rgba(13, 17, 23, 0.8);
      border: 1px solid var(--border);
      border-radius: var(--radius-sm);
      padding: 11px 14px;
      color: #fff;
      font-family: var(--font);
      font-size: 0.92rem;
      transition: all 0.2s;
      width: 100%;
    }
    input:focus, textarea:focus, select:focus {
      outline: none;
      border-color: var(--primary);
      box-shadow: 0 0 0 3px var(--primary-glow);
    }
    textarea {
      font-family: var(--mono);
      font-size: 0.82rem;
      resize: vertical;
      min-height: 120px;
    }

    /* Buttons */
    .btn {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      gap: 8px;
      background: linear-gradient(135deg, var(--primary), #4f46e5);
      color: white;
      font-family: var(--font);
      font-weight: 600;
      font-size: 0.92rem;
      padding: 12px 24px;
      border-radius: var(--radius-sm);
      border: none;
      cursor: pointer;
      transition: all 0.2s ease;
      box-shadow: 0 4px 14px rgba(99, 102, 241, 0.35);
    }
    .btn:hover {
      transform: translateY(-1px);
      box-shadow: 0 6px 20px rgba(99, 102, 241, 0.5);
    }
    .btn:disabled {
      opacity: 0.5;
      cursor: not-allowed;
      transform: none;
    }
    .btn-secondary {
      background: rgba(255, 255, 255, 0.08);
      color: var(--text);
      box-shadow: none;
      border: 1px solid var(--border);
    }
    .btn-secondary:hover {
      background: rgba(255, 255, 255, 0.12);
      border-color: rgba(255, 255, 255, 0.2);
      box-shadow: none;
    }

    /* Enrichment Output View */
    .result-container {
      margin-top: 24px;
      display: none;
      border-top: 1px solid var(--border);
      padding-top: 24px;
    }

    .song-preview-card {
      display: grid;
      grid-template-columns: 160px 1fr;
      gap: 24px;
      background: rgba(13, 17, 23, 0.6);
      border: 1px solid var(--border);
      border-radius: var(--radius-md);
      padding: 20px;
      margin-bottom: 20px;
    }
    @media (max-width: 768px) {
      .song-preview-card { grid-template-columns: 1fr; }
    }

    .album-art {
      width: 100%;
      aspect-ratio: 1/1;
      border-radius: var(--radius-sm);
      object-fit: cover;
      background: #1f2937;
      border: 1px solid rgba(255, 255, 255, 0.1);
      box-shadow: 0 10px 25px rgba(0,0,0,0.5);
    }

    .song-meta-header {
      display: flex;
      flex-direction: column;
      gap: 6px;
    }
    .meta-title {
      font-size: 1.6rem;
      font-weight: 800;
      color: #fff;
      line-height: 1.2;
    }
    .meta-artist {
      font-size: 1.05rem;
      font-weight: 600;
      color: #a5b4fc;
    }
    .meta-album-info {
      font-size: 0.88rem;
      color: var(--text-muted);
    }

    .badges-row {
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
      margin-top: 10px;
    }
    .pill {
      font-size: 0.74rem;
      font-weight: 600;
      padding: 4px 10px;
      border-radius: 999px;
      display: inline-flex;
      align-items: center;
      gap: 5px;
    }
    .pill-green { background: rgba(16, 185, 129, 0.15); color: #34d399; border: 1px solid rgba(16, 185, 129, 0.3); }
    .pill-blue { background: rgba(99, 102, 241, 0.15); color: #a5b4fc; border: 1px solid rgba(99, 102, 241, 0.3); }
    .pill-pink { background: rgba(236, 72, 153, 0.15); color: #f472b6; border: 1px solid rgba(236, 72, 153, 0.3); }
    .pill-cyan { background: rgba(6, 182, 212, 0.15); color: #67e8f9; border: 1px solid rgba(6, 182, 212, 0.3); }

    /* Metadata Table */
    .table-container {
      overflow-x: auto;
      border-radius: var(--radius-sm);
      border: 1px solid var(--border);
      background: rgba(13, 17, 23, 0.5);
    }
    table {
      width: 100%;
      border-collapse: collapse;
      text-align: left;
      font-size: 0.86rem;
    }
    th {
      background: rgba(255, 255, 255, 0.03);
      padding: 12px 16px;
      color: var(--text-muted);
      font-weight: 600;
      border-bottom: 1px solid var(--border);
    }
    td {
      padding: 12px 16px;
      border-bottom: 1px solid rgba(255, 255, 255, 0.04);
      color: var(--text);
    }
    tr:last-child td { border-bottom: none; }
    .code-val {
      font-family: var(--mono);
      font-size: 0.8rem;
      color: #93c5fd;
      background: rgba(59, 130, 246, 0.1);
      padding: 2px 6px;
      border-radius: 4px;
    }
    .tag-updated {
      font-size: 0.7rem;
      background: rgba(16, 185, 129, 0.2);
      color: #34d399;
      padding: 2px 6px;
      border-radius: 4px;
      margin-left: 6px;
      font-weight: 600;
    }
    .tag-corrected {
      font-size: 0.7rem;
      background: rgba(245, 158, 11, 0.2);
      color: #fbbf24;
      padding: 2px 6px;
      border-radius: 4px;
      margin-left: 6px;
      font-weight: 600;
    }

    /* LRC Karaoke Visualizer */
    .karaoke-player {
      background: rgba(13, 17, 23, 0.9);
      border: 1px solid var(--border);
      border-radius: var(--radius-md);
      padding: 20px;
      height: 380px;
      overflow-y: auto;
      scroll-behavior: smooth;
      position: relative;
    }
    .lrc-line {
      padding: 10px 14px;
      border-radius: 6px;
      color: var(--text-dim);
      font-size: 1.1rem;
      font-weight: 500;
      transition: all 0.25s ease;
      cursor: pointer;
      display: flex;
      align-items: center;
      gap: 12px;
    }
    .lrc-time {
      font-family: var(--mono);
      font-size: 0.74rem;
      color: var(--text-dim);
      min-width: 60px;
    }
    .lrc-line:hover {
      background: rgba(255, 255, 255, 0.03);
      color: var(--text-muted);
    }
    .lrc-line.active {
      color: #fff;
      font-size: 1.35rem;
      font-weight: 700;
      color: #a5b4fc;
      background: rgba(99, 102, 241, 0.15);
      border-left: 4px solid var(--primary);
      transform: translateX(4px);
      text-shadow: 0 0 20px rgba(99, 102, 241, 0.6);
    }

    /* Code Block */
    pre {
      background: #0d1117;
      border: 1px solid var(--border);
      border-radius: var(--radius-sm);
      padding: 16px;
      overflow-x: auto;
      font-family: var(--mono);
      font-size: 0.82rem;
      color: #e6edf3;
      line-height: 1.6;
    }

    .loader {
      display: inline-block;
      width: 18px;
      height: 18px;
      border: 2px solid rgba(255,255,255,0.3);
      border-radius: 50%;
      border-top-color: white;
      animation: spin 0.8s linear infinite;
    }
    @keyframes spin { to { transform: rotate(360deg); } }
  </style>
</head>
<body>
  <div class="container">
    <header>
      <div class="brand">
        <div class="brand-icon">
          <svg viewBox="0 0 24 24"><path d="M12 3v10.55c-.59-.34-1.27-.55-2-.55-2.21 0-4 1.79-4 4s1.79 4 4 4 4-1.79 4-4V7h4V3h-6z"/></svg>
        </div>
        <div>
          <div style="display:flex; align-items:center; gap:8px;">
            <h1 class="brand-title">MetaEnricher</h1>
            <span class="brand-tag">Rust Core</span>
          </div>
          <div style="font-size:0.8rem; color:var(--text-muted);">High-Performance OfflineMusic Server &bull; Wilooper/Lyrica Synced Engine</div>
        </div>
      </div>
      <div style="display:flex; align-items:center; gap:12px; flex-wrap:wrap;">
        <div style="display:flex; align-items:center; gap:8px; background:rgba(255,255,255,0.06); padding:6px 12px; border-radius:var(--radius-sm); border:1px solid var(--border);">
          <span style="font-size:0.75rem; color:var(--text-muted); font-weight:600;">API KEY:</span>
          <input type="password" id="global-api-key" style="background:transparent; border:none; color:#a5b4fc; font-family:var(--mono); font-size:0.8rem; outline:none; width:170px;" placeholder="metaenricher_secret_key" oninput="saveApiKey(this.value)">
          <button onclick="toggleKeyVisibility()" style="background:transparent; border:none; color:var(--text-muted); cursor:pointer; font-size:0.75rem;" title="Toggle Visibility">👁️</button>
        </div>
        <span class="status-badge">
          <span class="status-dot"></span>
          Engine Online &amp; Protected
        </span>
      </div>
    </header>

    <!-- Stats Bar -->
    <div class="stats-grid">
      <div class="stat-card">
        <div class="stat-label">Song Enrichments</div>
        <div class="stat-value" id="stat-song-count">0</div>
        <div class="stat-sub">Track queries processed</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">Lyrics Retrieved</div>
        <div class="stat-value" id="stat-lyric-count">0</div>
        <div class="stat-sub">Synced LRC &amp; plain</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">Metadata Cache</div>
        <div class="stat-value" id="stat-meta-cache">0</div>
        <div class="stat-sub">Hot entries in Moka</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">Server Uptime</div>
        <div class="stat-value" id="stat-uptime">0s</div>
        <div class="stat-sub">Blazing Tokio async runtime</div>
      </div>
    </div>

    <!-- Navigation Tabs -->
    <div class="tabs-nav">
      <button class="tab-btn active" onclick="switchTab('single')">
        <svg width="18" height="18" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 19V6l12-3v13M9 19c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zm12-3c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zM9 10l12-3"/></svg>
        Single Song Enricher
      </button>
      <button class="tab-btn" onclick="switchTab('lyrics')">
        <svg width="18" height="18" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 11a7 7 0 01-7 7m0 0a7 7 0 01-7-7m7 7v4m0 0H8m4 0h4m-4-8a3 3 0 100-6 3 3 0 000 6z"/></svg>
        Lyrica Karaoke Player
      </button>
      <button class="tab-btn" onclick="switchTab('batch')">
        <svg width="18" height="18" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10"/></svg>
        Batch Enricher (Songs &amp; Lyrics)
      </button>
      <button class="tab-btn" onclick="switchTab('docs')">
        <svg width="18" height="18" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4"/></svg>
        Android Integration &amp; Endpoints
      </button>
    </div>

    <!-- TAB 1: Single Song Enricher -->
    <div id="tab-single" class="tab-content active">
      <div class="card">
        <div class="card-title">
          <span>Extreme Deep Search &bull; Single Song Enricher</span>
        </div>
        <div class="card-desc">
          Sends unverified / messy track metadata, extracts artists formatted as <code>" / "</code>, cleans YouTube artifacts, queries Deezer + iTunes + MusicBrainz + LRCLIB, and validates all extended creative credits.
        </div>

        <div class="form-grid">
          <div class="form-group">
            <label>Track Title (Messy / Incomplete)</label>
            <input type="text" id="input-title" value="Starboy (Official Music Video) [4K] ft. Daft Punk" placeholder="e.g. Starboy (Official Video)">
          </div>
          <div class="form-group">
            <label>Artist(s)</label>
            <input type="text" id="input-artist" value="The Weeknd" placeholder="e.g. The Weeknd">
          </div>
          <div class="form-group">
            <label>Album (Optional)</label>
            <input type="text" id="input-album" value="" placeholder="Leave empty for auto-discovery">
          </div>
          <div class="form-group">
            <label>Source</label>
            <select id="input-source">
              <option value="LOCAL" selected>LOCAL (Device)</option>
              <option value="YOUTUBE">YOUTUBE</option>
              <option value="LUCIDA">LUCIDA</option>
            </select>
          </div>
        </div>

        <div style="display:flex; gap:12px; align-items:center;">
          <button class="btn" id="btn-enrich" onclick="runSingleEnrich()">
            <svg width="18" height="18" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z"/></svg>
            PUT /song (Deep Search)
          </button>
          <button class="btn btn-secondary" onclick="presetSample('blinding')">Preset: Blinding Lights</button>
          <button class="btn btn-secondary" onclick="presetSample('eminem')">Preset: Eminem ft. Rihanna</button>
          <button class="btn btn-secondary" onclick="presetSample('pava')">Preset: Pa Va (Podimeesha)</button>
        </div>

        <!-- Result Container -->
        <div id="single-result" class="result-container">
          <div class="song-preview-card">
            <img id="preview-art" class="album-art" src="" alt="Album Art">
            <div class="song-meta-header">
              <div class="meta-title" id="preview-title">Track Title</div>
              <div class="meta-artist" id="preview-artist">Artist / Collaborator</div>
              <div class="meta-album-info" id="preview-album">Album &bull; 2024</div>
              <div class="badges-row">
                <span class="pill pill-green" id="badge-metachecked">&check; metachecked = true</span>
                <span class="pill pill-blue" id="badge-status">status: CONFIRMED</span>
                <span class="pill pill-pink" id="badge-lyrics">&hearts; lyrics available</span>
                <span class="pill pill-cyan" id="badge-mood">Mood: Chill</span>
                <span class="pill pill-blue" id="badge-lang">Lang: English</span>
              </div>
            </div>
          </div>

          <h3 style="font-size:1rem; margin-bottom:12px; font-weight:700;">Complete TrackEntity Model Output (OfflineMusic App Ready)</h3>
          <div class="table-container">
            <table>
              <thead>
                <tr>
                  <th>Field</th>
                  <th>Value</th>
                  <th>Status &amp; Verification</th>
                </tr>
              </thead>
              <tbody id="metadata-table-body">
              </tbody>
            </table>
          </div>
        </div>
      </div>
    </div>

    <!-- TAB 2: Lyrica Karaoke Player -->
    <div id="tab-lyrics" class="tab-content">
      <div class="card">
        <div class="card-title">
          <span>Lyrica Synced Lyrics Engine (.lrc)</span>
        </div>
        <div class="card-desc">
          Aggregates LRCLIB &amp; NetEase with intelligent line-synced fallback, timestamp validation, and direct .lrc file generation.
        </div>

        <div class="form-grid">
          <div class="form-group">
            <label>Song Title</label>
            <input type="text" id="lrc-title" value="Starboy" placeholder="Song title">
          </div>
          <div class="form-group">
            <label>Artist</label>
            <input type="text" id="lrc-artist" value="The Weeknd" placeholder="Artist name">
          </div>
        </div>

        <div style="display:flex; gap:12px; margin-bottom:20px; align-items:center;">
          <button class="btn" id="btn-fetch-lyric" onclick="fetchLyric()">
            <svg width="18" height="18" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"/></svg>
            PUT /lyric (Fetch LRC)
          </button>
          <button class="btn btn-secondary" id="btn-play-karaoke" onclick="toggleKaraokePlay()" disabled>
            <svg width="18" height="18" fill="currentColor" viewBox="0 0 24 24"><path d="M8 5v14l11-7z"/></svg>
            Simulate Karaoke Playback
          </button>
          <a class="btn btn-secondary" id="btn-download-lrc" href="#" target="_blank" style="display:none;">
            Download .lrc File
          </a>
        </div>

        <div id="karaoke-box" class="karaoke-player">
          <div style="text-align:center; color:var(--text-dim); padding-top:120px;">
            Click "PUT /lyric" to load synchronized lyrics from LRCLIB / NetEase.
          </div>
        </div>
      </div>
    </div>

    <!-- TAB 3: Batch Enricher -->
    <div id="tab-batch" class="tab-content">
      <div class="card">
        <div class="card-title">
          <span>Batch Deep Enricher &bull; Songs &amp; Lyrics</span>
        </div>
        <div class="card-desc">
          Send entire playlists or library slices via <code>PUT /songs</code> or <code>PUT /lyrics</code>. Parallelized with bounded concurrency semaphore.
        </div>

        <div class="form-group" style="margin-bottom:16px;">
          <label>Batch Input JSON (Array of TrackEntity objects)</label>
          <textarea id="batch-input"></textarea>
        </div>

        <div style="display:flex; gap:12px; margin-bottom:20px;">
          <button class="btn" id="btn-batch-songs" onclick="runBatchSongs()">
            PUT /songs (Batch Songs)
          </button>
          <button class="btn btn-secondary" id="btn-batch-lyrics" onclick="runBatchLyrics()">
            PUT /lyrics (Batch Lyrics)
          </button>
          <button class="btn btn-secondary" onclick="loadSampleBatch()">Load Sample Library</button>
        </div>

        <div id="batch-results-area" style="display:none;">
          <h3 style="font-size:1rem; margin-bottom:12px; font-weight:700;">Batch Results</h3>
          <div class="table-container">
            <table>
              <thead>
                <tr>
                  <th>#</th>
                  <th>Title</th>
                  <th>Artists (" / ")</th>
                  <th>Album</th>
                  <th>Year</th>
                  <th>Genre</th>
                  <th>Credits</th>
                  <th>Lyrics</th>
                </tr>
              </thead>
              <tbody id="batch-table-body"></tbody>
            </table>
          </div>
        </div>
      </div>
    </div>

    <!-- TAB 4: Android Docs -->
    <div id="tab-docs" class="tab-content">
      <div class="card">
        <div class="card-title"><span>API Endpoints &amp; Android Retrofit Example</span></div>
        <div class="card-desc">Seamlessly plugs into your OfflineMusic repository (TrackEntity.kt &amp; Song.kt).</div>

        <h4 style="color:#a5b4fc; margin-bottom:8px;">1. Endpoints Implemented</h4>
        <div class="table-container" style="margin-bottom:20px;">
          <table>
            <thead><tr><th>Method</th><th>Endpoint</th><th>Description</th></tr></thead>
            <tbody>
              <tr><td><code>PUT</code> / <code>POST</code></td><td><span class="code-val">/song</span></td><td>Enrich single song metadata (deep search, clean title, format " / " artists, creative credits)</td></tr>
              <tr><td><code>PUT</code> / <code>POST</code></td><td><span class="code-val">/songs</span></td><td>Batch enrich song metadata array (concurrency controlled)</td></tr>
              <tr><td><code>PUT</code> / <code>POST</code> / <code>GET</code></td><td><span class="code-val">/lyric</span></td><td>Get synced LRC / plain lyrics (LRCLIB + NetEase multi-source)</td></tr>
              <tr><td><code>PUT</code> / <code>POST</code></td><td><span class="code-val">/lyrics</span></td><td>Batch retrieve lyrics for multiple songs</td></tr>
              <tr><td><code>GET</code></td><td><span class="code-val">/lyric/file?id={id}&amp;title=...&amp;artist=...</span></td><td>Direct .lrc file download (sets <code>text/plain; charset=utf-8</code> for <code>lyricsUri</code>)</td></tr>
              <tr><td><code>GET</code></td><td><span class="code-val">/health</span></td><td>Telemetry, uptime, and cache statistics</td></tr>
            </tbody>
          </table>
        </div>

        <h4 style="color:#a5b4fc; margin-bottom:8px;">2. OfflineMusic Android Kotlin Snippet with Authorization</h4>
        <pre><code>// 1. Configure OkHttpClient with Secret API Key (or Basic Auth)
val okHttpClient = OkHttpClient.Builder()
    .addInterceptor { chain ->
        val original = chain.request()
        val request = original.newBuilder()
            .header("X-API-Key", "metaenricher_secret_key") // Your configured API_KEY
            // Or Basic Auth: .header("Authorization", Credentials.basic("admin", "metaenricher_secret"))
            .build()
        chain.proceed(request)
    }
    .build()

// 2. Retrofit API Definition
interface MetaEnricherApi {
    @PUT("song")
    suspend fun enrichSong(@Body track: TrackEntity): SingleSongResponse

    @PUT("songs")
    suspend fun enrichSongsBatch(@Body tracks: List&lt;TrackEntity&gt;): BatchSongResponse

    @PUT("lyric")
    suspend fun getLyrics(@Body query: LyricQuery): LyricResponse

    @PUT("lyrics")
    suspend fun getLyricsBatch(@Body queries: List&lt;LyricQuery&gt;): BatchLyricResponse

    @GET("lyric/file")
    suspend fun downloadLrc(
        @Query("title") title: String, 
        @Query("artist") artist: String,
        @Query("key") apiKey: String = "metaenricher_secret_key"
    ): okhttp3.ResponseBody
}

// 3. In your Repository or Worker:
val enriched = api.enrichSong(trackEntity)
if (enriched.success) {
    trackDao.updateTrack(enriched.song) // Saves metachecked=true, CONFIRMED status, canonical " / " artists
}</code></pre>
      </div>
    </div>
  </div>

  <script>
    // Tab Switching
    function switchTab(name) {
      document.querySelectorAll('.tab-btn').forEach(b => b.classList.remove('active'));
      document.querySelectorAll('.tab-content').forEach(c => c.classList.remove('active'));
      event.currentTarget.classList.add('active');
      document.getElementById('tab-' + name).classList.add('active');
    }

    // Refresh Server Stats
    async function updateStats() {
      try {
        const res = await fetch('/health');
        if (res.ok) {
          const data = await res.json();
          document.getElementById('stat-song-count').textContent = data.totalSongRequests;
          document.getElementById('stat-lyric-count').textContent = data.totalLyricRequests;
          document.getElementById('stat-meta-cache').textContent = data.metadataCacheSize;
          document.getElementById('stat-uptime').textContent = data.uptimeSeconds + 's';
        }
      } catch (e) {}
    }
    setInterval(updateStats, 3000);
    updateStats();

    // Preset Buttons
    function presetSample(type) {
      if (type === 'blinding') {
        document.getElementById('input-title').value = "Blinding Lights [Official Audio]";
        document.getElementById('input-artist').value = "The Weeknd";
      } else if (type === 'eminem') {
        document.getElementById('input-title').value = "Love The Way You Lie (Music Video) feat. Rihanna";
        document.getElementById('input-artist').value = "Eminem";
      } else if (type === 'pava') {
        document.getElementById('input-title').value = "podimmesha mulakkana kaalam";
        document.getElementById('input-artist').value = "Pa Va, P Jayachandran, Santhosh Varma, Anand Madhusoodanan";
      }
    }

    // Security & API Key Helpers
    function getApiKey() {
      return localStorage.getItem('metaenricher_api_key') || 'metaenricher_secret_key';
    }

    function saveApiKey(val) {
      localStorage.setItem('metaenricher_api_key', val.trim());
    }

    function toggleKeyVisibility() {
      const el = document.getElementById('global-api-key');
      el.type = el.type === 'password' ? 'text' : 'password';
    }

    document.getElementById('global-api-key').value = getApiKey();

    async function secureFetch(url, options = {}) {
      options.headers = options.headers || {};
      options.headers['X-API-Key'] = getApiKey();
      const res = await fetch(url, options);
      if (res.status === 401) {
        alert("Unauthorized (401)! Please enter the correct API Key in the top header bar.");
        throw new Error("Unauthorized");
      }
      if (res.status === 429) {
        alert("Rate Limit Exceeded (429)! MetaEnricher allows up to 60 requests/minute. Please wait a moment.");
        throw new Error("Rate limit exceeded");
      }
      return res;
    }

    // Single Song Enricher
    async function runSingleEnrich() {
      const btn = document.getElementById('btn-enrich');
      const origText = btn.innerHTML;
      btn.disabled = true;
      btn.innerHTML = '<span class="loader"></span> Deep Searching...';

      const payload = {
        title: document.getElementById('input-title').value,
        artist: document.getElementById('input-artist').value,
        album: document.getElementById('input-album').value || null,
        source: document.getElementById('input-source').value,
        durationMs: 230000
      };

      try {
        const res = await secureFetch('/song', {
          method: 'PUT',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(payload)
        });
        const data = await res.json();
        renderSingleResult(data);
        updateStats();
      } catch (err) {
        alert('Enrichment failed: ' + err);
      } finally {
        btn.disabled = false;
        btn.innerHTML = origText;
      }
    }

    function renderSingleResult(data) {
      const s = data.song;
      const a = data.audit;

      document.getElementById('single-result').style.display = 'block';
      document.getElementById('preview-art').src = s.artworkUri || 'https://images.unsplash.com/photo-1511671782779-c97d3d27a1d4?w=400';
      document.getElementById('preview-title').textContent = s.title;
      document.getElementById('preview-artist').textContent = s.artist;
      document.getElementById('preview-album').textContent = (s.album || 'Unknown Album') + ' \u2022 ' + (s.year || 'Unknown Year');

      document.getElementById('badge-metachecked').innerHTML = s.metachecked ? '&check; metachecked = true' : '&times; metachecked = false';
      document.getElementById('badge-status').textContent = 'status: ' + s.metadataStatus;
      document.getElementById('badge-lyrics').textContent = s.lyricsAvailable ? '\u2665 lyrics available' : 'No lyrics';
      document.getElementById('badge-mood').textContent = 'Mood: ' + (s.mood || 'N/A');
      document.getElementById('badge-lang').textContent = 'Lang: ' + (s.language || 'N/A');

      const tbody = document.getElementById('metadata-table-body');
      tbody.innerHTML = '';

      const fields = [
        ['title', s.title],
        ['artist', s.artist],
        ['album', s.album || '—'],
        ['year', s.year || '—'],
        ['genre', s.genre || '—'],
        ['trackNumber', s.trackNumber || '—'],
        ['producer', s.producer || '—'],
        ['composer', s.composer || '—'],
        ['language', s.language || '—'],
        ['mood', s.mood || '—'],
        ['source', s.source],
        ['lyrics_available', s.lyricsAvailable.toString()],
        ['metachecked', s.metachecked.toString()],
        ['artworkUri', s.artworkUri ? s.artworkUri.substring(0, 50) + '...' : '—'],
        ['lyricsUri', s.lyricsUri || '—'],
        ['metadataStatus', s.metadataStatus]
      ];

      for (const [key, val] of fields) {
        const tr = document.createElement('tr');
        let statusTag = '';
        if (a.fieldsCorrected && a.fieldsCorrected.includes(key)) {
          statusTag = '<span class="tag-corrected">CORRECTED</span>';
        } else if (a.fieldsUpdated && a.fieldsUpdated.includes(key)) {
          statusTag = '<span class="tag-updated">DISCOVERED</span>';
        }
        tr.innerHTML = `
          <td style="font-weight:600; font-family:var(--mono);">${key}</td>
          <td>${val}</td>
          <td>${statusTag || '<span style="color:var(--text-dim);">Verified</span>'}</td>
        `;
        tbody.appendChild(tr);
      }
    }

    // LRC Karaoke State
    let parsedLrc = [];
    let karaokeTimer = null;
    let karaokePlaying = false;
    let currentLrcIndex = 0;

    async function fetchLyric() {
      const title = document.getElementById('lrc-title').value;
      const artist = document.getElementById('lrc-artist').value;
      const btn = document.getElementById('btn-fetch-lyric');
      const origText = btn.innerHTML;
      btn.disabled = true;
      btn.innerHTML = '<span class="loader"></span> Fetching...';

      try {
        const res = await secureFetch('/lyric', {
          method: 'PUT',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ title, artist })
        });
        const data = await res.json();
        renderLrc(data);
        updateStats();
      } catch (e) {
        alert('Failed: ' + e);
      } finally {
        btn.disabled = false;
        btn.innerHTML = origText;
      }
    }

    function renderLrc(data) {
      const box = document.getElementById('karaoke-box');
      box.innerHTML = '';
      parsedLrc = [];

      if (!data.lyricsAvailable || (!data.syncedLyrics && !data.plainLyrics)) {
        box.innerHTML = '<div style="text-align:center; padding-top:120px; color:var(--text-dim);">No lyrics found for this song.</div>';
        document.getElementById('btn-play-karaoke').disabled = true;
        document.getElementById('btn-download-lrc').style.display = 'none';
        return;
      }

      const raw = data.syncedLyrics || data.plainLyrics;
      const lines = raw.split('\n');

      const timeRe = /\[(\d{2}):(\d{2}(?:\.\d{2,3})?)\]/;
      lines.forEach((line, i) => {
        const match = timeRe.exec(line);
        let seconds = null;
        let text = line.replace(timeRe, '').trim();

        if (match) {
          const mins = parseInt(match[1]);
          const secs = parseFloat(match[2]);
          seconds = mins * 60 + secs;
        }

        if (text || seconds !== null) {
          parsedLrc.push({ seconds, text, index: i });
          const row = document.createElement('div');
          row.className = 'lrc-line';
          row.id = 'lrc-line-' + i;
          row.innerHTML = `<span class="lrc-time">${match ? match[0] : ''}</span> <span>${text || '...'}</span>`;
          row.onclick = () => jumpToLine(i);
          box.appendChild(row);
        }
      });

      document.getElementById('btn-play-karaoke').disabled = parsedLrc.length === 0;
      const dlBtn = document.getElementById('btn-download-lrc');
      dlBtn.style.display = 'inline-flex';
      dlBtn.href = `/lyric/file?title=${encodeURIComponent(data.title)}&artist=${encodeURIComponent(data.artist)}`;
    }

    function toggleKaraokePlay() {
      const btn = document.getElementById('btn-play-karaoke');
      if (karaokePlaying) {
        clearInterval(karaokeTimer);
        karaokePlaying = false;
        btn.innerHTML = '<svg width="18" height="18" fill="currentColor" viewBox="0 0 24 24"><path d="M8 5v14l11-7z"/></svg> Simulate Karaoke Playback';
      } else {
        karaokePlaying = true;
        btn.innerHTML = '<svg width="18" height="18" fill="currentColor" viewBox="0 0 24 24"><path d="M6 19h4V5H6v14zm8-14v14h4V5h-4z"/></svg> Pause Playback';
        currentLrcIndex = 0;
        karaokeTimer = setInterval(() => {
          if (currentLrcIndex >= parsedLrc.length) {
            clearInterval(karaokeTimer);
            karaokePlaying = false;
            return;
          }
          highlightLine(currentLrcIndex);
          currentLrcIndex++;
        }, 1800);
      }
    }

    function highlightLine(idx) {
      document.querySelectorAll('.lrc-line').forEach(l => l.classList.remove('active'));
      const activeEl = document.getElementById('lrc-line-' + parsedLrc[idx].index);
      if (activeEl) {
        activeEl.classList.add('active');
        activeEl.scrollIntoView({ behavior: 'smooth', block: 'center' });
      }
    }

    function jumpToLine(idx) {
      highlightLine(idx);
      currentLrcIndex = idx;
    }

    // Batch Sample Data
    function loadSampleBatch() {
      const sample = [
        {
          title: "Starboy (Official Video) ft. Daft Punk",
          artist: "The Weeknd",
          source: "LOCAL",
          durationMs: 230000
        },
        {
          title: "Levitating [Clean Audio]",
          artist: "Dua Lipa & DaBaby",
          source: "YOUTUBE",
          durationMs: 203000
        },
        {
          title: "Stay (Lyrics)",
          artist: "The Kid LAROI and Justin Bieber",
          source: "LOCAL",
          durationMs: 141000
        }
      ];
      document.getElementById('batch-input').value = JSON.stringify(sample, null, 2);
    }
    loadSampleBatch();

    async function runBatchSongs() {
      const btn = document.getElementById('btn-batch-songs');
      btn.disabled = true;
      btn.innerHTML = '<span class="loader"></span> Processing Batch...';

      try {
        const raw = document.getElementById('batch-input').value;
        const tracks = JSON.parse(raw);
        const res = await secureFetch('/songs', {
          method: 'PUT',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(tracks)
        });
        const data = await res.json();
        renderBatchTable(data.tracks);
        updateStats();
      } catch (e) {
        alert('Error: ' + e);
      } finally {
        btn.disabled = false;
        btn.innerHTML = 'PUT /songs (Batch Songs)';
      }
    }

    async function runBatchLyrics() {
      const btn = document.getElementById('btn-batch-lyrics');
      btn.disabled = true;
      btn.innerHTML = '<span class="loader"></span> Fetching Lyrics...';

      try {
        const raw = document.getElementById('batch-input').value;
        const tracks = JSON.parse(raw);
        const res = await secureFetch('/lyrics', {
          method: 'PUT',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(tracks)
        });
        const data = await res.json();
        alert(`Retrieved lyrics for ${data.found} / ${data.total} tracks!`);
        updateStats();
      } catch (e) {
        alert('Error: ' + e);
      } finally {
        btn.disabled = false;
        btn.innerHTML = 'PUT /lyrics (Batch Lyrics)';
      }
    }

    function renderBatchTable(tracks) {
      document.getElementById('batch-results-area').style.display = 'block';
      const tbody = document.getElementById('batch-table-body');
      tbody.innerHTML = '';

      tracks.forEach((item, i) => {
        const s = item.song;
        const tr = document.createElement('tr');
        tr.innerHTML = `
          <td>${i + 1}</td>
          <td style="font-weight:600;">${s.title}</td>
          <td style="color:#a5b4fc;">${s.artist}</td>
          <td>${s.album || '—'}</td>
          <td>${s.year || '—'}</td>
          <td>${s.genre || '—'}</td>
          <td style="font-size:0.75rem;">${(s.producer || s.composer) ? (s.producer || s.composer).substring(0, 24) + '...' : '—'}</td>
          <td>${s.lyricsAvailable ? '<span class="pill pill-green">&check; LRC</span>' : '<span class="pill pill-blue">&times; None</span>'}</td>
        `;
        tbody.appendChild(tr);
      });
    }
  </script>
</body>
</html>
"###;
