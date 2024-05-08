pub use libtfslite::client::keys::{PrivateKey, PublicKey, Signature, SigningError};

use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        use wasm_bindgen::prelude::*;
        use libtfslite::client::keys::Signer;
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[derive(Debug, Clone)]
    pub type JsSigner;

    #[wasm_bindgen(structural, method, catch)]
    pub fn sign(this: &JsSigner, buf: &[u8]) -> Result<Signature, JsValue>;

    #[wasm_bindgen(structural, method, catch)]
    pub fn public_key(this: &JsSigner) -> Result<PublicKey, JsValue>;
}

#[cfg(target_arch = "wasm32")]
impl Signer for JsSigner {
    fn sign(&self, data: &[u8]) -> Result<Signature, SigningError> {
        let signature = JsSigner::sign(self, data)?;
        Ok(signature)
    }

    fn public_key(&self) -> Result<PublicKey, SigningError> {
        let public_key = JsSigner::public_key(self)?;
        Ok(public_key)
    }

    fn clone_box(&self) -> Box<dyn Signer> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use crate::tests::test_signing_common;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_signing() {
        test_signing_common()
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_signing() {
        test_signing_common()
    }
}
