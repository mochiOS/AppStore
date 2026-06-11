<?php

return function (ApiContext $ctx): bool {
    if ($ctx->path === '/auth/csrf') {
        if ($ctx->method !== 'GET') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        ApiResponse::json([
            'csrf_token' => $_SESSION['csrf_token'],
        ]);
        return true;
    }

    if ($ctx->path === '/auth/me') {
        if ($ctx->method !== 'GET') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        $userId = $_SESSION['user_id'] ?? null;
        $developerId = $_SESSION['developer_id'] ?? null;
        $user = null;

        if ($userId) {
            $stmt = $ctx->db->prepare(
                'SELECT id, provider, provider_user_id, username, display_name, avatar_url
                 FROM users
                 WHERE id = :id
                 LIMIT 1'
            );

            $stmt->execute([
                ':id' => $userId,
            ]);

            $user = $stmt->fetch(PDO::FETCH_ASSOC) ?: null;
        }

        if (!$userId && !$developerId) {
            ApiResponse::error('UNAUTHORIZED', 'Not logged in', 401);
            return true;
        }

        ApiResponse::json([
            'authenticated' => true,
            'developer_id' => $developerId,
            'user' => $user,
            'csrf_token' => $_SESSION['csrf_token'],
        ]);
        return true;
    }

    if ($ctx->path === '/developers/me') {
        if ($ctx->method !== 'GET') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        $developerId = requireDeveloperId();
        $developer = $ctx->developerRepo->findById($developerId);

        if ($developer === null) {
            ApiResponse::error('UNAUTHORIZED', 'Developer not found', 401);
            return true;
        }

        ApiResponse::json([
            'developer' => $developer,
        ]);
        return true;
    }

    if ($ctx->path === '/auth/logout') {
        if ($ctx->method !== 'POST') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        $_SESSION = [];

        if (session_status() === PHP_SESSION_ACTIVE) {
            $params = session_get_cookie_params();
            setcookie(session_name(), '', [
                'expires' => time() - 42000,
                'path' => $params['path'] ?? '/',
                'domain' => $params['domain'] ?? '',
                'secure' => (bool) ($params['secure'] ?? false),
                'httponly' => (bool) ($params['httponly'] ?? true),
                'samesite' => $params['samesite'] ?? 'Lax',
            ]);
            session_destroy();
        }

        ApiResponse::json([
            'ok' => true,
        ]);
        return true;
    }

    return false;
};
