use reqwest::header::HeaderMap;
use crate::routes::{FetchError, FetchResult, PackedFetch};


pub fn only_200<T>(result: FetchResult<PackedFetch<T>>) -> FetchResult<(HeaderMap, T)> {
    let (status, headers, body) = result?;
    if status.is_success() {
        Ok((headers, body))
    } else {
        Err(FetchError::StatusError(status))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::StatusCode;

    #[test]
    fn passes_a_success_status_through_with_its_body() {
        let result = Ok((StatusCode::OK, HeaderMap::new(), 42));
        let (_, body) = only_200(result).unwrap();
        assert_eq!(body, 42);
    }

    #[test]
    fn turns_a_non_success_status_into_a_status_error() {
        let result = Ok((StatusCode::IM_A_TEAPOT, HeaderMap::new(), 0));
        assert!(matches!(
            only_200(result),
            Err(FetchError::StatusError(StatusCode::IM_A_TEAPOT)),
        ));
    }

    #[test]
    fn propagates_an_upstream_error_untouched() {
        let result: FetchResult<PackedFetch<i32>> = Err(FetchError::Logout);
        assert!(matches!(only_200(result), Err(FetchError::Logout)));
    }
}