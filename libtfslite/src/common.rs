use sha2::{Digest, Sha512};

pub const FAMILY_NAME: &str = "tfslite";
pub const FAMILY_VERSION: &str = "0.1";
pub const FILE_CREATE_COST: u64 = 100000000;

pub fn get_tfslite_prefix() -> String {
    hex::encode(Sha512::digest(b"tfslite"))[..6].to_string()
}
