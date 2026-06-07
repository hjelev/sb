// Copy-to-clipboard buttons
document.querySelectorAll('.copy-btn').forEach((btn) => {
  btn.addEventListener('click', async () => {
    const text = btn.getAttribute('data-copy') || '';
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      const ta = document.createElement('textarea');
      ta.value = text;
      document.body.appendChild(ta);
      ta.select();
      document.execCommand('copy');
      document.body.removeChild(ta);
    }
    const original = btn.textContent;
    btn.textContent = 'copied!';
    btn.classList.add('copied');
    setTimeout(() => {
      btn.textContent = original;
      btn.classList.remove('copied');
    }, 1400);
  });
});

// Mobile slide-in nav drawer
const navToggle = document.querySelector('.nav__toggle');
const navDrawer = document.getElementById('mobileNav');
const navBackdrop = document.getElementById('navBackdrop');

function setNav(open) {
  document.body.classList.toggle('nav-open', open);
  if (navToggle) navToggle.setAttribute('aria-expanded', String(open));
  if (navDrawer) navDrawer.setAttribute('aria-hidden', String(!open));
  if (navBackdrop) navBackdrop.hidden = !open;
}
if (navToggle) {
  navToggle.addEventListener('click', () =>
    setNav(!document.body.classList.contains('nav-open'))
  );
}
if (navBackdrop) navBackdrop.addEventListener('click', () => setNav(false));
if (navDrawer) {
  navDrawer.querySelectorAll('a').forEach((a) =>
    a.addEventListener('click', () => setNav(false))
  );
}

// Lightbox for gallery screenshots
const lightbox = document.getElementById('lightbox');
if (lightbox) {
  const lightboxImg = lightbox.querySelector('img');
  const lightboxClose = lightbox.querySelector('.lightbox__close');

  document.querySelectorAll('.gallery .shot img').forEach((img) => {
    img.addEventListener('click', () => {
      lightboxImg.src = img.src;
      lightboxImg.alt = img.alt;
      lightbox.classList.add('open');
      lightbox.setAttribute('aria-hidden', 'false');
    });
  });

  var closeLightbox = function () {
    lightbox.classList.remove('open');
    lightbox.setAttribute('aria-hidden', 'true');
    lightboxImg.src = '';
  };
  lightbox.addEventListener('click', closeLightbox);
  lightboxClose.addEventListener('click', closeLightbox);
}

// Escape closes drawer and lightbox
document.addEventListener('keydown', (e) => {
  if (e.key !== 'Escape') return;
  setNav(false);
  if (lightbox) closeLightbox();
});

// Scroll reveal animations
const revealTargets = document.querySelectorAll(
  '.card, .shot, .install-card, .keymap__col, .shell-integration, .cli-wrap, .section__head'
);
revealTargets.forEach((el) => el.classList.add('reveal'));

const io = new IntersectionObserver(
  (entries) => {
    entries.forEach((entry) => {
      if (entry.isIntersecting) {
        entry.target.classList.add('visible');
        io.unobserve(entry.target);
      }
    });
  },
  { threshold: 0.12 }
);
revealTargets.forEach((el) => io.observe(el));
