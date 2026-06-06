<?php

it('publishes root ca certificate', function (): void {
    $response = apiRequest('/ca/root');
    $payload = decodeJson($response['body']);

    assertSame(200, $response['status']);
    assertTrue($payload['ca']['configured']);
    assertContains('BEGIN CERTIFICATE', $payload['ca']['certificate_pem']);
});

it('issues and revokes developer certificates through the admin workflow', function (): void {
    $pdo = Database::get();
    $developerId = 'dev_ca_flow';

    $stmt = $pdo->prepare(
        'INSERT INTO developers (developer_id, created_at, status)
         VALUES (:developer_id, :created_at, :status)'
    );
    $stmt->execute([
        ':developer_id' => $developerId,
        ':created_at' => date('c'),
        ':status' => 'active',
    ]);

    $developerSession = [
        'developer_id' => $developerId,
    ];

    $verificationRequest = apiJsonRequest(
        '/developer-verifications/request',
        [],
        'POST',
        ['note' => 'please verify me'],
        $developerSession
    );
    $verificationPayload = decodeJson($verificationRequest['body']);
    assertSame(201, $verificationRequest['status']);
    assertSame('pending', $verificationPayload['verification']['verification_status']);

    $_SERVER['HTTP_X_ADMIN_TOKEN'] = 'test-admin-token';
    $verifyResponse = apiJsonRequest(
        '/admin/developers/' . $developerId . '/verification',
        [],
        'POST',
        ['verification_status' => 'verified', 'note' => 'approved'],
        []
    );
    unset($_SERVER['HTTP_X_ADMIN_TOKEN']);
    $verifiedPayload = decodeJson($verifyResponse['body']);
    assertSame(200, $verifyResponse['status']);
    assertSame('verified', $verifiedPayload['verification']['verification_status']);

    $csr = generatePemKeyAndCsr($developerId . '.apps.mochios');

    $csrResponse = apiJsonRequest(
        '/certificate-requests',
        [],
        'POST',
        ['csr_pem' => $csr['csr_pem']],
        $developerSession
    );
    $csrPayload = decodeJson($csrResponse['body']);
    assertSame(201, $csrResponse['status']);
    assertSame('pending', $csrPayload['certificate_request']['status']);

    $_SERVER['HTTP_X_ADMIN_TOKEN'] = 'test-admin-token';
    $issueResponse = apiJsonRequest(
        '/admin/certificate-requests/' . $csrPayload['certificate_request']['csr_id'] . '/issue',
        [],
        'POST',
        [],
        []
    );
    unset($_SERVER['HTTP_X_ADMIN_TOKEN']);
    $certificatePayload = decodeJson($issueResponse['body']);
    assertSame(201, $issueResponse['status']);
    assertSame('active', $certificatePayload['certificate']['status']);
    assertContains('BEGIN CERTIFICATE', $certificatePayload['certificate']['certificate_pem']);

    $listResponse = apiJsonRequest('/certificates', [], 'GET', null, $developerSession);
    $listPayload = decodeJson($listResponse['body']);
    assertSame(200, $listResponse['status']);
    assertSame(1, count($listPayload['certificates']));

    $_SERVER['HTTP_X_ADMIN_TOKEN'] = 'test-admin-token';
    $revokeResponse = apiJsonRequest(
        '/admin/certificates/' . $certificatePayload['certificate']['certificate_id'] . '/revoke',
        [],
        'POST',
        ['reason' => 'compromised'],
        []
    );
    unset($_SERVER['HTTP_X_ADMIN_TOKEN']);
    $revokePayload = decodeJson($revokeResponse['body']);
    assertSame(200, $revokeResponse['status']);
    assertSame('revoked', $revokePayload['certificate']['status']);
    assertSame('compromised', $revokePayload['certificate']['revocation_reason']);
});
