use regex::Regex;

use crate::inference::InferenceResult;

#[derive(Debug, Clone)]
struct BracketInfo {
    token: String,
    start: usize,
    matched: bool,
}

pub struct LatexPostProcessor {
    redundant_sup: Regex,
    redundant_sub: Regex,
    trailing_whitespace: Regex,
    unnecessary_after_lbrace: Regex,
    unnecessary_before_rbrace: Regex,
    empty_patterns: Vec<Regex>,
}

impl LatexPostProcessor {
    pub fn new() -> InferenceResult<Self> {
        let whitespace_symbols = [
            r"\\ +",
            r"\\quad\s*",
            r"\\qquad\s*",
            r"\\,\s*",
            r"\\:\s*",
            r"\\;\s*",
            r"\\enspace\s*",
            r"\\thinspace\s*",
            r"\\!\s*",
        ]
        .join("|");

        let empty_patterns = [
            r"\\hat\s*\{\s*\}",
            r"\^\s*\{\s*\}",
            r"_\s*\{\s*\}",
            r"\\text\s*\{\s*\}",
            r"\\tilde\s*\{\s*\}",
            r"\\bar\s*\{\s*\}",
            r"\\vec\s*\{\s*\}",
            r"\\acute\s*\{\s*\}",
            r"\\grave\s*\{\s*\}",
            r"\\breve\s*\{\s*\}",
            r"\\overline\s*\{\s*\}",
            r"\\dot\s*\{\s*\}",
            r"\\ddot\s*\{\s*\}",
            r"\\widehat\s*\{\s*\}",
            r"\\widetilde\s*\{\s*\}",
        ]
        .into_iter()
        .map(Regex::new)
        .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            redundant_sup: Regex::new(r"^\^\s*\{\s*(.*?)\s*\}")?,
            redundant_sub: Regex::new(r"^_\s*\{\s*(.*?)\s*\}")?,
            trailing_whitespace: Regex::new(&format!(r"(?:{})+$", whitespace_symbols))?,
            unnecessary_after_lbrace: Regex::new(r"(\{)\s+")?,
            unnecessary_before_rbrace: Regex::new(r"\s+(\})")?,
            empty_patterns,
        })
    }

    pub fn process(&self, raw: &str) -> String {
        let mut text = self.remove_redundant_script(raw);
        text = self.remove_trailing_whitespace(&text);
        text = self.replace_illegal_symbols(&text);
        for _ in 0..10 {
            let next = self.remove_empty_text(&text);
            if next == text {
                break;
            }
            text = next;
        }
        text = self.fix_left_right_pairs(&text);
        text = self.remove_unnecessary_spaces(&text);
        text.trim().to_string()
    }

    fn remove_redundant_script(&self, text: &str) -> String {
        let text = self.redundant_sup.replace(text, "$1").into_owned();
        self.redundant_sub.replace(&text, "$1").trim().to_string()
    }

    fn remove_trailing_whitespace(&self, latex: &str) -> String {
        self.trailing_whitespace.replace(latex, "").trim().to_string()
    }

    fn replace_illegal_symbols(&self, text: &str) -> String {
        text.replace(r"\.", r"\ .")
            .replace(r"\=", r"\ =")
            .replace(r"\-", r"\ -")
            .replace(r"\~", r"\ ~")
    }

    fn remove_empty_text(&self, text: &str) -> String {
        let mut current = text.to_string();
        for pattern in &self.empty_patterns {
            current = pattern.replace_all(&current, "").into_owned();
        }
        current.trim().to_string()
    }

    pub fn fix_left_right_pairs(&self, latex: &str) -> String {
        let mut current = latex.to_string();
        let mut lefts = find_all_left_or_right(&current, "left");
        let mut rights = find_all_left_or_right(&current, "right");

        for left in &mut lefts {
            if let Some(right) = rights
                .iter_mut()
                .find(|right| !right.matched && right.start > left.start && match_left_right(&left.token, &right.token))
            {
                left.matched = true;
                right.matched = true;
            }
        }

        for left in lefts.iter().rev() {
            if !left.matched {
                let end = left.start + "\\left".len();
                current.replace_range(left.start..end, "     ");
            }
        }
        for right in rights.iter().rev() {
            if !right.matched {
                let end = right.start + "\\right".len();
                current.replace_range(right.start..end, "      ");
            }
        }

        collapse_whitespace(&current)
    }

    fn remove_unnecessary_spaces(&self, latex: &str) -> String {
        let mut out = String::with_capacity(latex.len());
        let chars: Vec<char> = latex.chars().collect();
        let mut index = 0;

        while index < chars.len() {
            let ch = chars[index];
            if ch == '\\' {
                out.push(ch);
                index += 1;
                while index < chars.len() && chars[index].is_ascii_alphabetic() {
                    out.push(chars[index]);
                    index += 1;
                }
                let mut next_index = index;
                while next_index < chars.len() && chars[next_index].is_whitespace() {
                    next_index += 1;
                }
                if next_index > index {
                    if next_index < chars.len() && chars[next_index].is_ascii_alphabetic() {
                        out.push(' ');
                    }
                    index = next_index;
                }
                continue;
            }

            if ch == '{' {
                out.push(ch);
                index += 1;
                while index < chars.len() && chars[index].is_whitespace() {
                    index += 1;
                }
                continue;
            }

            if ch == '}' {
                while out.ends_with(' ') {
                    out.pop();
                }
                out.push(ch);
                index += 1;
                continue;
            }

            if ch == '^' || ch == '_' {
                while out.ends_with(' ') {
                    out.pop();
                }
                out.push(ch);
                index += 1;
                while index < chars.len() && chars[index].is_whitespace() {
                    index += 1;
                }
                continue;
            }

            if matches!(ch, '+' | '-' | '=') {
                if !out.ends_with('\\') {
                    while out.ends_with(' ') {
                        out.pop();
                    }
                    out.push(ch);
                    index += 1;
                    while index < chars.len() && chars[index].is_whitespace() {
                        index += 1;
                    }
                    continue;
                }
            }

            if ch.is_whitespace() {
                if !out.ends_with(' ') {
                    out.push(' ');
                }
                index += 1;
                continue;
            }

            out.push(ch);
            index += 1;
        }

        let out = self
            .unnecessary_after_lbrace
            .replace_all(&out, "$1")
            .into_owned();
        let out = self
            .unnecessary_before_rbrace
            .replace_all(&out, "$1")
            .into_owned();
        tighten_token_spacing(&out).trim().to_string()
    }
}

fn collapse_whitespace(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut saw_space = false;
    for ch in input.chars() {
        if ch.is_whitespace() {
            if !saw_space {
                out.push(' ');
                saw_space = true;
            }
        } else {
            saw_space = false;
            out.push(ch);
        }
    }
    out.trim().to_string()
}

fn find_all_left_or_right(latex: &str, kind: &str) -> Vec<BracketInfo> {
    let pattern = format!(r"\{}\s*", if kind == "left" { "\\left" } else { "\\right" });
    let regex = Regex::new(&pattern).expect("valid left/right regex");
    let mut infos = Vec::new();

    for capture in regex.find_iter(latex) {
        let start = capture.start();
        let mut cursor = capture.end();
        while let Some(ch) = latex[cursor..].chars().next() {
            if ch.is_whitespace() {
                cursor += ch.len_utf8();
            } else {
                break;
            }
        }
        if cursor >= latex.len() {
            continue;
        }

        let mut end = cursor;
        let Some(first_char) = latex[end..].chars().next() else {
            continue;
        };
        end += first_char.len_utf8();
        if first_char.is_ascii_alphabetic() {
            continue;
        }

        while end < latex.len() {
            let Some(next_char) = latex[end..].chars().next() else {
                break;
            };
            if next_char == '\\' || next_char == ' ' {
                end += next_char.len_utf8();
                while end < latex.len() {
                    let Some(alpha) = latex[end..].chars().next() else {
                        break;
                    };
                    if alpha.is_ascii_alphabetic() {
                        end += alpha.len_utf8();
                    } else {
                        break;
                    }
                }
            } else {
                break;
            }
        }

        infos.push(BracketInfo {
            token: latex[cursor..end].trim().to_string(),
            start,
            matched: false,
        });
    }

    infos.sort_by_key(|info| info.start);
    infos
}

fn match_left_right(left_token: &str, right_token: &str) -> bool {
    let left = normalize_bracket_token(left_token);
    let right = normalize_bracket_token(right_token);
    matches!(
        (left.as_str(), right.as_str()),
        ("", "")
            | ("(", ")")
            | (r"\{", ".")
            | ("⟮", "⟯")
            | ("[", "]")
            | ("⟨", "⟩")
            | ("{", "}")
            | ("⌈", "⌉")
            | ("┌", "┐")
            | ("⌊", "⌋")
            | ("└", "┘")
            | ("⎰", "⎱")
            | ("lt", "gt")
            | ("lang", "rang")
            | (r"\langle", r"\rangle")
            | (r"\lbrace", r"\rbrace")
            | ("lBrace", "rBrace")
            | (r"\lbracket", r"\rbracket")
            | (r"\lceil", r"\rceil")
            | ("lcorner", "rcorner")
            | (r"\lfloor", r"\rfloor")
            | (r"\lgroup", r"\rgroup")
            | (r"\lmoustache", r"\rmoustache")
            | (r"\lparen", r"\rparen")
            | (r"\lvert", r"\rvert")
            | (r"\lVert", r"\rVert")
    )
}

fn normalize_bracket_token(token: &str) -> String {
    let mut current = token.trim().replace(' ', "");

    loop {
        if !current.starts_with('\\') {
            break;
        }
        let rest = &current[1..];
        let letters_len = rest.chars().take_while(|ch| ch.is_ascii_alphabetic()).count();
        if letters_len == 0 {
            break;
        }
        let suffix = &rest[letters_len..];
        if suffix.is_empty() {
            break;
        }
        current = suffix.to_string();
    }

    current
}

fn tighten_token_spacing(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());

    for index in 0..chars.len() {
        let ch = chars[index];
        if ch != ' ' {
            out.push(ch);
            continue;
        }

        let prev_non_space = previous_non_space(&chars, index);
        let next_non_space = next_non_space(&chars, index);
        let (prev_token, prev_is_command) = previous_token(&chars, index);
        let (next_token, next_is_command) = next_token(&chars, index);

        let should_remove = matches!((prev_non_space, next_non_space), (Some('}'), Some('{')))
            || (!prev_is_command
                && !next_is_command
                && prev_token.len() == 1
                && next_token.len() == 1
                && prev_token.chars().all(|ch| ch.is_ascii_alphanumeric())
                && next_token.chars().all(|ch| ch.is_ascii_alphanumeric()))
            || (!prev_is_command
                && prev_token.chars().all(|ch| ch.is_ascii_digit())
                && next_token.len() == 1
                && next_token.chars().all(|ch| ch.is_ascii_alphabetic()))
            || (!prev_is_command && prev_token.len() == 1 && matches!(next_non_space, Some('\\')));

        if !should_remove && !out.ends_with(' ') {
            out.push(' ');
        }
    }

    out
}

fn previous_non_space(chars: &[char], index: usize) -> Option<char> {
    chars[..index].iter().rev().copied().find(|ch| !ch.is_whitespace())
}

fn next_non_space(chars: &[char], index: usize) -> Option<char> {
    chars[index + 1..]
        .iter()
        .copied()
        .find(|ch| !ch.is_whitespace())
}

fn previous_token(chars: &[char], index: usize) -> (String, bool) {
    let mut end = index;
    while end > 0 && chars[end - 1].is_whitespace() {
        end -= 1;
    }
    let mut start = end;
    while start > 0 && chars[start - 1].is_ascii_alphanumeric() {
        start -= 1;
    }
    let token = chars[start..end].iter().collect::<String>();
    let is_command = start > 0 && chars[start - 1] == '\\';
    (token, is_command)
}

fn next_token(chars: &[char], index: usize) -> (String, bool) {
    let mut start = index + 1;
    while start < chars.len() && chars[start].is_whitespace() {
        start += 1;
    }
    let is_command = start > 0 && chars[start - 1] == '\\';
    let mut end = start;
    while end < chars.len() && chars[end].is_ascii_alphanumeric() {
        end += 1;
    }
    (chars[start..end].iter().collect::<String>(), is_command)
}
