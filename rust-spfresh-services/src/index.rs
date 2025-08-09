use anyhow::Result;

pub trait VecIndex: Send + Sync {
    fn append(&self, vec: &[f32]) -> Result<usize>;
    fn get(&self, id: usize) -> Result<Vec<f32>>;
    fn search(&self, q: &[f32], top_k: usize) -> Result<Vec<(usize, f32)>>;
    fn dim(&self) -> usize;
}