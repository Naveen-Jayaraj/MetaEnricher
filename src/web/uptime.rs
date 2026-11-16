use crate::services::uptime::{UptimeLogEntry, UptimeSummary};
use crate::AppState;
use axum::{
    extract::{Query, State},
    response::Html,
    Json,
};
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
pub struct LogParams {
    pub limit: Option<usize>,
}

pub async fn uptime_handler() -> Html<&'static str> {
    Html(UPTIME_HTML)
}

pub async fn uptime_logs_handler(
    State(state): State<AppState>,
    Query(params): Query<LogParams>,
) -> Json<Vec<UptimeLogEntry>> {
    let limit = params.limit.unwrap_or(100).min(500);
    let logs = state.uptime.get_logs(limit).await;
    Json(logs)
}

pub async fn uptime_stats_handler(State(state): State<AppState>) -> Json<UptimeSummary> {
    let summary = state.uptime.get_summary().await;
    Json(summary)
}

pub async fn manual_purge_handler(State(state): State<AppState>) -> Json<serde_json::Value> {
    let m = state.enricher.purge_memory_cache().await;
    let (l1, l2) = state.lyrics.purge_memory_cache().await;
    state.uptime.record_purge(m, l1 + l2).await;

    Json(json!({
        "success": true,
        "message": "Render in-memory cache purged. Metadata held exclusively in Neon DB.",
        "purgedMetadata": m,
        "purgedLyrics": l1 + l2
    }))
}

const UPTIME_HTML: &str = r###"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>System Uptime &amp; Dual-Storage Telemetry &bull; MetaEnricher</title>
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
  <link href="https://fonts.googleapis.com/css2?family=Outfit:wght@300;400;500;600;700;800&family=JetBrains+Mono:wght@400;500;700&display=swap" rel="stylesheet">
  <style>
    :root {
      --bg: #07090e;
      --bg-card: rgba(16, 22, 34, 0.75);
      --bg-card-hover: rgba(24, 32, 50, 0.9);
      --border: rgba(255, 255, 255, 0.08);
      --border-accent: rgba(99, 102, 241, 0.4);
      --primary: #6366f1;
      --primary-glow: rgba(99, 102, 241, 0.35);
      --secondary: #ec4899;
      --accent: #06b6d4;
      --success: #10b981;
      --warning: #f59e0b;
      --danger: #ef4444;
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
        radial-gradient(at 100% 0%, rgba(6, 182, 212, 0.12) 0px, transparent 50%),
        radial-gradient(at 50% 100%, rgba(236, 72, 153, 0.10) 0px, transparent 50%);
      background-attachment: fixed;
      color: var(--text);
      font-family: var(--font);
      min-height: 100vh;
      line-height: 1.5;
    }

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
      margin-bottom: 28px;
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
      background: linear-gradient(135deg, var(--accent), var(--primary));
      border-radius: var(--radius-sm);
      display: flex;
      align-items: center;
      justify-content: center;
      box-shadow: 0 0 20px rgba(6, 182, 212, 0.35);
    }

    .brand-icon svg { width: 24px; height: 24px; fill: white; }

    .brand-title {
      font-size: 1.45rem;
      font-weight: 800;
      letter-spacing: -0.02em;
      background: linear-gradient(135deg, #ffffff 40%, #a5f3fc);
      -webkit-background-clip: text;
      -webkit-text-fill-color: transparent;
    }

    .brand-sub {
      font-size: 0.8rem;
      color: var(--text-dim);
      font-weight: 400;
    }

    .header-actions {
      display: flex;
      align-items: center;
      gap: 12px;
      flex-wrap: wrap;
    }

    .nav-btn {
      display: inline-flex;
      align-items: center;
      gap: 8px;
      background: rgba(255, 255, 255, 0.06);
      border: 1px solid var(--border);
      color: var(--text);
      text-decoration: none;
      padding: 8px 16px;
      border-radius: var(--radius-sm);
      font-size: 0.85rem;
      font-weight: 500;
      transition: all 0.2s ease;
      cursor: pointer;
    }

    .nav-btn:hover {
      background: rgba(255, 255, 255, 0.12);
      border-color: var(--border-accent);
      color: #fff;
    }

    .nav-btn.primary {
      background: linear-gradient(135deg, var(--primary), #4f46e5);
      border-color: transparent;
      color: #fff;
    }

    .nav-btn.primary:hover {
      box-shadow: 0 0 15px var(--primary-glow);
    }

    .status-badge {
      display: inline-flex;
      align-items: center;
      gap: 8px;
      padding: 6px 14px;
      background: rgba(16, 185, 129, 0.12);
      border: 1px solid rgba(16, 185, 129, 0.3);
      color: #34d399;
      border-radius: 9999px;
      font-size: 0.8rem;
      font-weight: 600;
      font-family: var(--mono);
    }

    .status-dot {
      width: 8px;
      height: 8px;
      background-color: var(--success);
      border-radius: 50%;
      box-shadow: 0 0 8px var(--success);
      animation: pulse 2s infinite;
    }

    .status-dot.dot-amber {
      background-color: var(--warning);
      box-shadow: 0 0 8px var(--warning);
    }

    @keyframes pulse {
      0%, 100% { opacity: 1; transform: scale(1); }
      50% { opacity: 0.4; transform: scale(0.85); }
    }

    /* Redundancy Banner */
    .banner {
      background: linear-gradient(135deg, rgba(99, 102, 241, 0.12), rgba(6, 182, 212, 0.12));
      border: 1px solid var(--border-accent);
      border-radius: var(--radius-md);
      padding: 16px 20px;
      margin-bottom: 24px;
      display: flex;
      justify-content: space-between;
      align-items: center;
      gap: 16px;
      flex-wrap: wrap;
    }

    .banner-title {
      font-size: 0.95rem;
      font-weight: 700;
      color: #fff;
      display: flex;
      align-items: center;
      gap: 8px;
      margin-bottom: 4px;
    }

    .banner-desc {
      font-size: 0.8rem;
      color: var(--text-muted);
    }

    /* Grid of status cards */
    .metrics-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
      gap: 16px;
      margin-bottom: 28px;
    }

    .metric-card {
      background: var(--bg-card);
      backdrop-filter: blur(16px);
      border: 1px solid var(--border);
      border-radius: var(--radius-md);
      padding: 20px;
      position: relative;
      overflow: hidden;
      transition: transform 0.2s, border-color 0.2s;
    }

    .metric-card:hover {
      border-color: var(--border-accent);
      transform: translateY(-2px);
    }

    .metric-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-bottom: 12px;
    }

    .metric-service-title {
      font-size: 0.8rem;
      text-transform: uppercase;
      letter-spacing: 0.08em;
      color: var(--text-dim);
      font-weight: 700;
    }

    .service-pill {
      font-size: 0.7rem;
      padding: 3px 8px;
      border-radius: 6px;
      font-weight: 700;
      font-family: var(--mono);
    }

    .pill-green { background: rgba(16, 185, 129, 0.15); color: #34d399; border: 1px solid rgba(16, 185, 129, 0.3); }
    .pill-blue { background: rgba(6, 182, 212, 0.15); color: #38bdf8; border: 1px solid rgba(6, 182, 212, 0.3); }
    .pill-purple { background: rgba(168, 85, 247, 0.15); color: #c084fc; border: 1px solid rgba(168, 85, 247, 0.3); }
    .pill-amber { background: rgba(245, 158, 11, 0.15); color: #fbbf24; border: 1px solid rgba(245, 158, 11, 0.3); }

    .metric-value {
      font-size: 1.85rem;
      font-weight: 800;
      letter-spacing: -0.02em;
      margin-bottom: 6px;
      font-family: var(--mono);
    }

    .metric-sub {
      font-size: 0.8rem;
      color: var(--text-muted);
    }

    .metric-details {
      margin-top: 14px;
      padding-top: 12px;
      border-top: 1px solid rgba(255, 255, 255, 0.05);
      font-size: 0.75rem;
      color: var(--text-dim);
      display: flex;
      flex-direction: column;
      gap: 4px;
      font-family: var(--mono);
    }

    /* Logs Section */
    .card {
      background: var(--bg-card);
      backdrop-filter: blur(16px);
      border: 1px solid var(--border);
      border-radius: var(--radius-lg);
      padding: 24px;
      margin-bottom: 30px;
    }

    .card-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-bottom: 20px;
      flex-wrap: wrap;
      gap: 12px;
    }

    .card-title {
      font-size: 1.15rem;
      font-weight: 700;
      display: flex;
      align-items: center;
      gap: 10px;
    }

    .table-container {
      overflow-x: auto;
      border: 1px solid var(--border);
      border-radius: var(--radius-sm);
    }

    table {
      width: 100%;
      border-collapse: collapse;
      text-align: left;
      font-size: 0.82rem;
    }

    th {
      background: rgba(255, 255, 255, 0.03);
      padding: 12px 14px;
      font-weight: 600;
      color: var(--text-dim);
      text-transform: uppercase;
      font-size: 0.72rem;
      letter-spacing: 0.05em;
      border-bottom: 1px solid var(--border);
    }

    td {
      padding: 12px 14px;
      border-bottom: 1px solid rgba(255, 255, 255, 0.04);
      color: var(--text);
    }

    tr:hover td {
      background: rgba(255, 255, 255, 0.02);
    }

    .tag-event {
      display: inline-block;
      padding: 2px 8px;
      border-radius: 4px;
      font-size: 0.7rem;
      font-weight: 700;
      font-family: var(--mono);
    }

    .event-heartbeat { background: rgba(16, 185, 129, 0.15); color: #34d399; }
    .event-purge { background: rgba(168, 85, 247, 0.18); color: #d8b4fe; }
    .event-startup { background: rgba(99, 102, 241, 0.18); color: #a5b4fc; }
    .event-alert { background: rgba(239, 68, 68, 0.18); color: #f87171; }

    .loader {
      display: inline-block;
      width: 14px;
      height: 14px;
      border: 2px solid rgba(255,255,255,0.3);
      border-radius: 50%;
      border-top-color: #fff;
      animation: spin 0.8s linear infinite;
    }

    @keyframes spin {
      to { transform: rotate(360deg); }
    }
  </style>
</head>
<body>
  <div class="container">
    <header>
      <div class="brand">
        <div class="brand-icon">
          <svg viewBox="0 0 24 24"><path d="M13 3L4 14h7v7l9-11h-7z"/></svg>
        </div>
        <div>
          <div class="brand-title">System Telemetry &amp; Dual Uptime</div>
          <div class="brand-sub">Render Service &bull; Neon Serverless PostgreSQL &bull; Cross-Redundant Logs</div>
        </div>
      </div>
      <div class="header-actions">
        <a href="/" class="nav-btn">&larr; Main Dashboard</a>
        <button class="nav-btn primary" onclick="refreshAll(true)" id="btn-refresh">
          <svg width="14" height="14" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"/></svg>
          Refresh Telemetry
        </button>
        <span class="status-badge" id="status-badge">
          <span class="status-dot"></span>
          Render &bull; Neon Synced
        </span>
      </div>
    </header>

    <!-- Failover & Redundancy Policy Banner -->
    <div class="banner">
      <div>
        <div class="banner-title">
          <svg width="18" height="18" fill="none" stroke="#a5b4fc" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z"/></svg>
          Dual Uptime Redundancy &amp; Stateless Policy Active
        </div>
        <div class="banner-desc">
          Both Render and Neon DB maintain copies of each other's uptime logs. If Render restarts, historical logs load from Neon. If Neon is offline, Render logs locally. In-memory metadata on Render is automatically deleted periodically to preserve memory, with Neon serving as the authoritative permanent database.
        </div>
      </div>
      <div>
        <button class="nav-btn" onclick="triggerManualPurge()" id="btn-purge">
          🧹 Purge Render Memory Now
        </button>
      </div>
    </div>

    <!-- Metrics Grid -->
    <div class="metrics-grid">
      <!-- Card 1: Render Instance -->
      <div class="metric-card">
        <div class="metric-header">
          <span class="metric-service-title">Render Web Engine</span>
          <span class="service-pill pill-green" id="card-render-pill">ONLINE</span>
        </div>
        <div class="metric-value" id="card-render-uptime">0s</div>
        <div class="metric-sub">Continuous process uptime</div>
        <div class="metric-details">
          <span>HOST: Render Free Tier (Docker)</span>
          <span>STORAGE: Ephemeral (Purged every 2m)</span>
        </div>
      </div>

      <!-- Card 2: Neon PostgreSQL -->
      <div class="metric-card">
        <div class="metric-header">
          <span class="metric-service-title">Neon Database</span>
          <span class="service-pill pill-blue" id="card-neon-pill">CONNECTED</span>
        </div>
        <div class="metric-value" id="card-neon-latency">0 ms</div>
        <div class="metric-sub">Round-trip pooler ping latency</div>
        <div class="metric-details">
          <span>HOST: ep-flat-heart-bbtb6258</span>
          <span>REGION: AWS us-east-1 (SSL+SCRAM)</span>
        </div>
      </div>

      <!-- Card 3: Authoritative Neon Cache -->
      <div class="metric-card">
        <div class="metric-header">
          <span class="metric-service-title">Authoritative Cloud Cache</span>
          <span class="service-pill pill-purple">PERSISTENT</span>
        </div>
        <div class="metric-value" id="card-neon-records">0 / 0</div>
        <div class="metric-sub">Metadata &amp; Lyrics cached in Neon</div>
        <div class="metric-details">
          <span>TABLES: metadata_cache &bull; lyrics_cache</span>
          <span>DATA RETENTION: Permanent</span>
        </div>
      </div>

      <!-- Card 4: Dual Storage Sync -->
      <div class="metric-card">
        <div class="metric-header">
          <span class="metric-service-title">Dual Store Logs</span>
          <span class="service-pill pill-amber" id="card-sync-pill">SYNCED</span>
        </div>
        <div class="metric-value" id="card-logs-count">0 logs</div>
        <div class="metric-sub">Uptime heartbeats preserved</div>
        <div class="metric-details">
          <span id="card-local-logs">Render Local: 0</span>
          <span id="card-db-logs">Neon DB: 0</span>
        </div>
      </div>
    </div>

    <!-- Logs Table Card -->
    <div class="card">
      <div class="card-header">
        <div class="card-title">
          <span>Uptime &amp; Heartbeat Historical Audit Log</span>
          <span style="font-size:0.75rem; color:var(--text-dim); font-weight:400;">(Auto-refreshed every 5s)</span>
        </div>
        <div style="font-size:0.8rem; color:var(--text-muted); font-family:var(--mono);" id="last-updated">
          Updating...
        </div>
      </div>

      <div class="table-container">
        <table>
          <thead>
            <tr>
              <th>Timestamp (UTC)</th>
              <th>Render Status</th>
              <th>Render Uptime</th>
              <th>Neon DB Status</th>
              <th>Neon Latency</th>
              <th>Neon Cached (Meta/Lrc)</th>
              <th>Event</th>
              <th>Diagnostics &amp; Details</th>
            </tr>
          </thead>
          <tbody id="logs-table-body">
            <tr><td colspan="8" style="text-align:center; padding:30px; color:var(--text-dim);">Loading logs...</td></tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>

  <script>
    async function fetchSummary() {
      try {
        const res = await fetch('/api/uptime/stats', { cache: 'no-store' });
        if (res.ok) {
          const s = await res.json();
          document.getElementById('card-render-uptime').textContent = s.renderUptimeFormatted || (s.renderUptimeSeconds + 's');
          document.getElementById('card-neon-latency').textContent = (s.neonLatencyMs || 0) + ' ms';
          document.getElementById('card-neon-records').textContent = `${s.neonMetadataCached || 0} / ${s.neonLyricsCached || 0}`;
          document.getElementById('card-logs-count').textContent = (s.databaseLogsCount || s.localLogsCount || 0) + ' entries';
          document.getElementById('card-local-logs').textContent = `Render Local: ${s.localLogsCount || 0}`;
          document.getElementById('card-db-logs').textContent = `Neon DB: ${s.databaseLogsCount || 0}`;

          const neonPill = document.getElementById('card-neon-pill');
          if (s.neonStatus === 'ONLINE') {
            neonPill.className = 'service-pill pill-blue';
            neonPill.textContent = 'ONLINE';
          } else {
            neonPill.className = 'service-pill pill-amber';
            neonPill.textContent = s.neonStatus || 'OFFLINE';
          }
        }
      } catch (e) {
        console.warn('Failed to fetch summary:', e);
      }
    }

    async function fetchLogs() {
      try {
        const res = await fetch('/api/uptime/logs?limit=50', { cache: 'no-store' });
        if (res.ok) {
          const logs = await res.json();
          renderTable(logs);
          document.getElementById('last-updated').textContent = 'Last synced: ' + new Date().toLocaleTimeString();
        }
      } catch (e) {
        console.warn('Failed to fetch logs:', e);
      }
    }

    function renderTable(logs) {
      const tbody = document.getElementById('logs-table-body');
      if (!logs || logs.length === 0) {
        tbody.innerHTML = '<tr><td colspan="8" style="text-align:center; padding:30px; color:var(--text-dim);">No uptime logs recorded yet.</td></tr>';
        return;
      }

      tbody.innerHTML = '';
      logs.forEach(item => {
        const tr = document.createElement('tr');
        
        let eventClass = 'event-heartbeat';
        if (item.eventType === 'CACHE_PURGE') eventClass = 'event-purge';
        else if (item.eventType === 'STARTUP') eventClass = 'event-startup';
        else if (item.eventType.includes('ALERT') || item.neonStatus !== 'ONLINE') eventClass = 'event-alert';

        const d = new Date(item.timestamp);
        const timeStr = d.toLocaleDateString() + ' ' + d.toLocaleTimeString();

        tr.innerHTML = `
          <td style="font-family:var(--mono); font-size:0.75rem; color:#c7d2fe;">${timeStr}</td>
          <td><span class="service-pill pill-green">${item.renderStatus}</span></td>
          <td style="font-family:var(--mono);">${item.renderUptimeSeconds}s</td>
          <td><span class="service-pill ${item.neonStatus === 'ONLINE' ? 'pill-blue' : 'pill-amber'}">${item.neonStatus}</span></td>
          <td style="font-family:var(--mono);">${item.neonLatencyMs ? item.neonLatencyMs + 'ms' : '—'}</td>
          <td style="font-family:var(--mono); color:#a5b4fc;">${item.neonMetadataCount} / ${item.neonLyricsCount}</td>
          <td><span class="tag-event ${eventClass}">${item.eventType}</span></td>
          <td style="font-size:0.78rem; color:var(--text-muted);">${item.message}</td>
        `;
        tbody.appendChild(tr);
      });
    }

    async function triggerManualPurge() {
      const btn = document.getElementById('btn-purge');
      const orig = btn.innerHTML;
      btn.disabled = true;
      btn.innerHTML = '<span class="loader"></span> Purging...';

      try {
        const res = await fetch('/api/cache/purge', { method: 'POST' });
        const data = await res.json();
        alert(`Memory Purge Complete!\nPurged ${data.purgedMetadata} metadata & ${data.purgedLyrics} lyrics entries from Render memory.\nAll data is preserved in Neon DB.`);
        refreshAll(false);
      } catch (e) {
        alert('Purge failed: ' + e);
      } finally {
        btn.disabled = false;
        btn.innerHTML = orig;
      }
    }

    function refreshAll(showAnimation = false) {
      if (showAnimation) {
        const btn = document.getElementById('btn-refresh');
        btn.innerHTML = '<span class="loader"></span> Syncing...';
        setTimeout(() => {
          btn.innerHTML = '<svg width="14" height="14" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"/></svg> Refresh Telemetry';
        }, 600);
      }
      fetchSummary();
      fetchLogs();
    }

    setInterval(() => refreshAll(false), 5000);
    refreshAll(false);
  </script>
</body>
</html>
"###;
