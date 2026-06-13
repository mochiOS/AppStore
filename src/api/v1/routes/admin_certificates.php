<?php

return function (ApiContext $ctx): bool {
    if (preg_match('#^/admin/developers/([^/]+)/verification$#', $ctx->path, $matches) === 1) {
        if ($ctx->method !== 'POST') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        $admin = requireAdminDeveloper($ctx);
        $adminId = $admin['developer_id'];
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

        $verification = $ctx->certificateRepo->updateVerification(
            $developerId,
            $status,
            $note,
            $adminId
        );

        writeAuditLog($ctx, $adminId, 'developer.verification.update', 'developer', $developerId, [
            'verification_status' => $status,
        ]);

        ApiResponse::json([
            'verification' => $verification,
        ]);
        return true;
    }

    if (preg_match('#^/admin/certificate-requests/([^/]+)/issue$#', $ctx->path, $matches) === 1) {
        if ($ctx->method !== 'POST') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        $admin = requireOwnerDeveloper($ctx);
        $adminId = $admin['developer_id'];

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

        writeAuditLog($ctx, $adminId, 'certificate.issue', 'certificate', $certificate['certificate_id'], [
            'csr_id' => $csr['csr_id'],
            'developer_id' => $csr['developer_id'],
        ]);

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

        $admin = requireAdminDeveloper($ctx);
        $adminId = $admin['developer_id'];
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

        writeAuditLog($ctx, $adminId, 'certificate_request.reject', 'certificate_request', $matches[1]);

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

        $admin = requireOwnerDeveloper($ctx);

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

        writeAuditLog($ctx, $admin['developer_id'], 'certificate.revoke', 'certificate', $matches[1], [
            'reason' => $reason,
        ]);

        ApiResponse::json([
            'certificate' => $certificate,
        ]);
        return true;
    }

    return false;
};
