<?php
// var_dump() of scalars and arrays, for
// crates/php2rust-value/tests/data/var_dump.json (scripts/record.sh
// value-var-dump): each case is the typed arguments and the printed text.

ini_set('display_errors', 'stderr');
require __DIR__.'/lib/typed.php';

$values = [
    null, true, false, 0, 1, -1, PHP_INT_MAX, PHP_INT_MIN,
    0.0, -0.0, 1.0, -1.5, 0.1, 0.3 - 0.2, 1e15, 1e16, 1e20, 1e21, 1e25, 1.5e-5, 1e-4, 0.0001234, 123456789012345.678,
    INF, -INF, NAN, M_PI,
    '', 'foo', "a\"b", "multi\nline", "\0\xff", 'été',
    [], [1, 2, 3], ['foo' => 'foo', 'bar' => 'bar'], [5 => 'a', 'k' => [1, ['x' => null]], -3 => true],
    [[]], ['a' => [], 'b' => [[[1.5]]]], ['"quoted"' => 1, '' => 2, '12' => 3, '012' => 4],
];

function dump(mixed ...$args): string
{
    ob_start();
    var_dump(...$args);

    return ob_get_clean();
}

$cases = [];
foreach ($values as $v) {
    $cases[] = ['args' => [typed_value($v)], 'out' => base64_encode(dump($v))];
}
$cases[] = ['args' => [typed_value('foo'), typed_value('bar')], 'out' => base64_encode(dump('foo', 'bar'))];
$cases[] = ['args' => [typed_value(1), typed_value([2]), typed_value(null)], 'out' => base64_encode(dump(1, [2], null))];

echo json_encode(['cases' => $cases], JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES), "\n";
