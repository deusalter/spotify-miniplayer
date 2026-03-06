const { invoke } = window.__TAURI__.core;
const { getCurrentWebviewWindow } = window.__TAURI__.webviewWindow;

const appWindow = getCurrentWebviewWindow();

// State
let isPlaying = false;
let currentProgress = 0;
let currentDuration = 0;
let fadeTimeout = null;
let progressInterval = null;

// DOM elements
const widget = document.getElementById('widget');
const trackName = document.getElementById('track-name');
const artistName = document.getElementById('artist-name');
const albumArt = document.getElementById('album-art');
const albumArtPlaceholder = document.querySelector('.album-art-placeholder');
const currentTime = document.getElementById('current-time');
const totalTime = document.getElementById('total-time');
const progressFill = document.getElementById('progress-fill');
const btnPlay = document.getElementById('btn-play');
const btnNext = document.getElementById('btn-next');
const btnPrev = document.getElementById('btn-prev');
const progressContainer = document.getElementById('progress-container');

// Format milliseconds to m:ss
function formatTime(ms) {
  const totalSeconds = Math.floor(ms / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${seconds.toString().padStart(2, '0')}`;
}

// Update the play/pause icon SVG
function updatePlayIcon(playing) {
  const playIcon = document.getElementById('play-icon');
  if (playing) {
    // Pause icon (two vertical bars)
    playIcon.innerHTML = '<path d="M6 19h4V5H6v14zm8-14v14h4V5h-4z"/>';
  } else {
    // Play icon (triangle)
    playIcon.innerHTML = '<path d="M8 5v14l11-7z"/>';
  }
}

// Update progress display
function updateProgress() {
  if (currentDuration > 0) {
    const pct = (currentProgress / currentDuration) * 100;
    progressFill.style.width = `${Math.min(pct, 100)}%`;
    currentTime.textContent = formatTime(currentProgress);
    totalTime.textContent = formatTime(currentDuration);
  }
}

// Show widget with animation
async function showWidget() {
  widget.classList.add('visible');
  try { await appWindow.show(); } catch(e) {}
}

// Hide widget with animation
function hideWidget() {
  widget.classList.remove('visible');
  // Wait for CSS fade-out animation before hiding the window
  setTimeout(async () => {
    try { await appWindow.hide(); } catch(e) {}
  }, 400);
}

// Fade timeout management
function clearFadeTimeout() {
  if (fadeTimeout) {
    clearTimeout(fadeTimeout);
    fadeTimeout = null;
  }
}

function startFadeTimeout() {
  if (fadeTimeout) return; // already scheduled
  fadeTimeout = setTimeout(() => {
    hideWidget();
    fadeTimeout = null;
  }, 3000);
}

// Update UI from playback state
function updateUI(state) {
  trackName.textContent = state.track_name;
  artistName.textContent = state.artist_name;
  currentProgress = state.progress_ms;
  currentDuration = state.duration_ms;
  updateProgress();
  updatePlayIcon(state.is_playing);
  isPlaying = state.is_playing;

  // Update album art (only if URL changed)
  if (albumArt.dataset.currentUrl !== state.album_art_url) {
    albumArt.dataset.currentUrl = state.album_art_url;
    if (state.album_art_url) {
      albumArt.src = state.album_art_url;
      albumArt.style.display = 'block';
      albumArtPlaceholder.style.display = 'none';
    } else {
      albumArt.style.display = 'none';
      albumArtPlaceholder.style.display = 'block';
    }
  }
}

// Poll Spotify playback every 2 seconds
async function pollPlayback() {
  try {
    const state = await invoke('get_playback');

    if (state && state.is_playing) {
      updateUI(state);
      showWidget();
      clearFadeTimeout();
    } else if (state && !state.is_playing) {
      updateUI(state);
      startFadeTimeout();
    } else {
      // No active device
      startFadeTimeout();
    }
  } catch (e) {
    console.error('Poll failed:', e);
    // Don't hide on error — might be temporary
  }
}

// Local progress interpolation — smooth bar between polls
function startProgressInterpolation() {
  progressInterval = setInterval(() => {
    if (isPlaying && currentDuration > 0) {
      currentProgress += 100;
      if (currentProgress > currentDuration) {
        currentProgress = currentDuration;
      }
      updateProgress();
    }
  }, 100);
}

// Album art error handler
albumArt.onerror = function() {
  this.style.display = 'none';
  albumArtPlaceholder.style.display = 'block';
};

// Button handlers
btnPlay.addEventListener('click', async (e) => {
  e.stopPropagation();
  try { await invoke('play_pause'); } catch(e) { console.error(e); }
  // Immediately poll to update UI
  setTimeout(pollPlayback, 300);
});

btnNext.addEventListener('click', async (e) => {
  e.stopPropagation();
  try { await invoke('next_track'); } catch(e) { console.error(e); }
  setTimeout(pollPlayback, 300);
});

btnPrev.addEventListener('click', async (e) => {
  e.stopPropagation();
  try { await invoke('previous_track'); } catch(e) { console.error(e); }
  setTimeout(pollPlayback, 300);
});

// Progress bar click to seek
progressContainer.addEventListener('click', async (e) => {
  e.stopPropagation();
  const bar = progressContainer.querySelector('.progress-bar');
  const rect = bar.getBoundingClientRect();
  const pct = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
  const posMs = Math.floor(pct * currentDuration);
  try {
    await invoke('seek_to', { positionMs: posMs });
    currentProgress = posMs;
    updateProgress();
  } catch(e) { console.error(e); }
});

// Start everything
setInterval(pollPlayback, 2000);
pollPlayback(); // immediate first poll
startProgressInterpolation();
