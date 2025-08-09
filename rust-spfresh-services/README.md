### เพิ่ม component ที่มี dlltool ให้กับ GNU toolchain
```bash
rustup component add rust-mingw --toolchain stable-x86_64-pc-windows-gnu
```
### ใส่โฟลเดอร์ที่มี dlltool เข้า PATH เฉพาะเทอร์มินัลนี้
```bash
$toolBin = Join-Path -Path (Get-Location) -ChildPath ".toolchains\mingw64\x86_64-w64-mingw32\bin"
$gccBin  = Join-Path -Path (Get-Location) -ChildPath ".toolchains\mingw64\bin"
$env:DLLTOOL = (Join-Path -Path $toolBin -ChildPath "dlltool.exe")
$env:Path    = "$toolBin;$gccBin;$env:Path"
```

```bash
set RUST_LOG=info
cargo run --features with-spfresh
```

### CLI test

#### Insert Review

```bash
curl -X POST http://localhost:8000/reviews \
-H "Content-Type: application/json" \
-d '{
  "review": {
    "review_title": "Good product",
    "review_body": "The build quality is great and works perfectly.",
    "product_id": "P001",
    "review_rating": 5
  }
}'
```

#### Bulk Insert

```bash
curl -X POST http://localhost:8000/reviews/bulk \
-H "Content-Type: application/json" \
-d '{
  "reviews": [
    {
      "review_title": "Excellent service",
      "review_body": "Fast delivery and great packaging.",
      "product_id": "P002",
      "review_rating": 5
    },
    {
      "review_title": "Not worth the price",
      "review_body": "The product broke after 2 days.",
      "product_id": "P003",
      "review_rating": 1
    }
  ]
}'
```

#### Search

```bash
curl -X POST http://localhost:8000/search \
-H "Content-Type: application/json" \
-d '{"query":"Excellent  service", "top_k":3}'
```
