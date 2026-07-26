function escapeHtml(value) {
    return String(value ?? '')
        .replaceAll('&', '&amp;')
        .replaceAll('<', '&lt;')
        .replaceAll('>', '&gt;')
        .replaceAll('"', '&quot;')
        .replaceAll("'", '&#039;');
}

function openBundlePage() {
    const button = document.querySelector('.sb-link[data-page="bundles"]');

    if (button) {
        gotoPage(button, 'bundles');
    }
}

async function loadAppBundleOptions() {
    const select = document.getElementById('appBundleIdSelect');

    if (!select) {
        return;
    }

    const result = await listBundleIds();
    const bundles = result && result.data
        ? (Array.isArray(result.data) ? result.data : result.data.bundle_ids || [])
        : [];

    if (!result || !result.ok) {
        select.innerHTML = '<option value="">Bundle IDを取得できませんでした</option>';
        return;
    }

    if (!bundles.length) {
        select.innerHTML = '<option value="">登録済みBundle IDがありません</option>';
        return;
    }

    select.innerHTML = [
        '<option value="">Bundle IDを選択してください</option>',
        ...bundles.map(bundle => {
            const bundleId = escapeHtml(bundle.bundle_id || '');
            const appName = escapeHtml(bundle.app_name || bundle.bundle_id || '');

            return `<option value="${bundleId}">${appName} — ${bundleId}</option>`;
        })
    ].join('');
}



function toast(message, isError = false) {
    const el = document.getElementById('toast');

    el.textContent = message;
    el.style.background = isError ? '#e53935' : 'var(--black)';
    el.classList.add('show');

    setTimeout(() => {
        el.classList.remove('show');
    }, 2800);
}

async function init() {
    const auth = await showMe();

    if (!auth || !auth.developer_id) {
        document.getElementById('loginScreen').classList.add('show');
        return;
    }

    const navUser = document.getElementById('navUser');
    const navUsername = document.getElementById('navUsername');
    navUser.style.display = 'flex';
    document.getElementById('logoutBtn').style.display = '';
    navUsername.textContent = auth.developer_id;

    document.getElementById('pd-devId').textContent = auth.developer_id || '—';

    await loadApps();
}

async function loadApps() {
    const result = await listDeveloperApps();

    console.log("developer apps response:", result);

    const apps = result && result.data
        ? (
            Array.isArray(result.data)
                ? result.data
                : result.data.apps
                || result.data.developer_apps
                || result.data.items
                || []
        )
        : [];

    const el = document.getElementById('appList');

    if (!result || !result.ok) {
        el.innerHTML = '<div class="empty"><div class="empty-text">アプリ一覧を取得できませんでした</div></div>';
        return;
    }

    let teams = [];

    try {
        const teamResult = await listTeams();

        teams = teamResult && teamResult.data
            ? (
                Array.isArray(teamResult.data)
                    ? teamResult.data
                    : teamResult.data.teams || []
            )
            : [];

        cachedTeams = teams;
    } catch (error) {
        console.error(error);
        teams = [];
    }

    if (!apps.length) {
        el.innerHTML = '<div class="empty"><div class="empty-text">アプリが登録されていません</div></div>';
        return;
    }

    el.innerHTML = apps.map((app, index) => {
        const rawBundleId = app.bundle_id || '';
        const bundleId = escapeHtml(rawBundleId);
        const name = escapeHtml(app.display_name || app.app_name || '—');
        const visibility = escapeHtml(app.visibility || 'private');
        const version = escapeHtml(app.latest_version || '未リリース');
        const currentTeamId = app.team_id || '';
        const teamSelectId = `appTeamSelect-${index}`;

        const teamOptions = [
            '<option value="">個人</option>',
            ...teams.map(team => {
                const teamId = escapeHtml(team.team_id || '');
                const teamName = escapeHtml(team.name || team.slug || team.team_id || '');
                const selected = currentTeamId === team.team_id ? 'selected' : '';

                return `<option value="${teamId}" ${selected}>${teamName}</option>`;
            })
        ].join('');

        return `
                <div class="list-item">
                    <div class="bicon">
                        <svg viewBox="0 0 24 24">
                            <rect x="4" y="4" width="16" height="16" rx="3"/>
                            <path d="M8 9h8M8 13h5"/>
                        </svg>
                    </div>

                    <div class="item-main">
                        <div class="item-title">${name}</div>
                        <div class="item-sub">${bundleId}</div>
                        <div class="item-meta">version: ${version}</div>

                        <div class="inline-row app-team-row">
                            <select id="${teamSelectId}" class="inline-select">
                                ${teamOptions}
                            </select>
                            <button class="btn btn-ghost btn-sm" onclick="doSetAppTeam('${bundleId}', '${teamSelectId}')">
                                所属を保存
                            </button>
                        </div>
                    </div>

                    <span class="badge ${visibility === 'public' ? 'badge-green' : 'badge-gray'}">${visibility}</span>
                    <button class="btn btn-ghost btn-sm" onclick="openReleasesPage('${bundleId}')">リリース</button>
                </div>
            `;
    }).join('');
}

async function doCreateApp() {
    const bundleId = document.getElementById('appBundleIdSelect').value.trim();
    const displayName = document.getElementById('appDisplayNameInput').value.trim();
    const description = document.getElementById('appDescriptionInput').value.trim();

    if (!bundleId || !displayName) {
        toast('Bundle IDと表示名を入力してください', true);
        return;
    }

    const result = await createDeveloperApp(
        bundleId,
        displayName,
        description
    );

    if (result && result.ok) {
        toast('アプリを作成しました');

        document.getElementById('appBundleIdSelect').value = '';
        document.getElementById('appDisplayNameInput').value = '';
        document.getElementById('appDescriptionInput').value = '';

        await loadApps();
        await loadAppBundleOptions();
        return;
    }

    const message = result?.data?.error?.message || 'アプリ作成に失敗しました';
    toast(message, true);
}

async function doSetAppTeam(bundleId, selectId) {
    const select = document.getElementById(selectId);

    if (!select) {
        return;
    }

    const teamId = select.value;

    const result = await setDeveloperAppTeam(bundleId, teamId);

    if (result && result.ok) {
        toast(teamId ? 'アプリをチームに所属させました' : 'アプリを個人所有に戻しました');
        await loadApps();
        return;
    }

    const message = result?.data?.error?.message || '所属チームの更新に失敗しました';
    toast(message, true);
}

async function loadKeys() {
    const result = await listKeys();
    const keys = result && result.data
        ? (Array.isArray(result.data) ? result.data : result.data.keys || [])
        : [];

    const el = document.getElementById('keyList');

    if (!result || !result.ok) {
        el.innerHTML = '<div class="empty"><div class="empty-text">公開鍵一覧を取得できませんでした</div></div>';
        return;
    }

    if (!keys.length) {
        el.innerHTML = '<div class="empty"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><path d="M21 2l-2 2m-7.61 7.61a5.5 5.5 0 1 1-7.778 7.778 5.5 5.5 0 0 1 7.777-7.777zm0 0L15.5 7.5m0 0l3 3L22 7l-3-3m-3.5 3.5L19 4"/></svg><div class="empty-text">公開鍵が登録されていません</div></div>';
        return;
    }

    el.innerHTML = keys.map(key => {
        const id = escapeHtml(key.key_id || key.id || '');
        const publicKey = key.public_key || key.key || '';
        const preview = escapeHtml(publicKey.slice(0, 48) + (publicKey.length > 48 ? '…' : ''));
        const createdAt = key.created_at
            ? new Date(key.created_at).toLocaleDateString('ja-JP')
            : '';
        const revoked = Boolean(key.revoked_at);

        return `
            <div class="list-item">
                <div class="item-main">
                    <div class="item-title">${preview}</div>
                    <div class="item-sub">${id}</div>
                    ${createdAt ? `<div class="item-meta">${escapeHtml(createdAt)} 登録</div>` : ''}
                </div>
                ${
            revoked
                ? '<span class="badge badge-gray">revoked</span>'
                : `<button class="btn btn-danger btn-sm" onclick="quickRevoke('${id}')">失効</button>`
        }
            </div>
        `;
    }).join('');
}

async function doCreateKey() {
    const keyId = document.getElementById('keyIdInput').value.trim();
    const publicKey = document.getElementById('publicKeyInput').value.trim();

    if (!keyId || !publicKey) {
        toast('Key IDと公開鍵を入力してください', true);
        return;
    }

    const result = await createKey(keyId, publicKey);

    if (result && result.ok) {
        toast('公開鍵を登録しました');
        document.getElementById('keyIdInput').value = '';
        document.getElementById('publicKeyInput').value = '';
        await loadKeys();
        return;
    }

    const message = result?.data?.error?.message || '登録に失敗しました';
    toast(message, true);
}

async function doRevokeKey() {
    const keyId = document.getElementById('revokeKeyIdInput').value.trim();

    if (!keyId) {
        toast('Key IDを入力してください', true);
        return;
    }

    const result = await revokeKey(keyId);

    if (result && result.ok) {
        toast('公開鍵を失効させました');
        document.getElementById('revokeKeyIdInput').value = '';
        await loadKeys();
        return;
    }

    const message = result?.data?.error?.message || '失効に失敗しました';
    toast(message, true);
}

async function quickRevoke(keyId) {
    if (!confirm(`この公開鍵を失効させますか？\n${keyId}`)) {
        return;
    }

    const result = await revokeKey(keyId);

    if (result && result.ok) {
        toast('公開鍵を失効させました');
        await loadKeys();
        return;
    }

    const message = result?.data?.error?.message || '失効に失敗しました';
    toast(message, true);
}

async function loadBundles() {
    const result = await listBundleIds();
    const bundles = result && result.data
        ? (Array.isArray(result.data) ? result.data : result.data.bundle_ids || [])
        : [];

    const el = document.getElementById('bundleList');

    if (!result || !result.ok) {
        el.innerHTML = '<div class="empty"><div class="empty-text">Bundle ID一覧を取得できませんでした</div></div>';
        return;
    }

    if (!bundles.length) {
        el.innerHTML = '<div class="empty"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><rect x="3" y="3" width="8" height="8" rx="1.5"/><rect x="13" y="3" width="8" height="8" rx="1.5"/><rect x="3" y="13" width="8" height="8" rx="1.5"/><rect x="13" y="13" width="8" height="8" rx="1.5"/></svg><div class="empty-text">Bundle IDが登録されていません</div></div>';
        return;
    }

    el.innerHTML = bundles.map(bundle => {
        const appName = escapeHtml(bundle.app_name || '—');
        const bundleId = escapeHtml(bundle.bundle_id || '');
        const status = escapeHtml(bundle.status || 'reserved');

        return `
                <div class="list-item">
                    <div class="bicon">
                        <svg viewBox="0 0 24 24">
                            <rect x="3" y="3" width="8" height="8" rx="1.5"/>
                            <rect x="13" y="3" width="8" height="8" rx="1.5"/>
                            <rect x="3" y="13" width="8" height="8" rx="1.5"/>
                            <rect x="13" y="13" width="8" height="8" rx="1.5"/>
                        </svg>
                    </div>
                    <div class="item-main">
                        <div class="item-title">${appName}</div>
                        <div class="item-sub">${bundleId}</div>
                    </div>
                    <span class="badge ${status === 'reserved' ? 'badge-green' : 'badge-gray'}">${status}</span>
                </div>
            `;
    }).join('');
}

async function doCreateBundle() {
    const bundleId = document.getElementById('bundleIdInput').value.trim();
    const appName = document.getElementById('appNameInput').value.trim();

    if (!bundleId || !appName) {
        toast('Bundle IDとアプリ名を入力してください', true);
        return;
    }

    const result = await createBundleId(bundleId, appName);

    if (result && result.ok) {
        toast('Bundle IDを登録しました');
        document.getElementById('bundleIdInput').value = '';
        document.getElementById('appNameInput').value = '';
        await loadBundles();
        await loadAppBundleOptions();
        return;
    }

    const message = result?.data?.error?.message || '登録に失敗しました';
    toast(message, true);
}

async function loadReleaseAppOptions() {
    const select = document.getElementById('releaseAppSelect');

    if (!select) {
        return;
    }

    const current = select.value;
    const result = await listDeveloperApps();
    const apps = result && result.data
        ? (Array.isArray(result.data) ? result.data : result.data.apps || [])
        : [];

    if (!result || !result.ok) {
        select.innerHTML = '<option value="">アプリを取得できませんでした</option>';
        await loadReleases();
        return;
    }

    if (!apps.length) {
        select.innerHTML = '<option value="">登録済みアプリがありません</option>';
        await loadReleases();
        return;
    }

    select.innerHTML = [
        '<option value="">アプリを選択してください</option>',
        ...apps.map(app => {
            const bundleId = escapeHtml(app.bundle_id || '');
            const name = escapeHtml(app.display_name || app.app_name || app.bundle_id || '');

            return `<option value="${bundleId}">${name} — ${bundleId}</option>`;
        })
    ].join('');

    if (current && [...select.options].some(option => option.value === current)) {
        select.value = current;
    }

    await loadReleases();
}

async function loadReleases() {
    const select = document.getElementById('releaseAppSelect');
    const list = document.getElementById('releaseList');

    if (!select || !list) {
        return;
    }

    const bundleId = select.value;

    if (!bundleId) {
        list.innerHTML = '<div class="empty"><div class="empty-text">アプリを選択してください</div></div>';
        return;
    }

    list.innerHTML = '<div class="empty"><div class="empty-text">リリース一覧を更新しています...</div></div>';

    const result = await listDeveloperReleases(bundleId);
    const releases = result && result.data
        ? (Array.isArray(result.data) ? result.data : result.data.releases || [])
        : [];

    if (!result || !result.ok) {
        list.innerHTML = '<div class="empty"><div class="empty-text">リリース一覧を取得できませんでした</div></div>';
        return;
    }

    if (!releases.length) {
        list.innerHTML = '<div class="empty"><div class="empty-text">リリースがありません</div></div>';
        return;
    }

    list.innerHTML = releases.map(release => {
        const releaseId = escapeHtml(release.release_id || '');
        const version = escapeHtml(release.version || '—');
        const status = escapeHtml(release.status || 'draft');
        const packageSize = Number(release.package_size || 0);
        const createdAt = release.created_at
            ? new Date(release.created_at).toLocaleDateString('ja-JP')
            : '';
        const sizeText = packageSize > 0
            ? `${Math.ceil(packageSize / 1024)} KiB`
            : 'size unknown';

        const canSubmit = status === 'draft' || status === 'rejected';

        return `
            <div class="list-item">
                <div class="item-main">
                    <div class="item-title">v${version}</div>
                    <div class="item-sub">${releaseId}</div>
                    <div class="item-meta">${escapeHtml(createdAt)} / ${escapeHtml(sizeText)}</div>
                </div>
                <span class="badge ${status === 'published' ? 'badge-green' : 'badge-gray'}">${status}</span>
                ${
            canSubmit
                ? `<button class="btn btn-dark btn-sm" onclick="doSubmitRelease('${releaseId}')">提出</button>`
                : ''
        }
            </div>
        `;
    }).join('');
}

async function doUploadRelease() {
    const bundleId = document.getElementById('releaseAppSelect').value;
    const fileInput = document.getElementById('releasePackageInput');
    const changelog = document.getElementById('releaseChangelogInput').value.trim();

    if (!bundleId) {
        toast('アプリを選択してください', true);
        return;
    }

    if (!fileInput.files || fileInput.files.length === 0) {
        toast('.pkgファイルを選択してください', true);
        return;
    }

    const result = await uploadDeveloperRelease(
        bundleId,
        fileInput.files[0],
        changelog
    );

    if (result && result.ok) {
        toast('リリースをアップロードしました');

        fileInput.value = '';
        document.getElementById('releaseChangelogInput').value = '';

        await loadReleases();
        await loadApps();
        return;
    }

    const message = result?.data?.error?.message || 'アップロードに失敗しました';
    toast(message, true);
}

async function doSubmitRelease(releaseId) {
    if (!confirm('このリリースを審査に提出しますか？')) {
        return;
    }

    const result = await submitDeveloperRelease(releaseId);

    if (result && result.ok) {
        toast('リリースを提出しました');

        const select = document.getElementById('releaseAppSelect');
        const currentBundleId = select ? select.value : '';

        if (currentBundleId) {
            await loadReleases();
        }

        return;
    }

    const message = result?.data?.error?.message || '提出に失敗しました';
    toast(message, true);
}

async function doLogout() {
    await logout();
}

async function updateAdminNavigation() {
    const sections = document.querySelectorAll('.adminSection');

    if (sections.length === 0) {
        return false;
    }

    sections.forEach(section => {
        section.style.display = 'none';
    });

    try {
        const access = await checkAdminAccess();

        if (access.is_admin) {
            sections.forEach(section => {
                section.style.display = '';
            });

            return true;
        }
    } catch (error) {
        console.error(error);
    }

    return false;
}

init().then(() => {});

updateAdminNavigation().catch(error => {
    console.error(error);
});
loadTeams()
    .then(() => loadApps())
    .catch(error => {
        console.error(error);
    });
loadAppBundleOptions().catch(error => {
    console.error(error);
});
loadReleaseAppOptions().catch(error => {
    console.error(error);
});
