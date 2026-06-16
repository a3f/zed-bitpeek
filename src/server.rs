fn main() {
    use std::collections::HashMap;

    use lsp_server::{Connection, Message};

    let (connection, io_threads) = Connection::stdio();

    use lsp_types::{
        CodeActionProviderCapability, HoverProviderCapability, ServerCapabilities,
        TextDocumentSyncCapability, TextDocumentSyncKind,
    };

    let server_caps = ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
        ..Default::default()
    };

    let init_params = connection
        .initialize(serde_json::to_value(server_caps).unwrap())
        .unwrap();

    let mut server = Server {
        settings: Settings::default(),
        documents: HashMap::new(),
    };

    if let Some(opts) = init_params.get("initializationOptions") {
        server.apply_settings_from_value(opts);
    }

    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req).unwrap_or(false) {
                    io_threads.join().unwrap();
                    return;
                }
                let response = server.handle_request(req);
                connection.sender.send(Message::Response(response)).unwrap();
            }
            Message::Notification(not) => {
                server.handle_notification(not);
            }
            Message::Response(_) => {}
        }
    }

    io_threads.join().unwrap();
}

use std::collections::HashMap;

use lsp_server::{Request, RequestId, Response, ResponseError};
use lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams,
    DidChangeConfigurationParams, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, Hover, HoverContents, HoverParams, MarkupContent, MarkupKind,
    Position, Range, TextEdit, Url, WorkspaceEdit,
};
use num_bigint::BigUint;
use num_traits::{One, Zero};
use serde_json::Value;

// ─── constants ───────────────────────────────────────────────────────────

const MAX_MACRO_BIT_INDEX: u32 = 4096;
const CODE_GROUP_SEPARATOR: &str = "\u{2005}";
const CODE_GROUP_PAIR_SEPARATOR: &str = "\u{2004}";

// ─── internal types ──────────────────────────────────────────────────────

#[derive(Clone)]
struct Settings {
    group_spacing: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            group_spacing: true,
        }
    }
}

#[derive(Clone)]
struct Row {
    label: String,
    value: String,
}

struct ParsedNumber {
    word: String,
    big_num: BigUint,
    hexs: String,
    literal_kind: String,
    has_negative_prefix: bool,
    macro_str: Option<String>,
    range: Range,
}

struct BitRange {
    highest: usize,
    lowest: usize,
}

// ─── server ───────────────────────────────────────────────────────────────

struct Server {
    settings: Settings,
    documents: HashMap<Url, String>,
}

impl Server {
    fn handle_request(&mut self, req: Request) -> Response {
        match req.method.as_str() {
            "textDocument/hover" => self.handle_hover(req),
            "textDocument/codeAction" => self.handle_code_action(req),
            _ => Response {
                id: req.id,
                result: None,
                error: Some(ResponseError {
                    code: lsp_server::ErrorCode::MethodNotFound as i32,
                    message: format!("Method not found: {}", req.method),
                    data: None,
                }),
            },
        }
    }

    fn handle_notification(&mut self, not: lsp_server::Notification) {
        match not.method.as_str() {
            "textDocument/didOpen" => {
                if let Ok(p) = serde_json::from_value::<DidOpenTextDocumentParams>(not.params) {
                    self.documents
                        .insert(p.text_document.uri, p.text_document.text);
                }
            }
            "textDocument/didChange" => {
                if let Ok(p) = serde_json::from_value::<DidChangeTextDocumentParams>(not.params) {
                    if let Some(change) = p.content_changes.into_iter().next() {
                        self.documents.insert(p.text_document.uri, change.text);
                    }
                }
            }
            "textDocument/didClose" => {
                if let Ok(p) = serde_json::from_value::<DidCloseTextDocumentParams>(not.params) {
                    self.documents.remove(&p.text_document.uri);
                }
            }
            "workspace/didChangeConfiguration" => {
                if let Ok(p) = serde_json::from_value::<DidChangeConfigurationParams>(not.params) {
                    self.apply_settings_from_value(&p.settings);
                }
            }
            _ => {}
        }
    }

    fn handle_hover(&self, req: Request) -> Response {
        let params: HoverParams = match serde_json::from_value(req.params) {
            Ok(p) => p,
            Err(_) => {
                return Response {
                    id: req.id,
                    result: None,
                    error: Some(ResponseError {
                        code: lsp_server::ErrorCode::InvalidParams as i32,
                        message: "Invalid params".to_string(),
                        data: None,
                    }),
                };
            }
        };

        let tdp = params.text_document_position_params;
        let uri = &tdp.text_document.uri;
        let pos = &tdp.position;

        let doc_text = match self.documents.get(uri) {
            Some(t) => t,
            None => return ok_response(req.id, Value::Null),
        };

        let parsed = match parse_number_at_position(doc_text, pos) {
            Some(p) => p,
            None => return ok_response(req.id, Value::Null),
        };

        let num_le = compute_num_le(&parsed.hexs);
        let ascii = compute_ascii(&parsed.hexs);

        let mut rows = vec![
            Row {
                label: "Binary".to_string(),
                value: format!("0b{}", parsed.big_num.to_str_radix(2)),
            },
            Row {
                label: "Octal".to_string(),
                value: format!("0o{}", parsed.big_num.to_str_radix(8)),
            },
            Row {
                label: "Dec (BE)".to_string(),
                value: parsed.big_num.to_str_radix(10),
            },
            Row {
                label: "Dec (LE)".to_string(),
                value: num_le.to_str_radix(10),
            },
            Row {
                label: "Hex".to_string(),
                value: format!("0x{}", parsed.hexs),
            },
        ];

        if let Some(ref m) = parsed.macro_str {
            rows.push(Row {
                label: "Macro".to_string(),
                value: m.clone(),
            });
        }

        rows.push(Row {
            label: "Ascii".to_string(),
            value: ascii,
        });

        let (heading, rows) = promoted_hover_rows(&parsed.word, &rows);
        let heading_row = heading.unwrap_or(Row {
            label: String::new(),
            value: String::new(),
        });

        let header = hover_table_header(&heading_row, &self.settings);
        let row_lines: Vec<String> = rows
            .iter()
            .map(|row| hover_table_row(row, &self.settings))
            .collect();

        let hover_value = format!("{}\n| :--- | :--- |\n{}\n", header, row_lines.join("\n"));

        let hover = Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: hover_value,
            }),
            range: Some(parsed.range),
        };

        ok_response(req.id, serde_json::to_value(hover).unwrap())
    }

    fn handle_code_action(&self, req: Request) -> Response {
        let params: CodeActionParams = match serde_json::from_value(req.params) {
            Ok(p) => p,
            Err(_) => {
                return Response {
                    id: req.id,
                    result: None,
                    error: Some(ResponseError {
                        code: lsp_server::ErrorCode::InvalidParams as i32,
                        message: "Invalid params".to_string(),
                        data: None,
                    }),
                };
            }
        };

        let uri = &params.text_document.uri;

        let doc_text = match self.documents.get(uri) {
            Some(t) => t,
            None => {
                let empty: Vec<CodeActionOrCommand> = vec![];
                return ok_response(req.id, serde_json::to_value(empty).unwrap());
            }
        };

        let parsed = match parse_number_at_position(doc_text, &params.range.start) {
            Some(p) => p,
            None => {
                let empty: Vec<CodeActionOrCommand> = vec![];
                return ok_response(req.id, serde_json::to_value(empty).unwrap());
            }
        };

        let actions = macro_replacement_actions(uri.as_str(), &parsed);
        let actions: Vec<CodeActionOrCommand> = actions
            .into_iter()
            .map(CodeActionOrCommand::CodeAction)
            .collect();

        ok_response(req.id, serde_json::to_value(actions).unwrap())
    }

    fn apply_settings_from_value(&mut self, value: &Value) {
        let normalized = normalize_settings(value);
        if let Some(group_spacing) = normalized.group_spacing {
            self.settings.group_spacing = group_spacing;
        }
    }
}

fn ok_response(id: RequestId, result: Value) -> Response {
    Response {
        id,
        result: Some(result),
        error: None,
    }
}

// ─── settings normalization ──────────────────────────────────────────────

struct NormalizedSettings {
    group_spacing: Option<bool>,
}

fn normalize_settings(value: &Value) -> NormalizedSettings {
    if !value.is_object() {
        return NormalizedSettings {
            group_spacing: None,
        };
    }

    let source = value
        .get("settings")
        .and_then(|v| v.as_object())
        .map(|_| &value["settings"])
        .unwrap_or(value);

    NormalizedSettings {
        group_spacing: source.get("groupSpacing").and_then(|v| v.as_bool()),
    }
}

// ─── number parsing ──────────────────────────────────────────────────────

fn is_name_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '\''
}

fn strip_integer_suffix(word: &str) -> &str {
    const SUFFIXES: &[&str] = &["llu", "ull", "lu", "ul", "u", "LLU", "ULL", "LU", "UL", "U"];

    for suffix in SUFFIXES {
        if let Some(stripped) = word.strip_suffix(suffix) {
            return stripped;
        }
    }
    word
}

fn position_in_range(pos: &Position, start: u32, end: u32) -> bool {
    start <= pos.character && pos.character <= end
}

fn parse_bit_index(s: &str) -> Option<u32> {
    let bit: u32 = s.parse().ok()?;
    if bit <= MAX_MACRO_BIT_INDEX {
        Some(bit)
    } else {
        None
    }
}

fn parse_macro_at_position(line: &str, pos: &Position) -> Option<ParsedNumber> {
    if let Some(captured) = try_parse_genmask(line, pos) {
        return Some(captured);
    }
    if let Some(captured) = try_parse_bit(line, pos) {
        return Some(captured);
    }
    if let Some(captured) = try_parse_sz(line, pos) {
        return Some(captured);
    }
    None
}

fn try_parse_genmask(line: &str, pos: &Position) -> Option<ParsedNumber> {
    let mut search_start = 0usize;
    loop {
        let remainder = &line[search_start..];
        let idx = remainder.find("GENMASK(")?;
        let abs_start = search_start + idx;
        let after_keyword = &line[abs_start + "GENMASK(".len()..];

        let close = after_keyword.find(')')?;
        let args_str = &after_keyword[..close];
        let end_idx = abs_start + "GENMASK(".len() + close + 1;

        let mut parts = args_str.splitn(2, ',');
        let high_str = parts.next()?.trim();
        let low_str = parts.next()?.trim();

        let high = parse_bit_index(high_str)?;
        let low = parse_bit_index(low_str)?;
        if high < low {
            search_start = end_idx;
            continue;
        }

        let start_char = abs_start as u32;
        let end_char = end_idx as u32;

        if (start_char > 0 && is_name_char(line.chars().nth(start_char as usize - 1)?))
            || (end_char < line.len() as u32 && is_name_char(line.chars().nth(end_char as usize)?))
        {
            search_start = end_idx;
            continue;
        }

        if !position_in_range(pos, start_char, end_char) {
            search_start = end_idx;
            continue;
        }

        let word = line[abs_start..end_idx].to_string();
        let big_num =
            ((BigUint::one() << (high - low + 1) as usize) - BigUint::one()) << low as usize;
        let hexs = big_num.to_str_radix(16);
        let macro_str = format_macro(&big_num);

        return Some(ParsedNumber {
            word,
            big_num,
            hexs,
            literal_kind: "macro".to_string(),
            has_negative_prefix: false,
            macro_str,
            range: Range {
                start: Position {
                    line: pos.line,
                    character: start_char,
                },
                end: Position {
                    line: pos.line,
                    character: end_char,
                },
            },
        });
    }
}

fn try_parse_bit(line: &str, pos: &Position) -> Option<ParsedNumber> {
    let mut search_start = 0usize;
    loop {
        let remainder = &line[search_start..];
        let idx = remainder.find("BIT(")?;
        let abs_start = search_start + idx;
        let after_keyword = &line[abs_start + "BIT(".len()..];

        let close = after_keyword.find(')')?;
        let arg_str = after_keyword[..close].trim();
        let end_idx = abs_start + "BIT(".len() + close + 1;

        let bit = parse_bit_index(arg_str)?;

        let start_char = abs_start as u32;
        let end_char = end_idx as u32;

        if (start_char > 0 && is_name_char(line.chars().nth(start_char as usize - 1)?))
            || (end_char < line.len() as u32 && is_name_char(line.chars().nth(end_char as usize)?))
        {
            search_start = end_idx;
            continue;
        }

        if !position_in_range(pos, start_char, end_char) {
            search_start = end_idx;
            continue;
        }

        let word = line[abs_start..end_idx].to_string();
        let big_num = BigUint::one() << bit as usize;
        let hexs = big_num.to_str_radix(16);
        let macro_str = format_macro(&big_num);

        return Some(ParsedNumber {
            word,
            big_num,
            hexs,
            literal_kind: "macro".to_string(),
            has_negative_prefix: false,
            macro_str,
            range: Range {
                start: Position {
                    line: pos.line,
                    character: start_char,
                },
                end: Position {
                    line: pos.line,
                    character: end_char,
                },
            },
        });
    }
}

fn try_parse_sz(line: &str, pos: &Position) -> Option<ParsedNumber> {
    let mut search_start = 0usize;
    loop {
        let remainder = &line[search_start..];
        let idx = remainder.find("SZ_")?;
        let abs_start = search_start + idx;

        let after_prefix = &line[abs_start + "SZ_".len()..];
        let digits_end = after_prefix
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(after_prefix.len());

        if digits_end == 0 {
            search_start = abs_start + "SZ_".len();
            continue;
        }

        let digits = &after_prefix[..digits_end];
        let end_idx = abs_start + "SZ_".len() + digits_end;

        let has_k = after_prefix[digits_end..].starts_with('K');
        let end_idx = if has_k { end_idx + 1 } else { end_idx };

        let start_char = abs_start as u32;
        let end_char = end_idx as u32;

        if (start_char > 0 && is_name_char(line.chars().nth(start_char as usize - 1)?))
            || (end_char < line.len() as u32 && is_name_char(line.chars().nth(end_char as usize)?))
        {
            search_start = end_idx;
            continue;
        }

        if !position_in_range(pos, start_char, end_char) {
            search_start = end_idx;
            continue;
        }

        let base: BigUint = digits.parse().ok()?;
        let big_num = if has_k {
            base * BigUint::from(1024u32)
        } else {
            return None;
        };

        let word = line[abs_start..end_idx].to_string();
        let hexs = big_num.to_str_radix(16);
        let macro_str = format_macro(&big_num);

        return Some(ParsedNumber {
            word,
            big_num,
            hexs,
            literal_kind: "macro".to_string(),
            has_negative_prefix: false,
            macro_str,
            range: Range {
                start: Position {
                    line: pos.line,
                    character: start_char,
                },
                end: Position {
                    line: pos.line,
                    character: end_char,
                },
            },
        });
    }
}

fn parse_number_at_position(doc_text: &str, pos: &Position) -> Option<ParsedNumber> {
    let line = get_line(doc_text, pos.line as usize)?;

    if let Some(parsed) = parse_macro_at_position(line, pos) {
        return Some(parsed);
    }

    let chars: Vec<char> = line.chars().collect();
    let col = pos.character as usize;

    let mut word_start = col;
    while word_start > 0 && is_name_char(chars[word_start - 1]) {
        word_start -= 1;
    }

    let mut word_end = col;
    while word_end < chars.len() && is_name_char(chars[word_end]) {
        word_end += 1;
    }

    if word_start >= word_end {
        return None;
    }

    let word: String = chars[word_start..word_end].iter().collect();
    let mut has_negative_prefix = word_start > 0 && chars[word_start - 1] == '-';

    let (word, word_start) = if word.starts_with('-') && !has_negative_prefix {
        has_negative_prefix = true;
        (word[1..].to_string(), word_start + 1)
    } else {
        (word, word_start)
    };

    let numeric_word = strip_integer_suffix(&word);
    let chars: Vec<char> = numeric_word.chars().collect();

    let result = if let Some(r) = parse_binary(&chars) {
        r
    } else if let Some(r) = parse_octal(&chars) {
        r
    } else if let Some(r) = parse_hex(&chars) {
        r
    } else if let Some(r) = parse_decimal(&chars) {
        r
    } else {
        return None;
    };

    let hexs = if result.literal_kind == "hex" {
        numeric_word[2..].replace(['_', '\''], "")
    } else {
        result.big_num.to_str_radix(16)
    };

    let macro_str = format_macro(&result.big_num);

    Some(ParsedNumber {
        word: word.to_string(),
        big_num: result.big_num,
        hexs,
        literal_kind: result.literal_kind,
        has_negative_prefix,
        macro_str,
        range: Range {
            start: Position {
                line: pos.line,
                character: word_start as u32,
            },
            end: Position {
                line: pos.line,
                character: word_end as u32,
            },
        },
    })
}

fn get_line(text: &str, line_number: usize) -> Option<&str> {
    text.lines().nth(line_number)
}

struct ParseResult {
    big_num: BigUint,
    literal_kind: String,
}

fn parse_binary(chars: &[char]) -> Option<ParseResult> {
    if chars.len() < 3 {
        return None;
    }
    if chars[0] != '0' || (chars[1] != 'b' && chars[1] != 'B') {
        return None;
    }
    let digits: String = chars[2..]
        .iter()
        .filter(|c| **c != '_' && **c != '\'')
        .collect();
    if digits.is_empty() || !digits.chars().all(|c| c == '0' || c == '1') {
        return None;
    }

    let mut big_num = BigUint::zero();
    for ch in &chars[2..] {
        match ch {
            '0' | '1' => {
                let digit = BigUint::from((*ch as u8 - b'0') as u32);
                big_num = big_num * BigUint::from(2u32) + digit;
            }
            '_' | '\'' => {}
            _ => return None,
        }
    }

    Some(ParseResult {
        big_num,
        literal_kind: "binary".to_string(),
    })
}

fn parse_octal(chars: &[char]) -> Option<ParseResult> {
    if chars.len() < 2 {
        return None;
    }
    if chars[0] != '0' {
        return None;
    }

    let start = if chars.len() > 2 && (chars[1] == 'o' || chars[1] == 'O') {
        2
    } else {
        1
    };

    let digits: String = chars[start..]
        .iter()
        .filter(|c| **c != '_' && **c != '\'')
        .collect();
    if digits.is_empty() || !digits.chars().all(|c| ('0'..='7').contains(&c)) {
        return None;
    }

    let mut big_num = BigUint::zero();
    for ch in &chars[start..] {
        match ch {
            '0'..='7' => {
                let digit = BigUint::from((*ch as u8 - b'0') as u32);
                big_num = big_num * BigUint::from(8u32) + digit;
            }
            '_' | '\'' => {}
            _ => return None,
        }
    }

    Some(ParseResult {
        big_num,
        literal_kind: "octal".to_string(),
    })
}

fn parse_hex(chars: &[char]) -> Option<ParseResult> {
    if chars.len() < 3 {
        return None;
    }
    if chars[0] != '0' || (chars[1] != 'x' && chars[1] != 'X') {
        return None;
    }

    let digits: String = chars[2..]
        .iter()
        .filter(|c| **c != '_' && **c != '\'')
        .collect();
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }

    let mut big_num = BigUint::zero();
    for ch in &chars[2..] {
        match ch {
            c if c.is_ascii_hexdigit() => {
                let digit = BigUint::from(c.to_digit(16).unwrap());
                big_num = big_num * BigUint::from(16u32) + digit;
            }
            '_' | '\'' => {}
            _ => return None,
        }
    }

    Some(ParseResult {
        big_num,
        literal_kind: "hex".to_string(),
    })
}

fn parse_decimal(chars: &[char]) -> Option<ParseResult> {
    if chars.is_empty() {
        return None;
    }

    let digits: String = chars.iter().filter(|c| **c != '_' && **c != '\'').collect();
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }

    let mut big_num = BigUint::zero();
    for ch in chars {
        match ch {
            c if c.is_ascii_digit() => {
                let digit = BigUint::from(c.to_digit(10).unwrap());
                big_num = big_num * BigUint::from(10u32) + digit;
            }
            '_' | '\'' => {}
            _ => return None,
        }
    }

    Some(ParseResult {
        big_num,
        literal_kind: "decimal".to_string(),
    })
}

// ─── macro detection ─────────────────────────────────────────────────────

fn is_power_of_two(value: &BigUint) -> bool {
    *value > BigUint::zero() && (value & (value - BigUint::one())) == BigUint::zero()
}

fn highest_set_bit(value: &BigUint) -> usize {
    value.bits() as usize - 1
}

fn consecutive_set_bit_range(value: &BigUint) -> Option<BitRange> {
    let bits = value.to_str_radix(2);
    let first_zero = bits.find('0')?;
    let last_one = bits.rfind('1')?;

    if first_zero <= last_one {
        return None;
    }

    Some(BitRange {
        highest: bits.len() - 1,
        lowest: bits.len() - last_one - 1,
    })
}

fn format_size_macro(value: &BigUint) -> Option<String> {
    if *value <= BigUint::from(512u32) {
        return Some(format!("SZ_{}", value));
    }

    let units: [(u32, &str); 6] = [
        (60, "E"),
        (50, "P"),
        (40, "T"),
        (30, "G"),
        (20, "M"),
        (10, "K"),
    ];

    for (shift, suffix) in &units {
        let unit = BigUint::one() << *shift as usize;
        if *value >= unit && value % &unit == BigUint::zero() {
            let size = value / &unit;
            if size <= BigUint::from(512u32) {
                return Some(format!("SZ_{}{}", size, suffix));
            }
        }
    }

    None
}

fn format_macro(value: &BigUint) -> Option<String> {
    if *value == BigUint::zero() {
        return None;
    }

    if is_power_of_two(value) {
        let bit = highest_set_bit(value);
        if let Some(size_macro) = format_size_macro(value) {
            Some(format!("BIT({})  /* {} */", bit, size_macro))
        } else {
            Some(format!("BIT({})", bit))
        }
    } else if let Some(range) = consecutive_set_bit_range(value) {
        Some(format!("GENMASK({}, {})", range.highest, range.lowest))
    } else {
        None
    }
}

// ─── value formatting ────────────────────────────────────────────────────

fn compute_num_le(hexs: &str) -> BigUint {
    let mut num_le = BigUint::zero();

    let len = hexs.len();
    let mut i = len;
    while i > 0 {
        if i >= 2 {
            let byte_val = u8::from_str_radix(&hexs[i - 2..i], 16).unwrap_or(0);
            num_le = num_le * BigUint::from(256u32) + BigUint::from(byte_val as u32);
            i -= 2;
        } else {
            let byte_val = u8::from_str_radix(&hexs[0..1], 16).unwrap_or(0);
            num_le = num_le * BigUint::from(256u32) + BigUint::from(byte_val as u32);
            i -= 1;
        }
    }

    num_le
}

fn from_char_code(code: u8) -> String {
    match code {
        0..=31 => String::from(std::char::from_u32(code as u32 + 0x2400).unwrap()),
        32..=126 | 128..=255 => String::from(code as char),
        127 => "\u{2421}".to_string(),
    }
}

fn compute_ascii(hexs: &str) -> String {
    let mut result = String::new();

    let start = hexs.len() % 2;
    if start == 1 {
        let v = u8::from_str_radix(&hexs[0..1], 16).unwrap_or(0);
        result.push_str(&from_char_code(v));
    }

    let mut i = start;
    while i < hexs.len() {
        let v = u8::from_str_radix(&hexs[i..i + 2], 16).unwrap_or(0);
        result.push_str(&from_char_code(v));
        i += 2;
    }

    result
}

fn escape_md(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('`', "\\`")
        .replace('*', "\\*")
        .replace('_', "\\_")
}

fn group_separator(index: usize, group_count: usize, settings: &Settings) -> &'static str {
    if !settings.group_spacing {
        return "";
    }

    let groups_to_right = group_count - index - 1;
    if groups_to_right % 2 == 0 {
        CODE_GROUP_PAIR_SEPARATOR
    } else {
        CODE_GROUP_SEPARATOR
    }
}

fn grouped_code_spans(
    value: &str,
    prefix: &str,
    group_length: usize,
    settings: &Settings,
) -> String {
    let lower_val = value.to_lowercase();
    let lower_prefix = prefix.to_lowercase();
    let has_prefix = lower_val.starts_with(&lower_prefix);
    let digits = if has_prefix {
        &value[prefix.len()..]
    } else {
        value
    };

    let mut groups: Vec<&str> = Vec::new();
    let mut end = digits.len();
    while end > 0 {
        let start = if end >= group_length {
            end - group_length
        } else {
            0
        };
        groups.push(&digits[start..end]);
        end = start;
    }
    groups.reverse();

    if groups.is_empty() {
        if has_prefix {
            return escape_md(&value[..prefix.len()]);
        }
        return String::new();
    }

    let last = groups.len() - 1;
    let result: String = groups
        .iter()
        .enumerate()
        .map(|(i, group)| {
            let mut escaped = escape_md(group);
            if i == 0 && has_prefix {
                escaped = format!("{}{}", escape_md(&value[..prefix.len()]), escaped);
            }
            if i < last {
                escaped.push_str(group_separator(i, groups.len(), settings));
            }
            escaped
        })
        .collect();

    result
}

fn strip_macro_comment(value: &str) -> &str {
    if let Some(idx) = value.rfind("/*") {
        value[..idx].trim_end()
    } else {
        value.trim()
    }
}

fn normalize_macro_value(value: &str) -> Option<String> {
    let upper = value.to_uppercase();
    if let Some(rest) = upper.strip_prefix("BIT(") {
        if let Some(inner) = rest.strip_suffix(')') {
            let bit: u32 = inner.trim().parse().ok()?;
            return Some(format!("BIT({})", bit));
        }
    }
    if let Some(rest) = upper.strip_prefix("GENMASK(") {
        if let Some(inner) = rest.strip_suffix(')') {
            let mut parts = inner.splitn(2, ',');
            let high: u32 = parts.next()?.trim().parse().ok()?;
            let low: u32 = parts.next()?.trim().parse().ok()?;
            return Some(format!("GENMASK({}, {})", high, low));
        }
    }
    None
}

fn normalize_prefixed_integer(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() >= 3 && chars[0] == '0' && matches!(chars[1], 'b' | 'B' | 'o' | 'O' | 'x' | 'X')
    {
        let prefix = chars[1].to_lowercase().to_string();
        let digits: String = chars[2..].iter().filter(|c| **c != '0').collect();
        let digits = if digits.is_empty() {
            "0".to_string()
        } else {
            digits.to_lowercase()
        };
        format!("0{}{}", prefix, digits)
    } else {
        value.to_string()
    }
}

fn normalize_heading_value(value: &str) -> String {
    let without_comment = strip_macro_comment(value);
    if let Some(macro_val) = normalize_macro_value(without_comment) {
        return macro_val;
    }
    normalize_prefixed_integer(&without_comment.replace(['_', '\''], ""))
}

fn normalize_hovered_value(value: &str) -> String {
    normalize_heading_value(strip_integer_suffix(value)).to_lowercase()
}

fn normalize_translated_value(value: &str) -> String {
    normalize_heading_value(value).to_lowercase()
}

fn promoted_heading_value(row: &Row) -> String {
    if row.label == "Macro" {
        row.value.trim().to_string()
    } else {
        normalize_heading_value(&row.value)
    }
}

fn promoted_hover_rows(word: &str, rows: &[Row]) -> (Option<Row>, Vec<Row>) {
    let normalized_word = normalize_hovered_value(word);

    let heading_index = rows
        .iter()
        .position(|row| normalize_translated_value(&row.value) == normalized_word);

    match heading_index {
        None => (None, rows.to_vec()),
        Some(idx) => {
            let heading = Row {
                label: rows[idx].label.clone(),
                value: promoted_heading_value(&rows[idx]),
            };
            let rest: Vec<Row> = rows
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != idx)
                .map(|(_, row)| row.clone())
                .collect();
            (Some(heading), rest)
        }
    }
}

fn formatted_macro_value(value: &str) -> String {
    let text = value.trim();
    if let Some(idx) = text.find("/*") {
        let before = text[..idx].trim_end();
        let comment = text[idx..].trim_end();
        format!(
            "{}\u{00a0}\u{00a0}{}",
            escape_md(before),
            escape_md(comment)
        )
    } else {
        escape_md(text)
    }
}

fn formatted_hover_value(row: &Row, settings: &Settings) -> String {
    match row.label.as_str() {
        "Binary" => grouped_code_spans(&row.value, "0b", 4, settings),
        "Octal" => grouped_code_spans(&row.value, "0o", 3, settings),
        "Dec (BE)" | "Dec (LE)" => grouped_code_spans(&row.value, "", 3, settings),
        "Hex" => grouped_code_spans(&row.value, "0x", 4, settings),
        "Macro" => formatted_macro_value(&row.value),
        "Ascii" => escape_md(&row.value),
        _ => {
            if row.value.is_empty() {
                String::new()
            } else {
                escape_md(&row.value)
            }
        }
    }
}

fn hover_table_header(row: &Row, settings: &Settings) -> String {
    format!(
        "| {} | {} |",
        escape_md(&row.label),
        formatted_hover_value(row, settings)
    )
}

fn hover_table_row(row: &Row, settings: &Settings) -> String {
    format!(
        "| {} | {} |",
        escape_md(&row.label),
        formatted_hover_value(row, settings)
    )
}

// ─── code actions ────────────────────────────────────────────────────────

fn replacement_action(uri: &str, range: &Range, replacement: &str) -> CodeAction {
    let edit = WorkspaceEdit {
        changes: Some({
            let mut map = HashMap::new();
            map.insert(
                Url::parse(uri).unwrap(),
                vec![TextEdit {
                    range: *range,
                    new_text: replacement.to_string(),
                }],
            );
            map
        }),
        document_changes: None,
        change_annotations: None,
    };

    CodeAction {
        title: format!("Replace with {}", replacement),
        kind: Some(CodeActionKind::REFACTOR_REWRITE),
        diagnostics: None,
        edit: Some(edit),
        command: None,
        is_preferred: None,
        disabled: None,
        data: None,
    }
}

fn macro_replacement_actions(uri: &str, parsed: &ParsedNumber) -> Vec<CodeAction> {
    if parsed.literal_kind != "hex"
        || parsed.has_negative_prefix
        || parsed.big_num == BigUint::zero()
    {
        return vec![];
    }

    if is_power_of_two(&parsed.big_num) {
        let bit = highest_set_bit(&parsed.big_num);
        let mut actions = vec![replacement_action(
            uri,
            &parsed.range,
            &format!("BIT({})", bit),
        )];
        if let Some(size_macro) = format_size_macro(&parsed.big_num) {
            actions.push(replacement_action(uri, &parsed.range, &size_macro));
        }
        return actions;
    }

    if let Some(range) = consecutive_set_bit_range(&parsed.big_num) {
        return vec![replacement_action(
            uri,
            &parsed.range,
            &format!("GENMASK({}, {})", range.highest, range.lowest),
        )];
    }

    vec![]
}
