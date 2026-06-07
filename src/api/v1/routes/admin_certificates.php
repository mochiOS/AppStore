<?php

return function (ApiContext $ctx): bool {
    if (preg_match('#^/admin/developers/([^/]+)/verification$#', $ctx->path, $matches) === 1) {
        if ($ctx->method !== 'POST') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        $adminId = requireAdminToken($ctx->appConfig);
        $developerId = $matches[1];

        $developer = $ctx->developerRepo->findById($developerId);
        if ($developer === null) {
            ApiResponse::error('DEVELOPER_NOT_FOUND', 'Developer not found', 404);
            return true;
        }

        $payload = readJsonBody();
        $status = (string) ($payload['verification_status'] ?? '');
        $note = isset($payload['note']) ? trim((string) $payload['note']) : null;

        if (!in_array($status, ['verified', 'rejected'], true)) {
            ApiResponse::error(
                'VALIDATION_ERROR',
                'verification_status must be verified or rejected',
                422
            );
            return true;
        }

        ApiResponse::json([
            'verification' => $ctx->certificateRepo->updateVerification(
                $developerId,
                $status,
                $note,
                $adminId
            ),
        ]);
        return true;
    }

    if (preg_match('#^/admin/certificate-requests/([^/]+)/issue$#', $ctx->path, $matches) === 1) {
        if ($ctx->method !== 'POST') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        $adminId = requireAdminToken($ctx->appConfig);

        if (!$ctx->certificateAuthority->isConfigured()) {
            ApiResponse::error('CA_NOT_CONFIGURED', 'Certificate authority is not configured', 503);
            return true;
        }

        $csr = $ctx->certificateRepo->findCertificateSigningRequestById($matches[1]);
        if ($csr === null) {
            ApiResponse::error('CSR_NOT_FOUND', 'Certificate signing request not found', 404);
            return true;
        }

        $verification = $ctx->certificateRepo->findVerificationByDeveloperId($csr['developer_id']);
        if ($verification === null || $verification['verification_status'] !== 'verified') {
            ApiResponse::error('DEVELOPER_NOT_VERIFIED', 'Developer verification is required', 403);
            return true;
        }

        $issued = $ctx->certificateAuthority->issueCertificate($csr['csr_pem']);
        $certificate = $ctx->certificateRepo->issueCertificate(
            $csr['csr_id'],
            $adminId,
            $issued
        );

        ApiResponse::json([
            'certificate' => $certificate,
        ], 201);
        return true;
    }

    if (preg_match('#^/admin/certificate-requests/([^/]+)/reject$#', $ctx->path, $matches) === 1) {
        if ($ctx->method !== 'POST') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        $adminId = requireAdminToken($ctx->appConfig);
        $payload = readJsonBody();
        $reason = isset($payload['reason']) ? trim((string) $payload['reason']) : null;

        $csr = $ctx->certificateRepo->rejectCertificateSigningRequest(
            $matches[1],
            $adminId,
            $reason
        );

        if ($csr === null) {
            ApiResponse::error('CSR_NOT_FOUND', 'Certificate signing request not found', 404);
            return true;
        }

        ApiResponse::json([
            'certificate_request' => $csr,
        ]);
        return true;
    }

    if (preg_match('#^/admin/certificates/([^/]+)/revoke$#', $ctx->path, $matches) === 1) {
        if ($ctx->method !== 'POST') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        requireAdminToken($ctx->appConfig);

        $payload = readJsonBody();
        $reason = trim((string) ($payload['reason'] ?? ''));

        if ($reason === '') {
            ApiResponse::error('VALIDATION_ERROR', 'reason is required', 422);
            return true;
        }

        $certificate = $ctx->certificateRepo->revokeCertificate($matches[1], $reason);
        if ($certificate === null) {
            ApiResponse::error('CERTIFICATE_NOT_FOUND', 'Certificate not found', 404);
            return true;
        }

        ApiResponse::json([
            'certificate' => $certificate,
        ]);
        return true;
    }

    return false;
};