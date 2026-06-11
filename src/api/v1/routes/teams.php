<?php

if (!function_exists('normalizeTeamSlug')) {
    function normalizeTeamSlug(string $slug): string
    {
        $slug = strtolower(trim($slug));
        $slug = preg_replace('/[^a-z0-9-]+/', '-', $slug) ?? '';
        $slug = trim($slug, '-');

        return $slug;
    }
}

if (!function_exists('isValidDeveloperId')) {
    function isValidDeveloperId(string $developerId): bool
    {
        return preg_match('/^dev_[0-9a-f]{32}$/', $developerId) === 1;
    }
}

return function (ApiContext $ctx): bool {
    if ($ctx->path === '/teams') {
        $developerId = requireDeveloperId();

        if ($ctx->method === 'GET') {
            ApiResponse::json([
                'teams' => $ctx->teamRepo->listByDeveloperId($developerId),
            ]);
            return true;
        }

        if ($ctx->method !== 'POST') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        $payload = readJsonBody();

        $name = trim((string) ($payload['name'] ?? ''));
        $slug = normalizeTeamSlug((string) ($payload['slug'] ?? $name));

        if ($name === '') {
            ApiResponse::error('VALIDATION_ERROR', 'name is required', 422);
            return true;
        }

        if ($slug === '' || strlen($slug) < 2 || strlen($slug) > 48) {
            ApiResponse::error('VALIDATION_ERROR', 'slug is invalid', 422);
            return true;
        }

        try {
            $team = $ctx->teamRepo->create($developerId, $name, $slug);
        } catch (PDOException $e) {
            if ($e->getCode() === '23000') {
                ApiResponse::error('TEAM_ALREADY_EXISTS', 'Team slug already exists', 409);
                return true;
            }

            throw $e;
        }

        ApiResponse::json([
            'team' => $team,
        ], 201);
        return true;
    }

    if (preg_match('#^/teams/([^/]+)$#', $ctx->path, $matches) === 1) {
        $developerId = requireDeveloperId();
        $teamId = urldecode($matches[1]);

        if ($ctx->method !== 'GET') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        $team = $ctx->teamRepo->findByIdForDeveloper($teamId, $developerId);

        if ($team === null) {
            ApiResponse::error('TEAM_NOT_FOUND', 'Team not found', 404);
            return true;
        }

        ApiResponse::json([
            'team' => $team,
        ]);
        return true;
    }

    if (preg_match('#^/teams/([^/]+)/members$#', $ctx->path, $matches) === 1) {
        $developerId = requireDeveloperId();
        $teamId = urldecode($matches[1]);

        $team = $ctx->teamRepo->findByIdForDeveloper($teamId, $developerId);

        if ($team === null) {
            ApiResponse::error('TEAM_NOT_FOUND', 'Team not found', 404);
            return true;
        }

        if ($ctx->method === 'GET') {
            ApiResponse::json([
                'team_id' => $teamId,
                'members' => $ctx->teamRepo->listMembers($teamId),
            ]);
            return true;
        }

        if ($ctx->method !== 'POST') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        if (!$ctx->teamRepo->canManageMembers($teamId, $developerId)) {
            ApiResponse::error('FORBIDDEN', 'Team admin permission is required', 403);
            return true;
        }

        $payload = readJsonBody();

        $memberDeveloperId = trim((string) ($payload['developer_id'] ?? ''));
        $role = trim((string) ($payload['role'] ?? 'developer'));

        if (!isValidDeveloperId($memberDeveloperId)) {
            ApiResponse::error('VALIDATION_ERROR', 'developer_id is invalid', 422);
            return true;
        }

        if (!in_array($role, ['owner', 'admin', 'developer', 'viewer'], true)) {
            ApiResponse::error('VALIDATION_ERROR', 'role is invalid', 422);
            return true;
        }

        if ($role === 'owner' && !$ctx->teamRepo->canChangeOwnerRole($teamId, $developerId)) {
            ApiResponse::error('FORBIDDEN', 'Only owner can add another owner', 403);
            return true;
        }

        try {
            $member = $ctx->teamRepo->addMember($teamId, $memberDeveloperId, $role);
        } catch (PDOException $e) {
            if ($e->getCode() === '23000') {
                ApiResponse::error('DEVELOPER_NOT_FOUND', 'Developer not found', 404);
                return true;
            }

            throw $e;
        }

        ApiResponse::json([
            'member' => $member,
        ], 201);
        return true;
    }

    if (preg_match('#^/teams/([^/]+)/members/([^/]+)/role$#', $ctx->path, $matches) === 1) {
        $developerId = requireDeveloperId();
        $teamId = urldecode($matches[1]);
        $memberDeveloperId = urldecode($matches[2]);

        if ($ctx->method !== 'POST') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        if (!$ctx->teamRepo->canManageMembers($teamId, $developerId)) {
            ApiResponse::error('FORBIDDEN', 'Team admin permission is required', 403);
            return true;
        }

        $payload = readJsonBody();
        $role = trim((string) ($payload['role'] ?? ''));

        if (!in_array($role, ['owner', 'admin', 'developer', 'viewer'], true)) {
            ApiResponse::error('VALIDATION_ERROR', 'role is invalid', 422);
            return true;
        }

        if ($role === 'owner' && !$ctx->teamRepo->canChangeOwnerRole($teamId, $developerId)) {
            ApiResponse::error('FORBIDDEN', 'Only owner can promote owner', 403);
            return true;
        }

        try {
            $member = $ctx->teamRepo->updateMemberRole($teamId, $memberDeveloperId, $role);
        } catch (RuntimeException $e) {
            ApiResponse::error('TEAM_OWNER_REQUIRED', $e->getMessage(), 409);
            return true;
        }

        if ($member === null) {
            ApiResponse::error('MEMBER_NOT_FOUND', 'Member not found', 404);
            return true;
        }

        ApiResponse::json([
            'member' => $member,
        ]);
        return true;
    }

    if (preg_match('#^/teams/([^/]+)/members/([^/]+)/remove$#', $ctx->path, $matches) === 1) {
        $developerId = requireDeveloperId();
        $teamId = urldecode($matches[1]);
        $memberDeveloperId = urldecode($matches[2]);

        if ($ctx->method !== 'POST') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        if (!$ctx->teamRepo->canManageMembers($teamId, $developerId)) {
            ApiResponse::error('FORBIDDEN', 'Team admin permission is required', 403);
            return true;
        }

        $target = $ctx->teamRepo->findMember($teamId, $memberDeveloperId);

        if ($target !== null && $target['role'] === 'owner' && !$ctx->teamRepo->canChangeOwnerRole($teamId, $developerId)) {
            ApiResponse::error('FORBIDDEN', 'Only owner can remove owner', 403);
            return true;
        }

        try {
            $deleted = $ctx->teamRepo->removeMember($teamId, $memberDeveloperId);
        } catch (RuntimeException $e) {
            ApiResponse::error('TEAM_OWNER_REQUIRED', $e->getMessage(), 409);
            return true;
        }

        ApiResponse::json([
            'deleted' => $deleted,
        ]);
        return true;
    }

    return false;
};
