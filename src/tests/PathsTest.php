<?php

it('defaults data dir to data', function (): void {
    $original = getenv('APPSTORE_DATA_DIR');
    putenv('APPSTORE_DATA_DIR');

    assertSame(dirname(__DIR__, 2) . '/data', Paths::dataDir());

    if ($original === false) {
        putenv('APPSTORE_DATA_DIR');
    } else {
        putenv('APPSTORE_DATA_DIR=' . $original);
    }
});

it('uses APPSTORE_DATA_DIR when set', function (): void {
    $original = getenv('APPSTORE_DATA_DIR');
    putenv('APPSTORE_DATA_DIR=/tmp/appstore-path-test');

    assertSame('/tmp/appstore-path-test', Paths::dataDir());

    if ($original === false) {
        putenv('APPSTORE_DATA_DIR');
    } else {
        putenv('APPSTORE_DATA_DIR=' . $original);
    }
});

?>
