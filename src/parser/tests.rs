use super::*;
use crate::lexer::lex;

#[test]
fn parses_a_complete_program() {
    let source = r#"
            namespace demo;
            score timer = 0;
            @tick fn tick() {
                timer += 1;
                if timer >= 20 { announce(); } else { run "say waiting"; }
            }
            fn announce() { schedule tick() after 1 s append; }
        "#;
    let program = parse(lex(source, 0).unwrap()).unwrap();
    assert_eq!(program.namespace, "demo");
    assert_eq!(program.scores.len(), 1);
    assert_eq!(program.functions.len(), 2);
}
