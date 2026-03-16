'use strict';

document.addEventListener('DOMContentLoaded', () => {

  // ── Navbar mobile menu ─────────────────────────────────────
  const burger     = document.getElementById('navbar-burger-btn');
  const mobileMenu = document.getElementById('navbar-mobile-menu');
  if (burger && mobileMenu) {
    burger.addEventListener('click', () => {
      mobileMenu.classList.toggle('is-active');
    });
    // Close on outside click
    document.addEventListener('click', (e) => {
      if (!burger.contains(e.target) && !mobileMenu.contains(e.target)) {
        mobileMenu.classList.remove('is-active');
      }
    });
  }

  // ── Dismiss notifications ──────────────────────────────────
  document.querySelectorAll('.notification-close').forEach(btn => {
    btn.addEventListener('click', () => {
      const notification = btn.closest('.notification');
      if (notification) notification.remove();
    });
  });

  // ── File input display name ────────────────────────────────
  document.querySelectorAll('.file-input').forEach(input => {
    input.addEventListener('change', e => {
      const name    = e.target.files[0]?.name || 'No file selected';
      const display = input.closest('.file-upload-area')?.querySelector('.file-name-display');
      if (display) display.textContent = name;
    });
  });

  // ── "Only not voted" filter (index page) ──────────────────
  const toggle    = document.getElementById('only-not-voted-toggle');
  const cards     = Array.from(document.querySelectorAll('.paper-card'));
  const noResults = document.getElementById('dynamic-no-results');

  if (toggle && cards.length) {
    const apply = () => {
      const onlyUnvoted = toggle.checked;
      cards.forEach(card => {
        const state = Number(card.dataset.voteState || 0);
        card.style.display = (!onlyUnvoted || state === 0) ? '' : 'none';
      });
      if (noResults) {
        const visible = cards.filter(c => c.style.display !== 'none').length;
        noResults.classList.toggle('is-hidden', visible > 0);
      }
    };
    toggle.addEventListener('change', apply);
    apply();
  }

});
