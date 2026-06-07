function getAdminToken() {
    return localStorage.getItem("admin_token") || "";
}

function setAdminToken(token) {
    localStorage.setItem("admin_token", token);
}

function clearAdminToken() {
    localStorage.removeItem("admin_token");
}

async function adminFetch(path, options = {}) {
    const token = getAdminToken();

    return apiFetch(path, {
        ...options,
        headers: {
            ...(options.headers || {}),
            "X-Admin-Token": token
        }
    });
}

async function listAdminReleases(status = "submitted") {
    return adminFetch(`/v1/admin/releases?status=${encodeURIComponent(status)}`);
}

async function approveAdminRelease(releaseId) {
    return adminFetch(`/v1/admin/releases/${encodeURIComponent(releaseId)}/approve`, {
        method: "POST"
    });
}

async function rejectAdminRelease(releaseId, message) {
    return adminFetch(`/v1/admin/releases/${encodeURIComponent(releaseId)}/reject`, {
        method: "POST",
        headers: {
            "Content-Type": "application/json"
        },
        body: JSON.stringify({
            message
        })
    });
}

function loadAdminPage() {
    const tokenInput = document.getElementById('adminTokenInput');

    if (tokenInput) {
        tokenInput.value = getAdminToken();
    }

    loadAdminReleases().then(() => {})
}

function saveAdminToken() {
    const token = document.getElementById('adminTokenInput').value.trim();

    if (!token) {
        toast('Admin Tokenを入力してください', true);
        return;
    }

    setAdminToken(token);
    toast('Admin Tokenを保存しました');
    loadAdminReleases().then(() => {});
}

function removeAdminToken() {
    clearAdminToken();

    const tokenInput = document.getElementById('adminTokenInput');
    if (tokenInput) {
        tokenInput.value = '';
    }

    toast('Admin Tokenを削除しました');
}

async function loadAdminReleases() {
    const list = document.getElementById('adminReleaseList');
    const statusSelect = document.getElementById('adminReleaseStatusSelect');

    if (!list || !statusSelect) {
        return;
    }

    const status = statusSelect.value || 'submitted';

    if (!getAdminToken()) {
        list.innerHTML = '<div class="empty"><div class="empty-text">Admin Tokenを入力してください</div></div>';
        return;
    }

    list.innerHTML = '<div class="empty"><div class="empty-text">審査対象を取得しています...</div></div>';

    const result = await listAdminReleases(status);
    const releases = result && result.data
        ? (Array.isArray(result.data) ? result.data : result.data.releases || [])
        : [];

    if (!result || !result.ok) {
        const message = result?.data?.error?.message || '審査対象を取得できませんでした';
        list.innerHTML = `<div class="empty"><div class="empty-text">${escapeHtml(message)}</div></div>`;
        return;
    }

    if (!releases.length) {
        list.innerHTML = '<div class="empty"><div class="empty-text">リリースがありません</div></div>';
        return;
    }

    list.innerHTML = releases.map(release => {
        const releaseId = escapeHtml(release.release_id || '');
        const bundleId = escapeHtml(release.bundle_id || '');
        const version = escapeHtml(release.version || '—');
        const appName = escapeHtml(release.display_name || release.bundle_id || '—');
        const releaseStatus = escapeHtml(release.status || 'submitted');
        const changelog = escapeHtml(release.changelog || '');
        const packageSize = Number(release.package_size || 0);
        const sizeText = packageSize > 0
            ? `${Math.ceil(packageSize / 1024)} KiB`
            : 'size unknown';

        const submittedAt = release.submitted_at
            ? new Date(release.submitted_at).toLocaleString('ja-JP')
            : '';

        const canReview = release.status === 'submitted';

        return `
            <div class="list-item">
                <div class="item-main">
                    <div class="item-title">${appName} v${version}</div>
                    <div class="item-sub">${bundleId}</div>
                    <div class="item-meta">${releaseId}</div>
                    <div class="item-meta">${escapeHtml(submittedAt)} / ${escapeHtml(sizeText)}</div>
                    ${changelog ? `<div class="item-meta">changelog: ${changelog}</div>` : ''}
                </div>

                <span class="badge ${releaseStatus === 'published' ? 'badge-green' : 'badge-gray'}">${releaseStatus}</span>

                ${
            canReview
                ? `
                            <button class="btn btn-dark btn-sm" onclick="doApproveRelease('${releaseId}')">承認</button>
                            <button class="btn btn-danger btn-sm" onclick="doRejectRelease('${releaseId}')">却下</button>
                        `
                : ''
        }
            </div>
        `;
    }).join('');
}

async function doApproveRelease(releaseId) {
    if (!confirm('このリリースを承認して公開しますか？')) {
        return;
    }

    const result = await approveAdminRelease(releaseId);

    if (result && result.ok) {
        toast('リリースを承認しました');
        await loadAdminReleases();
        return;
    }

    const message = result?.data?.error?.message || '承認に失敗しました';
    toast(message, true);
}

async function doRejectRelease(releaseId) {
    const message = prompt('却下理由を入力してください');

    if (message === null) {
        return;
    }

    const reason = message.trim();

    if (!reason) {
        toast('却下理由を入力してください', true);
        return;
    }

    const result = await rejectAdminRelease(releaseId, reason);

    if (result && result.ok) {
        toast('リリースを却下しました');
        await loadAdminReleases();
        return;
    }

    const errorMessage = result?.data?.error?.message || '却下に失敗しました';
    toast(errorMessage, true);
}