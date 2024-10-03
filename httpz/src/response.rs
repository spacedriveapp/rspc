use http::Response;

use crate::Error;

/// TODO
pub trait HttpResponse {
    /// TODO
    fn into_response(self) -> Result<Response<Vec<u8>>, Error>;
}

impl HttpResponse for Response<Vec<u8>> {
    fn into_response(self) -> Result<Response<Vec<u8>>, Error> {
        Ok(self)
    }
}

impl<TResp> HttpResponse for Result<TResp, Error>
where
    TResp: HttpResponse,
{
    fn into_response(self) -> Result<Response<Vec<u8>>, Error> {
        self?.into_response()
    }
}
