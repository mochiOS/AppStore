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