use libra_types::proptest_types::arb_json_value;
use proptest::prelude::*;
use serde_json::Value;
use ureq::Response;

// This generates an arbitrary generic response returned by vault for various API calls.
prop_compose! {
    pub fn arb_generic_response(
    )(
        status in any::<u16>(),
        status_text in any::<String>(),
        value in arb_json_value(),
    ) -> Response {
        let value =
            serde_json::to_string::<Value>(&value).unwrap();
        Response::new(status, &status_text, &value)
    }
}

// Note: these tests ensure that the various fuzzers are maintained (i.e., not broken
// at some time in the future and only discovered when a fuzz test fails).
#[cfg(test)]
mod tests {
    use crate::{fuzzing::arb_generic_response, process_generic_response};
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(10))]

        #[test]
        fn process_generic_response_proptest(input in arb_generic_response()) {
            let _ = process_generic_response(input);
        }
    }
}
