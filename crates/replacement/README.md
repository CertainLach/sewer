# sewer-replacement

Simple inverse-regex style template language, for use in replacements

At replacements, provides several patterns:

| Pattern | Inserts |
| ------- | ------- |
| `$$` | Literal `"$"` |
| `$n`, where `n` - number | Indexed match, i.e `str.replace("aaa(bbb)", "$1aaa") == "bbbaaa"`. Zero index is reserved for full match. If specified group does not exists - then throws error, see below for error handling |
| `$<name>`, where `name` - string | Named match, i.e `str.replace("aaa(?P<name>bbb)", "$<name>aaa") == "bbbaaa"`. If specified group does not exists - then throws error |
| `\\` | Literal `"\"` |
| `\ ` | Literal `" "` |
| `\xnn`, where `n` - hex digit | Insert literal byte with hex code `0xnn` |
| `(expr1\|expr2\|expr3...)`, where expr - replacement expression | Try to execute `expr1`, if error is thrown (see above) - then try next expression, if no expression is matched - throw error |

Expression may start with `(?x)`, this makes parser ignore whitespace, and allows writing comments
```
(?x)
$<group1> # Insert group 1
==
$<group2> # Insert group 2
```
