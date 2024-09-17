$ArgList = @("run", "--features=threaded", "--release", "--", "4000")
Start-Process cargo -NoNewWindow -PassThru -WorkingDirectory "$PSScriptRoot\..\boids-simd-rs" -ArgumentList $ArgList