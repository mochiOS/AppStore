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

async function showMe() {
    const res = await fetch(`${window.APP_CONFIG.API_URL}/v1/auth/me`, {
        credentials: "include"
    });

    if (!res.ok) {
        return null;
    }

    return res.json();
}

async function showDeveloper() {
    const res = await fetch(`${window.APP_CONFIG.API_URL}/v1/developers/me`, {
        credentials: "include"
    });

    if (!res.ok) {
        return null;
    }

    return res.json();
}

async function logout() {
    try {
        const response = await fetch(
            `${window.APP_CONFIG.API_URL}/v1/auth/logout`,
            {
                method: "POST",
                credentials: "include"
            }
        );

        if (!response.ok) {
            Error("Logout failed");
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
