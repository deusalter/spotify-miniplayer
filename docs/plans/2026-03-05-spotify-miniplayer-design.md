# Spotify Mini Player Widget — Design Document

**Date**: 2026-03-05
**Status**: Approved
**Author**: Abhinav Namboori

## Overview

A cross-platform (macOS first, Windows later) always-on-top Spotify mini player widget built with Tauri v2. The widget auto-appears when music plays and fades out when paused. **Visual design quality is the #1 priority** — this is a "make my desktop beautiful" project.

## Architecture

```
Tauri v2 App
├── Rust Backend
│   ├── Spotify OAuth PKCE flow
│   ├── Token management (auto-refresh)
│   ├── Playback state polling (every 2s)
│   ├── Playback control commands (play/pause/skip/seek)
│   ├── Album art caching
│   └── Window show/hide logic
├── Web Frontend (HTML/CSS/JS)
│   ├── Widget UI (~350x350px panel)
│   ├── Glassmorphism styling
│   ├── Drag-to-reposition
│   ├── Smooth animations (appear/disappear)
│   └── Local progress bar interpolation
└── System Tray
    └── Right-click: Quit, Show/Hide, Re-login
```

## Widget Specifications

- **Size**: ~350x350px
- **Window**: Frameless, transparent, always-on-top, rounded corners
- **Position**: Draggable anywhere, position persisted to config file
- **Style**: Glassmorphism (frosted glass blur, semi-transparent dark bg)
- **Visual identity**: Unique — not a Spotify clone. Design is the core value proposition.

## UI Components

- **Album art**: ~280x280px, rounded corners, subtle shadow
- **Track info**: Song name, artist name, current time
- **Progress bar**: Clickable for seeking, smooth local interpolation between polls
- **Controls**: Previous, Play/Pause, Next
- **Animations**: Fade + scale-up on appear, fade-out on disappear (~3s after pause)

## Behavior

- **Auto-appear**: Widget shows when Spotify starts playing (detected via API poll)
- **Auto-hide**: Widget fades out ~3 seconds after playback pauses/stops
- **No active device**: Widget stays hidden, no error shown
- **Startup**: App starts in system tray, no visible window until music plays

## Spotify API

- **Auth**: OAuth 2.0 with PKCE, browser-based login, local callback server
- **Polling**: `GET /v1/me/player` every 2 seconds
- **Controls**: Standard playback endpoints (play, pause, next, previous, seek)
- **Album art**: Cached locally, 300x300px version from API
- **Token refresh**: Automatic, handled in Rust backend
- **Requirement**: Spotify Premium required

## Error Handling

All errors are silent or graceful — never break the visual experience:
- No Spotify / no device → widget stays hidden
- Token expired → auto-refresh, fallback to tray notification for re-login
- No internet → freeze on last state, controls subtly disabled
- Rate limited → back off polling to 10s, auto-recover
- Missing album art → gradient placeholder matching glassmorphism theme

## Tech Stack

- **Framework**: Tauri v2
- **Backend**: Rust
- **Frontend**: HTML/CSS/JS (framework TBD during implementation — vanilla, React, or Svelte)
- **Platform**: macOS first, Windows later (same codebase)

## Non-Goals (for MVP)

- Lyrics display
- Queue management
- Volume control
- Keyboard shortcuts
- Multiple Spotify accounts
- Audio visualizer (possible future enhancement)
