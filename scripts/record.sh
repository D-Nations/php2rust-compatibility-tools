#!/usr/bin/env bash
# Record test vectors from real PHP (the oracle in oracle/, Docker) into the
# crates' tests/data/ files.
#
#   scripts/record.sh value        crates/php2rust-value/tests/data/{php,filter,parse_str,parse_url,loose_equals,http_build_query}.json
#   scripts/record.sh value-decode crates/php2rust-value/tests/data/{html_entity_decode,json_decode}.json
#   scripts/record.sh value-json   crates/php2rust-value/tests/data/json_encode.json
#   scripts/record.sh value-dates  crates/php2rust-value/tests/data/dates.json
#   scripts/record.sh value-args   crates/php2rust-value/tests/data/named_args.json
#   scripts/record.sh value-enums  crates/php2rust-value/tests/data/enums.json
#   scripts/record.sh date         crates/php2rust-date/tests/data/php_date.json, objects.json
#   scripts/record.sh dom          crates/php2rust-dom/tests/data/parse.json
#   scripts/record.sh dom-schema   crates/php2rust-dom/tests/data/schema.json
#   scripts/record.sh all          all of the above
#
# The dom targets also read Symfony's XML test fixtures and call Symfony's
# XmlUtils and XliffUtils, so they need a symfony-rs checkout (with
# upstream/ fetched) and a symfony/demo checkout with vendor/ installed. See
# oracle/compose.yaml for the paths.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
export MSYS_NO_PATHCONV=1

oracle() { docker compose -f oracle/compose.yaml run --rm -T php "$@"; }
oracle_symfony() { docker compose -f oracle/compose.yaml run --rm -T php-symfony "$@"; }

# record RUNNER RECORDER OUT CHECK: run RECORDER, check the JSON with the
# Python expression CHECK (d is the decoded document), then move it to OUT.
record() {
    local runner=$1 recorder=$2 out=$3 check=$4 tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    "$runner" "oracle/recorders/$recorder" > "$tmp"
    python -c "import json,sys; d=json.load(open(sys.argv[1], encoding='utf-8')); print($check)" "$tmp"
    mv "$tmp" "$out"
    echo "written to $out"
}

value_dir=crates/php2rust-value/tests/data
dom_dir=crates/php2rust-dom/tests/data

value() {
    record oracle php_value.php "$value_dir/php.json" '"PHP value vectors"'
    record oracle filter_var.php "$value_dir/filter.json" 'len(d["cases"]), "filter_var() cases"'
    record oracle php_value_parse_str.php "$value_dir/parse_str.json" 'len(d), "parse_str() cases"'
    record oracle php_value_parse_url.php "$value_dir/parse_url.json" 'len(d), "parse_url() cases"'
    record oracle php_value_loose_equals.php "$value_dir/loose_equals.json" 'len(d), "== pairs"'
    record oracle php_value_http_build_query.php "$value_dir/http_build_query.json" 'len(d), "http_build_query() cases"'
}

value_decode() {
    record oracle php_value_html_entity_decode.php "$value_dir/html_entity_decode.json" 'len(d), "html_entity_decode() cases"'
    record oracle php_value_json_decode.php "$value_dir/json_decode.json" 'len(d), "json_decode() cases"'
}

value_args() {
    record oracle php_value_named_args.php "$value_dir/named_args.json" 'len(d), "argument bindings"'
}

value_json() {
    record oracle php_value_json.php "$value_dir/json_encode.json" 'len(d["encode"]), "json_encode() calls,", len(d["decode"]), "json_decode() calls"'
}

value_dates() {
    record oracle php_value_dates.php "$value_dir/dates.json" '"date vectors"'
}

value_enums() {
    record oracle php_value_enums.php "$value_dir/enums.json" 'len(d["cases"]), "enum cases"'
}

value_var_dump() {
    record oracle php_value_var_dump.php "$value_dir/var_dump.json" 'len(d["cases"]), "var_dump() calls"'
}

php_iconv() {
    record oracle php_iconv.php crates/php2rust-iconv/tests/data/iconv.json 'len(d["cases"]), "iconv() calls"'
}

php_date() {
    record oracle php_date.php crates/php2rust-date/tests/data/php_date.json 'len(d["parse"]), "date texts,", len(d["modify"]), "modify() calls"'
    record oracle php_date_objects.php crates/php2rust-date/tests/data/objects.json 'len(d["compare"]), "date comparisons"'
}

dom() {
    record oracle_symfony php_dom_parse.php "$dom_dir/parse.json" '"DOMDocument::loadXML() answers"'
}

dom_schema() {
    record oracle_symfony php_dom_schema.php "$dom_dir/schema.json" '"DOMDocument::schemaValidateSource() answers"'
}

case "${1:-}" in
    value) value ;;
    value-decode) value_decode ;;
    value-json) value_json ;;
    value-dates) value_dates ;;
    value-args) value_args ;;
    value-enums) value_enums ;;
    value-var-dump) value_var_dump ;;
    date) php_date ;;
    iconv) php_iconv ;;
    dom) dom ;;
    dom-schema) dom_schema ;;
    all) value; value_decode; value_json; value_dates; value_args; value_enums; value_var_dump; php_date; php_iconv; dom; dom_schema ;;
    *) echo "usage: scripts/record.sh value|value-decode|value-json|value-dates|value-args|value-enums|value-var-dump|date|iconv|dom|dom-schema|all" >&2; exit 2 ;;
esac
