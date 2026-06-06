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

    const data = await res.json();
    return data.user;
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
    const user = await showMe();

    document.querySelectorAll(".isLogin").forEach(element => {
        element.style.display = user ? "" : "none";
    });

    document.querySelectorAll(".isNotLogin").forEach(element => {
        element.style.display = user ? "none" : "";
    });
}   updateLoginState().then(() => {});