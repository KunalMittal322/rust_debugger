use std::io::Write;

use codemap::CodeMap;
use codemap_diagnostic::{ColorConfig, Diagnostic, Emitter, Level, SpanLabel, SpanStyle};
use rust_sitter::errors::{ParseError, ParseErrorReason};

fn parse_int(text: &str) -> u64 {
    let text = text.trim();
    if text.starts_with("0x") {
        let text = text.split_at(2).1;
        u64::from_str_radix(text, 16).unwrap()
    } else {
        text.parse().unwrap()
    }
}

fn parse_symbol(text: &str) -> String {
    text.to_string()
}

#[rust_sitter::grammar("command")]
pub mod grammar {
    use crate::parser_debugger::parse_int;
    use crate::parser_debugger::parse_symbol;

    #[rust_sitter::language]
    pub enum CommandExpr {
        StepInto(#[rust_sitter::leaf(text = "s")] ()),
        Go(#[rust_sitter::leaf(text = "g")] ()),
        ReadRegisters(#[rust_sitter::leaf(text = "r")] ()),
        DisplayBytes(#[rust_sitter::leaf(text = "db")] (), Box<EvalExpr>),
        Evaluation(#[rust_sitter::leaf(text = "?")] (), Box<EvalExpr>),
        Quit(#[rust_sitter::leaf(text = "q")] ()),
        SetBreakpoint(#[rust_sitter::leaf(text = "bs")] (), Box<EvalExpr>),
        ListBreakPoint(#[rust_sitter::leaf(text = "bl")] ()),
        ClearBreakPoint(#[rust_sitter::leaf(text = "bc")] (), Box<EvalExpr>),
    }

    #[rust_sitter::language]
    pub enum EvalExpr {
        Number(
            #[rust_sitter::leaf(pattern = r"\s*(\d+|0x[0-9a-fA-F]+)\s*", transform = parse_int)]
            u64,
        ),
        Symbol(
            #[rust_sitter::leaf(pattern = r"(\s*([a-zA-Z0-9_@#.]+!)?[a-zA-Z0-9_@#.]+)", transform = parse_symbol)]
             String,
        ),
        #[rust_sitter::prec_left(1)]
        Add(
            Box<EvalExpr>,
            #[rust_sitter::leaf(text = "+")] (),
            Box<EvalExpr>,
        ),
    }
}

fn convert_parse_error_to_diagnostics(
    file_span: &codemap::Span,
    error: &ParseError,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match &error.reason {
        ParseErrorReason::MissingToken(tok) => diagnostics.push(Diagnostic {
            level: Level::Error,
            message: format!("Missing token: \"{tok}\""),
            code: Some("S000".to_string()),
            spans: vec![SpanLabel {
                span: file_span.subspan(error.start as u64, error.end as u64),
                style: SpanStyle::Primary,
                label: Some(format!("missing \"{tok}\"")),
            }],
        }),

        ParseErrorReason::UnexpectedToken(tok) => diagnostics.push(Diagnostic {
            level: Level::Error,
            message: format!("Unexpected token: \"{tok}\""),
            code: Some("S000".to_string()),
            spans: vec![SpanLabel {
                span: file_span.subspan(error.start as u64, error.end as u64),
                style: SpanStyle::Primary,
                label: Some(format!("unexpected \"{tok}\"")),
            }],
        }),

        ParseErrorReason::FailedNode(errors) => {
            if errors.is_empty() {
                diagnostics.push(Diagnostic {
                    level: Level::Error,
                    message: "Failed to parse node".to_string(),
                    code: Some("S000".to_string()),
                    spans: vec![SpanLabel {
                        span: file_span.subspan(error.start as u64, error.end as u64),
                        style: SpanStyle::Primary,
                        label: Some("failed".to_string()),
                    }],
                })
            } else {
                for error in errors {
                    convert_parse_error_to_diagnostics(file_span, error, diagnostics);
                }
            }
        }
    }
}

pub fn read_command() -> grammar::CommandExpr {
    let stdin = std::io::stdin();
    loop {
        print!(">");
        std::io::stdout().flush().unwrap();
        let mut input = String::new();

        stdin.read_line(&mut input).unwrap();
        let cmd = grammar::parse(input.trim());
        match cmd {
            Ok(c) => return c,
            Err(errs) => {
                let mut codemap = CodeMap::new();
                let file_span = codemap.add_file("<input>".to_string(), input.to_string());
                let mut diagnostics = vec![];
                for error in errs {
                    convert_parse_error_to_diagnostics(&file_span.span, &error, &mut diagnostics);
                }

                let mut emitter = Emitter::stderr(ColorConfig::Always, Some(&codemap));
                emitter.emit(&diagnostics);
            }
        }
    }
}
