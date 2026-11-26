<?php
// iconv($from, $to, $string) between UTF-8 and other charsets (glibc's
// iconv in the oracle), for crates/php2rust-iconv/tests/data/iconv.json
// (scripts/record.sh iconv). Each case is the charsets, the input
// and the result (base64), or false with the notice PHP raised.

ini_set('display_errors', 'stderr');

$texts = [
    '', 'plain ASCII', "tab\tnew\nline\0nul", 'café crème', 'Ærøskøbing ß',
    '€ euro', '“quotes” – dash …', '愛していますか？', 'ｶﾀｶﾅ half width',
    '日本語テキスト', '漢字かなカナ', '한국어', '中文简体', '😀 emoji', "\u{FEFF}bom",
    "\x80\x9f", "\xa0\xff", "\xc3", "\xff\xfe", "a\xe9b",
    "\xc0\xaf", "\xed\xa0\x80", "\xf4\x90\x80\x80", "\xe6\x84", "\x00\xd8a\x00", "\x00\xdc", "\xfe\xff\x00a",
    "\x00\x00\xfe\xff\x00\x00\x00a", "\x1b\$B0&\x1b(B", "\x1b\$B0", "\x1b(J\\~\x1b(B", "\x1b\$", "\x81\x8d\x8f\x90\x9d",
    '¥ yen ‾', '− minus ～ wave', 'Œuvre Š €', "\x7f\x01", "\x1b\$B0&\n0&\x1b(B", "\x1b\$B0&", "\x1b\$@0&\x1b(B", "\x1b(I1\x1b(B", "\x1b\$B\x2d\x21\x1b(B",
    "\x1b\$B\x75\x21\x1b(B", "\x1b(J\x80", "\x1b", "\x0e\x0f", "\xf5\x80\x80\x80", "\xf8\x88\x80\x80\x80", '①②', '〜～', 'ー－',
    '−', '－', '〜', '～', '‖', '∥', '¢', '￠', '£', '￡', '¬', '￢', '—', '―', '\\', '＼', '¥', '‾', '~',
];
$charsets = [
    'UTF-8', 'ASCII', 'US-ASCII', 'ISO-8859-1', 'latin1', 'ISO-8859-15', 'Windows-1252', 'CP1252',
    'UTF-16', 'UTF-16BE', 'UTF-16LE', 'UTF-32', 'UTF-32BE', 'UTF-32LE', 'UCS-2',
    'ISO-2022-JP', 'Shift_JIS', 'SJIS', 'EUC-JP', 'EUC-KR', 'GBK', 'GB18030', 'BIG5',
    'utf8', 'UTF8', 'iso88591', 'NOPE-42',
];

$notice = null;
set_error_handler(function (int $no, string $message) use (&$notice) {
    $notice = $message;

    return true;
});

function convert(string $from, string $to, string $s): array
{
    global $notice;
    $notice = null;
    $r = iconv($from, $to, $s);

    return false === $r ? ['false', $notice] : ['ok', base64_encode($r)];
}

$cases = [];
foreach ($charsets as $charset) {
    foreach ($texts as $t) {
        $there = convert('UTF-8', $charset, $t);
        $cases[] = ['from' => 'UTF-8', 'to' => $charset, 'in' => base64_encode($t), 'out' => $there];
        if ('ok' === $there[0]) {
            $back = convert($charset, 'UTF-8', base64_decode($there[1]));
            $cases[] = ['from' => $charset, 'to' => 'UTF-8', 'in' => $there[1], 'out' => $back];
        }
        // the raw bytes read as this charset
        $cases[] = ['from' => $charset, 'to' => 'UTF-8', 'in' => base64_encode($t), 'out' => convert($charset, 'UTF-8', $t)];
    }
}

echo json_encode(['cases' => $cases], JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES), "\n";
