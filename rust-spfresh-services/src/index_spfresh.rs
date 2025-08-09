mod spfresh_index {
    use super::*;
    use anyhow::{anyhow, Result};
    use spfresh::{Index as SIndex, OpenOptions as SOpen, SearchParams as SParams};
    use std::io::Write as _;

    pub struct SpfreshIndex {
        dim: usize,
        inner: Mutex<SIndex>,
        spf_path: PathBuf,  
        mirror_path: PathBuf, 
        auto_flush: bool,
    }

    impl SpfreshIndex {
        pub fn open(dir: impl Into<PathBuf>, dim: usize) -> Result<Self> {
            let dir = dir.into();
            std::fs::create_dir_all(&dir)?;
            let spf_path = dir.join("reviews.spfresh");
            let mirror_path = dir.join("reviews.index");

            {
                let _ = std::fs::OpenOptions::new().create(true).write(true).open(&spf_path)?;
                let _ = std::fs::OpenOptions::new().create(true).write(true).open(&mirror_path)?;
            }

            let spf_abs = std::fs::canonicalize(&spf_path).unwrap_or(spf_path.clone());
            let mir_abs = std::fs::canonicalize(&mirror_path).unwrap_or(mirror_path.clone());
            tracing::info!("spfresh data path = {}", spf_abs.display());
            tracing::info!("mirror  raw path  = {}", mir_abs.display());

            let opts = SOpen::new().create(true).append(true);
            let idx = SIndex::open(spf_abs.to_string_lossy().as_ref(), dim, &opts)
                .map_err(|e| anyhow!("{}", e))?;

            Ok(Self {
                dim,
                inner: Mutex::new(idx),
                spf_path: spf_abs,
                mirror_path: mir_abs,
                auto_flush: true,
            })
        }

        fn maybe_flush(&self, _idx: &mut SIndex) {
            if !self.auto_flush { return; }
        }
        
        #[inline]
        fn mirror_append(&self, vec: &[f32]) -> Result<()> {
            use std::io::{Seek, SeekFrom};

            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .open(&self.mirror_path)?;

            let before = f.metadata()?.len();

            f.seek(SeekFrom::End(0))?;

            let bytes = unsafe {
                std::slice::from_raw_parts(vec.as_ptr() as *const u8, vec.len() * 4)
            };
            f.write_all(bytes)?;
            f.flush()?;
            let _ = f.sync_all();

            let after = std::fs::metadata(&self.mirror_path)?.len();
            let expect_inc = (vec.len() * 4) as u64;
            anyhow::ensure!(
            after == before + expect_inc,
            "mirror write failed: {} (before) -> {} (after), expect +{} at {}",
            before, after, expect_inc, self.mirror_path.display()
        );

            tracing::info!(
            "mirror OK: +{} bytes, now {} @ {}",
            expect_inc, after, self.mirror_path.display()
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

            self.mirror_append(vec)?;

            self.maybe_flush(&mut idx);

            tracing::info!(
                "append OK: id={}, dim={}, spf={}, mirror={}",
                id, self.dim, self.spf_path.display(), self.mirror_path.display()
            );
            Ok(id)
        }

        fn get(&self, id: usize) -> Result<Vec<f32>> {
            let idx = self.inner.lock();
            Ok(idx.get(id).map_err(|e| anyhow!("{}", e))?)
        }

        fn search(&self, q: &[f32], top_k: usize) -> Result<Vec<(usize, f32)>> {
            anyhow::ensure!(q.len() == self.dim, "query dim mismatch: {} != {}", q.len(), self.dim);
            let idx = self.inner.lock();
            let params = SParams { top_k, ..Default::default() };
            Ok(idx.search(q, &params).map_err(|e| anyhow!("{}", e))?)
        }
    }

    pub use SpfreshIndex as DefaultIndex;
}
