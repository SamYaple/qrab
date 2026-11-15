use qapi_codegen::{parse_schema, parse_schema_with_version, parser_version, with_parser_version};

#[test]
fn explicit_file_grammar_restores_the_caller_after_success_and_failure() {
    let original = parser_version();
    with_parser_version((11, 1, 3), || {
        let legacy = "{'enum':'Kind','data':['one',]}";
        let schema = parse_schema_with_version(legacy, (2, 3, 0)).unwrap();
        assert_eq!(schema.len(), 1);
        assert_eq!(parser_version(), (11, 1, 3));
        assert!(parse_schema(legacy).is_err());
        assert!(parse_schema_with_version("invalid", (2, 3, 0)).is_err());
        assert_eq!(parser_version(), (11, 1, 3));
    });
    assert_eq!(parser_version(), original);
}

#[test]
fn grammar_scopes_are_nested_thread_local_and_unwind_safe() {
    let original = parser_version();
    let thread_default = std::thread::spawn(parser_version).join().unwrap();
    with_parser_version((2, 3, 0), || {
        let result = std::panic::catch_unwind(|| {
            with_parser_version((4, 2, 0), || {
                assert_eq!(parser_version(), (4, 2, 0));
                assert_eq!(
                    std::thread::spawn(parser_version).join().unwrap(),
                    thread_default
                );
                panic!("scope unwind");
            });
        });
        assert!(result.is_err());
        assert_eq!(parser_version(), (2, 3, 0));
    });
    assert_eq!(parser_version(), original);
}
