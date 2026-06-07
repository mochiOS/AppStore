<?php

return function (ApiContext $ctx): bool {
    if ($ctx->path === '/developer/apps') {
        $developerId = requireDeveloperId();

        if ($ctx->method === 'GET') {
            ApiResponse::json([
                'apps' => $ctx->developerAppRepo->listByDeveloperId($developerId),
            ]);
            return true;
        }

        if ($ctx->method !== 'POST') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        $payload = readJsonBody();

        $bundleId = trim((string) ($payload['bundle_id'] ?? ''));
        $displayName = trim((string) ($payload['display_name'] ?? ''));
        $description = trim((string) ($payload['description'] ?? ''));

        if ($bundleId === '') {
            ApiResponse::error('VALIDATION_ERROR', 'bundle_id is required', 422);
            return true;
        }

        if ($displayName === '') {
            ApiResponse::error('VALIDATION_ERROR', 'display_name is required', 422);
            return true;
        }

        if (
            !preg_match('/^[a-z0-9.-]+$/', $bundleId)
            || !str_contains($bundleId, '.')
        ) {
            ApiResponse::error('VALIDATION_ERROR', 'bundle_id is invalid', 422);
            return true;
        }

        try {
            $app = $ctx->developerAppRepo->create(
                $developerId,
                $bundleId,
                $displayName,
                $description === '' ? null : $description,
                null
            );
        } catch (PDOException $e) {
            if ($e->getCode() === '23000') {
                ApiResponse::error('APP_ALREADY_EXISTS', 'App already exists', 409);
                return true;
            }

            throw $e;
        }

        if ($app === null) {
            ApiResponse::error(
                'BUNDLE_ID_NOT_FOUND',
                'Bundle ID is not reserved by this developer',
                404
            );
            return true;
        }

        ApiResponse::json([
            'app' => $app,
        ], 201);
        return true;
    }

    if (preg_match('#^/developer/apps/([^/]+)/team$#', $ctx->path, $matches) === 1) {
        $developerId = requireDeveloperId();
        $bundleId = urldecode($matches[1]);

        if ($ctx->method !== 'POST') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        $app = $ctx->developerAppRepo->findOwnedByBundleId($bundleId, $developerId);

        if ($app === null) {
            ApiResponse::error('APP_NOT_FOUND', 'App not found', 404);
            return true;
        }

        $payload = readJsonBody();

        $teamIdValue = $payload['team_id'] ?? null;
        $teamId = null;

        if (is_string($teamIdValue)) {
            $teamId = trim($teamIdValue);

            if ($teamId === '') {
                $teamId = null;
            }
        }

        if ($teamId !== null) {
            if (!preg_match('/^team_[0-9a-f]{32}$/', $teamId)) {
                ApiResponse::error('VALIDATION_ERROR', 'team_id is invalid', 422);
                return true;
            }

            $team = $ctx->teamRepo->findByIdForDeveloper($teamId, $developerId);

            if ($team === null) {
                ApiResponse::error('TEAM_NOT_FOUND', 'Team not found', 404);
                return true;
            }

            if (!in_array($team['role'], ['owner', 'admin', 'developer'], true)) {
                ApiResponse::error(
                    'FORBIDDEN',
                    'You do not have permission to assign apps to this team',
                    403
                );
                return true;
            }
        }

        $updated = $ctx->developerAppRepo->setTeam(
            $developerId,
            $bundleId,
            $teamId
        );

        if ($updated === null) {
            ApiResponse::error('APP_NOT_FOUND', 'App not found', 404);
            return true;
        }

        ApiResponse::json([
            'app' => $updated,
        ]);

        return true;
    }

    if (preg_match('#^/developer/apps/([^/]+)$#', $ctx->path, $matches) === 1) {
        $developerId = requireDeveloperId();
        $bundleId = urldecode($matches[1]);

        if ($ctx->method !== 'GET') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        $app = $ctx->developerAppRepo->findOwnedByBundleId($bundleId, $developerId);

        if ($app === null) {
            ApiResponse::error('APP_NOT_FOUND', 'App not found', 404);
            return true;
        }

        ApiResponse::json([
            'app' => $app,
        ]);
        return true;
    }

    return false;
};