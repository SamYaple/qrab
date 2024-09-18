use crate::helpers::qtag;
use nom::branch::alt;
use nom::combinator::map;
use nom::IResult;

#[derive(Debug, Clone)]
pub struct QapiBool(pub bool);
impl QapiBool {
    pub fn parse(input: &str) -> IResult<&str, Self> {
        map(alt((qtag("true"), qtag("false"))), |b| {
            QapiBool(b.parse().unwrap())
        })(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_INPUTS: [&str; 3] = [
        "true",
        "false",
        r#" # some comment
        true"#,
    ];

    #[test]
    fn test_valid() {
        for input in VALID_INPUTS {
            let result = QapiBool::parse(input);
            match result {
                Ok((remaining, _)) => {
                    assert_eq!(remaining, "");
                }
                _ => panic!("Failed to parse: ```\n{input}\n```"),
            }
        }
    }
}
