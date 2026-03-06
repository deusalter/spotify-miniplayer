// Placeholder data so we can preview the widget design
document.getElementById('track-name').textContent = 'Bohemian Rhapsody';
document.getElementById('artist-name').textContent = 'Queen';
document.getElementById('current-time').textContent = '2:31';
document.getElementById('total-time').textContent = '5:55';
document.getElementById('progress-fill').style.width = '42%';
document.getElementById('widget').classList.add('visible');

// Use a sample album art URL for testing
document.getElementById('album-art').src = 'https://i.scdn.co/image/ab67616d0000b273ce4f1737bc8a646c8c4bd25a';
document.getElementById('album-art').onerror = function() {
  this.style.display = 'none';
  this.parentElement.querySelector('.album-art-placeholder').style.display = 'block';
};
