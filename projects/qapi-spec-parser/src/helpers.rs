use crate::QapiSchema;
use anyhow::Result;
use nom::bytes::complete::{tag, take_until};
use nom::character::complete::{line_ending, multispace0, not_line_ending};
use nom::combinator::{all_consuming, not, opt, peek, recognize};
use nom::multi::many1;
use nom::sequence::{delimited, preceded, terminated, tuple};
use nom::IResult;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

pub(crate) fn qcomment(input: &str) -> IResult<&str, &str> {
    delimited(
        tuple((tag("#"), not(peek(tag("#"))))),
        not_line_ending,
        line_ending,
    )(input)
}

pub(crate) fn clean_lines(input: &str) -> IResult<&str, &str> {
    recognize(tuple((
        multispace0,
        opt(tuple((many1(tuple((multispace0, qcomment))), multispace0))),
    )))(input)
}

pub(crate) fn qtag<'i>(t: &'static str) -> impl FnMut(&'i str) -> IResult<&'i str, &'i str> {
    preceded(clean_lines, tag(t))
}

pub(crate) fn qstring(input: &str) -> IResult<&str, &str> {
    delimited(qtag("'"), take_until("'"), tag("'"))(input)
}

pub(crate) fn dict<'i, I, O>(item_parser: I) -> impl FnMut(&'i str) -> IResult<&'i str, Vec<O>>
where
    I: FnMut(&'i str) -> IResult<&'i str, O>,
{
    delimited(
        qtag("{"),
        many1(terminated(item_parser, opt(qtag(",")))),
        qtag("}"),
    )
}

pub(crate) fn kv<'i, I1, I2, O1, O2>(
    key_parser: I1,
    value_parser: I2,
) -> impl FnMut(&'i str) -> IResult<&'i str, O2>
where
    I1: FnMut(&'i str) -> IResult<&'i str, O1>,
    I2: FnMut(&'i str) -> IResult<&'i str, O2>,
{
    delimited(
        tuple((qtag("'"), key_parser, qtag("'"), qtag(":"))),
        value_parser,
        clean_lines,
    )
}

fn read_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
}

pub fn walk_schemas(path: &Path, schemas: &mut HashMap<PathBuf, String>) -> Result<()> {
    if schemas.contains_key(path) {
        // we have already included this schema
        return Ok(());
    }

    let file_as_string = read_file(path)?;
    let pathbuf = path.to_path_buf();
    schemas.insert(pathbuf, file_as_string);

    let schema_str = schemas.get(path).unwrap();
    let (_, schema) = all_consuming(QapiSchema::parse)(schema_str)
        .expect("nom failed to parse the entire schema");

    let mut new_schema_paths = vec![];
    for schema_obj in schema {
        match schema_obj {
            QapiSchema::Include(include) => {
                let parent_path = path.parent().unwrap();
                let new_schema_path = parent_path.join(include.0);
                new_schema_paths.push(new_schema_path);
            }
            _ => {}
        }
    }
    for new_schema_path in new_schema_paths {
        walk_schemas(&new_schema_path, schemas)?;
    }
    Ok(())
}

pub fn process_schemas<'i>(
    schemas: &'i HashMap<PathBuf, String>,
) -> Result<HashMap<PathBuf, Vec<QapiSchema<'i>>>> {
    let mut processed = HashMap::new();
    for (path, schema_str) in schemas {
        let (_, schema) = all_consuming(QapiSchema::parse)(schema_str).unwrap();
        processed.insert(path.to_path_buf(), schema);
    }
    Ok(processed)
}
