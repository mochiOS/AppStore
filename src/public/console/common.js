window.APP_CONFIG = {
    API_URL:
        location.hostname === "localhost" || location.hostname === "127.0.0.1"
            ? "http://localhost:3001"
            : "https://api.mochios.org"
};

let csrfToken = null;

function login() {
    window.alert("AccountsとAppStore管理画面の認証統合は準備中です。");
}

function isStateChangingMethod(method) {
    return ["POST", "PUT", "PATCH", "DELETE"].includes(method.toUpperCase());
}

async function getCsrfToken() {
    if (csrfToken) {
        return csrfToken;
    }

    const response = await fetch(`${window.APP_CONFIG.API_URL}/v1/auth/csrf`, {
        credentials: "include",
        cache: "no-store"
    });

    if (!response.ok) {
        return null;
    }

    const data = await response.json();
    csrfToken = data.csrf_token || null;

    return csrfToken;
}

async function apiFetch(path, options = {}) {
    const method = (options.method || "GET").toUpperCase();
    const headers = new Headers(options.headers || {});

    if (isStateChangingMethod(method)) {
        const token = await getCsrfToken();

        if (token) {
            headers.set("X-CSRF-Token", token);
        }
    }

    const response = await fetch(`${window.APP_CONFIG.API_URL}${path}`, {
        credentials: "include",
        cache: "no-store",
        ...options,
        method,
        headers
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

    csrfToken = result.data.csrf_token || csrfToken;

    return result.data;
}

async function showDeveloper() {
    return null;
}

async function listKeys() {
    return apiFetch("/v1/keys");
}

async function createKey(keyId, publicKey) {
    return apiFetch("/v1/keys", {
        method: "POST",
        headers: {
            "Content-Type": "application/json"
        },
        body: JSON.stringify({
            key_id: keyId,
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
    const now = Date.now();

    return apiFetch(
        `/v1/developer/apps/${encodeURIComponent(bundleId)}/releases?_=${now}`
    );
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
