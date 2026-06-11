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
    $developerCsrfSession = csrfSession($developerSession);
    $headers = csrfHeaders();

    $verificationRequest = apiJsonRequest(
        '/developer-verifications/request',
        [],
        'POST',
        ['note' => 'please verify me'],
        $developerCsrfSession,
        $headers
    );
    $verificationPayload = decodeJson($verificationRequest['body']);
    assertSame(201, $verificationRequest['status']);
    assertSame('pending', $verificationPayload['verification']['verification_status']);

    $adminId = 'dev_ca_admin';
    $stmt->execute([
        ':developer_id' => $adminId,
        ':created_at' => date('c'),
        ':status' => 'active',
    ]);
    Database::get()
        ->prepare(
            'INSERT INTO admin_developers (developer_id, role, created_at)
             VALUES (:developer_id, :role, :created_at)'
        )
        ->execute([
            ':developer_id' => $adminId,
            ':role' => 'owner',
            ':created_at' => date('c'),
        ]);
    $adminSession = csrfSession(['developer_id' => $adminId]);

    $verifyResponse = apiJsonRequest(
        '/admin/developers/' . $developerId . '/verification',
        [],
        'POST',
        ['verification_status' => 'verified', 'note' => 'approved'],
        $adminSession,
        $headers
    );
    $verifiedPayload = decodeJson($verifyResponse['body']);
    assertSame(200, $verifyResponse['status']);
    assertSame('verified', $verifiedPayload['verification']['verification_status']);

    $csr = generatePemKeyAndCsr($developerId . '.apps.mochios');

    $csrResponse = apiJsonRequest(
        '/certificate-requests',
        [],
        'POST',
        ['csr_pem' => $csr['csr_pem']],
        $developerCsrfSession,
        $headers
    );
    $csrPayload = decodeJson($csrResponse['body']);
    assertSame(201, $csrResponse['status']);
    assertSame('pending', $csrPayload['certificate_request']['status']);

    $issueResponse = apiJsonRequest(
        '/admin/certificate-requests/' . $csrPayload['certificate_request']['csr_id'] . '/issue',
        [],
        'POST',
        [],
        $adminSession,
        $headers
    );
    $certificatePayload = decodeJson($issueResponse['body']);
    assertSame(201, $issueResponse['status']);
    assertSame('active', $certificatePayload['certificate']['status']);
    assertContains('BEGIN CERTIFICATE', $certificatePayload['certificate']['certificate_pem']);

    $listResponse = apiJsonRequest('/certificates', [], 'GET', null, $developerSession);
    $listPayload = decodeJson($listResponse['body']);
    assertSame(200, $listResponse['status']);
    assertSame(1, count($listPayload['certificates']));

    $revokeResponse = apiJsonRequest(
        '/admin/certificates/' . $certificatePayload['certificate']['certificate_id'] . '/revoke',
        [],
        'POST',
        ['reason' => 'compromised'],
        $adminSession,
        $headers
    );
    $revokePayload = decodeJson($revokeResponse['body']);
    assertSame(200, $revokeResponse['status']);
    assertSame('revoked', $revokePayload['certificate']['status']);
    assertSame('compromised', $revokePayload['certificate']['revocation_reason']);

    $auditCount = (int) Database::get()
        ->query("SELECT COUNT(*) FROM audit_logs WHERE action IN ('certificate.issue', 'certificate.revoke')")
        ->fetchColumn();
    assertSame(2, $auditCount);
});
