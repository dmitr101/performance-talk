$ArgList = @("-m", "cProfile", "-o", "$PSScriptRoot\..\py-traces\demo-trace.prof", "$PSScriptRoot\..\boids-py\main.py", 400)
$Proc = Start-Process -FilePath "python" -NoNewWindow -PassThru -ArgumentList $ArgList
Start-Sleep -Seconds 10
$Proc | Stop-Process
Start-Process -FilePath "snakeviz" -NoNewWindow -PassThru -ArgumentList "$PSScriptRoot\..\py-traces\demo-trace.prof"
