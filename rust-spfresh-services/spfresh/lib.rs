use std::error::Error;

pub struct Index;
pub struct OpenOptions { pub create: bool, pub append: bool }
impl OpenOptions {
    pub fn new() -> Self { Self { create: false, append: false } }
    pub fn create(mut self, b: bool) -> Self { self.create = b; self }
    pub fn append(mut self, b: bool) -> Self { self.append = b; self }
}
#[derive(Default)]
pub struct SearchParams { pub top_k: usize }

impl Index {
    pub fn open(_path: &str, _dim: usize, _opts: &OpenOptions)
        -> Result<Self, Box<dyn Error>> { Ok(Self) }
    pub fn append(&mut self, _vec: &[f32])
        -> Result<usize, Box<dyn Error>> { Ok(0) }
    pub fn get(&self, _id: usize)
        -> Result<Vec<f32>, Box<dyn Error>> { Ok(vec![]) }
    pub fn search(&self, _q: &[f32], _p: &SearchParams)
        -> Result<Vec<(usize,f32)>, Box<dyn Error>> { Ok(vec![]) }
}
