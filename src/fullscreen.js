// ============================================
//  Spotify Fullscreen — Now Playing (JS)
// ============================================

const { invoke } = window.__TAURI__.core;

// ---- State ----

let isPlaying = false;
let currentProgress = 0;
let currentDuration = 0;
let accentColor = [255, 255, 255];
let currentArtUrl = null;

// ---- DOM references ----

const fsBg = document.getElementById('fs-bg');
const fsAlbumArt = document.getElementById('fs-album-art');
const fsTrackName = document.getElementById('fs-track-name');
const fsArtistName = document.getElementById('fs-artist-name');
const fsBtnPrev = document.getElementById('fs-btn-prev');
const fsBtnPlay = document.getElementById('fs-btn-play');
const fsBtnNext = document.getElementById('fs-btn-next');
const fsPlayIcon = document.getElementById('fs-play-icon');
const fsProgressFill = document.getElementById('fs-progress-fill');
const fsProgressArea = document.getElementById('fs-progress-area');
const fsCurrentTime = document.getElementById('fs-current-time');
const fsTotalTime = document.getElementById('fs-total-time');
const fsPanel = document.getElementById('fs-panel');

// ---- Utilities ----

function formatTime(ms) {
  const totalSeconds = Math.floor(ms / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${seconds.toString().padStart(2, '0')}`;
}

// ---- Progress bar ----

function updateProgress() {
  if (currentDuration > 0) {
    const pct = Math.min(currentProgress / currentDuration, 1) * 100;
    fsProgressFill.style.width = `${pct}%`;
  } else {
    fsProgressFill.style.width = '0%';
  }
  fsCurrentTime.textContent = formatTime(currentProgress);
  fsTotalTime.textContent = formatTime(currentDuration);
}

// ---- Play/pause icon ----

function updatePlayIcon() {
  if (isPlaying) {
    fsPlayIcon.innerHTML = '<path d="M6 19h4V5H6v14zm8-14v14h4V5h-4z"/>';
  } else {
    fsPlayIcon.innerHTML = '<path d="M8 5v14l11-7z"/>';
  }
}

// ---- Dominant color extraction ----

function extractDominantColor(imgUrl) {
  const img = new Image();
  img.crossOrigin = 'anonymous';
  img.onload = () => {
    try {
      const canvas = document.createElement('canvas');
      const ctx = canvas.getContext('2d');
      canvas.width = 50;
      canvas.height = 50;
      ctx.drawImage(img, 0, 0, 50, 50);
      const data = ctx.getImageData(0, 0, 50, 50).data;

      let r = 0, g = 0, b = 0, count = 0;
      for (let i = 0; i < data.length; i += 16) {
        r += data[i];
        g += data[i + 1];
        b += data[i + 2];
        count++;
      }

      if (count > 0) {
        accentColor = [
          Math.round(r / count),
          Math.round(g / count),
          Math.round(b / count),
        ];
        applyAccentColor();
      }
    } catch (e) {
      console.error('Color extraction failed:', e);
    }
  };
  img.src = imgUrl;
}

// ---- Apply accent color ----

function applyAccentColor() {
  const [r, g, b] = accentColor;
  fsProgressFill.style.background = `rgb(${r}, ${g}, ${b})`;
}

// ---- UI update ----

function updateUI(state) {
  fsTrackName.textContent = state.track_name;
  fsArtistName.textContent = state.artist_name;

  currentProgress = state.progress_ms;
  currentDuration = state.duration_ms;
  isPlaying = state.is_playing;

  updateProgress();
  updatePlayIcon();

  if (state.album_art_url && state.album_art_url !== currentArtUrl) {
    currentArtUrl = state.album_art_url;

    fsBg.classList.add('fade-out');
    setTimeout(() => {
      fsBg.src = state.album_art_url;
      fsBg.onload = () => {
        fsBg.classList.remove('fade-out');
        fsBg.onload = null;
      };
    }, 300);

    fsAlbumArt.style.opacity = '0';
    setTimeout(() => {
      fsAlbumArt.src = state.album_art_url;
      fsAlbumArt.onload = () => {
        fsAlbumArt.style.opacity = '1';
        fsAlbumArt.onload = null;
      };
    }, 300);

    extractDominantColor(state.album_art_url);
  }
}

// ---- Polling ----

async function pollPlayback() {
  try {
    const state = await invoke('get_playback');
    if (state) updateUI(state);
  } catch (e) {
    console.error('Poll failed:', e);
  }
}

// ---- Progress interpolation ----

setInterval(() => {
  if (isPlaying && currentDuration > 0) {
    currentProgress += 100;
    if (currentProgress > currentDuration) currentProgress = currentDuration;
    updateProgress();
  }
}, 100);

// ---- Controls ----

fsBtnPlay.addEventListener('click', (e) => {
  e.stopPropagation();
  isPlaying = !isPlaying;
  updatePlayIcon();
  invoke('play_pause').catch((err) => console.error('play_pause failed:', err));
});

fsBtnNext.addEventListener('click', (e) => {
  e.stopPropagation();
  invoke('next_track')
    .then(() => setTimeout(pollPlayback, 200))
    .catch((err) => console.error('next_track failed:', err));
});

fsBtnPrev.addEventListener('click', (e) => {
  e.stopPropagation();
  invoke('previous_track')
    .then(() => setTimeout(pollPlayback, 200))
    .catch((err) => console.error('previous_track failed:', err));
});

// ---- Progress bar click to seek ----

fsProgressArea.addEventListener('click', async (e) => {
  e.stopPropagation();
  const bar = fsProgressArea.querySelector('.fs-progress-bar');
  const rect = bar.getBoundingClientRect();
  const pct = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
  const posMs = Math.floor(pct * currentDuration);
  try {
    await invoke('seek_to', { positionMs: posMs });
    currentProgress = posMs;
    updateProgress();
  } catch (err) { console.error(err); }
});

// ---- Exit handlers ----

document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') {
    invoke('exit_fullscreen').catch((err) => console.error('exit_fullscreen failed:', err));
  }
});

document.addEventListener('dblclick', (e) => {
  if (e.target.closest('button')) return;
  invoke('exit_fullscreen').catch((err) => console.error('exit_fullscreen failed:', err));
});

// ---- Kill default browser drag/select ----

document.addEventListener('dragstart', (e) => e.preventDefault());
document.addEventListener('selectstart', (e) => e.preventDefault());

// ---- Visualizer ----

const canvas = document.getElementById('fs-visualizer');
const ctx = canvas.getContext('2d');

const NUM_BARS = 28;
let targetBars = new Array(NUM_BARS).fill(0);
let displayBars = new Array(NUM_BARS).fill(0);
let velocities = new Array(NUM_BARS).fill(0);

function resizeCanvas() {
  canvas.width = window.innerWidth * window.devicePixelRatio;
  canvas.height = 120 * window.devicePixelRatio;
  canvas.style.width = window.innerWidth + 'px';
  canvas.style.height = '120px';
  ctx.scale(window.devicePixelRatio, window.devicePixelRatio);
}
resizeCanvas();
window.addEventListener('resize', resizeCanvas);

function renderVisualizer() {
  const w = window.innerWidth;
  const h = 120;
  ctx.clearRect(0, 0, w, h);

  const [r, g, b] = accentColor;
  const barWidth = w / NUM_BARS;
  const gap = 3;
  const maxH = h - 4;

  // Spring-based smoothing
  for (let i = 0; i < NUM_BARS; i++) {
    const diff = targetBars[i] - displayBars[i];
    velocities[i] += diff * 0.35;       // strong spring — responsive
    velocities[i] *= 0.55;              // light damping — fluid
    displayBars[i] += velocities[i];
    if (displayBars[i] < 0.001) displayBars[i] = 0;
  }

  for (let i = 0; i < NUM_BARS; i++) {
    const barH = Math.max(displayBars[i] * maxH, 1);
    const x = i * barWidth + gap / 2;
    const y = h - barH;

    const grad = ctx.createLinearGradient(x, y, x, h);
    grad.addColorStop(0, `rgba(${r}, ${g}, ${b}, 0.5)`);
    grad.addColorStop(1, `rgba(${r}, ${g}, ${b}, 0.02)`);

    ctx.fillStyle = grad;
    ctx.beginPath();
    ctx.roundRect(x, y, barWidth - gap, barH, 2);
    ctx.fill();
  }

  requestAnimationFrame(renderVisualizer);
}

// When new spectrum data arrives, update targets (decoupled from rendering)
function onSpectrumUpdate(data) {
  if (!data || !data.magnitudes) return;
  const raw = data.magnitudes;
  const rawLen = raw.length;

  // Resample raw (32) into NUM_BARS with averaging
  let vals = new Array(NUM_BARS);
  for (let i = 0; i < NUM_BARS; i++) {
    const startF = (i / NUM_BARS) * rawLen;
    const endF = ((i + 1) / NUM_BARS) * rawLen;
    const start = Math.floor(startF);
    const end = Math.min(Math.ceil(endF), rawLen);
    let sum = 0;
    for (let j = start; j < end; j++) sum += raw[j];
    const avg = sum / Math.max(end - start, 1);
    // Boost higher frequencies — low freqs naturally dominate
    const boost = 1 + (i / NUM_BARS) * 4;
    vals[i] = avg * boost;
  }

  // Normalize to 0-1
  let max = 0;
  for (let i = 0; i < NUM_BARS; i++) if (vals[i] > max) max = vals[i];
  if (max > 0) {
    for (let i = 0; i < NUM_BARS; i++) targetBars[i] = vals[i] / max;
  }
}

async function initVisualizer() {
  try {
    const { Channel } = window.__TAURI__.core;
    const onSpectrum = new Channel();
    onSpectrum.onmessage = onSpectrumUpdate;
    await invoke('start_visualizer', { onSpectrum });
  } catch (e) {
    console.error('Visualizer init failed:', e);
  }
}

requestAnimationFrame(renderVisualizer);
initVisualizer();

// ---- Start ----

setInterval(pollPlayback, 2000);
pollPlayback();
