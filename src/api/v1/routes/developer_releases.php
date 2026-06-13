<?php
if (!function_exists('packageSignatureStringField')) {
    function packageSignatureStringField(array $signature, array $keys): ?string
    {
        foreach ($keys as $key) {
            if (isset($signature[$key]) && is_string($signature[$key])) {
                $value = trim($signature[$key]);

                if ($value !== '') {
                    return $value;
                }
            }
        }

        return null;
    }
}

if (!function_exists('validateUploadedPackageSignature')) {
    function validateUploadedPackageSignature(
        ApiContext $ctx,
        string $developerId,
        string $packagePath,
        array $inspection
    ): ?array {
        $signature = $inspection['signature'] ?? null;

        if (!is_array($signature)) {
            ApiResponse::error(
                'SIGNATURE_REQUIRED',
                'Package is not signed',
                422
            );
            return null;
        }

        $declaredKeyId = packageSignatureStringField($signature, [
            'key_id',
            'key-id',
            'keyId',
            'key',
        ]);

        if ($declaredKeyId === null) {
            ApiResponse::error(
                'SIGNATURE_KEY_ID_REQUIRED',
                'Package signature does not contain key_id',
                422
            );
            return null;
        }

        $signatureValue = packageSignatureStringField($signature, [
            'signature',
            'sig',
        ]);

        if ($signatureValue === null) {
            ApiResponse::error(
                'SIGNATURE_VALUE_REQUIRED',
                'Package signature does not contain signature value',
                422
            );
            return null;
        }

        $publicKey = $ctx->publicKeyRepo->findActiveOwnedByKeyId(
            $declaredKeyId,
            $developerId
        );

        if ($publicKey === null) {
            ApiResponse::error(
                'PUBLIC_KEY_NOT_REGISTERED',
                'Package is signed, but the public key is not registered by this developer',
                403
            );
            return null;
        }

        $declaredPublicKey = packageSignatureStringField($signature, [
            'public_key',
            'public-key',
            'publicKey',
        ]);

        if (
            $declaredPublicKey !== null
            && !PublicKeyRepository::publicKeyMaterialEquals($publicKey['public_key'], $declaredPublicKey)
        ) {
            ApiResponse::error(
                'SIGNATURE_PUBLIC_KEY_MISMATCH',
                'Package signature public_key does not match the registered public key',
                422
            );
            return null;
        }

        try {
            $verified = $ctx->packageSignatureVerifier->verifyWithPublicKey(
                $packagePath,
                $publicKey['public_key']
            );
        } catch (RuntimeException $e) {
            ApiResponse::error(
                'SIGNATURE_INVALID',
                $e->getMessage(),
                422
            );
            return null;
        }

        $verifiedKeyId = $verified['key_id'];

        if (!hash_equals($declaredKeyId, $verifiedKeyId)) {
            ApiResponse::error(
                'SIGNATURE_KEY_ID_MISMATCH',
                'Package signature key_id does not match msign verify result',
                422
            );
            return null;
        }

        return [
            'key_id' => $verifiedKeyId,
            'signature' => $signatureValue,
            'public_key' => $publicKey,
            'msign_output' => $verified['output'],
        ];
    }
}

return function (ApiContext $ctx): bool {
    if (preg_match('#^/developer/apps/([^/]+)/releases$#', $ctx->path, $matches) === 1) {
        $developerId = requireDeveloperId();
        $bundleId = urldecode($matches[1]);

        $app = $ctx->developerAppRepo->findOwnedByBundleId($bundleId, $developerId);
        if ($app === null) {
            ApiResponse::error('APP_NOT_FOUND', 'App not found', 404);
            return true;
        }

        if ($ctx->method === 'GET') {
            ApiResponse::json([
                'bundle_id' => $bundleId,
                'releases' => $ctx->developerReleaseRepo->listByBundleIdForDeveloper(
                    $developerId,
                    $bundleId
                ),
            ]);
            return true;
        }

        if ($ctx->method !== 'POST') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        if (!isset($_FILES['package'])) {
            ApiResponse::error('VALIDATION_ERROR', 'package is required', 422);
            return true;
        }

        $changelog = isset($_POST['changelog'])
            ? trim((string) $_POST['changelog'])
            : null;

        if ($changelog === '') {
            $changelog = null;
        }

        try {
            $ctx->packageUploadService->validateUploadedPackage($_FILES['package']);
            $inspection = $ctx->packageInspectService->inspect($_FILES['package']['tmp_name']);

            $about = $inspection['about'];
            $packageBundleId = (string) $about['bundle_id'];
            $version = (string) $about['version'];

            $signatureCheck = validateUploadedPackageSignature(
                $ctx,
                $developerId,
                $_FILES['package']['tmp_name'],
                $inspection
            );

            if ($signatureCheck === null) {
                return true;
            }

            if ($packageBundleId !== $bundleId) {
                ApiResponse::error(
                    'BUNDLE_ID_MISMATCH',
                    'Package bundle_id does not match URL bundle_id',
                    422
                );
                return true;
            }

            if ($ctx->developerReleaseRepo->versionExists($bundleId, $version)) {
                ApiResponse::error('RELEASE_ALREADY_EXISTS', 'Release version already exists', 409);
                return true;
            }

            $stored = $ctx->packageUploadService->storeUploadedPackage(
                $_FILES['package'],
                $bundleId,
                $version
            );

            $signature = $signatureCheck['signature'];
            // TODO: Replace this key_id placeholder when Developer CA certificate binding is integrated.
            $certificateId = $signatureCheck['key_id'];

            $release = $ctx->developerReleaseRepo->createDraft(
                developerId: $developerId,
                bundleId: $bundleId,
                version: $version,
                manifestHash: $inspection['hashes']['manifest_hash'],
                packageHash: $inspection['hashes']['content_hash'],
                signature: $signature,
                certificateId: $certificateId,
                packagePath: $stored['relative_path'],
                packageSize: (int) $stored['size'],
                changelog: $changelog
            );

            if ($release === null) {
                ApiResponse::error(
                    'BUNDLE_ID_NOT_OWNED',
                    'Bundle ID is not owned by this developer',
                    403
                );
                return true;
            }

            ApiResponse::json([
                'release' => $release,
                'package' => [
                    'path' => $stored['relative_path'],
                    'size' => $stored['size'],
                    'sha256' => $stored['sha256'],
                ],
                'inspection' => [
                    'about' => $inspection['about'],
                    'hashes' => $inspection['hashes'],
                ],
            ], 201);
            return true;
        } catch (RuntimeException $e) {
            ApiResponse::error('PACKAGE_INVALID', $e->getMessage(), 422);
            return true;
        }
    }

    if (preg_match('#^/developer/releases/([^/]+)$#', $ctx->path, $matches) === 1) {
        $developerId = requireDeveloperId();
        $releaseId = urldecode($matches[1]);

        if ($ctx->method !== 'GET') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        $release = $ctx->developerReleaseRepo->findOwnedById($releaseId, $developerId);
        if ($release === null) {
            ApiResponse::error('RELEASE_NOT_FOUND', 'Release not found', 404);
            return true;
        }

        ApiResponse::json([
            'release' => $release,
        ]);
        return true;
    }

    if (preg_match('#^/developer/releases/([^/]+)/submit$#', $ctx->path, $matches) === 1) {
        $developerId = requireDeveloperId();
        $releaseId = urldecode($matches[1]);

        if ($ctx->method !== 'POST') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        $current = $ctx->developerReleaseRepo->findOwnedById($releaseId, $developerId);
        if ($current === null) {
            ApiResponse::error('RELEASE_NOT_FOUND', 'Release not found', 404);
            return true;
        }

        if (!in_array($current['status'], ['draft', 'rejected'], true)) {
            ApiResponse::error(
                'INVALID_RELEASE_STATUS',
                'Only draft or rejected releases can be submitted',
                409
            );
            return true;
        }

        $keyId = (string) ($current['certificate_id'] ?? '');
        if ($keyId === '' || $ctx->publicKeyRepo->findActiveOwnedByKeyId($keyId, $developerId) === null) {
            ApiResponse::error(
                'SIGNING_KEY_NOT_ACTIVE',
                'Release signing key is not active',
                409
            );
            return true;
        }

        $release = $ctx->developerReleaseRepo->submitOwned($releaseId, $developerId);

        ApiResponse::json([
            'release' => $release,
        ]);
        return true;
    }

    return false;
};
