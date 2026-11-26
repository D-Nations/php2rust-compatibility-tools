<?php
// The date classes as objects, for
// crates/php2rust-date/tests/data/objects.json (scripts/record.sh date):
//
// - dates: (array) $date, instanceof, and the read methods of
//   DateTimeImmutable and DateTime objects in several zones;
// - compare: ==, <, > and <=> between every pair of them;
// - zones: (array) new DateTimeZone($name) and getName();
// - calls: setTimezone() and modify() on DateTimeImmutable, and the
//   exceptions of wrong arguments.

ini_set('display_errors', 'stderr');

function thrown(Throwable $e): array
{
    return ['class' => $e::class, 'message' => $e->getMessage()];
}

function attempt(callable $f): mixed
{
    try {
        return $f();
    } catch (Throwable $e) {
        return thrown($e);
    }
}

$specs = [
    ['DateTimeImmutable', '2010-10-04 13:45', 'Europe/Paris'],
    ['DateTime', '2010-10-04 13:45', 'Europe/Paris'],
    ['DateTime', '2010-10-04 11:45', 'UTC'],
    ['DateTimeImmutable', '2010-10-04 07:45', 'America/New_York'],
    ['DateTimeImmutable', '2010-10-04 13:45:00.5', 'Europe/Paris'],
    ['DateTime', '2010-10-04 19:45 +08:00', 'UTC'],
    ['DateTimeImmutable', '1964-01-02 03:04', 'Z'],
    ['DateTime', '2016-10-30 02:30', 'Europe/Paris'],
];
$dates = [];
foreach ($specs as [$class, $text, $zone]) {
    $d = new $class($text, new DateTimeZone($zone));
    $dates[] = $d;
    $out['dates'][] = [
        'class' => $class, 'text' => $text, 'zone' => $zone,
        'array' => (array) $d,
        'interface' => $d instanceof DateTimeInterface,
        'format' => $d->format('c u'),
        'timestamp' => $d->getTimestamp(),
        'microsecond' => $d->getMicrosecond(),
        'offset' => $d->getOffset(),
        'timezone' => $d->getTimezone()->getName(),
    ];
}
foreach ($dates as $i => $a) {
    foreach ($dates as $j => $b) {
        $out['compare'][] = ['a' => $i, 'b' => $j, 'eq' => $a == $b, 'lt' => $a < $b, 'gt' => $a > $b, 'cmp' => $a <=> $b];
    }
}
foreach (['UTC', 'Europe/Paris', 'europe/paris', '+02:00', '-05:30', 'Z', 'GMT', 'America/New_York'] as $name) {
    $z = new DateTimeZone($name);
    $out['zones'][] = ['name' => $name, 'array' => (array) $z, 'get_name' => $z->getName()];
}
$base = new DateTimeImmutable('2010-10-04 13:45', new DateTimeZone('Europe/Paris'));
foreach (['Asia/Hong_Kong', 'UTC', '+05:30', 'America/Chicago'] as $zone) {
    $out['calls'][] = ['call' => 'setTimezone', 'arg' => $zone, 'result' => $base->setTimezone(new DateTimeZone($zone))->format('c e')];
}
foreach (['-1day', '+1 hour', '+1 month', 'Bad Date', 'tomorrow'] as $m) {
    $out['calls'][] = ['call' => 'modify', 'arg' => $m, 'result' => attempt(static fn () => $base->modify($m)->format('c e'))];
}
$out['errors'] = [
    'format_array' => attempt(static fn () => $base->format([])),
    'format_null' => attempt(static fn () => $base->format(null)),
    'modify_array' => attempt(static fn () => $base->modify([])),
    'set_timezone_string' => attempt(static fn () => $base->setTimezone('UTC')),
    'datetime_format_array' => attempt(static fn () => (new DateTime('2010-10-04 13:45'))->format([])),
];

echo json_encode($out, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_PRESERVE_ZERO_FRACTION | JSON_THROW_ON_ERROR), "\n";
