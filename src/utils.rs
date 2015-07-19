use url;
use hyper;
use hyper::Client;
use hyper::client::response;
use init::SensuError;

pub fn api_url(path: &str) -> Result<url::Url, SensuError> {
    let base = url::Url::parse("http://localhost:4567").unwrap();
    match url::UrlParser::new().base_url(&base).parse(path) {
        Ok(v) => Ok(v),
        Err(e) => Err(SensuError::ParseError(format!("bad {}", e)))
    }
}

pub fn api_get(path: &str) -> response::Response {//SensuResult<Result<A,B>> {
    //let path = api_url(path);
    let c = Client::new();
    let result = c.get(path).send().unwrap();
    // let req = match path {
    //     Ok(path) => Client::new().get(path).send().unwrap(),
    //     Err(ref e) => return Err(SensuError::ParseError(
    //         format!("{:?} is not a valid url part {:?}", path, e)))
    // };
    //let result = try!(try!(req).start()).send();
    result
}

pub fn stash_exists(stash: &str) -> bool {
    let path = "/stash/".to_string() + stash;
    match api_get(path.as_ref()).status {
            hyper::status::StatusCode::Ok => true,
            _ => false
    }
    // match api_get(path.as_slice()) {
    //     Ok(result) => match result.status {
    //         hyper::status::StatusCode::Ok => true,
    //         _ => false
    //     },
    //     Err(_) => false
    // }
}
