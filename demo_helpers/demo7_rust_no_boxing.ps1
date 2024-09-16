$ArgList = @("run", "--features=static_update,no_life_history,no_boxing,profile", "--release", "--", "4000")
Start-Process cargo -NoNewWindow -PassThru -WorkingDirectory "$PSScriptRoot\..\boids-rs" -ArgumentList $ArgList
