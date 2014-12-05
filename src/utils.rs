use url;
use hyper::client::{request,response};
use init::{SensuResult, SensuError};

pub fn api_url(path: &str) -> Result<url::Url, SensuError> {
    let base = url::Url::parse("http://localhost:4567").unwrap();
    match url::UrlParser::new().base_url(&base).parse(path) {
        Ok(v) => Ok(v),
        Err(e) => Err(SensuError::ParseError(format!("bad {}", e)))
    }
}

pub fn api_get(path: &str) -> SensuResult<response::Response> {
    let path = api_url(path);
    let req = match path {
        Ok(path) => request::Request::get(path),
        Err(ref e) => return Err(SensuError::ParseError(
            format!("{} is not a valid url part {}", path, e)))
    };
    let result = try!(try!(req).start()).send();
    result.or_else(|e| Err(SensuError::HttpError(e)))
}
