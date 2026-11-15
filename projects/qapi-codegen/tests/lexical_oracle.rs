//! Compare lexical edge cases with the parser shipped in the pinned QEMU tree.
use qapi_codegen::{parse_schema, set_parser_version};
use std::{path::Path, process::Command};

#[test]
fn lexical_acceptance_matches_qemu() {
    set_parser_version((11, 1, 3));
    let tokens = [
        "{", "'enum'", ":", "'E'", ",", "'data'", ":", "[", "'one'", ",", "'two'", "]", "}",
    ];
    let mut cases = Vec::new();
    for whitespace in [
        "",
        " ",
        "\t",
        "\n",
        "\r\n",
        "\r",
        "\u{b}",
        "\u{c}",
        "\u{85}",
        "\u{a0}",
        "\u{1680}",
        "\u{2000}",
        "\u{2028}",
        "\u{2029}",
        "\u{202f}",
        "\u{205f}",
        "\u{3000}",
        "\u{1c}",
        "\u{1d}",
        "\u{1e}",
        "\u{1f}",
        "\u{200b}",
        "\u{feff}",
        "# comment\n",
        "# comment\r\n",
        "# comment\r",
    ] {
        cases.push(tokens.join(whitespace));
        for index in 0..tokens.len() - 1 {
            let mut input = tokens[..=index].concat();
            input.push_str(whitespace);
            input.push_str(&tokens[index + 1..].concat());
            cases.push(input);
        }
    }
    for literal in [
        "''",
        "'white space'",
        r"'back\\slash'",
        r"'bad\n'",
        r"'bad\'quote'",
        "'snowman ☃'",
        "'DEL\u{7f}'",
        "'NUL\0'",
        "'newline\n'",
        "\"double\"",
        "'unterminated",
    ] {
        cases.push(format!(
            "{{'pragma':{{'command-returns-exceptions':[{literal}]}}}}"
        ));
    }
    for boolean in [
        "true",
        "false",
        "truefalse",
        "truee",
        "False",
        "TRUE",
        "null",
        "0",
        "",
        " true ",
    ] {
        cases.push(format!("{{'command':'test','gen':{boolean}}}"));
    }
    let valid = tokens.concat();
    let documented =
        "##\n# @E:\n# Summary.\n#\n# @one: First.\n# @two: Second.\n#\n# Since: 1.0\n##\n";
    for ending in ["\n", "\r\n", "\r"] {
        cases.push(format!(
            "{}{valid}",
            "##\n# = Section\n#\n# Body.\n##\n".replace('\n', ending)
        ));
        cases.push(format!("{}{valid}", documented.replace('\n', ending)));
        cases.push(format!(
            "{}{valid}",
            format!("##\n# = Section\n#\n# Body.\n##\n{documented}").replace('\n', ending)
        ));
    }
    cases.extend([
        valid.clone(),
        format!("{valid}{valid}"),
        format!("{valid}# final comment"),
        valid.replace("'one',", "'one'"),
        valid.replace("]}", ",]}"),
        valid.replace("]}", "],}"),
        valid.replace("'two'", "'one'"),
        "{'struct':'S','data':{'x':'str','x':'int'}}".into(),
    ]);
    let temp = tempfile::tempdir().unwrap();
    for (index, input) in cases.iter().enumerate() {
        std::fs::write(temp.path().join(format!("{index}.json")), input).unwrap();
    }
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/qemu");
    let output = Command::new("python3")
        .arg("-c")
        .arg(
            r#"
import os, sys
sys.path.insert(0, os.path.join(sys.argv[1], 'scripts'))
from qapi.parser import QAPISchemaParser
from qapi.error import QAPIError
for index in range(int(sys.argv[3])):
    try:
        QAPISchemaParser(os.path.join(sys.argv[2], str(index) + '.json'))
    except QAPIError:
        print('0')
    else:
        print('1')
"#,
        )
        .arg(repo)
        .arg(temp.path())
        .arg(cases.len().to_string())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected = String::from_utf8(output.stdout).unwrap();
    let expected: Vec<_> = expected.lines().collect();
    assert_eq!(expected.len(), cases.len());
    let mismatches: Vec<_> = cases
        .iter()
        .zip(expected)
        .filter_map(|(input, expected)| {
            let accepted = parse_schema(input).is_ok();
            (accepted != (expected == "1")).then(|| {
                format!(
                    "QEMU accepted: {}, Rust accepted: {accepted}: {input:?}",
                    expected == "1"
                )
            })
        })
        .collect();
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}
