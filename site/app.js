(() => {
  'use strict';

  const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  const navToggle = document.querySelector('.nav-toggle');
  const navLinks = document.querySelector('.nav-links');

  if (navToggle && navLinks) {
    navToggle.addEventListener('click', () => {
      const open = navToggle.getAttribute('aria-expanded') !== 'true';
      navToggle.setAttribute('aria-expanded', String(open));
      navLinks.classList.toggle('is-open', open);
    });
    navLinks.addEventListener('click', (event) => {
      if (event.target.closest('a')) {
        navToggle.setAttribute('aria-expanded', 'false');
        navLinks.classList.remove('is-open');
      }
    });
  }

  const reveals = document.querySelectorAll('.reveal');
  reveals.forEach((element) => {
    const delay = Number(element.dataset.delay || 0);
    element.style.setProperty('--delay', `${delay}ms`);
  });
  if ('IntersectionObserver' in window && !reducedMotion) {
    const observer = new IntersectionObserver((entries) => {
      entries.forEach((entry) => {
        if (entry.isIntersecting) {
          entry.target.classList.add('is-visible');
          observer.unobserve(entry.target);
        }
      });
    }, { threshold: 0.12, rootMargin: '0px 0px -45px' });
    reveals.forEach((element) => observer.observe(element));
  } else {
    reveals.forEach((element) => element.classList.add('is-visible'));
  }

  document.querySelectorAll('.copy-button').forEach((button) => {
    button.addEventListener('click', async () => {
      try {
        await navigator.clipboard.writeText(button.dataset.copy || '');
        const original = button.textContent;
        button.textContent = 'Copied';
        window.setTimeout(() => { button.textContent = original; }, 1400);
      } catch {
        button.textContent = 'Select text';
      }
    });
  });

  const canvas = document.getElementById('graph-canvas');
  if (!canvas || reducedMotion) return;
  const context = canvas.getContext('2d');
  if (!context) return;

  let width = 0;
  let height = 0;
  let dpr = 1;
  let nodes = [];
  let frame = 0;
  const pointer = { x: -1000, y: -1000 };

  function resize() {
    width = window.innerWidth;
    height = Math.min(820, window.innerHeight + 180);
    dpr = Math.min(window.devicePixelRatio || 1, 2);
    canvas.width = Math.round(width * dpr);
    canvas.height = Math.round(height * dpr);
    canvas.style.width = `${width}px`;
    canvas.style.height = `${height}px`;
    context.setTransform(dpr, 0, 0, dpr, 0, 0);
    const count = Math.max(24, Math.min(62, Math.floor(width / 24)));
    nodes = Array.from({ length: count }, (_, index) => ({
      x: Math.random() * width,
      y: 70 + Math.random() * (height - 130),
      vx: (Math.random() - .5) * .13,
      vy: (Math.random() - .5) * .1,
      radius: index % 9 === 0 ? 2.2 : 1.25,
      accent: index % 11 === 0,
    }));
  }

  window.addEventListener('resize', resize, { passive: true });
  window.addEventListener('pointermove', (event) => {
    pointer.x = event.clientX;
    pointer.y = event.clientY;
  }, { passive: true });
  window.addEventListener('pointerleave', () => { pointer.x = -1000; pointer.y = -1000; });

  function draw() {
    context.clearRect(0, 0, width, height);
    for (let i = 0; i < nodes.length; i += 1) {
      const node = nodes[i];
      node.x += node.vx;
      node.y += node.vy;
      if (node.x < -20) node.x = width + 20;
      if (node.x > width + 20) node.x = -20;
      if (node.y < 50 || node.y > height - 20) node.vy *= -1;
      const pdx = pointer.x - node.x;
      const pdy = pointer.y - node.y;
      const pointerDistance = Math.hypot(pdx, pdy);
      if (pointerDistance < 150 && pointerDistance > 1) {
        node.x -= (pdx / pointerDistance) * .09;
        node.y -= (pdy / pointerDistance) * .09;
      }
      for (let j = i + 1; j < nodes.length; j += 1) {
        const other = nodes[j];
        const dx = node.x - other.x;
        const dy = node.y - other.y;
        const distance = Math.hypot(dx, dy);
        if (distance < 128) {
          const alpha = (1 - distance / 128) * .18;
          context.strokeStyle = `rgba(108, 183, 206, ${alpha})`;
          context.lineWidth = .65;
          context.beginPath();
          context.moveTo(node.x, node.y);
          context.lineTo(other.x, other.y);
          context.stroke();
        }
      }
    }
    nodes.forEach((node) => {
      context.beginPath();
      context.arc(node.x, node.y, node.radius, 0, Math.PI * 2);
      context.fillStyle = node.accent ? 'rgba(101,246,212,.7)' : 'rgba(138,170,204,.42)';
      context.fill();
    });
    frame = window.requestAnimationFrame(draw);
  }

  document.addEventListener('visibilitychange', () => {
    if (document.hidden) window.cancelAnimationFrame(frame);
    else draw();
  });
  resize();
  draw();
})();
