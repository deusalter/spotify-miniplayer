const { invoke } = window.__TAURI__.core;
const { getCurrentWebviewWindow } = window.__TAURI__.webviewWindow;
const { currentMonitor } = window.__TAURI__.window;
const { PhysicalPosition, PhysicalSize } = window.__TAURI__.dpi;

const appWindow = getCurrentWebviewWindow();

// State
let isPlaying = false;
let currentProgress = 0;
let currentDuration = 0;
let fadeTimeout = null;
let progressInterval = null;
let snapDebounce = null;
let isSnapping = false;

// DOM
const widget = document.getElementById('widget');
const trackName = document.getElementById('track-name');
const artistName = document.getElementById('artist-name');
const albumArt = document.getElementById('album-art');
const bgPlaceholder = document.getElementById('bg-placeholder');
const currentTimeEl = document.getElementById('current-time');
const totalTimeEl = document.getElementById('total-time');
const progressFill = document.getElementById('progress-fill');
const btnPlay = document.getElementById('btn-play');
const btnNext = document.getElementById('btn-next');
const btnPrev = document.getElementById('btn-prev');
const progressContainer = document.getElementById('progress-container');

// --- Utilities ---

function formatTime(ms) {
  const totalSeconds = Math.floor(ms / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${seconds.toString().padStart(2, '0')}`;
}

function updatePlayIcon(playing) {
  const playIcon = document.getElementById('play-icon');
  if (playing) {
    playIcon.innerHTML = '<path d="M6 19h4V5H6v14zm8-14v14h4V5h-4z"/>';
  } else {
    playIcon.innerHTML = '<path d="M8 5v14l11-7z"/>';
  }
}

function updateProgress() {
  if (currentDuration > 0) {
    const pct = (currentProgress / currentDuration) * 100;
    progressFill.style.width = `${Math.min(pct, 100)}%`;
    currentTimeEl.textContent = formatTime(currentProgress);
    totalTimeEl.textContent = formatTime(currentDuration);
  }
}

function animateButton(btn) {
  btn.classList.add('clicked');
  setTimeout(() => btn.classList.remove('clicked'), 180);
}

// --- Widget show/hide ---

async function showWidget() {
  widget.classList.add('visible');
  try { await appWindow.show(); } catch(e) {}
}

function hideWidget() {
  widget.classList.remove('visible');
  setTimeout(async () => {
    try { await appWindow.hide(); } catch(e) {}
  }, 400);
}

function clearFadeTimeout() {
  if (fadeTimeout) { clearTimeout(fadeTimeout); fadeTimeout = null; }
}

function startFadeTimeout() {
  if (fadeTimeout) return;
  fadeTimeout = setTimeout(() => { hideWidget(); fadeTimeout = null; }, 3000);
}

// --- UI Update ---

function updateUI(state) {
  trackName.textContent = state.track_name;
  artistName.textContent = state.artist_name;

  requestAnimationFrame(() => {
    if (trackName.scrollWidth > trackName.clientWidth) {
      trackName.classList.add('scrolling');
    } else {
      trackName.classList.remove('scrolling');
    }
  });

  currentProgress = state.progress_ms;
  currentDuration = state.duration_ms;
  updateProgress();
  updatePlayIcon(state.is_playing);
  isPlaying = state.is_playing;

  if (albumArt.dataset.currentUrl !== state.album_art_url) {
    albumArt.dataset.currentUrl = state.album_art_url;
    if (state.album_art_url) {
      albumArt.classList.add('fade-out');
      setTimeout(() => {
        albumArt.src = state.album_art_url;
        albumArt.style.display = 'block';
        bgPlaceholder.style.display = 'none';
        albumArt.onload = () => {
          albumArt.classList.remove('fade-out');
          albumArt.onload = null;
        };
      }, 300);
    } else {
      albumArt.style.display = 'none';
      bgPlaceholder.style.display = 'block';
    }
  }
}

// --- Polling ---

async function pollPlayback() {
  try {
    const state = await invoke('get_playback');
    if (state) {
      updateUI(state);
      showWidget();
      clearFadeTimeout();
    } else {
      // No active playback at all — hide after timeout
      startFadeTimeout();
    }
  } catch (e) {
    console.error('Poll failed:', e);
  }
}

function startProgressInterpolation() {
  progressInterval = setInterval(() => {
    if (isPlaying && currentDuration > 0) {
      currentProgress += 100;
      if (currentProgress > currentDuration) currentProgress = currentDuration;
      updateProgress();
    }
  }, 100);
}

// --- Edge Snapping ---

const SNAP_MARGIN = 6; // pixels from screen edge
const SNAP_THRESHOLD = 120; // how close to edge before snapping
const MENU_BAR_HEIGHT = 25; // macOS menu bar offset

async function snapToNearestEdge() {
  if (isSnapping) return;
  try {
    const monitor = await currentMonitor();
    if (!monitor) return;

    const scaleFactor = monitor.scaleFactor;
    const screenW = monitor.size.width / scaleFactor;
    const screenH = monitor.size.height / scaleFactor;
    const screenX = monitor.position.x / scaleFactor;
    const screenY = monitor.position.y / scaleFactor;

    const factor = await appWindow.scaleFactor();
    const winSize = await appWindow.outerSize();
    const winPos = await appWindow.outerPosition();

    const winW = winSize.width / factor;
    const winH = winSize.height / factor;
    const winX = winPos.x / factor;
    const winY = winPos.y / factor;

    // Usable area (below menu bar)
    const usableTop = screenY + MENU_BAR_HEIGHT;

    let targetX = winX;
    let targetY = winY;

    const distLeft = winX - screenX;
    const distRight = (screenX + screenW) - (winX + winW);
    const distTop = winY - usableTop;
    const distBottom = (screenY + screenH) - (winY + winH);

    if (distLeft < SNAP_THRESHOLD) {
      targetX = screenX + SNAP_MARGIN;
    } else if (distRight < SNAP_THRESHOLD) {
      targetX = screenX + screenW - winW - SNAP_MARGIN;
    }

    if (distTop < SNAP_THRESHOLD) {
      targetY = usableTop + SNAP_MARGIN;
    } else if (distBottom < SNAP_THRESHOLD) {
      targetY = screenY + screenH - winH - SNAP_MARGIN;
    }

    const dx = Math.abs(targetX - winX);
    const dy = Math.abs(targetY - winY);
    if (dx > 1 || dy > 1) {
      isSnapping = true;
      await animateToPosition(winX, winY, targetX, targetY, factor);
      isSnapping = false;
    }
  } catch(e) {
    isSnapping = false;
    console.error('Snap failed:', e);
  }
}

async function animateToPosition(fromX, fromY, toX, toY, scaleFactor) {
  const totalMs = 280;
  const frameMs = 1000 / 120; // target 120fps for smoothness
  const frames = Math.ceil(totalMs / frameMs);

  for (let i = 1; i <= frames; i++) {
    const t = i / frames;
    // Ease-out expo — very smooth deceleration
    const ease = 1 - Math.pow(2, -10 * t);
    const x = fromX + (toX - fromX) * ease;
    const y = fromY + (toY - fromY) * ease;
    await appWindow.setPosition(new PhysicalPosition(
      Math.round(x * scaleFactor),
      Math.round(y * scaleFactor)
    ));
    await new Promise(r => setTimeout(r, frameMs));
  }
  // Ensure final position is exact
  await appWindow.setPosition(new PhysicalPosition(
    Math.round(toX * scaleFactor),
    Math.round(toY * scaleFactor)
  ));
}

// --- Event Handlers ---

albumArt.onerror = function() {
  this.style.display = 'none';
  bgPlaceholder.style.display = 'block';
};

btnPlay.addEventListener('click', (e) => {
  e.stopPropagation();
  animateButton(btnPlay);
  // Optimistic: toggle icon immediately
  isPlaying = !isPlaying;
  updatePlayIcon(isPlaying);
  invoke('play_pause').then(() => pollPlayback()).catch(e => console.error(e));
});

btnNext.addEventListener('click', (e) => {
  e.stopPropagation();
  animateButton(btnNext);
  invoke('next_track').then(() => setTimeout(pollPlayback, 200)).catch(e => console.error(e));
});

btnPrev.addEventListener('click', (e) => {
  e.stopPropagation();
  animateButton(btnPrev);
  invoke('previous_track').then(() => setTimeout(pollPlayback, 200)).catch(e => console.error(e));
});

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

// --- Dragging + Double-click to fullscreen ---
let lastMousedownTime = 0;
let dragTimer = null;
document.addEventListener('mousedown', async (e) => {
  if (e.target.closest('button')) return;
  if (e.target.closest('.progress-bar')) return;
  if (e.target.closest('.progress-area')) return;
  if (e.button !== 0) return;
  e.preventDefault();

  const now = Date.now();
  if (now - lastMousedownTime < 350) {
    // Double-click detected — cancel pending drag, enter fullscreen
    if (dragTimer) { clearTimeout(dragTimer); dragTimer = null; }
    lastMousedownTime = 0;
    try { await invoke('enter_fullscreen'); } catch(err) { console.error(err); }
    return;
  }
  lastMousedownTime = now;

  // Delay drag start so second click has a chance to fire
  dragTimer = setTimeout(async () => {
    dragTimer = null;
    try {
      await appWindow.startDragging();
    } catch(err) {
      console.error('Drag failed:', err);
    }
  }, 180);
});

// Kill all default browser drag/select behavior
document.addEventListener('dragstart', (e) => e.preventDefault());
document.addEventListener('selectstart', (e) => e.preventDefault());

// --- Snap on move end ---

appWindow.onMoved(({ payload }) => {
  invoke('save_window_position', { x: payload.x, y: payload.y });

  // Don't re-trigger snap while already snapping
  if (isSnapping) return;

  if (snapDebounce) clearTimeout(snapDebounce);
  snapDebounce = setTimeout(() => {
    snapToNearestEdge();
    snapDebounce = null;
  }, 200);
});

// --- Start ---

widget.classList.add('visible');
setInterval(pollPlayback, 2000);
pollPlayback();
startProgressInterpolation();

// Restore position on load
(async () => {
  try {
    const pos = await invoke('load_window_position');
    if (pos) {
      await appWindow.setPosition(new PhysicalPosition(pos[0], pos[1]));
    }
  } catch (e) {
    console.error('Failed to restore position:', e);
  }
})();
