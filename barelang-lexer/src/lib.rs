use error::SingleTokenError;
use miette::{Error, SourceSpan};

mod error;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TokenKind {
    Task,
    Ident,
    LeftBrace,
    RightBrace,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Token<'de> {
    pub origin: &'de str,
    pub offset: usize,
    pub kind: TokenKind,
}

pub struct Lexer<'de> {
    whole: &'de str,
    rest: &'de str,
    byte: usize,
}

impl<'de> Lexer<'de> {
    pub fn new(input: &'de str) -> Self {
        Self {
            whole: input,
            rest: input,
            byte: 0,
        }
    }
}

impl<'de> Iterator for Lexer<'de> {
    type Item = Result<Token<'de>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let mut chars = self.rest.chars();
            let c = chars.next()?;
            let c_at = self.byte;
            let c_str = &self.rest[..c.len_utf8()];
            let c_onwards = self.rest;

            self.rest = chars.as_str();
            self.byte += c.len_utf8();

            let just = |kind: TokenKind| Token {
                origin: c_str,
                offset: c_at,
                kind,
            };

            enum Started {
                Ident,
            }

            let started = match c {
                '{' => return Some(Ok(just(TokenKind::LeftBrace))),
                '}' => return Some(Ok(just(TokenKind::RightBrace))),
                'a'..='z' | 'A'..='Z' | '_' => Started::Ident,
                c if c.is_whitespace() => continue,
                _ => {
                    return Some(Err(SingleTokenError {
                        src: self.whole.to_string(),
                        token: c,
                        err_span: SourceSpan::from(self.byte - c.len_utf8()..self.byte),
                    }
                    .into()))
                }
            };

            break match started {
                Started::Ident => {
                    let first_char_that_is_not_an_ident = c_onwards
                        .find(|c| !matches!(c, 'a'..='z' | 'A'..='Z' | '_' | '0' ..='9'))
                        .unwrap_or(c_onwards.len());

                    let literal = &c_onwards[..first_char_that_is_not_an_ident];
                    let bytes_unaccounted_for = literal.len() - c.len_utf8();
                    self.byte += bytes_unaccounted_for;
                    self.rest = &self.rest[bytes_unaccounted_for..];

                    let kind = match literal {
                        "task" => TokenKind::Task,
                        _ => TokenKind::Ident,
                    };

                    Some(Ok(Token {
                        origin: literal,
                        offset: c_at,
                        kind,
                    }))
                }
            };
        }
    }
}

#[cfg(test)]
mod test {
    use miette::Error;
    use quickcheck::{Arbitrary, TestResult};
    use quickcheck_macros::quickcheck;

    use crate::{error::SingleTokenError, Lexer, Token, TokenKind};

    macro_rules! test_token_kinds {
        ($name:ident, $input:literal, $res:expr) => {
            #[test]
            fn $name() {
                let res: Vec<TokenKind> = $res;
                let lexer = super::Lexer::new($input);
                let got = lexer
                    .map(|t| t.unwrap())
                    .map(|t| t.kind)
                    .collect::<Vec<_>>();
                assert_eq!(res, got);
            }
        };
    }

    test_token_kinds!(test_empty, "", vec![]);
    test_token_kinds!(
        test_braces,
        "{}",
        vec![TokenKind::LeftBrace, TokenKind::RightBrace]
    );
    test_token_kinds!(
        test_braces_with_newlines,
        "\n\n{\n}",
        vec![TokenKind::LeftBrace, TokenKind::RightBrace]
    );
    test_token_kinds!(
        test_empty_task,
        "task foo {}",
        vec![
            TokenKind::Task,
            TokenKind::Ident,
            TokenKind::LeftBrace,
            TokenKind::RightBrace
        ]
    );
    test_token_kinds!(
        test_empty_task_with_number_in_ident,
        "task foo3 {}",
        vec![
            TokenKind::Task,
            TokenKind::Ident,
            TokenKind::LeftBrace,
            TokenKind::RightBrace
        ]
    );
    test_token_kinds!(
        test_identifiers_can_start_with_underscore,
        "task _foo3 {}",
        vec![
            TokenKind::Task,
            TokenKind::Ident,
            TokenKind::LeftBrace,
            TokenKind::RightBrace
        ]
    );
    test_token_kinds!(
        test_empty_task_with_new_lines,
        "task foo\n{\n}",
        vec![
            TokenKind::Task,
            TokenKind::Ident,
            TokenKind::LeftBrace,
            TokenKind::RightBrace
        ]
    );

    test_token_kinds!(
        test_plugin_ident,
        "task foo\n{\n foo {}}",
        vec![
            TokenKind::Task,
            TokenKind::Ident,
            TokenKind::LeftBrace,
            TokenKind::Ident,
            TokenKind::LeftBrace,
            TokenKind::RightBrace,
            TokenKind::RightBrace
        ]
    );

    #[derive(Clone, Debug)]
    struct PropIdent(String);

    impl Arbitrary for PropIdent {
        fn arbitrary(g: &mut quickcheck::Gen) -> Self {
            let allowed: Vec<char> = (0..0xff_u8)
                .filter(|i| (*i as char).is_alphanumeric() && (*i as char).is_ascii())
                .map(|i| i as char)
                .collect();
            let len = g.size().min(200);
            let mut s = (0..len)
                .map(|_| g.choose(&allowed).unwrap())
                .collect::<String>();
            if s.chars().next().unwrap().is_numeric() {
                s = format!("_{s}");
            }
            // println!("{s}");
            Self(s)
        }

        fn shrink(&self) -> Box<dyn Iterator<Item = Self>> {
            let len = self.0.len();
            let s = self.0.to_owned();
            Box::new((1..len).map(move |i| Self(s[..s.len() - i].to_owned())))
        }
    }

    #[quickcheck]
    fn prop_test_task_ident(ident: PropIdent) -> TestResult {
        let ident = ident.0;
        let input = format!("task {} {{}}", ident);
        let tokens = super::Lexer::new(&input);
        let tokens = tokens
            .map(|t| t.map(|t| t.kind))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        if vec![
            TokenKind::Task,
            TokenKind::Ident,
            TokenKind::LeftBrace,
            TokenKind::RightBrace,
        ] == tokens
        {
            return TestResult::passed();
        }

        TestResult::failed()
    }

    #[test]
    fn it_fails_with_an_error_when_hitting_an_invalid_char() {
        let lexer = Lexer::new("  #");
        let res: Result<Vec<Token>, Error> = lexer.collect();
        let Err(e) = res else {
            panic!("should have failed");
        };
        let e = e.downcast_ref::<SingleTokenError>().unwrap();
        assert_eq!(2, e.err_span.offset());
        assert_eq!(1, e.err_span.len());
        assert_eq!('#', e.token);
        assert_eq!(1, e.line());

        let lexer = Lexer::new("task foo {}\n$");
        let res: Result<Vec<Token>, Error> = lexer.collect();
        let Err(e) = res else {
            panic!("should have failed");
        };
        let e = e.downcast_ref::<SingleTokenError>().unwrap();
        assert_eq!(12, e.err_span.offset());
        assert_eq!(1, e.err_span.len());
        assert_eq!('$', e.token);
        assert_eq!(2, e.line());
    }
}
