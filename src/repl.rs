use std::{
    cell::RefCell,
    io,
    rc::Rc,
    sync::{Arc, Mutex},
};

use rustyline::completion::Completer;
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::history::DefaultHistory;
use rustyline::validate::{ValidationContext, ValidationResult, Validator};
use rustyline::{
    Cmd, CompletionType, ConditionalEventHandler, Config, Context, EditMode, Editor, Event,
    EventContext, EventHandler, Helper, KeyCode, KeyEvent, Modifiers, RepeatCount,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    environment, evaluator, lexer,
    macro_expansion::{define_macros, expand_macros},
    parser,
    token::TokenType,
};

const PROMPT: &str = "monkey> ";
const CONTINUATION_PROMPT: &str = "    ... ";
const MONKEY_FACE: &str = r#"
            __,__
   .--.  .-"     "-.  .--.
  / .. \/  .-. .-.  \/ .. \
 | |  '|  /   Y   \  |'  | |
 | \   \  \ 0 | 0 /  /   / |
  \ '- ,\.-"""""""-./, -' /
   ''-' /_   ^ ^   _\ '-''
       |  \._   _./  |
       \   \ '~' /   /
        '._ '-=-' _.'
           '-----'
"#;

#[derive(Clone, Default)]
struct InputValidator {
    replacement: Arc<Mutex<Option<(usize, String)>>>,
    token_yank: Arc<Mutex<Option<String>>>,
}

impl Helper for InputValidator {}

impl Completer for InputValidator {
    type Candidate = String;

    fn complete(
        &self,
        _line: &str,
        _position: usize,
        _context: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<String>)> {
        Ok(match self.replacement.lock().unwrap().take() {
            Some((start, replacement)) => (start, vec![replacement]),
            None => (0, Vec::new()),
        })
    }
}

impl Hinter for InputValidator {
    type Hint = String;
}

impl Highlighter for InputValidator {}

impl Validator for InputValidator {
    fn validate(&self, context: &mut ValidationContext<'_>) -> rustyline::Result<ValidationResult> {
        Ok(validate_input(context.input()))
    }
}

fn validate_input(input: &str) -> ValidationResult {
    match delimiter_state(input) {
        Ok((delimiters, in_string)) if in_string || !delimiters.is_empty() => {
            ValidationResult::Incomplete
        }
        Ok(_) => ValidationResult::Valid(None),
        Err(message) => ValidationResult::Invalid(Some(message)),
    }
}

fn delimiter_state(input: &str) -> Result<(Vec<char>, bool), String> {
    let mut delimiters = Vec::new();
    let mut in_string = false;

    for character in input.chars() {
        if character == '"' {
            in_string = !in_string;
            continue;
        }
        if in_string {
            continue;
        }
        match character {
            '(' => delimiters.push(')'),
            '[' => delimiters.push(']'),
            '{' => delimiters.push('}'),
            ')' | ']' | '}' => {
                if delimiters.pop() != Some(character) {
                    return Err(format!("unexpected closing delimiter: {character}"));
                }
            }
            _ => {}
        }
    }

    Ok((delimiters, in_string))
}

struct AutoIndent;

impl ConditionalEventHandler for AutoIndent {
    fn handle(
        &self,
        _event: &Event,
        _count: RepeatCount,
        _positive: bool,
        context: &EventContext<'_>,
    ) -> Option<Cmd> {
        indented_newline(context.line(), context.pos())
    }
}

fn indented_newline(input: &str, position: usize) -> Option<Cmd> {
    if !matches!(validate_input(input), ValidationResult::Incomplete) {
        return None;
    }

    let (delimiters, in_string) = delimiter_state(&input[..position]).ok()?;
    let indentation = if in_string { 0 } else { delimiters.len() * 4 };
    Some(Cmd::Insert(1, format!("\n{}", " ".repeat(indentation))))
}

struct AutoDedent {
    closing: char,
    helper: InputValidator,
}

impl ConditionalEventHandler for AutoDedent {
    fn handle(
        &self,
        _event: &Event,
        count: RepeatCount,
        _positive: bool,
        context: &EventContext<'_>,
    ) -> Option<Cmd> {
        if count != 1 {
            return None;
        }
        let replacement = dedented_closer(context.line(), context.pos(), self.closing)?;
        *self.helper.replacement.lock().unwrap() = Some(replacement);
        Some(Cmd::Complete)
    }
}

fn dedented_closer(input: &str, position: usize, closing: char) -> Option<(usize, String)> {
    let prefix = &input[..position];
    let line = prefix.rsplit('\n').next()?;
    if line.is_empty()
        || !line
            .chars()
            .all(|character| matches!(character, ' ' | '\t'))
    {
        return None;
    }

    let (mut delimiters, in_string) = delimiter_state(prefix).ok()?;
    if in_string || delimiters.pop() != Some(closing) {
        return None;
    }

    Some((
        position - line.len(),
        format!("{}{closing}", " ".repeat(delimiters.len() * 4)),
    ))
}

struct TokenEditing {
    helper: InputValidator,
}

impl ConditionalEventHandler for TokenEditing {
    fn handle(
        &self,
        event: &Event,
        count: RepeatCount,
        positive: bool,
        context: &EventContext<'_>,
    ) -> Option<Cmd> {
        let Event::KeySeq(keys) = event else {
            return None;
        };
        let [key] = keys.as_slice() else {
            return None;
        };
        self.command(*key, context.line(), context.pos(), count, positive)
    }
}

impl TokenEditing {
    fn command(
        &self,
        key: KeyEvent,
        input: &str,
        position: usize,
        count: RepeatCount,
        positive: bool,
    ) -> Option<Cmd> {
        if key == KeyEvent::ctrl('W') {
            return Some(if positive {
                self.delete_token(input, position, count)
            } else {
                Cmd::Noop
            });
        }
        if key == KeyEvent::ctrl('Y') {
            return self
                .helper
                .token_yank
                .lock()
                .unwrap()
                .as_ref()
                .map(|text| Cmd::Insert(count, text.clone()));
        }
        if matches!(
            key,
            KeyEvent(KeyCode::Char('U' | 'K'), Modifiers::CTRL)
                | KeyEvent(
                    KeyCode::Char('d' | 'D') | KeyCode::Backspace,
                    Modifiers::ALT
                )
        ) {
            *self.helper.token_yank.lock().unwrap() = None;
        }
        None
    }
    fn delete_token(&self, input: &str, position: usize, count: RepeatCount) -> Cmd {
        let Some(start) = previous_token_start(input, position, count) else {
            return Cmd::Noop;
        };
        *self.helper.token_yank.lock().unwrap() = Some(input[start..position].to_owned());
        *self.helper.replacement.lock().unwrap() = Some((start, String::new()));
        Cmd::Complete
    }
}

fn previous_token_start(input: &str, position: usize, count: RepeatCount) -> Option<usize> {
    if position == 0 || count == 0 {
        return None;
    }

    let prefix = &input[..position];
    let mut lexer = lexer::Lexer::new(prefix);
    let mut starts = Vec::new();
    loop {
        let (token, span) = lexer.next_token_with_span();
        if token.token_type() == TokenType::EOF {
            break;
        }
        starts.push(span.start);
    }
    let start = starts
        .iter()
        .rev()
        .nth(usize::from(count - 1))
        .copied()
        .unwrap_or(0);
    prefix
        .grapheme_indices(true)
        .find(|(offset, grapheme)| offset + grapheme.len() > start)
        .map(|(offset, _)| offset)
}

pub fn start_interactive() -> rustyline::Result<()> {
    let config = Config::builder()
        .edit_mode(EditMode::Emacs)
        .completion_type(CompletionType::List)
        .build();
    let mut editor = Editor::<InputValidator, DefaultHistory>::with_config(config)?;
    let helper = InputValidator::default();
    editor.set_helper(Some(helper.clone()));
    editor.bind_sequence(KeyEvent::ctrl('p'), Cmd::LineUpOrPreviousHistory(1));
    editor.bind_sequence(KeyEvent::ctrl('n'), Cmd::LineDownOrNextHistory(1));
    editor.bind_sequence(
        Event::Any,
        EventHandler::Conditional(Box::new(TokenEditing {
            helper: helper.clone(),
        })),
    );
    editor.bind_sequence(
        KeyEvent::from('\r'),
        EventHandler::Conditional(Box::new(AutoIndent)),
    );
    for closing in [')', ']', '}'] {
        editor.bind_sequence(
            KeyEvent::from(closing),
            EventHandler::Conditional(Box::new(AutoDedent {
                closing,
                helper: helper.clone(),
            })),
        );
    }
    let env = Rc::new(RefCell::new(environment::Environment::new()));
    let macro_env = Rc::new(RefCell::new(environment::Environment::new()));
    let mut output = io::stdout();

    loop {
        match editor.readline(PROMPT) {
            Ok(buffer) => {
                if buffer.trim().is_empty() {
                    continue;
                }
                editor.add_history_entry(buffer.as_str())?;
                evaluate(&buffer, &env, &macro_env, &mut output);
            }
            Err(ReadlineError::Interrupted | ReadlineError::Eof) => {
                println!("Goodbye!");
                break;
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

pub fn start(input: &mut dyn io::BufRead, output: &mut dyn io::Write) {
    let env = Rc::new(RefCell::new(environment::Environment::new()));
    let macro_env = Rc::new(RefCell::new(environment::Environment::new()));
    let mut buffer = String::new();
    loop {
        let prompt = if buffer.is_empty() {
            PROMPT
        } else {
            CONTINUATION_PROMPT
        };
        write!(output, "{prompt}").unwrap();
        output.flush().unwrap();

        if input.read_line(&mut buffer).unwrap() == 0 {
            if !buffer.trim().is_empty() {
                print_parser_errors(output, &["unexpected end of input".to_string()]);
            }
            break;
        }

        if buffer.trim().is_empty() {
            buffer.clear();
            continue;
        }

        match validate_input(&buffer) {
            ValidationResult::Incomplete => continue,
            ValidationResult::Invalid(message) => {
                if let Some(message) = message {
                    print_parser_errors(output, &[message]);
                }
            }
            _ => evaluate(&buffer, &env, &macro_env, output),
        }
        buffer.clear();
    }
}

fn evaluate(
    input: &str,
    env: &environment::EnvironmentRef,
    macro_env: &environment::EnvironmentRef,
    output: &mut dyn io::Write,
) {
    let mut lexer = lexer::Lexer::new(input);
    let mut parser = parser::Parser::new(&mut lexer);
    let mut program = parser.parse_program();
    if !parser.errors().is_empty() {
        print_parser_errors(output, parser.errors());
        return;
    }

    define_macros(&mut program, macro_env);
    expand_macros(&mut program, macro_env).unwrap();

    let object = evaluator::eval(program, env);
    writeln!(output, "{}", object.inspect()).unwrap();
}

fn print_parser_errors(output: &mut dyn io::Write, errors: &[String]) {
    writeln!(output, "{}", MONKEY_FACE).unwrap();
    writeln!(output, "Woops! We ran into some monkey business here!").unwrap();
    writeln!(output, " parser errors:").unwrap();
    for error in errors {
        let _ = writeln!(output, "\t{error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deletes_monkey_tokens_instead_of_whitespace_delimited_words() {
        for (input, deleted_characters) in [
            ("addTwo(2)", 1),
            ("addTwo(2", 1),
            ("addTwo(", 1),
            ("addTwo", 6),
            ("x ==", 2),
            ("x != \t", 4),
            ("let _name", 5),
            ("foo123", 3),
            ("x + 123  ", 5),
            ("x;\n    ", 6),
            (" \t\n", 3),
            ("\"hello world\"", 13),
            ("\"hello world", 12),
            ("\"你好\"", 4),
            ("\"e\u{301}🙂\"", 4),
            ("e\u{301}", 1),
            ("x @", 1),
        ] {
            assert_eq!(
                previous_token_start(input, input.len(), 1)
                    .map(|start| input[start..].graphemes(true).count()),
                Some(deleted_characters),
                "{input:?}"
            );
        }
    }

    #[test]
    fn deletes_only_before_the_cursor_and_supports_repeat_counts() {
        assert_eq!(
            previous_token_start("addTwo(123)", "addTwo(12".len(), 1),
            Some("addTwo(".len())
        );
        assert_eq!(
            previous_token_start("addTwo(2)", "addTwo(2)".len(), 2),
            Some("addTwo(".len())
        );
        assert_eq!(
            previous_token_start("addTwo(2)", "addTwo(2)".len(), 10),
            Some(0)
        );
        assert_eq!(previous_token_start("addTwo(2)", 0, 1), None);
        assert_eq!(previous_token_start("addTwo(2)", 9, 0), None);
    }

    #[test]
    fn queues_token_deletion_and_saves_the_deleted_text() {
        let helper = InputValidator::default();
        let editing = TokenEditing {
            helper: helper.clone(),
        };
        let input = "addTwo(123)";
        assert_eq!(
            editing.delete_token(input, "addTwo(12".len(), 1),
            Cmd::Complete
        );
        assert_eq!(*helper.token_yank.lock().unwrap(), Some("12".to_owned()));
        let history = DefaultHistory::new();
        assert_eq!(
            helper
                .complete(input, "addTwo(12".len(), &Context::new(&history))
                .unwrap(),
            ("addTwo(".len(), vec![String::new()])
        );
    }

    #[test]
    fn handles_normalized_control_keys_and_falls_back_for_native_kills() {
        let editing = TokenEditing {
            helper: InputValidator::default(),
        };
        assert_eq!(
            editing.command(KeyEvent::ctrl('W'), "x ==", 4, 1, true),
            Some(Cmd::Complete)
        );
        assert_eq!(
            editing.command(KeyEvent::ctrl('Y'), "x ", 2, 1, true),
            Some(Cmd::Insert(1, "==".to_owned()))
        );
        assert_eq!(editing.command(KeyEvent::ctrl('K'), "x ", 0, 1, true), None);
        assert_eq!(editing.command(KeyEvent::ctrl('Y'), "", 0, 1, true), None);
    }

    #[test]
    fn consumes_the_dedent_replacement_once() {
        let helper = InputValidator::default();
        let history = DefaultHistory::new();
        let context = Context::new(&history);
        let input = "fn() {\n    ";
        *helper.replacement.lock().unwrap() = dedented_closer(input, input.len(), '}');
        assert_eq!(
            helper.complete(input, input.len(), &context).unwrap(),
            ("fn() {\n".len(), vec!["}".to_owned()])
        );
        assert_eq!(
            helper.complete(input, input.len(), &context).unwrap(),
            (0, Vec::<String>::new())
        );
    }

    #[test]
    fn dedents_matching_closers_at_the_start_of_a_line() {
        for (input, closing, expected) in [
            ("fn() {\n    ", '}', "}"),
            ("fn() {\n    if (true) {\n        ", '}', "    }"),
            ("[1,\n    ", ']', "]"),
            ("add(1,\n    ", ')', ")"),
            ("fn() {\n    \"{[(\";\n    ", '}', "}"),
            ("fn() {\n\t", '}', "}"),
        ] {
            assert_eq!(
                dedented_closer(input, input.len(), closing),
                Some((input.rfind('\n').unwrap() + 1, expected.to_owned())),
                "{input:?}"
            );
        }
    }

    #[test]
    fn leaves_strings_inline_closers_and_mismatched_delimiters_unchanged() {
        for (input, closing) in [
            ("fn() {\n    \"hello\n    ", '}'),
            ("fn() {\n    x; ", '}'),
            ("fn() {\n    ", ']'),
            ("([)]\n    ", ')'),
            ("    ", '}'),
            ("fn() {\n", '}'),
        ] {
            assert_eq!(
                dedented_closer(input, input.len(), closing),
                None,
                "{input:?}"
            );
        }
    }

    #[test]
    fn dedents_at_the_cursor_without_replacing_the_rest_of_the_line() {
        let prefix = "fn() {\n    ";
        let input = format!("{prefix};\n1 + 2");
        assert_eq!(
            dedented_closer(&input, prefix.len(), '}'),
            Some(("fn() {\n".len(), "}".to_owned()))
        );
    }

    #[test]
    fn indents_continuations_without_changing_strings() {
        for (input, expected) in [
            ("fn(x) {", "\n    "),
            ("fn(x) {\n    x + 2;", "\n    "),
            ("fn(x) {\n    if (true) {", "\n        "),
            ("fn(x) {\n    if (true) { x; }", "\n    "),
            ("[1,", "\n    "),
            ("add(", "\n    "),
            ("fn() {\n    \"{[(\";", "\n    "),
            ("fn() {\n    \"hello", "\n"),
        ] {
            assert_eq!(
                indented_newline(input, input.len()),
                Some(Cmd::Insert(1, expected.to_owned())),
                "{input:?}"
            );
        }
    }

    #[test]
    fn indents_at_the_cursor_and_preserves_default_submission() {
        let input = "fn() {\n    if (true) {";
        assert_eq!(
            indented_newline(input, "fn() {".len()),
            Some(Cmd::Insert(1, "\n    ".to_owned()))
        );
        for input in ["", "1 + 2", "fn() {}", "([)]"] {
            assert_eq!(indented_newline(input, input.len()), None, "{input:?}");
        }
    }

    #[test]
    fn waits_for_unclosed_delimiters_and_strings() {
        for input in ["fn(arr, f) {", "[1,\n2,", "add(1,", "\"hello", "fn() {\n\n"] {
            assert!(
                matches!(validate_input(input), ValidationResult::Incomplete),
                "{input:?}"
            );
        }
        for input in [
            "fn(arr, f) { f(arr); };",
            "[1,\n2]",
            "\"{[(\"",
            "\"hello\nworld\"",
        ] {
            assert!(
                matches!(validate_input(input), ValidationResult::Valid(_)),
                "{input:?}"
            );
        }
        for input in ["([)]", "}"] {
            assert!(
                matches!(validate_input(input), ValidationResult::Invalid(_)),
                "{input:?}"
            );
        }
    }

    #[test]
    fn evaluates_multiline_function_after_closing_brace() {
        let mut input = &b"let addTwo = fn(x) {\n\n x + 2;\n};\naddTwo(2)\n"[..];
        let mut output = Vec::new();
        start(&mut input, &mut output);
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("(x + 2)"), "{output}");
        assert!(output.contains("monkey> 4\n"), "{output}");
        assert!(!output.contains("parser errors:"), "{output}");
    }

    #[test]
    fn rejects_unfinished_function_at_end_of_input() {
        let mut input = &b"let unfinished = fn(x) {\nx + 1;\n"[..];
        let mut output = Vec::new();
        start(&mut input, &mut output);
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("unexpected end of input"), "{output}");
        assert!(!output.contains("fn(x)"), "{output}");
    }
}
