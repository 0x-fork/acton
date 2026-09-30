use expect_test::expect;
use tolk_compiler::compiler::{CompilerCheckResult, CompilerInternalResult};
use tolk_compiler::{Compiler, CompilerError, CompilerResult};

#[test]
fn structured_diagnostics_preserve_cross_file_notes() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let main = root.join("main.tolk");
    std::fs::write(root.join("types.tolk"), "struct Record { value: int }\n").unwrap();
    std::fs::write(
        &main,
        "import \"types\"\nfun main(): Record { return Record { unknown: 1 }; }\n",
    )
    .unwrap();
    let mut compiler = Compiler::new();
    compiler.json_errors = true;
    let CompilerResult::Error(error) = compiler.compile(&main, false) else {
        panic!("invalid struct field must produce a diagnostic");
    };
    let json = serde_json::to_string_pretty(&error.errors).unwrap();
    let normalized_root = dunce::canonicalize(root).unwrap();
    let json = json.replace(normalized_root.to_str().unwrap(), "$ROOT");
    let expected = expect![[r#"
        [
          {
            "message": "field `unknown` not found in struct `Record`",
            "range": {
              "file_name": "$ROOT/main.tolk",
              "start_line_no": 2,
              "start_char_no": 38,
              "end_line_no": 2,
              "end_char_no": 45,
              "text_inside": "unknown"
            },
            "in_function": "in function main",
            "secondary_locations": [
              {
                "note": "struct declared here",
                "range": {
                  "file_name": "$ROOT/types.tolk",
                  "start_line_no": 1,
                  "start_char_no": 8,
                  "end_line_no": 1,
                  "end_char_no": 14,
                  "text_inside": "Record"
                }
              }
            ]
          }
        ]"#]];
    expected.assert_eq(&json);
    let checked = compiler.check(&main).unwrap();
    expected.assert_eq(
        &serde_json::to_string_pretty(&checked)
            .unwrap()
            .replace(normalized_root.to_str().unwrap(), "$ROOT"),
    );
}

#[test]
fn diagnostics_can_have_no_range_and_retain_hints() {
    let error: CompilerError = serde_json::from_str(
        r#"{
        "message": "cannot compile\nhint: update the input",
        "secondary_locations": [{"note": "check the compiler settings"}]
    }"#,
    )
    .unwrap();
    expect![[r#"
        {
          "message": "cannot compile\nhint: update the input",
          "range": null,
          "in_function": null,
          "secondary_locations": [
            {
              "note": "check the compiler settings",
              "range": null
            }
          ]
        }"#]]
    .assert_eq(&serde_json::to_string_pretty(&error).unwrap());
}

#[test]
fn native_failures_keep_their_message_without_source_diagnostics() {
    let json = r#"{"status":"error","message":"cannot read compiler input"}"#;
    let CompilerInternalResult::Error(compiled) = serde_json::from_str(json).unwrap() else {
        panic!("expected compilation failure")
    };
    let CompilerCheckResult::Error(checked) = serde_json::from_str(json).unwrap() else {
        panic!("expected check failure")
    };
    expect![[r"
        compile: cannot read compiler input (0)
        check: cannot read compiler input (0)"]]
    .assert_eq(&format!(
        "compile: {} ({})\ncheck: {} ({})",
        compiled.message,
        compiled.errors.len(),
        checked.message,
        checked.errors.len()
    ));
}
