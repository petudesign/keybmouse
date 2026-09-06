# Uses the isolated toolchain when present; otherwise uses Rust on PATH.
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot
if (Test-Path '.toolchain\cargo\bin\cargo.exe') {
    $env:RUSTUP_HOME = Join-Path $PSScriptRoot '.toolchain\rustup'
    $env:CARGO_HOME = Join-Path $PSScriptRoot '.toolchain\cargo'
    $env:PATH = "$env:CARGO_HOME\bin;$env:PATH"
    $mingw = Get-ChildItem '.toolchain' -Directory -Filter 'llvm-mingw-*-ucrt-x86_64' | Select-Object -First 1
    if ($mingw) {
        $env:PATH = "$($mingw.FullName)\bin;$env:PATH"
        $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = Join-Path $env:RUSTUP_HOME 'toolchains\stable-x86_64-pc-windows-gnu\lib\rustlib\x86_64-pc-windows-gnu\bin\self-contained\x86_64-w64-mingw32-gcc.exe'
    }
}
if ($args.Count -eq 0) { cargo run --release } else { cargo @args }
exit $LASTEXITCODE
