use crate::error::FitError;
use std::io::Write;

pub const DECODER_LIBRARY: &str = "fitparser";
pub const DECODER_LIBRARY_VERSION: &str = "0.11.0";
pub const DECODER_VERSION: &str = "0.11.0+runs.2";
pub const PROFILE_VERSION: &str = fitparser::profile::VERSION;
pub const ARCHIVE_SCHEMA_VERSION: &str = "2.0.0";
pub const NORMALIZED_SCHEMA_VERSION: &str = "2.0.0";
pub const NORMALIZER_VERSION: &str = "native-runs-stream.1";
pub const DECODE_OPTIONS: &[&str] = &[
    "KeepCompositeFields",
    "PreserveInvalidValues",
    "PreserveUnknownDeveloperFields",
];

// SHA-256 of the checked-in implementation. Update when these sources change.
// Vendor digest: sorted src/**/*.rs relative path, NUL, bytes, NUL per file.
pub const VENDOR_SOURCE_SHA256: &str = "5cba14de1b4f88f3c3a55d755d27ceb8e7fb782027e3786b347b2b1e0a597bf1";
pub const PROFILE_SOURCE_SHA256: &str = "fe9695a0ee955c4cdfad275bf11bc83706bdd87868a497894f8327e527145f9e";
pub const NORMALIZER_SOURCE_SHA256: &str = "37b2d91074e867440b7e942205729c47570ee4b91cb9fd67e81e3775631367db";
pub const ARCHIVE_SOURCE_SHA256: &str = "73a6cb5e19a7e1610e185faad8f74b448327ba2ae59df80f284275535a751dd4";
pub const RAW_PROJECTION_SOURCE_SHA256: &str = "3b983feaabf607b735170d6c58e442c39562c95de44da5f15df64c633583dff5";
pub const JSON_FLOAT_ROUNDTRIP: bool = true;

/// Actual implementation identifiers shared by decode and stored revisions.
pub fn decoder_metadata() -> serde_json::Value {
    serde_json::json!({
        "library": DECODER_LIBRARY,
        "libraryVersion": DECODER_LIBRARY_VERSION,
        "version": DECODER_VERSION,
        "profileVersion": PROFILE_VERSION,
        "options": DECODE_OPTIONS,
        "hrMerge": false,
        "archiveSchemaVersion": ARCHIVE_SCHEMA_VERSION,
        "normalizedSchemaVersion": NORMALIZED_SCHEMA_VERSION,
        "normalizerVersion": NORMALIZER_VERSION,
        "vendorSourceSha256": VENDOR_SOURCE_SHA256,
        "profileSourceSha256": PROFILE_SOURCE_SHA256,
        "normalizerSourceSha256": NORMALIZER_SOURCE_SHA256,
        "archiveSourceSha256": ARCHIVE_SOURCE_SHA256,
        "rawProjectionSourceSha256": RAW_PROJECTION_SOURCE_SHA256,
        "jsonFloatRoundtrip": JSON_FLOAT_ROUNDTRIP,
    })
}

/// Write the full decoded archive, normalized run, and preserved v1 projection.
/// The caller owns the private sink and must discard it on failure.
pub fn decode_run_to_writer(bytes: &[u8], output: &mut dyn Write) -> Result<(), FitError> {
    super::stream::decode_run_to_writer(bytes, output)
}
