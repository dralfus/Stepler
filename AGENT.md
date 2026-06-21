# Stepler Agent Notes

## Conversation Aliases

- `P` = `Pause`
- `CP` = `Control+Pause`

## Local Run Notes

- After rebuilding Stepler, start it for the user before finishing the turn.
- Start Stepler from the built distribution folder, normally:
  `F:\distr\system\Stepler\dist\Stepler\Stepler.exe`
- Use an out-of-sandbox/elevated `Start-Process` when needed. If Stepler is started from a sandboxed shell/session, Windows processes may appear briefly and then disappear when that session is cleaned up.
- Typical restart flow:
  1. Stop only Stepler processes running from the same dist folder.
  2. Build with `.\scripts\build-release.ps1 -DistDir 'F:\distr\system\Stepler\dist\Stepler'`.
  3. Start with `Start-Process -FilePath 'F:\distr\system\Stepler\dist\Stepler\Stepler.exe' -WorkingDirectory 'F:\distr\system\Stepler\dist\Stepler' -WindowStyle Hidden`.
  4. Verify `Stepler.exe` and `stepler-cli.exe` are still alive a few seconds later.
