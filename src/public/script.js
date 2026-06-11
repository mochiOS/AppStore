function showPage(id, el) {
    document.querySelectorAll('.page').forEach(p => p.classList.remove('active'));
    document.getElementById('page-' + id).classList.add('active');
    document.querySelectorAll('.nav-tab').forEach(t => t.classList.remove('active'));
    document.querySelectorAll('.sb-link').forEach(l => l.classList.remove('active'));

    if (el) {
        if (el.classList.contains('nav-tab')) el.classList.add('active');
        if (el.classList.contains('sb-link')) el.classList.add('active');
    }
}

let toastTimer;
function toastMsg(msg) {
    const t = document.getElementById('toast');
    t.textContent = msg;
    t.classList.add('show');
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => t.classList.remove('show'), 2200);
}

function handleSearch(val) {
    const trending = document.getElementById('search-trending');
    const results  = document.getElementById('search-results');
    const label    = document.getElementById('search-result-label');

    if (val.trim().length === 0) {
        trending.style.display = '';
        results.style.display  = 'none';
    } else {
        trending.style.display = 'none';
        results.style.display  = '';
        label.textContent = `"${val}" の検索結果`;
        // TODO: 必要なデータ - 検索API連携 / 実際の検索結果をここに挿入
        const list = document.getElementById('search-result-list');
        const empty = document.createElement('div');

        empty.style.textAlign = 'center';
        empty.style.padding = '40px 20px';
        empty.style.color = 'var(--g400)';
        empty.style.fontSize = '13px';
        empty.textContent = `「${val}」に関するアプリが表示されます`;

        list.replaceChildren(empty);
    }
}

document.querySelectorAll('.cat-chip').forEach(chip => {
    chip.addEventListener('click', function() {
        const parent = this.closest('.cat-chips');
        parent.querySelectorAll('.cat-chip').forEach(c => c.classList.remove('active'));
        this.classList.add('active');
    });
});
