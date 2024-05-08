use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use cylinder;
use cylinder::Context;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

#[cfg_attr(feature = "wasm", wasm_bindgen)]
#[derive(Debug)]
pub struct Signature(cylinder::Signature);

impl From<cylinder::Signature> for Signature {
    fn from(value: cylinder::Signature) -> Self {
        Signature(value)
    }
}

impl TryFrom<&str> for Signature {
    type Error = SignatureParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match cylinder::Signature::from_hex(value) {
            Ok(sig) => Ok(sig.into()),
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
impl Signature {
    #[cfg(feature = "wasm")]
    #[wasm_bindgen(constructor)]
    pub fn new(hex: String) -> Result<Signature, SignatureParseError> {
        hex.as_str().try_into()
    }

    pub fn as_hex(&self) -> String {
        self.0.as_hex()
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
#[derive(Debug)]
pub struct SigningError(cylinder::SigningError);

impl From<cylinder::SigningError> for SigningError {
    fn from(value: cylinder::SigningError) -> Self {
        SigningError(value)
    }
}

#[cfg(feature = "wasm")]
impl From<JsValue> for SigningError {
    fn from(value: JsValue) -> Self {
        cylinder::SigningError::Internal(
            value
                .as_string()
                .unwrap_or("Unknown Signing Error"
                    .to_string()))
            .into()
    }
}

impl Display for SigningError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug)]
pub struct KeyLoadError(cylinder::KeyLoadError);

impl From<cylinder::KeyLoadError> for KeyLoadError {
    fn from(value: cylinder::KeyLoadError) -> Self {
        KeyLoadError(value)
    }
}

impl Display for KeyLoadError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
#[derive(Debug)]
pub struct KeyParseError(cylinder::KeyParseError);

impl From<cylinder::KeyParseError> for KeyParseError {
    fn from(value: cylinder::KeyParseError) -> Self {
        KeyParseError(value)
    }
}

impl Display for KeyParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
#[derive(Debug)]
pub struct SignatureParseError(cylinder::SignatureParseError);

impl From<cylinder::SignatureParseError> for SignatureParseError {
    fn from(value: cylinder::SignatureParseError) -> Self {
        SignatureParseError(value)
    }
}

impl Display for SignatureParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
#[derive(Debug)]
pub struct VerificationError(cylinder::VerificationError);

impl From<cylinder::VerificationError> for VerificationError {
    fn from(value: cylinder::VerificationError) -> Self {
        VerificationError(value)
    }
}

impl Display for VerificationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub trait Signer {
    fn sign(&self, data: &[u8]) -> Result<Signature, SigningError>;
    fn public_key(&self) -> Result<PublicKey, SigningError>;
    fn clone_box(&self) -> Box<dyn Signer>;
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
#[derive(Clone)]
pub struct PrivateKey {
    private_key: cylinder::PrivateKey,
    signer: Box<dyn cylinder::Signer>,
}

impl From<cylinder::PrivateKey> for PrivateKey {
    fn from(value: cylinder::PrivateKey) -> Self {
        PrivateKey::from_cylinder_private_key(value)
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
impl PrivateKey {
    fn from_cylinder_private_key(private_key: cylinder::PrivateKey) -> Self {
        let context = cylinder::secp256k1::Secp256k1Context::new();
        let signer = context.new_signer(private_key.clone());

        PrivateKey{
            private_key,
            signer
        }
    }

    pub fn load_from_bytes(key_bytes: &[u8]) -> Self {
        let private_key = cylinder::PrivateKey::new(key_bytes.to_vec());
        Self::from_cylinder_private_key(private_key)
    }

    pub fn load_from_hex(key_hex: &str) -> Result<PrivateKey, KeyParseError> {
        let private_key = cylinder::PrivateKey::new_from_hex(key_hex)?;
        Ok(Self::from_cylinder_private_key(private_key))
    }

    pub fn generate_random_key() -> Self {
        let context = cylinder::secp256k1::Secp256k1Context::new();
        let private_key = context.new_random_private_key();
        Self::from_cylinder_private_key(private_key)
    }

    pub fn as_hex(&self) -> String {
        self.private_key.as_hex()
    }

    #[cfg(feature = "wasm")]
    #[wasm_bindgen(constructor)]
    pub fn new(hex: String) -> Result<PrivateKey, KeyParseError> {
        PrivateKey::load_from_hex(hex.as_str())
    }

    #[cfg(feature = "wasm")]
    pub fn sign(&self, data: &[u8]) -> Result<Signature, JsValue> {
        Ok(Signer::sign(self, data)?)
    }

    #[cfg(feature = "wasm")]
    pub fn public_key(&self) -> Result<PublicKey, JsValue> {
        Ok(Signer::public_key(self)?)
    }
}

impl PrivateKey {
    pub fn load_from_file(key_file: PathBuf) -> Result<Self, KeyLoadError> {
        let private_key = cylinder::load_key_from_path(key_file.as_path())?;
        Ok(Self::from_cylinder_private_key(private_key))
    }

    pub fn as_slice(&self) -> &[u8] {
        self.private_key.as_slice()
    }
}

impl Signer for PrivateKey {
    fn sign(&self, data: &[u8]) -> Result<Signature, SigningError> {
        let result = self.signer.sign(data)?;
        Ok(result.into())
    }

    fn public_key(&self) -> Result<PublicKey, SigningError> {
        let public_key = self.signer.public_key()?;
        Ok(public_key.into())
    }

    fn clone_box(&self) -> Box<dyn Signer> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn Signer> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

pub trait Verifier {
    fn verify(&self, data: &[u8], signature: &Signature) -> Result<bool, VerificationError>;
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub struct PublicKey {
    verifier: Box<dyn cylinder::Verifier>,
    public_key: cylinder::PublicKey,
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
impl PublicKey {
    fn from_cylinder_public_key(public_key: cylinder::PublicKey) -> Self {
        let context = cylinder::secp256k1::Secp256k1Context::new();
        let verifier = context.new_verifier();
        PublicKey {
            verifier,
            public_key,
        }
    }

    pub fn load_from_bytes(key_bytes: &[u8]) -> Self {
        let public_key = cylinder::PublicKey::new(key_bytes.to_vec());
        Self::from_cylinder_public_key(public_key)
    }

    pub fn load_from_hex(key_hex: &str) -> Result<PublicKey, KeyParseError> {
        let public_key = cylinder::PublicKey::new_from_hex(key_hex)?;
        Ok(Self::from_cylinder_public_key(public_key))
    }

    pub fn as_hex(&self) -> String {
        self.public_key.as_hex()
    }

    #[cfg(feature = "wasm")]
    #[wasm_bindgen(constructor)]
    pub fn new(hex: String) -> Result<PublicKey, KeyParseError> {
        PublicKey::load_from_hex(hex.as_str())
    }
}

impl Verifier for PublicKey {
    fn verify(&self, data: &[u8], signature: &Signature) -> Result<bool, VerificationError> {
        let signature_bytes = signature.0.as_slice().to_vec();
        let signature = cylinder::Signature::new(signature_bytes);
        let result = self.verifier.verify(data, &signature, &self.public_key)?;
        Ok(result)
    }
}

impl From<cylinder::PublicKey> for PublicKey {
    fn from(value: cylinder::PublicKey) -> Self {
        PublicKey::from_cylinder_public_key(value)
    }
}

impl PublicKey {
    pub fn as_slice(&self) -> &[u8] {
        self.public_key.as_slice()
    }
}
