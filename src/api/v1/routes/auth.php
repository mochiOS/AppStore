<?php

// Accountsとのbackend統合が完了するまでAppStore管理画面が使用する暫定bridgeです。
// OAuth、Account作成、Developer作成、外部Identity管理は行いません。
return function (ApiContext $ctx): bool {
    if ($ctx->path === '/auth/csrf') {
        if ($ctx->method !== 'GET') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        ApiResponse::json([
            'csrf_token' => $_SESSION['csrf_token'] ?? null,
        ]);
        return true;
    }

    if ($ctx->path === '/auth/me') {
        if ($ctx->method !== 'GET') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        $developerId = $_SESSION['developer_id'] ?? null;
        if (!is_string($developerId) || $developerId === '') {
            ApiResponse::error('UNAUTHORIZED', 'Not logged in', 401);
            return true;
        }

        ApiResponse::json([
            'developer_id' => $developerId,
            'csrf_token' => $_SESSION['csrf_token'] ?? null,
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
            session_regenerate_id(true);
        }
        ApiResponse::json(['logged_out' => true]);
        return true;
    }

    return false;
};
