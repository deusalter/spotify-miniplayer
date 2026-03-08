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
const fsProgressFill = document.getElementById('fs-progress-fill');
const fsProgressEmpty = document.getElementById('fs-progress-empty');
const fsCurrentTime = document.getElementById('fs-current-time');
const fsTotalTime = document.getElementById('fs-total-time');
const fsTuiPanel = document.getElementById('fs-tui-panel');
const fsVisualizer = document.getElementById('fs-visualizer');

// ---- Constants ----

const PROGRESS_CHARS = 22;

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
    const ratio = Math.min(currentProgress / currentDuration, 1);
    const filled = Math.round(ratio * PROGRESS_CHARS);
    const empty = PROGRESS_CHARS - filled;
    fsProgressFill.textContent = '\u2501'.repeat(filled);   // ━
    fsProgressEmpty.textContent = '\u254D'.repeat(empty);    // ╍
  } else {
    fsProgressFill.textContent = '';
    fsProgressEmpty.textContent = '\u254D'.repeat(PROGRESS_CHARS);
  }
  fsCurrentTime.textContent = formatTime(currentProgress);
  fsTotalTime.textContent = formatTime(currentDuration);
}

// ---- Play/pause icon ----

function updatePlayIcon() {
  fsBtnPlay.textContent = isPlaying ? '\u2590\u2590' : '\u25B6'; // ▐▐ or ▶
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
      for (let i = 0; i < data.length; i += 16) { // every 4th pixel (4 channels * 4)
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

// ---- Apply accent color to UI elements ----

function applyAccentColor() {
  const [r, g, b] = accentColor;
  fsProgressFill.style.color = `rgb(${r}, ${g}, ${b})`;
  fsTuiPanel.style.borderColor = `rgba(${r}, ${g}, ${b}, 0.25)`;
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

  // Album art + background crossfade
  if (state.album_art_url && state.album_art_url !== currentArtUrl) {
    currentArtUrl = state.album_art_url;

    // Crossfade background
    fsBg.classList.add('fade-out');
    setTimeout(() => {
      fsBg.src = state.album_art_url;
      fsBg.onload = () => {
        fsBg.classList.remove('fade-out');
        fsBg.onload = null;
      };
    }, 300);

    // Crossfade album art
    fsAlbumArt.style.opacity = '0';
    setTimeout(() => {
      fsAlbumArt.src = state.album_art_url;
      fsAlbumArt.onload = () => {
        fsAlbumArt.style.opacity = '1';
        fsAlbumArt.onload = null;
      };
    }, 300);

    // Extract dominant color from new art
    extractDominantColor(state.album_art_url);
  }
}

// ---- Polling ----

async function pollPlayback() {
  try {
    const state = await invoke('get_playback');
    if (state) {
      updateUI(state);
    }
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
  // Optimistic toggle
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

// ---- Exit handlers ----

document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') {
    invoke('exit_fullscreen').catch((err) => console.error('exit_fullscreen failed:', err));
  }
});

document.addEventListener('dblclick', () => {
  invoke('exit_fullscreen').catch((err) => console.error('exit_fullscreen failed:', err));
});

// ---- Kill default browser drag/select ----

document.addEventListener('dragstart', (e) => e.preventDefault());
document.addEventListener('selectstart', (e) => e.preventDefault());

// ---- Visualizer ----

const canvas = document.getElementById('fs-visualizer');
const ctx = canvas.getContext('2d');
let spectrumData = null;

function resizeCanvas() {
  canvas.width = window.innerWidth;
  canvas.height = 140;
}
resizeCanvas();
window.addEventListener('resize', resizeCanvas);

function renderVisualizer() {
  ctx.clearRect(0, 0, canvas.width, canvas.height);

  if (spectrumData && spectrumData.magnitudes) {
    const bars = spectrumData.magnitudes;
    const numBars = bars.length;
    const barWidth = canvas.width / numBars;
    const gap = 2;
    const maxHeight = canvas.height - 10;
    const [r, g, b] = accentColor;

    for (let i = 0; i < numBars; i++) {
      const barHeight = bars[i] * maxHeight;
      const x = i * barWidth + gap / 2;
      const y = canvas.height - barHeight;

      const gradient = ctx.createLinearGradient(x, y, x, canvas.height);
      gradient.addColorStop(0, `rgba(${r}, ${g}, ${b}, 0.8)`);
      gradient.addColorStop(1, `rgba(${r}, ${g}, ${b}, 0.1)`);

      ctx.fillStyle = gradient;
      ctx.beginPath();
      ctx.roundRect(x, y, barWidth - gap, barHeight, 2);
      ctx.fill();
    }
  }

  requestAnimationFrame(renderVisualizer);
}

async function initVisualizer() {
  try {
    const { Channel } = window.__TAURI__.core;
    const onSpectrum = new Channel();
    onSpectrum.onmessage = (data) => {
      spectrumData = data;
    };
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
