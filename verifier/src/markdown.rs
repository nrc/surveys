use std::{error::Error, vec};

pub fn parse<'a>(markdown: &'a str) -> Result<Vec<Question<'a>>, Box<dyn Error>> {
    let mut questions = Vec::new();
    let mut state = ParserState::None;
    macro_rules! bail {
        ($($args : tt) *) => {
            return Err(format!("markdown error: {}", format_args!($($args)*)).into())
        };
    }
    for line in markdown
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .filter(|l| !l.starts_with(">"))
    {
        if line.starts_with("###") {
            let old_state = std::mem::replace(&mut state, ParserState::Text(&line[3..].trim()));
            match old_state {
                ParserState::Question(q) if !q.is_empty() => questions.push(q),
                ParserState::None => {}
                ParserState::Text(q) => bail!("question without answers '{}'", q),
                ParserState::Question(q) => bail!("question without answers '{}'", q.text),
                ParserState::HalfMatrixText(q) => {
                    bail!("question without answers '{}'", q)
                }
                ParserState::HalfMatrix {
                    answers, text: q, ..
                } if answers.is_empty() => {
                    bail!("question without answers '{}'", q)
                }
                ParserState::HalfMatrix { text: q, .. } => {
                    bail!("matrix question without second half of answers '{}'", q)
                }
            }
        } else if line.starts_with("Type: ") {
            if let ParserState::Text(text) = state {
                let typ = &line[6..].trim();
                state = if typ.starts_with("select one") {
                    ParserState::Question(Question {
                        text,
                        answers: Answers::SelectOne(vec![]),
                    })
                } else if typ.starts_with("free form") {
                    ParserState::Question(Question {
                        text,
                        answers: Answers::FreeForm,
                    })
                } else if typ.starts_with("select all that apply") {
                    ParserState::Question(Question {
                        text,
                        answers: Answers::SelectMany(vec![]),
                    })
                } else if typ.starts_with("matrix") {
                    ParserState::HalfMatrixText(text)
                } else {
                    bail!("illegal question type: type='{}' question='{}'", typ, text);
                }
            } else {
                bail!(
                    "illegal parser state: found type when state is '{:?}'",
                    state
                );
            }
        } else if line.starts_with("-") {
            match &mut state {
                ParserState::Question(Question {
                    answers: Answers::SelectOne(ref mut a),
                    ..
                }) => a.push(line[1..].trim()),
                ParserState::Question(Question {
                    answers: Answers::SelectMany(ref mut a),
                    ..
                }) => a.push(line[1..].trim()),
                ParserState::Question(Question {
                    answers:
                        Answers::Matrix {
                            ref mut answers2, ..
                        },
                    ..
                }) => {
                    answers2.push(line[1..].trim());
                }
                ParserState::HalfMatrix { answers, .. } => {
                    answers.push(line[1..].trim());
                }
                _ => {
                    //     bail!("illegal state. found answer when state is {:?}", state)
                }
            }
        } else if line.starts_with("REPEAT") {
            let previous = questions.last().ok_or_else(|| {
                match state.question_text() {
                    Some(t) => format!("question repeats previous answer but there is no previous question '{}'", t),
                    None => format!("question repeats previous answer but there is no previous question or text for the current question"),
                }
            })?;
            state = match (state, &previous.answers) {
                (
                    ParserState::HalfMatrixText(q),
                    Answers::Matrix {
                        label1, answers1, ..
                    },
                ) => ParserState::HalfMatrix {
                    text: q,
                    label: label1,
                    answers: answers1.clone(),
                },
                (
                    ParserState::HalfMatrix {
                        text,
                        label,
                        answers,
                    },
                    Answers::Matrix {
                        label1, answers1, ..
                    },
                ) if answers.is_empty() && *label1 == label => ParserState::HalfMatrix {
                    text,
                    label,
                    answers: answers1.clone(),
                },
                (
                    ParserState::Question(Question {
                        text,
                        answers: Answers::SelectMany(a1),
                    }),
                    new,
                ) if a1.is_empty() => ParserState::Question(Question {
                    text,
                    answers: new.clone(),
                }),
                (state, _) => bail!(
                    "unexpected placement of the REPEAT keyword. State={:?}",
                    state
                ),
            };
        } else if line.ends_with(":") {
            state = match state {
                ParserState::HalfMatrixText(q) => ParserState::HalfMatrix {
                    text: q,
                    label: line,
                    answers: vec![],
                },
                ParserState::HalfMatrix {
                    text,
                    label,
                    answers,
                } if !answers.is_empty() => ParserState::Question(Question {
                    text,
                    answers: Answers::Matrix {
                        label1: label,
                        label2: line,
                        answers1: answers,
                        answers2: vec![],
                    },
                }),
                ParserState::HalfMatrix { text, .. } => {
                    bail!("matrix question has no answers in first section '{}'", text)
                }
                _ => state,
            };
        } else {
            log::warn!("Unhandled line: {}", line);
        }
    }
    Ok(questions)
}

#[derive(Debug)]
pub struct Question<'a> {
    pub text: &'a str,
    pub answers: Answers<'a>,
}

impl<'a> Question<'a> {
    fn is_empty(&self) -> bool {
        self.answers.is_empty()
    }
}

#[derive(Debug, Clone)]
pub enum Answers<'a> {
    FreeForm,
    SelectOne(Vec<&'a str>),
    SelectMany(Vec<&'a str>),
    Matrix {
        label1: &'a str,
        answers1: Vec<&'a str>,
        label2: &'a str,
        answers2: Vec<&'a str>,
    },
}

impl Answers<'_> {
    fn is_empty(&self) -> bool {
        match self {
            Self::SelectOne(a) => a.is_empty(),
            Self::SelectMany(a) => a.is_empty(),
            Self::Matrix {
                answers1, answers2, ..
            } => answers1.is_empty() || answers2.is_empty(),
            Self::FreeForm => false,
        }
    }
}

#[derive(Debug)]
enum ParserState<'a> {
    // We are starting from nothing
    None,
    // We have the question text and now need answers
    Text(&'a str),
    // building up the question with answers
    Question(Question<'a>),
    // Got the text for a matrix question
    HalfMatrixText(&'a str),
    // On the first half of a matrix question
    HalfMatrix {
        text: &'a str,
        label: &'a str,
        answers: Vec<&'a str>,
    },
}

impl<'a> ParserState<'a> {
    fn question_text(&self) -> Option<&'a str> {
        match self {
            Self::None => None,
            Self::Text(t) => Some(t),
            Self::Question(q) => Some(q.text),
            Self::HalfMatrixText(q) => Some(q),
            Self::HalfMatrix { text, .. } => Some(text),
        }
    }
}
