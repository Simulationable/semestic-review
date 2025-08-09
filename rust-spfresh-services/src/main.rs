use axum::{extract::State, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use std::{
    collections::{hash_map::DefaultHasher, HashSet},
    fs::{File, OpenOptions},
    hash::{Hash, Hasher},
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    sync::Arc,
};
use http::{header, Method};
use parking_lot::Mutex;
use anyhow::Result;
use tracing::info;
use tracing_subscriber::EnvFilter;
use tower_http::cors::{Any, CorsLayer};
use std::io::Read;

// =========== Embedding (TF-IDF hashing) ===========
trait Embedder: Send + Sync {
    fn embed_index(&self, text: &str) -> Result<Vec<f32>>;
    fn embed_query(&self, text: &str) -> Result<Vec<f32>>;
}

struct TfIdfEmbedder {
    dim: usize,
    df: Mutex<Vec<u32>>,
    docs: Mutex<u32>,
}
impl TfIdfEmbedder {
    fn new(dim: usize) -> Self {
        Self { dim, df: Mutex::new(vec![0; dim]), docs: Mutex::new(0) }
    }
    #[inline]
    fn bucket(&self, token: &str) -> usize {
        let mut h = DefaultHasher::new();
        token.to_lowercase().hash(&mut h);
        (h.finish() as usize) % self.dim
    }
    fn idf(&self, df_i: u32, docs_now: f32) -> f32 {
        ((docs_now + 1.0) / (df_i as f32 + 1.0)).ln() + 1.0
    }
    fn l2_normalize(vec: &mut [f32]) {
        let norm = (vec.iter().map(|x| x * x).sum::<f32>()).sqrt().max(1e-6);
        for x in vec.iter_mut() { *x /= norm; }
    }
    fn featurize_index(&self, text: &str) -> Vec<f32> {
        let mut v = vec![0f32; self.dim];
        let mut seen = HashSet::new();
        for tok in text.split(|c: char| !c.is_alphanumeric()).filter(|t| !t.is_empty()) {
            let i = self.bucket(tok);
            v[i] += 1.0;
            seen.insert(i);
        }
        { let mut df = self.df.lock(); for &i in &seen { df[i] = df[i].saturating_add(1); } }
        let docs_now = { let mut d = self.docs.lock(); *d = d.saturating_add(1); *d as f32 };
        let df = self.df.lock();
        for i in 0..self.dim { if v[i] > 0.0 { v[i] *= self.idf(df[i], docs_now); } }
        Self::l2_normalize(&mut v); v
    }
    fn featurize_query(&self, text: &str) -> Vec<f32> {
        let mut v = vec![0f32; self.dim];
        for tok in text.split(|c: char| !c.is_alphanumeric()).filter(|t| !t.is_empty()) {
            let i = self.bucket(tok); v[i] += 1.0;
        }
        let docs_now = (*self.docs.lock()).max(1) as f32;
        let df = self.df.lock();
        for i in 0..self.dim { if v[i] > 0.0 { v[i] *= self.idf(df[i], docs_now); } }
        Self::l2_normalize(&mut v); v
    }
}
impl Embedder for TfIdfEmbedder {
    fn embed_index(&self, text: &str) -> Result<Vec<f32>> { Ok(self.featurize_index(text)) }
    fn embed_query(&self, text: &str) -> Result<Vec<f32>> { Ok(self.featurize_query(text)) }
}

trait VecIndex: Send + Sync {
    fn dim(&self) -> usize;
    fn append(&self, vec: &[f32]) -> Result<usize>;
    fn get(&self, id: usize) -> Result<Vec<f32>>;
}

mod spfresh_index {
    use super::*;
    use anyhow::{anyhow, Result};
    use spfresh::{Index as SIndex, OpenOptions as SOpen};
    use std::io::{Seek, SeekFrom, Write};

    pub struct SpfreshIndex {
        dim: usize,
        inner: Mutex<SIndex>,
        spf_path: PathBuf,
        mirror_path: PathBuf,
        mirror_file: Mutex<std::fs::File>,
        bytes_per_vec: u64,
    }

    impl SpfreshIndex {
        pub fn open(dir: impl Into<PathBuf>, dim: usize) -> Result<Self> {
            let dir = dir.into();
            std::fs::create_dir_all(&dir)?;
            let spf_path = dir.join("reviews.spfresh");
            let mirror_path = dir.join("reviews.index");
            let _ = std::fs::OpenOptions::new().create(true).write(true).open(&spf_path)?;
            let _ = std::fs::OpenOptions::new().create(true).write(true).open(&mirror_path)?;
            let spf_abs = std::fs::canonicalize(&spf_path).unwrap_or(spf_path.clone());
            let mir_abs = std::fs::canonicalize(&mirror_path).unwrap_or(mirror_path.clone());
            tracing::info!("spfresh data path = {}", spf_abs.display());
            tracing::info!("mirror  raw path  = {}", mir_abs.display());
            let opts = SOpen::new().create(true).append(true);
            let idx = SIndex::open(spf_abs.to_string_lossy().as_ref(), dim, &opts)
                .map_err(|e| anyhow!("{}", e))?;
            let mut mf = std::fs::OpenOptions::new().create(true).read(true).write(true).open(&mir_abs)?;
            mf.seek(SeekFrom::End(0))?;
            Ok(Self {
                dim,
                inner: Mutex::new(idx),
                spf_path: spf_abs,
                mirror_path: mir_abs,
                mirror_file: Mutex::new(mf),
                bytes_per_vec: (dim * 4) as u64,
            })
        }

        #[inline]
        fn mirror_append_checked(&self, vec: &[f32]) -> Result<()> {
            let mut f = self.mirror_file.lock();
            let before = std::fs::metadata(&self.mirror_path)?.len();
            f.seek(SeekFrom::End(0))?;
            let bytes = unsafe {
                std::slice::from_raw_parts(vec.as_ptr() as *const u8, vec.len() * 4)
            };
            f.write_all(bytes)?;
            f.flush()?;
            let _ = f.sync_all();
            let after = std::fs::metadata(&self.mirror_path)?.len();
            anyhow::ensure!(
                after == before + self.bytes_per_vec,
                "mirror write failed: {} -> {} (expect +{}) @ {}",
                before, after, self.bytes_per_vec, self.mirror_path.display()
            );
            tracing::info!(
                "mirror OK: +{} bytes -> {} @ {}",
                self.bytes_per_vec, after, self.mirror_path.display()
            );
            Ok(())
        }
    }

    impl super::VecIndex for SpfreshIndex {
        fn dim(&self) -> usize { self.dim }
        fn append(&self, vec: &[f32]) -> Result<usize> {
            anyhow::ensure!(vec.len() == self.dim, "dim mismatch: {} != {}", vec.len(), self.dim);
            let mut idx = self.inner.lock();
            let id = idx.append(vec).map_err(|e| anyhow!("{}", e))?;
            self.mirror_append_checked(vec)?; // เขียน reviews.index ทุกครั้ง
            tracing::info!(
                "append OK: id={}, spf={}, mirror={}",
                id, self.spf_path.display(), self.mirror_path.display()
            );
            Ok(id)
        }
        fn get(&self, id: usize) -> Result<Vec<f32>> {
            let idx = self.inner.lock();
            Ok(idx.get(id).map_err(|e| anyhow!("{}", e))?)
        }
    }

    pub use SpfreshIndex as DefaultIndex;
}

struct MetaStore {
    meta_path: PathBuf,
}
impl MetaStore {
    fn open(dir: impl Into<PathBuf>) -> Result<Self> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)?;
        let meta_path = dir.join("reviews.jsonl");
        if !meta_path.exists() { File::create(&meta_path)?; }
        Ok(Self { meta_path })
    }
    fn append(&self, review: &Review) -> Result<()> {
        let mut meta = OpenOptions::new().append(true).open(&self.meta_path)?;
        let line = serde_json::to_string(review)?;
        meta.write_all(line.as_bytes())?;
        meta.write_all(b"\n")?;
        Ok(())
    }
    fn read_review_by_line(&self, id: usize) -> Result<Review> {
        let file = File::open(&self.meta_path)?;
        let reader = BufReader::new(file);
        let line = reader
            .lines()
            .nth(id)
            .ok_or_else(|| anyhow::anyhow!("metadata line not found"))??;
        let r: Review = serde_json::from_str(&line)?;
        Ok(r)
    }
    fn count(&self) -> anyhow::Result<usize> {
        let f = File::open(&self.meta_path)?;
        let rdr = BufReader::new(f);
        Ok(rdr.lines().count())
    }
}

#[derive(Clone)]
struct AppState {
    meta: Arc<MetaStore>,
    vindex: Arc<dyn VecIndex>,
    embedder: Arc<dyn Embedder>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Review {
    review_title: String,
    review_body: String,
    product_id: String,
    review_rating: i32,
}
#[derive(Serialize, Deserialize)]
struct ReviewResp { id: usize }
#[derive(Serialize, Deserialize)]
struct BulkResp { inserted: usize }
#[derive(Serialize, Deserialize)]
struct SearchReq { query: String, top_k: Option<usize> }
#[derive(Serialize, Deserialize)]
struct SearchHit { id: usize, score: f32, review: Review }
#[derive(Serialize, Deserialize)]
struct SearchResp { hits: Vec<SearchHit> }

#[derive(Deserialize)]
struct InsertReq { review: Review }

async fn insert_one(State(st): State<AppState>, Json(req): Json<InsertReq>) -> Json<ReviewResp> {
    tracing::info!("insert_one: {}", req.review.review_title);
    let txt = format!("{} {}", req.review.review_title, req.review.review_body);
    let vec = st.embedder.embed_index(&txt).expect("embed fail");
    let id = st.vindex.append(&vec).expect("append vec fail");
    st.meta.append(&req.review).expect("append meta fail");
    Json(ReviewResp { id })
}

#[derive(Deserialize)]
struct BulkInsertReq { reviews: Vec<Review> }

async fn insert_bulk(State(st): State<AppState>, Json(req): Json<BulkInsertReq>) -> Json<BulkResp> {
    let mut ok = 0usize;
    for r in req.reviews {
        let txt = format!("{} {}", r.review_title, r.review_body);
        let vec = st.embedder.embed_index(&txt).expect("embed fail");
        let _ = st.vindex.append(&vec).expect("append vec fail");
        st.meta.append(&r).expect("append meta fail");
        ok += 1;
    }
    Json(BulkResp { inserted: ok })
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let len = a.len().min(b.len());
    if len == 0 { return 0.0; }
    let mut s = 0f32;
    for i in 0..len { s += a[i] * b[i]; }
    s
}

async fn search(State(st): State<AppState>, Json(req): Json<SearchReq>) -> Json<SearchResp> {
    let k = req.top_k.unwrap_or(5).min(100);
    let qv = match st.embedder.embed_query(&req.query) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("embed_query fail: {e}");
            return Json(SearchResp { hits: vec![] });
        }
    };
    let dim = qv.len();
    let meta_count = match st.meta.count() {
        Ok(n) => n,
        Err(e) => { tracing::error!("meta count fail: {e}"); return Json(SearchResp { hits: vec![] }); }
    };

    // อ่านเวกเตอร์จากไฟล์ mirror ที่เราเขียนไว้ทุกครั้ง: data/reviews.index
    let data_path = std::env::current_dir().unwrap_or_else(|_| ".".into())
        .join("data").join("reviews.index");
    let mut buf = Vec::new();
    match std::fs::File::open(&data_path).and_then(|mut f| f.read_to_end(&mut buf)) {
        Ok(_) => {},
        Err(e) => {
            tracing::error!("open/read {} fail: {}", data_path.display(), e);
            return Json(SearchResp { hits: vec![] });
        }
    }

    let bytes_per_vec = (dim * 4) as usize;
    if buf.len() < bytes_per_vec {
        tracing::warn!("mirror empty or dim mismatch: {} bytes, need {}", buf.len(), bytes_per_vec);
        return Json(SearchResp { hits: vec![] });
    }
    let total_vecs = buf.len() / bytes_per_vec;
    // ป้องกัน meta กับ mirror ไม่เท่ากัน: ใช้อันที่น้อยกว่า
    let n = std::cmp::min(meta_count, total_vecs);

    let mut scored: Vec<(usize, f32)> = Vec::with_capacity(n);
    for id in 0..n {
        let off = id * bytes_per_vec;
        let chunk = &buf[off..off + bytes_per_vec];
        let mut v = vec![0f32; dim];
        // SAFETY: chunk size = dim * 4 bytes (LE)
        let src = unsafe {
            std::slice::from_raw_parts(chunk.as_ptr() as *const f32, dim)
        };
        v.copy_from_slice(src);

        let s = cosine(&qv, &v);
        scored.push((id, s));
    }

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(k);

    let mut out = Vec::with_capacity(scored.len());
    for (id, score) in scored {
        if let Ok(rev) = st.meta.read_review_by_line(id) {
            out.push(SearchHit { id, score, review: rev });
        } else {
            tracing::warn!("meta read id={} failed", id);
        }
    }
    Json(SearchResp { hits: out })
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let data_dir: PathBuf = std::env::current_dir()?.join("data");
    std::fs::create_dir_all(&data_dir)?;
    info!("data dir = {}", std::fs::canonicalize(&data_dir)?.display());

    let dim = 4096;
    let meta = Arc::new(MetaStore::open(&data_dir)?);
    let vindex: Arc<dyn VecIndex> = Arc::new(spfresh_index::DefaultIndex::open(&data_dir, dim)?);
    let embedder: Arc<dyn Embedder> = Arc::new(TfIdfEmbedder::new(dim));

    let state = AppState { meta, vindex, embedder };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/reviews", post(insert_one))
        .route("/reviews/bulk", post(insert_bulk))
        .route("/search", post(search))
        .with_state(state)
        .layer(cors);
    
    info!("listening on 0.0.0.0:8000");
    axum::serve(tokio::net::TcpListener::bind("0.0.0.0:8000").await?, app).await?;
    Ok(())
}
