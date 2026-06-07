async function listTeams() {
    return apiFetch("/v1/teams");
}

async function createTeam(name, slug) {
    return apiFetch("/v1/teams", {
        method: "POST",
        headers: {
            "Content-Type": "application/json"
        },
        body: JSON.stringify({
            name,
            slug
        })
    });
}

async function getTeam(teamId) {
    return apiFetch(`/v1/teams/${encodeURIComponent(teamId)}`);
}

async function listTeamMembers(teamId) {
    return apiFetch(`/v1/teams/${encodeURIComponent(teamId)}/members`);
}

async function addTeamMember(teamId, developerId, role = "developer") {
    return apiFetch(`/v1/teams/${encodeURIComponent(teamId)}/members`, {
        method: "POST",
        headers: {
            "Content-Type": "application/json"
        },
        body: JSON.stringify({
            developer_id: developerId,
            role
        })
    });
}

async function updateTeamMemberRole(teamId, developerId, role) {
    return apiFetch(`/v1/teams/${encodeURIComponent(teamId)}/members/${encodeURIComponent(developerId)}/role`, {
        method: "POST",
        headers: {
            "Content-Type": "application/json"
        },
        body: JSON.stringify({
            role
        })
    });
}

async function removeTeamMember(teamId, developerId) {
    return apiFetch(`/v1/teams/${encodeURIComponent(teamId)}/members/${encodeURIComponent(developerId)}/remove`, {
        method: "POST"
    });
}

async function setDeveloperAppTeam(bundleId, teamId) {
    return apiFetch(`/v1/developer/apps/${encodeURIComponent(bundleId)}/team`, {
        method: "POST",
        headers: {
            "Content-Type": "application/json"
        },
        body: JSON.stringify({
            team_id: teamId || null
        })
    });
}

let cachedTeams = [];

function normalizeSlug(value) {
    return String(value || '')
        .trim()
        .toLowerCase()
        .replace(/[^a-z0-9-]+/g, '-')
        .replace(/^-+|-+$/g, '');
}

async function loadTeams() {
    const result = await listTeams();
    const teams = result && result.data
        ? (Array.isArray(result.data) ? result.data : result.data.teams || [])
        : [];

    cachedTeams = teams;

    renderTeamList(teams);
    renderTeamSelectOptions();

    return teams;
}

function renderTeamSelectOptions() {
    const memberSelect = document.getElementById('teamMemberTeamSelect');

    if (!memberSelect) {
        return;
    }

    const current = memberSelect.value;

    if (!cachedTeams.length) {
        memberSelect.innerHTML = '<option value="">チームがありません</option>';
        return;
    }

    memberSelect.innerHTML = [
        '<option value="">チームを選択してください</option>',
        ...cachedTeams.map(team => {
            const teamId = escapeHtml(team.team_id || '');
            const name = escapeHtml(team.name || team.slug || team.team_id || '');
            const role = escapeHtml(team.role || '');

            return `<option value="${teamId}">${name}${role ? ` / ${role}` : ''}</option>`;
        })
    ].join('');

    if (current && [...memberSelect.options].some(option => option.value === current)) {
        memberSelect.value = current;
    }
}

function renderTeamList(teams) {
    const el = document.getElementById('teamList');

    if (!el) {
        return;
    }

    if (!teams.length) {
        el.innerHTML = '<div class="empty"><div class="empty-text">チームがありません</div></div>';
        return;
    }

    el.innerHTML = teams.map(team => {
        const teamId = escapeHtml(team.team_id || '');
        const name = escapeHtml(team.name || '—');
        const slug = escapeHtml(team.slug || '—');
        const role = escapeHtml(team.role || 'viewer');
        const createdAt = team.created_at
            ? new Date(team.created_at).toLocaleDateString('ja-JP')
            : '';

        return `
            <div class="list-item">
                <div class="item-main">
                    <div class="item-title">${name}</div>
                    <div class="item-sub">${slug}</div>
                    <div class="item-meta">${teamId}${createdAt ? ` / ${escapeHtml(createdAt)}` : ''}</div>
                </div>
                <span class="badge ${role === 'owner' || role === 'admin' ? 'badge-green' : 'badge-gray'}">${role}</span>
                <button class="btn btn-ghost btn-sm" onclick="selectTeamForMembers('${teamId}')">メンバー</button>
            </div>
        `;
    }).join('');
}

function selectTeamForMembers(teamId) {
    const select = document.getElementById('teamMemberTeamSelect');

    if (!select) {
        return;
    }

    select.value = teamId;
    loadTeamMembers();
}

async function doCreateTeam() {
    const name = document.getElementById('teamNameInput').value.trim();
    let slug = document.getElementById('teamSlugInput').value.trim();

    if (!name) {
        toast('チーム名を入力してください', true);
        return;
    }

    if (!slug) {
        slug = normalizeSlug(name);
    } else {
        slug = normalizeSlug(slug);
    }

    if (!slug) {
        toast('Slugが不正です', true);
        return;
    }

    const result = await createTeam(name, slug);

    if (result && result.ok) {
        toast('チームを作成しました');

        document.getElementById('teamNameInput').value = '';
        document.getElementById('teamSlugInput').value = '';

        await loadTeams();
        await loadApps();
        return;
    }

    const message = result?.data?.error?.message || 'チーム作成に失敗しました';
    toast(message, true);
}

async function loadTeamMembers() {
    const select = document.getElementById('teamMemberTeamSelect');
    const list = document.getElementById('teamMemberList');

    if (!select || !list) {
        return;
    }

    const teamId = select.value;

    if (!teamId) {
        list.innerHTML = '<div class="empty"><div class="empty-text">チームを選択してください</div></div>';
        return;
    }

    list.innerHTML = '<div class="empty"><div class="empty-text">メンバーを取得しています...</div></div>';

    const result = await listTeamMembers(teamId);
    const members = result && result.data
        ? (Array.isArray(result.data) ? result.data : result.data.members || [])
        : [];

    if (!result || !result.ok) {
        const message = result?.data?.error?.message || 'メンバー一覧を取得できませんでした';
        list.innerHTML = `<div class="empty"><div class="empty-text">${escapeHtml(message)}</div></div>`;
        return;
    }

    if (!members.length) {
        list.innerHTML = '<div class="empty"><div class="empty-text">メンバーがいません</div></div>';
        return;
    }

    list.innerHTML = members.map(member => {
        const developerId = escapeHtml(member.developer_id || '');
        const username = escapeHtml(member.provider_username || '');
        const role = escapeHtml(member.role || 'viewer');
        const joinedAt = member.joined_at
            ? new Date(member.joined_at).toLocaleDateString('ja-JP')
            : '';

        return `
            <div class="list-item">
                <div class="item-main">
                    <div class="item-title">${username || developerId}</div>
                    <div class="item-sub">${developerId}</div>
                    <div class="item-meta">${escapeHtml(joinedAt)}</div>
                </div>

                <select class="inline-select" onchange="doUpdateTeamMemberRole('${teamId}', '${developerId}', this.value)">
                    <option value="owner" ${role === 'owner' ? 'selected' : ''}>owner</option>
                    <option value="admin" ${role === 'admin' ? 'selected' : ''}>admin</option>
                    <option value="developer" ${role === 'developer' ? 'selected' : ''}>developer</option>
                    <option value="viewer" ${role === 'viewer' ? 'selected' : ''}>viewer</option>
                </select>

                <button class="btn btn-danger btn-sm" onclick="doRemoveTeamMember('${teamId}', '${developerId}')">削除</button>
            </div>
        `;
    }).join('');
}

async function doAddTeamMember() {
    const teamId = document.getElementById('teamMemberTeamSelect').value;
    const developerId = document.getElementById('teamMemberDeveloperIdInput').value.trim();
    const role = document.getElementById('teamMemberRoleInput').value;

    if (!teamId) {
        toast('チームを選択してください', true);
        return;
    }

    if (!developerId) {
        toast('Developer IDを入力してください', true);
        return;
    }

    const result = await addTeamMember(teamId, developerId, role);

    if (result && result.ok) {
        toast('メンバーを追加しました');
        document.getElementById('teamMemberDeveloperIdInput').value = '';
        await loadTeamMembers();
        return;
    }

    const message = result?.data?.error?.message || 'メンバー追加に失敗しました';
    toast(message, true);
}

async function doUpdateTeamMemberRole(teamId, developerId, role) {
    const result = await updateTeamMemberRole(teamId, developerId, role);

    if (result && result.ok) {
        toast('権限を更新しました');
        await loadTeamMembers();
        return;
    }

    const message = result?.data?.error?.message || '権限更新に失敗しました';
    toast(message, true);
    await loadTeamMembers();
}

async function doRemoveTeamMember(teamId, developerId) {
    if (!confirm('このメンバーを削除しますか？')) {
        return;
    }

    const result = await removeTeamMember(teamId, developerId);

    if (result && result.ok) {
        toast('メンバーを削除しました');
        await loadTeamMembers();
        return;
    }

    const message = result?.data?.error?.message || 'メンバー削除に失敗しました';
    toast(message, true);
}