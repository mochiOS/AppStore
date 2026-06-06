const chart = document.getElementById('chart');
const values = [22, 18, 31, 27, 38, 42, 35, 29, 44, 51, 48, 39, 33, 27, 42, 55, 61, 58, 45, 39, 28, 34, 47, 63, 71, 68, 59, 52, 62, 74];
const max = Math.max(...values);

values.forEach((v, i) => {
  const bar = document.createElement('div');
  bar.className = 'chart-bar' + (i >= 27 ? ' highlight' : '');
  bar.style.height = Math.max(3, (v / max) * 100) + '%';
  chart.appendChild(bar);
});

const modalOverlay = document.getElementById('modalOverlay');
const notice = document.getElementById('notice');

function openModal() {
  modalOverlay.classList.add('open');
}

function closeModal() {
  modalOverlay.classList.remove('open');
}

document.querySelectorAll('[data-action="open-modal"]').forEach((element) => {
  element.addEventListener('click', openModal);
});

document.querySelectorAll('[data-action="close-modal"]').forEach((element) => {
  element.addEventListener('click', closeModal);
});

document.querySelector('[data-action="close-notice"]').addEventListener('click', () => {
  notice.remove();
});

modalOverlay.addEventListener('click', (event) => {
  if (event.target === modalOverlay) {
    closeModal();
  }
});

document.addEventListener('keydown', (event) => {
  if (event.key === 'Escape') {
    closeModal();
  }
});

document.querySelectorAll('.nav-item').forEach((item) => {
  item.addEventListener('click', function (event) {
    event.preventDefault();
    document.querySelectorAll('.nav-item').forEach((nav) => nav.classList.remove('active'));
    this.classList.add('active');
  });
});
