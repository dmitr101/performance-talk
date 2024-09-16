$ArgList = @("run", "--features=profile_threaded_better", "--release", "--", "4000")
Start-Process cargo -NoNewWindow -PassThru -WorkingDirectory "$PSScriptRoot\..\boids-rs" -ArgumentList $ArgList