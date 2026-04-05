use super::qapi_variant::QapiVariant;
use nom::IResult;

pub type QapiAlternative<'i> = QapiVariant<'i>;

pub fn take_alternative(input: &str) -> IResult<&str, QapiAlternative<'_>> {
    QapiVariant::parse(input)
}
