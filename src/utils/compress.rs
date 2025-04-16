use std::borrow::Borrow;
use zstd::{encode_all, decode_all};
use crate::Result;

pub struct Zstd;
impl Zstd {
    pub fn compress(input: impl AsRef<str>) -> Result<Vec<u8>>{
        let compressed = encode_all(input.as_ref().as_bytes(), 3)?;
        Ok(compressed)
    }

    pub fn decompress(input: impl Borrow<Vec<u8>>) -> Result<String>{
        let vec: &Vec<u8> = input.borrow();
        let decompressed = decode_all(vec.as_slice())?;
        let string = String::from_utf8_lossy(decompressed.as_slice()).to_string();
        Ok(string)
    }
}

pub trait Compress {
    fn compress(&self) -> Result<Vec<u8>>;
}

impl<T> Compress for T 
where
    T: AsRef<str>,
{
    fn compress(&self) -> Result<Vec<u8>> {
        Zstd::compress(self)
    }
}

pub trait Decompress<T> {
    fn decompress(&self) -> Result<T>;
}

impl<T> Decompress<String> for T where T: AsRef<[u8]> {
    fn decompress(&self) -> Result<String> {
        Zstd::decompress(self.as_ref().to_vec())
    }
}