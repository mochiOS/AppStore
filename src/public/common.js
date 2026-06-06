window.APP_CONFIG = {
    API_URL:
        location.hostname === "localhost"
            ? "http://localhost:3001"
            : "https://api.mochios.org"
};

function login(provider) {
    location.href =
        `${window.APP_CONFIG.API_URL}/v1/oauth/index.php?provider=${encodeURIComponent(provider)}`;
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
}

updateLoginState().then(() => {});
