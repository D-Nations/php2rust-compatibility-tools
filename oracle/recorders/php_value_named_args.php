<?php
// Argument binding answers for crates/php2rust-value/tests/data/named_args.json
// (scripts/record.sh value-args): `$object->$method(...$arguments)` with int
// and string keys (PHP 8.1 named arguments), for methods with required and
// optional parameters. An entry is [signature, parameters, arguments,
// result], the signature numbered as the class Signature<n> is named:
// parameters are [name, default or null when required]; the result is the
// bound parameter values plus any extra arguments (func_get_args() after
// the declared ones), or ["throws", class, message] with " in FILE on line
// N" taken out of the message.

error_reporting(E_ALL);
ini_set('display_errors', 'stderr');

require __DIR__ . '/lib/typed.php';

$signatures = [
    [],
    [['a', null]],
    [['a', ['default', 'A']]],
    [['a', null], ['b', null]],
    [['a', null], ['b', ['default', 'B']]],
    [['a', ['default', null]], ['b', ['default', null]]],
    [['a', null], ['b', ['default', 'B']], ['c', ['default', null]]],
    [['a', ['default', 1]], ['b', null]],
];
$names = ['a', 'b', 'c', 'd'];
$argumentLists = [[]];
// up to 3 arguments, each positional (int key, in order) or named
foreach ([1, 2, 3] as $n) {
    $keys = [[]];
    for ($i = 0; $i < $n; ++$i) {
        $next = [];
        foreach ($keys as $k) {
            $next[] = array_merge($k, ['#']);
            foreach ($names as $name) {
                $next[] = array_merge($k, [$name]);
            }
        }
        $keys = $next;
    }
    foreach ($keys as $k) {
        $list = [];
        $position = 0;
        foreach ($k as $i => $key) {
            $list[] = ['#' === $key ? $position++ : $key, 'v'.$i];
        }
        $argumentLists[] = $list;
    }
}

$out = [];
foreach ($signatures as $s => $signature) {
    $params = [];
    $body = [];
    foreach ($signature as [$name, $default]) {
        $params[] = '$'.$name.(null === $default ? '' : ' = '.var_export($default[1], true));
        $body[] = '$'.$name;
    }
    $class = 'Signature'.$s;
    eval('class '.$class.' { public function m('.implode(', ', $params).') { return array_merge(['.implode(', ', $body).'], array_slice(func_get_args(), '.count($signature).')); } }');
    $object = new $class();
    foreach ($argumentLists as $list) {
        $arguments = [];
        foreach ($list as [$key, $value]) {
            // a repeated key would overwrite in the PHP array itself
            if (array_key_exists($key, $arguments)) {
                continue 2;
            }
            $arguments[$key] = $value;
        }
        try {
            $result = typed_value($object->m(...$arguments));
        } catch (\Throwable $e) {
            $message = preg_replace('/ in \S+ on line \d+/', '', $e->getMessage());
            $result = ['throws', $e::class, $message];
        }
        $out[] = [
            $s,
            array_map(fn ($p) => [$p[0], null === $p[1] ? null : typed_value($p[1][1])], $signature),
            typed_pairs($arguments),
            $result,
        ];
    }
}

echo json_encode($out, JSON_UNESCAPED_SLASHES | JSON_THROW_ON_ERROR), "\n";
