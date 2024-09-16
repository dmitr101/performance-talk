$ArgList = @("run", "--features=static_update,profile", "--release", "--", "4000")
Start-Process cargo -NoNewWindow -PassThru -WorkingDirectory "$PSScriptRoot\..\boids-rs" -ArgumentList $ArgList
