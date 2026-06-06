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

it('returns unauthorized for developer profile without login', function (): void {
    $response = apiRequest('/developers/me');
    $payload = decodeJson($response['body']);

    assertSame(401, $response['status']);
    assertSame('UNAUTHORIZED', $payload['error']['code']);
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

    $session = [
        'developer_id' => $developerId,
    ];

    $profileResponse = apiJsonRequest('/developers/me', [], 'GET', null, $session);
    $profile = decodeJson($profileResponse['body']);
    assertSame(200, $profileResponse['status']);
    assertSame($developerId, $profile['developer']['developer_id']);

    $keyResponse = apiJsonRequest(
        '/keys',
        [],
        'POST',
        ['public_key' => 'ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAITestKey mochios@example'],
        $session
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
        $session
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
        $session
    );
    $bundlePayload = decodeJson($bundleResponse['body']);
    assertSame(201, $bundleResponse['status']);
    assertSame('reserved', $bundlePayload['bundle_id']['status']);

    $listBundlesResponse = apiJsonRequest('/bundle-ids', [], 'GET', null, $session);
    $listBundles = decodeJson($listBundlesResponse['body']);
    assertSame(200, $listBundlesResponse['status']);
    assertSame('org.mochios.example', $listBundles['bundle_ids'][0]['bundle_id']);
});

