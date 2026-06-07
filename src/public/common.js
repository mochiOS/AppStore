window.APP_CONFIG = {
    API_URL:
        location.hostname === "localhost" || location.hostname === "127.0.0.1"
            ? "http://localhost:3001"
            : "https://api.mochios.org"
};

function login(provider = "github") {
    location.href =
        `${window.APP_CONFIG.API_URL}/v1/oauth?provider=${encodeURIComponent(provider)}`;
}

async function apiFetch(path, options = {}) {
    const response = await fetch(`${window.APP_CONFIG.API_URL}${path}`, {
        credentials: "include",
        ...options
    });

    const text = await response.text();
    let data = null;

    if (text !== "") {
        try {
            data = JSON.parse(text);
        } catch (error) {
            data = {
                raw: text
            };
        }
    }

    return {
        ok: response.ok,
        status: response.status,
        data
    };
}

async function showMe() {
    const result = await apiFetch("/v1/auth/me");

    if (!result.ok) {
        return null;
    }

    return result.data;
}

async function showDeveloper() {
    const result = await apiFetch("/v1/developers/me");

    if (!result.ok) {
        return null;
    }

    return result.data;
}

function getDeveloperRecord(developerResponse) {
    if (!developerResponse) {
        return null;
    }

    if (developerResponse.developer) {
        return developerResponse.developer;
    }

    return developerResponse;
}

function getDeveloperUsername(auth, developerResponse) {
    const developer = getDeveloperRecord(developerResponse);

    return (
        developer?.github_login ||
        developer?.github_username ||
        developer?.provider_username ||
        developer?.username ||
        developer?.oauth?.provider_username ||
        developer?.oauth?.github_username ||
        auth?.user?.username ||
        null
    );
}

function githubAvatarUrl(username, size = 56) {
    if (!username) {
        return null;
    }

    return `https://github.com/${encodeURIComponent(username)}.png?size=${encodeURIComponent(size)}`;
}

function renderGitHubAvatar(element, username, size = 56) {
    if (!element || !username) {
        return;
    }

    const avatarUrl = githubAvatarUrl(username, size);

    if (!avatarUrl) {
        return;
    }

    const img = document.createElement("img");
    img.src = avatarUrl;
    img.alt = username;
    img.width = size;
    img.height = size;
    img.loading = "lazy";
    img.referrerPolicy = "no-referrer";

    element.innerHTML = "";
    element.appendChild(img);
}

async function listKeys() {
    return apiFetch("/v1/keys");
}

async function createKey(publicKey) {
    return apiFetch("/v1/keys", {
        method: "POST",
        headers: {
            "Content-Type": "application/json"
        },
        body: JSON.stringify({
            public_key: publicKey
        })
    });
}

async function revokeKey(keyId) {
    return apiFetch(`/v1/keys/${encodeURIComponent(keyId)}/revoke`, {
        method: "POST"
    });
}

async function listBundleIds() {
    return apiFetch("/v1/bundle-ids");
}

async function createBundleId(bundleId, appName) {
    return apiFetch("/v1/bundle-ids", {
        method: "POST",
        headers: {
            "Content-Type": "application/json"
        },
        body: JSON.stringify({
            bundle_id: bundleId,
            app_name: appName
        })
    });
}

async function listDeveloperApps() {
    return apiFetch("/v1/developer/apps");
}

async function getDeveloperApp(bundleId) {
    return apiFetch(`/v1/developer/apps/${encodeURIComponent(bundleId)}`);
}

async function createDeveloperApp(bundleId, displayName, description = "") {
    return apiFetch("/v1/developer/apps", {
        method: "POST",
        headers: {
            "Content-Type": "application/json"
        },
        body: JSON.stringify({
            bundle_id: bundleId,
            display_name: displayName,
            description
        })
    });
}

async function listDeveloperReleases(bundleId) {
    return apiFetch(`/v1/developer/apps/${encodeURIComponent(bundleId)}/releases`);
}

async function uploadDeveloperRelease(bundleId, packageFile, changelog = "") {
    const form = new FormData();

    form.append("package", packageFile);

    if (changelog !== "") {
        form.append("changelog", changelog);
    }

    return apiFetch(`/v1/developer/apps/${encodeURIComponent(bundleId)}/releases`, {
        method: "POST",
        body: form
    });
}

async function getDeveloperRelease(releaseId) {
    return apiFetch(`/v1/developer/releases/${encodeURIComponent(releaseId)}`);
}

async function submitDeveloperRelease(releaseId) {
    return apiFetch(`/v1/developer/releases/${encodeURIComponent(releaseId)}/submit`, {
        method: "POST"
    });
}

async function logout() {
    try {
        const response = await apiFetch("/v1/auth/logout", {
            method: "POST"
        });

        if (!response.ok) {
            console.error("Logout failed");
            return;
        }

        location.href = "/";
    } catch (error) {
        console.error(error);
    }
}

async function updateLoginState() {
    const auth = await showMe();
    const isLoggedIn = Boolean(auth && (auth.developer_id || auth.user));

    document.querySelectorAll(".isLogin").forEach(element => {
        element.style.display = isLoggedIn ? "" : "none";
    });

    document.querySelectorAll(".isNotLogin").forEach(element => {
        element.style.display = isLoggedIn ? "none" : "";
    });

    return {
        auth,
        isLoggedIn
    };
}

updateLoginState().then(() => {});