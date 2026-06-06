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


