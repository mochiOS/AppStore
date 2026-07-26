<?php

it('lists apps', function (): void {
    $response = apiRequest('/apps');
    $payload = decodeJson($response['body']);

    assertSame(200, $response['status']);
    assertTrue(isset($payload['apps']));
    assertSame('com.example', $payload['apps'][0]['bundle_id']);
});

it('returns app detail with releases', function (): void {
    $response = apiRequest('/apps/com.example');
    $payload = decodeJson($response['body']);

    assertSame(200, $response['status']);
    assertSame('com.example', $payload['bundle_id']);
    assertSame('0.1.0', $payload['releases'][0]['version']);
});

it('returns release detail', function (): void {
    $response = apiRequest('/apps/com.example/releases/0.1.0');
    $payload = decodeJson($response['body']);

    assertSame(200, $response['status']);
    assertSame('0.1.0', $payload['version']);
    assertSame('/apps/com.example/download?version=0.1.0', $payload['download_url']);
});

it('streams package download from data or override dir', function (): void {
    $response = apiRequest('/apps/com.example/download', ['version' => '0.1.0']);

    assertSame(200, $response['status']);
    assertSame(50, strlen($response['body']));
    assertContains('placeholder package', $response['body']);
});

it('returns release not found for missing release', function (): void {
    $response = apiRequest('/apps/com.example/releases/9.9.9');
    $payload = decodeJson($response['body']);

    assertSame(404, $response['status']);
    assertSame('RELEASE_NOT_FOUND', $payload['error']['code']);
});

it('creates and fetches developer scoped resources', function (): void {
    $pdo = Database::get();
    $developerId = 'dev_test_suite';
    $createdAt = '2026-06-06T00:00:00+09:00';

    $stmt = $pdo->prepare(
        'INSERT INTO developers (developer_id, created_at, status)
         VALUES (:developer_id, :created_at, :status)'
    );
    $stmt->execute([
        ':developer_id' => $developerId,
        ':created_at' => $createdAt,
        ':status' => 'active',
    ]);

    $session = csrfSession([
        'developer_id' => $developerId,
    ]);
    $headers = csrfHeaders();

    $keyResponse = apiJsonRequest(
        '/keys',
        [],
        'POST',
        [
            'key_id' => 'test-key-1',
            'public_key' => base64_encode(str_repeat('a', 32)),
        ],
        $session,
        $headers
    );
    $keyPayload = decodeJson($keyResponse['body']);
    assertSame(201, $keyResponse['status']);
    assertSame($developerId, $keyPayload['key']['developer_id']);

    $listKeysResponse = apiJsonRequest('/keys', [], 'GET', null, $session);
    $listKeys = decodeJson($listKeysResponse['body']);
    assertSame(200, $listKeysResponse['status']);
    assertSame(1, count($listKeys['keys']));

    $revokeResponse = apiJsonRequest(
        '/keys/' . $keyPayload['key']['key_id'] . '/revoke',
        [],
        'POST',
        [],
        $session,
        $headers
    );
    $revoked = decodeJson($revokeResponse['body']);
    assertSame(200, $revokeResponse['status']);
    assertTrue($revoked['key']['revoked_at'] !== null);

    $bundleResponse = apiJsonRequest(
        '/bundle-ids',
        [],
        'POST',
        [
            'bundle_id' => 'org.mochios.example',
            'app_name' => 'Example App',
        ],
        $session,
        $headers
    );
    $bundlePayload = decodeJson($bundleResponse['body']);
    assertSame(201, $bundleResponse['status']);
    assertSame('reserved', $bundlePayload['bundle_id']['status']);

    $listBundlesResponse = apiJsonRequest('/bundle-ids', [], 'GET', null, $session);
    $listBundles = decodeJson($listBundlesResponse['body']);
    assertSame(200, $listBundlesResponse['status']);
    assertSame('org.mochios.example', $listBundles['bundle_ids'][0]['bundle_id']);
});

it('rejects state-changing requests without csrf token', function (): void {
    $pdo = Database::get();
    $developerId = '019b9b17-6f1e-7d18-8a62-9306c63e41a2';

    $stmt = $pdo->prepare(
        'INSERT INTO developers (developer_id, created_at, status)
         VALUES (:developer_id, :created_at, :status)'
    );
    $stmt->execute([
        ':developer_id' => $developerId,
        ':created_at' => date('c'),
        ':status' => 'active',
    ]);

    $response = apiJsonRequest(
        '/keys',
        [],
        'POST',
        [
            'key_id' => 'csrf-key',
            'public_key' => base64_encode(str_repeat('b', 32)),
        ],
        ['developer_id' => $developerId, 'csrf_token' => 'expected-token']
    );
    $payload = decodeJson($response['body']);

    assertSame(403, $response['status']);
    assertSame('CSRF_TOKEN_INVALID', $payload['error']['code']);

    $submitResponse = apiJsonRequest(
        '/developer/releases/rel_missing/submit',
        [],
        'POST',
        [],
        ['developer_id' => $developerId, 'csrf_token' => 'expected-token']
    );
    $submitPayload = decodeJson($submitResponse['body']);
    assertSame(403, $submitResponse['status']);
    assertSame('CSRF_TOKEN_INVALID', $submitPayload['error']['code']);

    $adminResponse = apiJsonRequest(
        '/admin/releases/rel_missing/approve',
        [],
        'POST',
        [],
        ['developer_id' => 'admin-dev', 'csrf_token' => 'expected-token']
    );
    $adminPayload = decodeJson($adminResponse['body']);
    assertSame(403, $adminResponse['status']);
    assertSame('CSRF_TOKEN_INVALID', $adminPayload['error']['code']);
});

it('rejects state-changing requests from invalid origins even with csrf token', function (): void {
    $session = csrfSession(['developer_id' => 'dev_origin_suite']);

    $originResponse = apiJsonRequest(
        '/keys',
        [],
        'POST',
        [
            'key_id' => 'origin-key',
            'public_key' => base64_encode(str_repeat('c', 32)),
        ],
        $session,
        csrfHeaders(['Origin' => 'https://evil.example'])
    );
    $originPayload = decodeJson($originResponse['body']);
    assertSame(403, $originResponse['status']);
    assertSame('CSRF_ORIGIN_INVALID', $originPayload['error']['code']);

    $fetchResponse = apiJsonRequest(
        '/keys',
        [],
        'POST',
        [
            'key_id' => 'fetch-key',
            'public_key' => base64_encode(str_repeat('d', 32)),
        ],
        $session,
        csrfHeaders(['Sec-Fetch-Site' => 'cross-site'])
    );
    $fetchPayload = decodeJson($fetchResponse['body']);
    assertSame(403, $fetchResponse['status']);
    assertSame('CSRF_ORIGIN_INVALID', $fetchPayload['error']['code']);
});

it('rejects submit and approve when the signing key was revoked', function (): void {
    $pdo = Database::get();
    $developerId = 'dev_revoked_key_suite';
    $adminId = 'dev_revoked_key_admin';
    $createdAt = date('c');
    $publicKey = str_repeat('e', 32);

    $stmt = $pdo->prepare(
        'INSERT INTO developers (developer_id, created_at, status)
         VALUES (:developer_id, :created_at, :status)'
    );
    foreach ([$developerId, $adminId] as $id) {
        $stmt->execute([
            ':developer_id' => $id,
            ':created_at' => $createdAt,
            ':status' => 'active',
        ]);
    }

    $pdo->prepare(
        'INSERT INTO admin_developers (developer_id, role, created_at)
         VALUES (:developer_id, :role, :created_at)'
    )->execute([
        ':developer_id' => $adminId,
        ':role' => 'owner',
        ':created_at' => $createdAt,
    ]);

    $pdo->prepare(
        'INSERT INTO public_keys (
            key_id,
            developer_id,
            public_key,
            fingerprint,
            created_at,
            revoked_at
        ) VALUES (
            :key_id,
            :developer_id,
            :public_key,
            :fingerprint,
            :created_at,
            :revoked_at
        )'
    )->execute([
        ':key_id' => 'revoked-key-suite',
        ':developer_id' => $developerId,
        ':public_key' => base64_encode($publicKey),
        ':fingerprint' => hash('sha256', $publicKey),
        ':created_at' => $createdAt,
        ':revoked_at' => $createdAt,
    ]);

    $pdo->prepare(
        'INSERT INTO bundle_ids (bundle_id, developer_id, app_name, status, created_at)
         VALUES (:bundle_id, :developer_id, :app_name, :status, :created_at)'
    )->execute([
        ':bundle_id' => 'org.mochios.revoked',
        ':developer_id' => $developerId,
        ':app_name' => 'Revoked Key App',
        ':status' => 'reserved',
        ':created_at' => $createdAt,
    ]);

    $releaseInsert = $pdo->prepare(
        'INSERT INTO developer_releases (
            release_id,
            bundle_id,
            version,
            manifest_hash,
            package_hash,
            signature,
            certificate_id,
            status,
            created_at,
            package_path,
            package_size,
            changelog
        ) VALUES (
            :release_id,
            :bundle_id,
            :version,
            :manifest_hash,
            :package_hash,
            :signature,
            :certificate_id,
            :status,
            :created_at,
            :package_path,
            :package_size,
            :changelog
        )'
    );

    foreach ([
        ['rel_revoked_submit', '1.0.0', 'draft'],
        ['rel_revoked_approve', '1.0.1', 'submitted'],
    ] as [$releaseId, $version, $status]) {
        $releaseInsert->execute([
            ':release_id' => $releaseId,
            ':bundle_id' => 'org.mochios.revoked',
            ':version' => $version,
            ':manifest_hash' => null,
            ':package_hash' => hash('sha256', $releaseId),
            ':signature' => 'signature',
            ':certificate_id' => 'revoked-key-suite',
            ':status' => $status,
            ':created_at' => $createdAt,
            ':package_path' => 'data/packages/org.mochios.revoked/' . $version . '.pkg',
            ':package_size' => 1,
            ':changelog' => null,
        ]);
    }

    $submit = apiJsonRequest(
        '/developer/releases/rel_revoked_submit/submit',
        [],
        'POST',
        [],
        csrfSession(['developer_id' => $developerId]),
        csrfHeaders()
    );
    $submitPayload = decodeJson($submit['body']);
    assertSame(409, $submit['status']);
    assertSame('SIGNING_KEY_NOT_ACTIVE', $submitPayload['error']['code']);

    $approve = apiJsonRequest(
        '/admin/releases/rel_revoked_approve/approve',
        [],
        'POST',
        [],
        csrfSession(['developer_id' => $adminId]),
        csrfHeaders()
    );
    $approvePayload = decodeJson($approve['body']);
    assertSame(409, $approve['status']);
    assertSame('SIGNING_KEY_NOT_ACTIVE', $approvePayload['error']['code']);
});
