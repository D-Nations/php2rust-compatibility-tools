<?php
// Enum cases as objects, for crates/php2rust-value/tests/data/enums.json
// (scripts/record.sh value-enums): instanceof, (array), and the static
// methods cases(), from() and tryFrom() called on a case.

ini_set('display_errors', 'stderr');

enum Suit
{
    case Hearts;
    case Spades;
}

enum Status: string
{
    case Active = 'active';
    case Inactive = 'inactive';
}

enum Level: int
{
    case Low = 1;
    case High = 10;
}

enum Nothing
{
}

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

function show(mixed $v): mixed
{
    if ($v instanceof UnitEnum) {
        return ['case' => $v::class.'::'.$v->name];
    }
    if (is_array($v)) {
        return array_map('show', $v);
    }

    return $v;
}

$out = [];
foreach ([Suit::Hearts, Status::Active, Level::High] as $case) {
    $row = [
        'case' => $case::class.'::'.$case->name,
        'unit' => $case instanceof UnitEnum,
        'backed' => $case instanceof BackedEnum,
        'array' => (array) $case,
        'cases' => show($case->cases()),
    ];
    foreach (['from', 'tryFrom'] as $method) {
        foreach (['active', 'nope', 1, 10, '10', 2] as $arg) {
            $row['calls'][] = ['method' => $method, 'arg' => $arg, 'result' => attempt(static fn () => show($case->$method($arg)))];
        }
    }
    $out['cases'][] = $row;
}
$out['enum_exists'] = ['Suit' => enum_exists('Suit'), 'suit' => enum_exists('suit'), 'Nope' => enum_exists('Nope'), 'Nothing' => enum_exists('Nothing')];
$out['empty_cases'] = Nothing::cases();

echo json_encode($out, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_THROW_ON_ERROR), "\n";
