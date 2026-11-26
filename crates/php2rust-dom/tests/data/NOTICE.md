# Third-party content in these test vectors

`parse.json` and `schema.json` are recorded from PHP 8.4.27. Their inputs
include files that other projects wrote. They are kept byte for byte, with
their own notices, only to test this crate.

| Content | From | Copyright and terms |
|---|---|---|
| XML test fixtures (`file symfony/...` cases) | Symfony's Config, Translation and Validator test fixtures | Copyright (c) 2004-present Fabien Potencier, MIT (see the repository LICENSE) |
| Translation files (`file demo/translations/...` cases) | symfony/demo | Copyright (c) 2015-present Fabien Potencier, MIT (see the repository LICENSE) |
| `constraint-mapping` schema | Symfony Validator (`Resources/config/schema/`) | Copyright (c) 2004-present Fabien Potencier, MIT |
| `xliff-1.2`, `xliff-2.0`, `xliff-2.2` schemas | The OASIS XLIFF schemas, as Symfony Translation ships them (`Resources/schemas/`) | Copyright (c) OASIS Open; the notice in each schema is kept unchanged |
| `xml.xsd` import | The W3C schema for the `xml:` namespace, as Symfony Translation ships it | Copyright (c) World Wide Web Consortium; W3C Software and Document License, notice kept unchanged in the file |

All other cases are hand-written for this crate.
