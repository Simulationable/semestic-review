### CLI Run

#### Set Target WASB
```bash
rustup target add wasm32-unknown-unknown
```


#### Compiler Setup
```bash
$toolBin = Join-Path -Path (Get-Location) -ChildPath ".toolchains\mingw64\x86_64-w64-mingw32\bin"
$gccBin  = Join-Path -Path (Get-Location) -ChildPath ".toolchains\mingw64\bin"
$env:DLLTOOL = (Join-Path -Path $toolBin -ChildPath "dlltool.exe")
$env:Path    = "$toolBin;$gccBin;$env:Path"
```

#### Install Tools Chain for WASB
```bash
cargo install trunk
```

#
```