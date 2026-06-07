<?php

return function (ApiContext $ctx): bool {
    if ($ctx->path === '/developer-verifications/me') {
        if ($ctx->method !== 'GET') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        $developerId = requireDeveloperId();

        ApiResponse::json([
            'verification' => $ctx->certificateRepo->findVerificationByDeveloperId($developerId),
        ]);
        return true;
    }

    if ($ctx->path === '/developer-verifications/request') {
        if ($ctx->method !== 'POST') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        $developerId = requireDeveloperId();
        $payload = readJsonBody();
        $note = isset($payload['note']) ? trim((string) $payload['note']) : null;

        ApiResponse::json([
            'verification' => $ctx->certificateRepo->requestVerification($developerId, $note),
        ], 201);
        return true;
    }

    if ($ctx->path === '/certificate-requests') {
        $developerId = requireDeveloperId();

        if ($ctx->method === 'GET') {
            ApiResponse::json([
                'certificate_requests' => $ctx->certificateRepo->listCertificateSigningRequestsByDeveloperId($developerId),
            ]);
            return true;
        }

        if ($ctx->method !== 'POST') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        $verification = $ctx->certificateRepo->findVerificationByDeveloperId($developerId);
        if ($verification === null || $verification['verification_status'] !== 'verified') {
            ApiResponse::error('DEVELOPER_NOT_VERIFIED', 'Developer verification is required', 403);
            return true;
        }

        $payload = readJsonBody();
        $csrPem = trim((string) ($payload['csr_pem'] ?? ''));

        if ($csrPem === '') {
            ApiResponse::error('VALIDATION_ERROR', 'csr_pem is required', 422);
            return true;
        }

        $csrInfo = $ctx->certificateAuthority->parseCsr($csrPem);
        $csr = $ctx->certificateRepo->createCertificateSigningRequest(
            $developerId,
            $csrPem,
            $csrInfo
        );

        ApiResponse::json([
            'certificate_request' => $csr,
        ], 201);
        return true;
    }

    if ($ctx->path === '/certificates') {
        if ($ctx->method !== 'GET') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        $developerId = requireDeveloperId();

        ApiResponse::json([
            'certificates' => $ctx->certificateRepo->listCertificatesByDeveloperId($developerId),
        ]);
        return true;
    }

    if (preg_match('#^/certificates/([^/]+)$#', $ctx->path, $matches) === 1) {
        if ($ctx->method !== 'GET') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        $developerId = requireDeveloperId();
        $certificate = $ctx->certificateRepo->findCertificateById($matches[1]);

        if ($certificate === null || $certificate['developer_id'] !== $developerId) {
            ApiResponse::error('CERTIFICATE_NOT_FOUND', 'Certificate not found', 404);
            return true;
        }

        ApiResponse::json([
            'certificate' => $certificate,
        ]);
        return true;
    }

    if ($ctx->path === '/ca/root') {
        if ($ctx->method !== 'GET') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        if (!$ctx->certificateAuthority->isConfigured()) {
            ApiResponse::error('CA_NOT_CONFIGURED', 'Certificate authority is not configured', 503);
            return true;
        }

        ApiResponse::json([
            'ca' => [
                'configured' => true,
                'fingerprint' => $ctx->certificateAuthority->rootFingerprint(),
                'certificate_pem' => $ctx->certificateAuthority->rootCertificatePem(),
            ],
        ]);
        return true;
    }

    return false;
};